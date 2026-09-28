//! Northbound actions example: one `run_check` export serves scheduled runs
//! and action invocations, mirroring the Go `sample-northbound` plugin.
//!
//! - Scheduled runs report the assignment's active run overrides, so the
//!   fault-injection actions below are visible between runs.
//! - `sample.device.lookup` answers device facts immediately, or defers with
//!   continuation state when `execution_mode` is `deferred` and finishes on
//!   the `poll` phase.
//! - `sample.fault.inject` leaves a time-bounded run override for later
//!   scheduled runs and emits the fault's opening OCSF event;
//!   `sample.fault.clear` ends the override early with the resolving event.
use std::collections::BTreeMap;

use serde_json::{Map, Value};
use serviceradar_sdk_rust as sdk;

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
struct Config {
    fault_kind: String,
    fault_duration_seconds: u32,
}

impl Config {
    fn normalized(mut self) -> Self {
        if self.fault_kind.trim().is_empty() {
            self.fault_kind = "link_degraded".to_string();
        }
        if self.fault_duration_seconds == 0 {
            self.fault_duration_seconds = 600;
        }
        self
    }
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
struct ActionInput {
    execution_mode: String,
    fault_id: String,
    target: String,
}

#[unsafe(no_mangle)]
pub extern "C" fn run_check() {
    let raw = match sdk::get_config_bytes() {
        Ok(raw) => raw,
        Err(err) => {
            let _ = sdk::execute(|| Err::<sdk::PluginResult, _>(err));
            return;
        }
    };
    let doc = match host_document(&raw) {
        Ok(doc) => doc,
        Err(err) => {
            let _ =
                sdk::execute(|| Err::<sdk::PluginResult, _>(sdk::Error::Message(err.to_string())));
            return;
        }
    };
    if doc.contains_key("action_invocation") {
        run_action(doc);
    } else {
        run_scheduled(doc);
    }
}

fn host_document(raw: &[u8]) -> Result<Map<String, Value>, serde_json::Error> {
    if raw.is_empty() {
        return Ok(Map::new());
    }
    serde_json::from_slice(raw)
}

fn run_scheduled(doc: Map<String, Value>) {
    let _ = sdk::execute(|| scheduled_check(doc));
}

fn scheduled_check(doc: Map<String, Value>) -> sdk::SdkResult<sdk::PluginResult> {
    let config: Config = serde_json::from_value(Value::Object(doc)).map(Config::normalized)?;
    let overrides = sdk::run_overrides()?;
    let mut table = BTreeMap::new();
    table.insert("Fault kind".to_string(), config.fault_kind.clone());
    table.insert("Active overrides".to_string(), overrides.len().to_string());
    for active in &overrides {
        table.insert(
            format!("Override {}", active.id),
            format!("{} expired={}", active.kind, active.expired),
        );
    }
    Ok(
        sdk::PluginResult::ok(format!("{} active run override(s)", overrides.len()))
            .with_table(table, "key-value"),
    )
}

fn run_action(doc: Map<String, Value>) {
    let _ = sdk::submit_action_result(&action_result(doc));
}

fn action_result(doc: Map<String, Value>) -> sdk::ActionResult {
    let host_config = match sdk::parse_action_config(
        serde_json::to_vec(&Value::Object(doc))
            .unwrap_or_default()
            .as_slice(),
    ) {
        Ok(config) => config,
        Err(err) => return sdk::ActionResult::failed("config_error", err.to_string()),
    };
    let input = match serde_json::from_value(Value::Object(
        host_config.action_invocation.input_values.clone(),
    )) {
        Ok(input) => input,
        Err(err) => return sdk::ActionResult::failed("config_error", err.to_string()),
    };
    match host_config.action_invocation.action_id.as_str() {
        "sample.device.lookup" => device_lookup(&host_config, &input),
        "sample.fault.inject" => fault_inject(&host_config, &input),
        "sample.fault.clear" => fault_clear(&host_config, &input),
        unknown => sdk::ActionResult::failed("unknown_action", format!("unknown action {unknown}")),
    }
}

fn device_lookup(host_config: &sdk::ActionHostConfig, input: &ActionInput) -> sdk::ActionResult {
    let invocation = &host_config.action_invocation;
    if invocation.phase.as_deref() == Some("poll") {
        return poll_finished(invocation);
    }
    if input.execution_mode.trim() == "deferred" {
        let mut state = Map::new();
        state.insert(
            "external_task_id".to_owned(),
            Value::String(format!("sample-{}", invocation.invocation_id)),
        );
        return sdk::ActionResult::deferred("external lookup accepted")
            .with_poll_mode(sdk::ActionPollMode::Poll)
            .with_continuation_state(state)
            .with_summary(
                "message",
                Value::String("queued in external system".to_owned()),
            );
    }
    let mut result = sdk::ActionResult::succeeded("device lookup complete");
    for target in &invocation.targets {
        let mut facts = Map::new();
        facts.insert(
            "device_uid".to_owned(),
            Value::String(target.device_uid.clone().unwrap_or_default()),
        );
        facts.insert("source".to_owned(), Value::String("sample-nms".to_owned()));
        result = result.with_target_result(sdk::ActionTargetResult {
            device_uid: target.device_uid.clone(),
            status: sdk::ActionStatus::Succeeded,
            result: facts,
            ..Default::default()
        });
    }
    result
}

fn poll_finished(invocation: &sdk::ActionInvocation) -> sdk::ActionResult {
    let mut result = sdk::ActionResult::succeeded("deferred lookup finished");
    for target in &invocation.targets {
        result = result.with_target_result(sdk::ActionTargetResult {
            device_uid: target.device_uid.clone(),
            status: sdk::ActionStatus::Succeeded,
            ..Default::default()
        });
    }
    result
}

fn fault_inject(host_config: &sdk::ActionHostConfig, input: &ActionInput) -> sdk::ActionResult {
    let config = match host_config.decode_plugin_config::<Config>() {
        Ok(config) => config.normalized(),
        Err(err) => return sdk::ActionResult::failed("config_error", err.to_string()),
    };
    let fault_id = if input.fault_id.trim().is_empty() {
        "fault-sample-1".to_string()
    } else {
        input.fault_id.clone()
    };
    let target = if input.target.trim().is_empty() {
        "device-1".to_string()
    } else {
        input.target.clone()
    };
    let mut params = Map::new();
    params.insert("source".to_owned(), Value::String("sample".to_owned()));
    let mut event = sdk::Event::log_activity(
        &format!("fault {fault_id} injected on {target}"),
        sdk::Severity::Warning,
    );
    event.id = format!("fault-{fault_id}-open");
    let _ = sdk::emit_ocsf_event(event);
    sdk::ActionResult::succeeded(format!("fault {fault_id} injected")).set_run_override(
        fault_id,
        config.fault_kind,
        Some(target),
        config.fault_duration_seconds,
        params,
    )
}

fn fault_clear(_host_config: &sdk::ActionHostConfig, input: &ActionInput) -> sdk::ActionResult {
    let fault_id = if input.fault_id.trim().is_empty() {
        "fault-sample-1".to_string()
    } else {
        input.fault_id.clone()
    };
    let mut event =
        sdk::Event::log_activity(&format!("fault {fault_id} cleared"), sdk::Severity::Info);
    event.id = format!("fault-{fault_id}-resolved");
    let _ = sdk::emit_ocsf_event(event);
    sdk::ActionResult::succeeded(format!("fault {fault_id} cleared")).end_run_override(fault_id)
}

fn main() {}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    fn invocation(action_id: &str, plugin: Value, input: Value) -> Map<String, Value> {
        let mut doc = plugin.as_object().cloned().unwrap_or_default();
        doc.insert(
            "action_invocation".to_string(),
            json!({
                "schema": "serviceradar.northbound_action_invocation.v1",
                "invocation_id": "inv-1",
                "action_id": action_id,
                "phase": "execute",
                "targets": [{
                    "kind": "device",
                    "device_uid": "sr:device-1"
                }],
                "input_values": input
            }),
        );
        doc
    }

    #[test]
    fn lookup_reads_execution_mode_from_input_values() {
        let deferred = action_result(invocation(
            "sample.device.lookup",
            json!({}),
            json!({"execution_mode": "deferred"}),
        ));
        assert_eq!(deferred.status, sdk::ActionStatus::Deferred);
        assert_eq!(deferred.poll_mode, Some(sdk::ActionPollMode::Poll));

        let immediate = action_result(invocation(
            "sample.device.lookup",
            json!({"execution_mode": "deferred"}),
            json!({}),
        ));
        assert_eq!(immediate.status, sdk::ActionStatus::Succeeded);
    }

    #[test]
    fn malformed_input_values_fail_the_action() {
        let result = action_result(invocation(
            "sample.device.lookup",
            json!({}),
            json!({"execution_mode": 1}),
        ));
        assert_eq!(result.status, sdk::ActionStatus::Failed);
        assert_eq!(result.error_class.as_deref(), Some("config_error"));
    }

    #[test]
    fn inject_applies_check_config_and_input_values() {
        let result = action_result(invocation(
            "sample.fault.inject",
            json!({"fault_kind": "power_loss", "fault_duration_seconds": 30}),
            json!({"fault_id": "fault-9", "target": "device-9"}),
        ));
        assert_eq!(result.status, sdk::ActionStatus::Succeeded);
        let op = &result.run_overrides[0];
        assert_eq!(op.id, "fault-9");
        assert_eq!(op.kind.as_deref(), Some("power_loss"));
        assert_eq!(op.target.as_deref(), Some("device-9"));
        assert_eq!(op.duration_seconds, Some(30));
    }

    #[test]
    fn inject_defaults_when_config_and_inputs_are_absent() {
        let result = action_result(invocation("sample.fault.inject", json!({}), json!({})));
        let op = &result.run_overrides[0];
        assert_eq!(op.id, "fault-sample-1");
        assert_eq!(op.kind.as_deref(), Some("link_degraded"));
        assert_eq!(op.target.as_deref(), Some("device-1"));
        assert_eq!(op.duration_seconds, Some(600));
    }

    #[test]
    fn inject_rejects_a_non_numeric_fault_duration() {
        let result = action_result(invocation(
            "sample.fault.inject",
            json!({"fault_duration_seconds": "30"}),
            json!({}),
        ));
        assert_eq!(result.status, sdk::ActionStatus::Failed);
        assert_eq!(result.error_class.as_deref(), Some("config_error"));
    }

    #[test]
    fn empty_host_config_is_a_scheduled_document() {
        let doc = host_document(b"").expect("empty config");
        assert!(!doc.contains_key("action_invocation"));
        let config = serde_json::from_value::<Config>(Value::Object(doc))
            .expect("default config")
            .normalized();
        assert_eq!(config.fault_kind, "link_degraded");
        assert_eq!(config.fault_duration_seconds, 600);
        assert!(host_document(b"{").is_err());
        assert!(host_document(br#"{"fault_duration_seconds":"30"}"#).is_ok());
    }

    #[test]
    fn scheduled_check_rejects_a_string_fault_duration() {
        let doc = json!({"fault_kind": "power_loss", "fault_duration_seconds": "30"})
            .as_object()
            .expect("object")
            .clone();
        let err = scheduled_check(doc).expect_err("string duration");
        assert!(err.to_string().contains("invalid type"), "{err}");
    }

    #[test]
    fn clear_reads_fault_id_from_input_values() {
        let result = action_result(invocation(
            "sample.fault.clear",
            json!({}),
            json!({"fault_id": "fault-9"}),
        ));
        assert_eq!(result.status, sdk::ActionStatus::Succeeded);
        assert_eq!(result.run_overrides.len(), 1);
        assert_eq!(result.run_overrides[0].op, sdk::RUN_OVERRIDE_OP_END);
        assert_eq!(result.run_overrides[0].id, "fault-9");
    }
}
