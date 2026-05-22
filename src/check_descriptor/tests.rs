use std::collections::BTreeMap;
use std::fs;

use serde_json::{Value, json};

use super::{CheckDescriptor, RESULT_SCHEMA_TARGET_CHECK_V1, TARGET_KIND_SERVICE};

#[test]
fn check_descriptor_serializes_target_scoped_contract() {
    let descriptor =
        CheckDescriptor::new("http.url.availability", "1.0.0", "HTTP URL availability")
            .with_target_kinds([TARGET_KIND_SERVICE])
            .with_service_kinds(["http"])
            .with_protocols(["http", "https"])
            .with_required_target_fields(["endpoint_url"])
            .with_required_capabilities(["http_request", "submit_result"])
            .with_credential_requirements(map([
                ("mode", json!("optional")),
                ("purpose", json!("http_auth")),
            ]))
            .with_timeout_bounds(map([("min_seconds", json!(1)), ("max_seconds", json!(30))]))
            .with_allowlist_policy(map([(
                "derive_from",
                json!(["target.host", "target.port", "target.path"]),
            )]));

    let encoded = serde_json::to_value(&descriptor).expect("serialize descriptor");
    assert_eq!(encoded["descriptor_id"], "http.url.availability");
    assert_eq!(
        encoded["result_schema_version"],
        RESULT_SCHEMA_TARGET_CHECK_V1
    );
}

#[test]
fn service_monitoring_descriptor_fixture_decodes() {
    let raw = fs::read_to_string("testdata/service_monitoring_descriptor.json").expect("fixture");
    let descriptor: CheckDescriptor = serde_json::from_str(&raw).expect("decode descriptor");

    assert_eq!(descriptor.descriptor_id, "http.url.availability");
    assert_eq!(
        descriptor.result_schema_version,
        RESULT_SCHEMA_TARGET_CHECK_V1
    );
}

fn map<const N: usize>(entries: [(&str, Value); N]) -> BTreeMap<String, Value> {
    entries
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}
