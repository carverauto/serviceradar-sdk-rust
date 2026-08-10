use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::{
    PRODUCER_SCHEDULE_COMMAND_PLUGIN_RUN_ACTION, PRODUCER_SCHEDULE_DISPATCH_ASSIGNMENT,
    ProducerScheduleContract,
};

#[test]
fn producer_schedule_contract_serializes_manifest_shape() {
    let mut settings_schema = BTreeMap::new();
    settings_schema.insert("type".to_string(), json!("object"));

    let mut credential_requirements = BTreeMap::new();
    credential_requirements.insert("refs".to_string(), json!(["feed_api_token"]));

    let mut payload_template = BTreeMap::new();
    payload_template.insert("feed_key".to_string(), json!("primary"));

    let payload = ProducerScheduleContract::new(
        "daily_advisory_refresh",
        "Refresh advisory feed",
        "advisory.refresh",
    )
    .with_description("Downloads and emits normalized advisory batches")
    .with_cadence(86_400, 3_600, 2_592_000)
    .with_jitter_seconds(120)
    .with_settings_schema(settings_schema)
    .with_credential_requirements(credential_requirements)
    .with_payload_template(payload_template)
    .with_timeout_seconds(600);

    let decoded: Value = serde_json::to_value(&payload).expect("serialize producer schedule");
    assert_eq!(decoded["schedule_id"], "daily_advisory_refresh");
    assert_eq!(
        decoded["command_type"],
        PRODUCER_SCHEDULE_COMMAND_PLUGIN_RUN_ACTION
    );
    assert_eq!(
        decoded["dispatch_scope"],
        PRODUCER_SCHEDULE_DISPATCH_ASSIGNMENT
    );
    assert_eq!(decoded["jitter_seconds"], 120);

    let encoded = decoded.to_string().to_lowercase();
    for provider in ["cisa", "nvd", "vulncheck", "osv", "trivy", "scalibr"] {
        assert!(
            !encoded.contains(provider),
            "schedule contract leaked provider-specific assumption {provider}: {encoded}"
        );
    }
}
