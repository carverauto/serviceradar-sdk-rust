//! Run overrides: time-bounded state a plugin action leaves for later
//! scheduled runs of the same assignment, such as an injected demo fault or a
//! maintenance window.
//!
//! Plugin runs are stateless, so the action returns the override in its
//! [`ActionResult`] and the ServiceRadar host delivers it to every later run
//! until it expires. The action's descriptor must declare
//! `max_override_duration_seconds`; ServiceRadar clamps every override to it and
//! ignores overrides from actions without it.
//!
//! After expiry a run receives the override once more with `expired` set, so the
//! plugin can emit its resolving event; the host keeps delivering it expired
//! until a run that received it submits a result.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::action::{ActionDescriptor, ActionResult};
use crate::result::OcsfEvent;
use crate::telemetry::{TelemetryBatch, TelemetryRecord, emit_telemetry};
use crate::{Error, SdkResult, get_config_bytes};

/// Host-owned config key carrying the delivered overrides.
pub const RUN_OVERRIDES_CONFIG_KEY: &str = "_serviceradar_run_overrides";
/// Schema of the delivered override envelope.
pub const RUN_OVERRIDES_SCHEMA_V1: &str = "serviceradar.plugin_run_overrides.v1";
/// Sets or refreshes an override.
pub const RUN_OVERRIDE_OP_SET: &str = "set";
/// Ends an override early.
pub const RUN_OVERRIDE_OP_END: &str = "end";

/// One override as a scheduled run receives it.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RunOverride {
    pub id: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub params: Map<String, Value>,
    /// RFC 3339 start time.
    pub starts_at: String,
    /// RFC 3339 expiry time.
    pub expires_at: String,
    /// Set on the delivery after `expires_at`; emit the resolving event.
    #[serde(default)]
    pub expired: bool,
}

impl RunOverride {
    pub fn starts_at(&self) -> SdkResult<OffsetDateTime> {
        parse_timestamp(&self.starts_at)
    }

    pub fn expires_at(&self) -> SdkResult<OffsetDateTime> {
        parse_timestamp(&self.expires_at)
    }

    /// Whether the override applies at `at` (started and not expired).
    pub fn active_at(&self, at: OffsetDateTime) -> SdkResult<bool> {
        Ok(at >= self.starts_at()? && at < self.expires_at()?)
    }
}

/// One change an action result makes to the overrides of its assignment.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct RunOverrideOperation {
    pub op: String,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub params: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub starts_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<u32>,
}

#[derive(Deserialize)]
struct RunOverrideEnvelope {
    schema: String,
    #[serde(default)]
    overrides: Vec<RunOverride>,
}

/// Returns the overrides delivered to the current scheduled run.
pub fn run_overrides() -> SdkResult<Vec<RunOverride>> {
    parse_run_overrides(&get_config_bytes()?)
}

/// Extracts the delivered overrides from a run config document. A config
/// without overrides yields an empty list.
pub fn parse_run_overrides(config_json: &[u8]) -> SdkResult<Vec<RunOverride>> {
    if config_json.is_empty() {
        return Ok(Vec::new());
    }

    let config: Map<String, Value> = serde_json::from_slice(config_json)?;
    let Some(raw) = config.get(RUN_OVERRIDES_CONFIG_KEY) else {
        return Ok(Vec::new());
    };

    let envelope: RunOverrideEnvelope = serde_json::from_value(raw.clone())?;
    if envelope.schema != RUN_OVERRIDES_SCHEMA_V1 {
        return Err(Error::Message(format!(
            "unsupported run overrides schema {:?}",
            envelope.schema
        )));
    }
    Ok(envelope.overrides)
}

impl ActionResult {
    /// Asks the host to deliver an override to later runs for `duration_seconds`
    /// (clamped to the descriptor's maximum).
    pub fn set_run_override(
        mut self,
        id: impl Into<String>,
        kind: impl Into<String>,
        target: Option<String>,
        duration_seconds: u32,
        params: Map<String, Value>,
    ) -> Self {
        self.run_overrides.push(RunOverrideOperation {
            op: RUN_OVERRIDE_OP_SET.to_string(),
            id: id.into(),
            kind: Some(kind.into()),
            target,
            params,
            duration_seconds: Some(duration_seconds),
            ..RunOverrideOperation::default()
        });
        self
    }

    /// Asks the host to stop delivering an override now.
    pub fn end_run_override(mut self, id: impl Into<String>) -> Self {
        self.run_overrides.push(RunOverrideOperation {
            op: RUN_OVERRIDE_OP_END.to_string(),
            id: id.into(),
            ..RunOverrideOperation::default()
        });
        self
    }
}

impl ActionDescriptor {
    /// Declares how long overrides returned by this action may last. Actions
    /// without it cannot set run overrides.
    pub fn with_max_override_duration(mut self, seconds: u32) -> Self {
        self.max_override_duration_seconds = Some(seconds);
        self
    }
}

/// Emits one OCSF event through the host telemetry path. Works from both
/// scheduled runs and action entrypoints (for example the opening event of a
/// fault an action injects) and requires the `emit_telemetry` capability.
pub fn emit_ocsf_event(event: OcsfEvent) -> SdkResult<()> {
    emit_telemetry(TelemetryBatch::new(vec![TelemetryRecord::ocsf_event(
        event,
    )?]))
}

fn parse_timestamp(value: &str) -> SdkResult<OffsetDateTime> {
    OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|err| Error::Message(format!("invalid run override timestamp {value:?}: {err}")))
}

#[cfg(test)]
mod tests;
