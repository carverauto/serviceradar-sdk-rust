use serde_json::{Map, Value, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use super::*;

fn at(value: &str) -> OffsetDateTime {
    OffsetDateTime::parse(value, &Rfc3339).unwrap()
}

#[test]
fn parses_the_shared_run_override_config_fixture() {
    let overrides = parse_run_overrides(include_bytes!(
        "../../fixtures/plugin_run_overrides_config.json"
    ))
    .expect("parse fixture");

    assert_eq!(overrides.len(), 2);
    let jam = &overrides[0];
    assert_eq!(jam.id, "fault-jam-7");
    assert_eq!(jam.kind, "conveyor_jam");
    assert_eq!(jam.target.as_deref(), Some("conveyor-7"));
    assert_eq!(jam.params.get("severity"), Some(&json!("critical")));
    assert!(!jam.expired);
    assert!(jam.active_at(at("2026-09-27T12:05:00Z")).unwrap());
    assert!(!jam.active_at(at("2026-09-27T12:10:00Z")).unwrap());
    assert!(overrides[1].expired);
}

#[test]
fn config_without_overrides_or_with_an_unknown_schema() {
    assert!(parse_run_overrides(br#"{"site":"a"}"#).unwrap().is_empty());
    assert!(parse_run_overrides(br#"{"_serviceradar_run_overrides":{"schema":"other"}}"#).is_err());
}

#[test]
fn action_result_run_overrides_match_the_shared_fixture() {
    let mut params = Map::new();
    params.insert("severity".to_string(), Value::from("critical"));

    let result = ActionResult::succeeded("Injected conveyor jam")
        .set_run_override(
            "fault-jam-7",
            "conveyor_jam",
            Some("conveyor-7".to_string()),
            600,
            params,
        )
        .end_run_override("fault-saturation-2");

    let got: Value = serde_json::from_slice(&result.serialize().unwrap()).unwrap();
    let want: Value = serde_json::from_str(include_str!(
        "../../fixtures/northbound_action_result_run_overrides.json"
    ))
    .unwrap();
    assert_eq!(got, want);
}

#[test]
fn descriptor_declares_max_override_duration() {
    let descriptor = ActionDescriptor::new("inject_fault", "Inject fault", vec![])
        .with_max_override_duration(1800);
    let value = serde_json::to_value(&descriptor).unwrap();
    assert_eq!(value["max_override_duration_seconds"], json!(1800));
}
