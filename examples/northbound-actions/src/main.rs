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
    let doc: Map<String, Value> = match serde_json::from_slice(&raw) {
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

fn run_scheduled(doc: Map<String, Value>) {
    let _ = sdk::execute(|| {
        let config: Config = serde_json::from_value(Value::Object(doc))
            .map(Config::normalized)
            .unwrap_or_default();
        let overrides = sdk::run_overrides().unwrap_or_default();
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
    });
}

fn run_action(doc: Map<String, Value>) {
    let host_config = match sdk::parse_action_config(
        serde_json::to_vec(&Value::Object(doc))
            .unwrap_or_default()
            .as_slice(),
    ) {
        Ok(config) => config,
        Err(err) => {
            let _ = sdk::submit_action_result(&sdk::ActionResult::failed(
                "config_error",
                err.to_string(),
            ));
            return;
        }
    };
    let input: ActionInput = host_config.decode_plugin_config().unwrap_or_default();
    let result = match host_config.action_invocation.action_id.as_str() {
        "sample.device.lookup" => device_lookup(&host_config, &input),
        "sample.fault.inject" => fault_inject(&host_config, &input),
        "sample.fault.clear" => fault_clear(&host_config, &input),
        unknown => sdk::ActionResult::failed("unknown_action", format!("unknown action {unknown}")),
    };
    let _ = sdk::submit_action_result(&result);
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

fn fault_inject(_host_config: &sdk::ActionHostConfig, input: &ActionInput) -> sdk::ActionResult {
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
        "link_degraded",
        Some(target),
        600,
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
