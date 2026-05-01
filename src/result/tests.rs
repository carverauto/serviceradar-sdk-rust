use std::collections::BTreeMap;

use serde_json::Value;

use super::{DisplayWidget, Metric, OcsfEvent, Result, Severity, Status, ThresholdSpec};
use crate::{DEVICE_DISCOVERY_SCHEMA_V1, DeviceDiscovery, DeviceLocation, DiscoveredDevice};

#[test]
fn serialize_includes_metrics_and_events() {
    let mut result = Result::new();
    let thresholds = ThresholdSpec::new(50.0, 100.0);

    result.set_status(Status::Warning);
    result.set_summary("latency high");
    result.add_metric("latency_ms", 75.0, "ms", Some(&thresholds));
    result.add_stat_card("Latency", "75ms", "warning");
    result.emit_event(Severity::Warning, "latency high", "latency_threshold");
    result.request_immediate_alert("latency_threshold");

    let payload = result.serialize().expect("serialize result");
    let decoded: Value = serde_json::from_slice(&payload).expect("decode result json");

    assert_eq!(decoded["status"], "WARNING");
    assert_eq!(decoded["summary"], "latency high");
    assert_eq!(decoded["alert_hint"], true);
    assert_eq!(decoded["condition_id"], "latency_threshold");
    assert!(
        decoded["events"]
            .as_array()
            .is_some_and(|events| !events.is_empty())
    );
}

#[test]
fn apply_thresholds_updates_status() {
    let mut result = Result::new();
    result.apply_thresholds(5.0, Some(10.0), Some(20.0));
    assert_eq!(result.status(), Status::Ok);

    let mut result = Result::new();
    result.apply_thresholds(12.0, Some(10.0), Some(20.0));
    assert_eq!(result.status(), Status::Warning);

    let mut result = Result::new();
    result.apply_thresholds(25.0, Some(10.0), Some(20.0));
    assert_eq!(result.status(), Status::Critical);
}

#[test]
fn serialize_defaults_without_mutation() {
    let result = Result::default();
    let payload = result.serialize().expect("serialize default result");
    let decoded: Value = serde_json::from_slice(&payload).expect("decode result");

    assert_eq!(result.summary(), None);
    assert_eq!(decoded["schema_version"], 1);
    assert_eq!(decoded["status"], "UNKNOWN");
    assert_eq!(decoded["summary"], "UNKNOWN");
    assert!(
        !decoded["observed_at"]
            .as_str()
            .unwrap_or_default()
            .is_empty()
    );
}

#[test]
fn add_table_serializes_key_value_data() {
    let mut result = Result::new();
    let mut data = BTreeMap::new();
    data.insert("Status".to_string(), "200".to_string());
    data.insert("URL".to_string(), "https://example.com".to_string());
    result.add_table(data, "full");

    let payload = result.serialize().expect("serialize result");
    let decoded: Value = serde_json::from_slice(&payload).expect("decode result");
    assert_eq!(decoded["display"][0]["widget"], "table");
}

#[test]
fn fluent_builders_cover_rich_result_fields() {
    let mut table = BTreeMap::new();
    table.insert("Status".to_string(), "200".to_string());

    let payload = Result::ok("ok")
        .with_table(table, "full")
        .with_sparkline("Latency", vec![1.0, 2.0, 3.0], "success")
        .with_markdown("**ok**")
        .with_event(Severity::Info, "event", "condition")
        .with_immediate_alert("condition")
        .with_thresholds(5.0, Some(10.0), Some(20.0))
        .serialize()
        .expect("serialize result");

    let decoded: Value = serde_json::from_slice(&payload).expect("decode result");
    assert_eq!(decoded["display"].as_array().map(Vec::len), Some(3));
    assert_eq!(decoded["events"].as_array().map(Vec::len), Some(1));
    assert_eq!(decoded["condition_id"], "condition");
}

#[test]
fn ocsf_event_helper_populates_expected_fields() {
    let event = OcsfEvent::log_activity("camera alert", Severity::Critical);
    assert_eq!(event.class_uid, 1008);
    assert_eq!(event.activity_id, 1);
    assert_eq!(event.severity_id, 5);
    assert_eq!(event.message.as_deref(), Some("camera alert"));
}

#[test]
fn associated_result_constructors_match_status() {
    assert_eq!(Result::ok("ok").status(), Status::Ok);
    assert_eq!(Result::warning("warn").status(), Status::Warning);
    assert_eq!(Result::critical("crit").status(), Status::Critical);
    assert_eq!(Result::unknown("unknown").status(), Status::Unknown);
}

#[test]
fn threshold_spec_builders_set_expected_values() {
    let thresholds = ThresholdSpec::new(50.0, 100.0).with_range(0.0, 250.0);
    assert_eq!(thresholds.warn, Some(50.0));
    assert_eq!(thresholds.crit, Some(100.0));
    assert_eq!(thresholds.min, Some(0.0));
    assert_eq!(thresholds.max, Some(250.0));
}

#[test]
fn result_default_matches_new() {
    assert_eq!(Result::default().status(), Result::new().status());
    assert_eq!(Result::default().summary(), Result::new().summary());
}

#[test]
fn status_and_severity_support_display_and_parsing() {
    assert_eq!(Status::Warning.to_string(), "WARNING");
    assert_eq!(
        "crit".parse::<Status>().expect("parse status"),
        Status::Critical
    );
    assert_eq!(Severity::Info.to_string(), "INFO");
    assert_eq!(
        "warning".parse::<Severity>().expect("parse severity"),
        Severity::Warning
    );
}

#[test]
fn domain_type_constructors_build_expected_shapes() {
    let metric = Metric::new("latency_ms", 42.0)
        .with_unit("ms")
        .with_thresholds(&ThresholdSpec::new(50.0, 100.0).with_range(0.0, 500.0));
    assert_eq!(metric.unit, "ms");
    assert_eq!(metric.warn, Some(50.0));

    let table = DisplayWidget::table(
        BTreeMap::from([("Status".to_string(), "200".to_string())]),
        "full",
    )
    .expect("table widget");
    assert_eq!(table.widget, "table");

    let markdown = DisplayWidget::markdown("**ok**").expect("markdown widget");
    assert_eq!(
        markdown.data.get("markdown"),
        Some(&Value::String("**ok**".to_string()))
    );

    let event = OcsfEvent::log_activity("camera alert", Severity::Critical);
    assert_eq!(event.message.as_deref(), Some("camera alert"));
}

#[test]
fn serialize_includes_device_discovery() {
    let location = DeviceLocation::at(29.9844, -95.3414)
        .with_site_code("IAH")
        .with_site_name("Houston");

    let device = DiscoveredDevice::named("NIAHAP-MDF001-WAP001")
        .with_serial("CNC3HN77NW")
        .with_device_type("access_point")
        .with_location(location)
        .with_label("site", "IAH")
        .with_metadata("radio_count", 2);

    let payload = Result::ok("discovered 1 device")
        .with_device_discovery(DeviceDiscovery::new("ual-network-map").with_device(device))
        .serialize()
        .expect("serialize result");

    let decoded: Value = serde_json::from_slice(&payload).expect("decode result");

    assert_eq!(
        decoded["device_discovery"][0]["schema"],
        DEVICE_DISCOVERY_SCHEMA_V1
    );
    assert_eq!(
        decoded["device_discovery"][0]["devices"][0]["hostname"],
        "NIAHAP-MDF001-WAP001"
    );
    assert_eq!(
        decoded["device_discovery"][0]["devices"][0]["type"],
        "access_point"
    );
    assert!(
        decoded["device_discovery"][0]["devices"][0]
            .get("device_type")
            .is_none()
    );
    assert_eq!(
        decoded["device_discovery"][0]["devices"][0]["location"]["site_code"],
        "IAH"
    );
    assert_eq!(
        decoded["device_discovery"][0]["devices"][0]["metadata"]["radio_count"],
        2
    );
}
