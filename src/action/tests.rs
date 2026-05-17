use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    ACTION_RESULT_SCHEMA_V1, ActionDescriptor, ActionPollMode, ActionResult, ActionSafety,
    ActionScope, ActionStatus, ActionTargetResult, parse_action_config,
};

#[test]
fn action_descriptor_serializes_manifest_shape() {
    let payload = serde_json::to_value(
        ActionDescriptor::new(
            "hpna.disable_port",
            "Disable switch port",
            vec![ActionScope::Interface],
        )
        .with_required_context(["device.ip", "interface.name"])
        .with_input_schema(
            json!({
                "type": "object",
                "properties": {
                    "reason": {"type": "string"}
                }
            })
            .as_object()
            .expect("schema object")
            .clone(),
        )
        .with_safety(ActionSafety::Destructive)
        .with_confirmation_required(true),
    )
    .expect("serialize descriptor");

    assert_eq!(payload["action_id"], "hpna.disable_port");
    assert_eq!(payload["scopes"], json!(["interface"]));
    assert_eq!(payload["safety_classification"], "destructive");
    assert_eq!(payload["result_schema_version"], ACTION_RESULT_SCHEMA_V1);
    assert_eq!(payload["requires_confirmation"], true);
}

#[test]
fn parse_action_config_extracts_invocation_and_plugin_config() {
    let config = parse_action_config(
        br#"{
            "api_url": "https://ncm.example",
            "action_invocation": {
                "schema": "serviceradar.northbound_action_invocation.v1",
                "phase": "poll",
                "invocation_id": "inv-1",
                "invocation_target_id": "ivt-1",
                "action_id": "hpna.disable_port",
                "targets": [{
                    "kind": "interface",
                    "northbound_job_id": "job-1",
                    "callback": {
                        "job_id": "job-1",
                        "path": "/api/northbound/action-callbacks/job-1",
                        "url": "https://service.example/api/northbound/action-callbacks/job-1",
                        "token": "callback-token",
                        "token_header": "x-serviceradar-callback-token"
                    },
                    "device_uid": "sr:device-1",
                    "device_ip": "10.0.0.1",
                    "interface_uid": "if-1",
                    "if_name": "Gi1/0/1",
                    "if_admin_status": 1,
                    "if_oper_status": 2
                }],
                "input_values": {"reason": "test"},
                "continuation_state": {"external_task_id": "task-123"},
                "external_correlation_id": "task-123",
                "poll_attempt_count": 2
            }
        }"#,
    )
    .expect("parse config");

    assert_eq!(config.action_invocation.invocation_id, "inv-1");
    assert_eq!(config.action_invocation.phase.as_deref(), Some("poll"));
    assert_eq!(
        config.action_invocation.invocation_target_id.as_deref(),
        Some("ivt-1")
    );
    assert_eq!(config.action_invocation.poll_attempt_count, 2);
    assert_eq!(
        config
            .action_invocation
            .continuation_state
            .get("external_task_id"),
        Some(&json!("task-123"))
    );
    assert_eq!(config.action_invocation.targets.len(), 1);
    assert_eq!(
        config.action_invocation.targets[0].address(),
        Some("10.0.0.1")
    );
    assert_eq!(
        config.action_invocation.targets[0]
            .northbound_job_id
            .as_deref(),
        Some("job-1")
    );
    assert_eq!(
        config.action_invocation.targets[0].callback.url.as_deref(),
        Some("https://service.example/api/northbound/action-callbacks/job-1")
    );
    assert_eq!(
        config.action_invocation.targets[0]
            .if_admin_status
            .as_deref(),
        Some("up")
    );
    assert_eq!(
        config.action_invocation.targets[0]
            .if_oper_status
            .as_deref(),
        Some("down")
    );

    #[derive(Deserialize)]
    struct PluginConfig {
        api_url: String,
    }

    let plugin_config: PluginConfig = config.decode_plugin_config().expect("plugin config");
    assert_eq!(plugin_config.api_url, "https://ncm.example");
}

#[test]
fn deferred_action_result_serializes_poll_and_webhook_fields() {
    let continuation = json!({"external_task_id": "job-123"})
        .as_object()
        .expect("continuation object")
        .clone();

    let payload = ActionResult::deferred("external task accepted")
        .with_correlation_id("job-123")
        .with_continuation_state(continuation.clone())
        .with_webhook_callback()
        .with_max_duration(300)
        .with_target_result(
            ActionTargetResult::new(ActionStatus::Deferred)
                .for_device("sr:device-1")
                .with_correlation_id("job-123")
                .with_continuation_state(continuation)
                .with_webhook_callback()
                .with_max_duration(300)
                .with_result("message", "queued in external system"),
        )
        .serialize()
        .expect("serialize deferred result");

    let decoded: Value = serde_json::from_slice(&payload).expect("decode deferred result");
    assert_eq!(decoded["status"], "deferred");
    assert_eq!(decoded["poll_mode"], "webhook");
    assert_eq!(decoded["max_duration_seconds"], 300);
    assert_eq!(decoded["targets"][0]["poll_mode"], "webhook");
    assert_eq!(decoded["targets"][0]["max_duration_seconds"], 300);
    assert!(decoded.get("next_poll_delay_seconds").is_none());
}

#[test]
fn action_result_serializes_handler_shape() {
    let payload = ActionResult::succeeded("disabled interface")
        .with_correlation_id("job-123")
        .with_target_result(
            ActionTargetResult::new(ActionStatus::Succeeded)
                .for_interface("sr:device-1", "if-1")
                .with_result("changed", true),
        )
        .serialize()
        .expect("serialize result");

    let decoded: Value = serde_json::from_slice(&payload).expect("decode result");
    assert_eq!(decoded["schema"], ACTION_RESULT_SCHEMA_V1);
    assert_eq!(decoded["status"], "succeeded");
    assert_eq!(decoded["external_correlation_id"], "job-123");
    assert_eq!(decoded["targets"][0]["device_uid"], "sr:device-1");
    assert_eq!(decoded["targets"][0]["result"]["changed"], true);
}

#[test]
fn action_fixtures_decode() {
    let descriptor: ActionDescriptor = serde_json::from_str(include_str!(
        "../../fixtures/northbound_action_descriptor.json"
    ))
    .expect("decode descriptor fixture");
    assert_eq!(descriptor.action_id, "hpna.disable_port");

    let invocation = include_str!("../../fixtures/northbound_action_invocation.json");
    let config = format!(r#"{{"timeout":"30s","action_invocation":{invocation}}}"#);
    let config = parse_action_config(config.as_bytes()).expect("decode invocation fixture");
    assert_eq!(
        config.action_invocation.targets[0].interface_uid.as_deref(),
        Some("if-1")
    );
    assert_eq!(
        config.action_invocation.targets[0]
            .callback
            .job_id
            .as_deref(),
        Some("target-123")
    );

    let result: ActionResult =
        serde_json::from_str(include_str!("../../fixtures/northbound_action_result.json"))
            .expect("decode result fixture");
    assert_eq!(result.status, ActionStatus::Succeeded);
}

#[test]
fn action_poll_mode_serializes_like_go_sdk() {
    let encoded = serde_json::to_value(ActionPollMode::Webhook).expect("serialize poll mode");
    assert_eq!(encoded, json!("webhook"));
}
