#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CAPABILITY_PRODUCER_SCHEDULE_V1: &str = "producer-schedule:v1";
pub const PRODUCER_SCHEDULE_RUN_SCHEMA_V1: &str = "serviceradar.producer_schedule_run.v1";

pub const PRODUCER_SCHEDULE_COMMAND_PLUGIN_RUN_ACTION: &str = "plugin.run_action";

pub const PRODUCER_SCHEDULE_TYPE_INTERVAL: &str = "interval";
pub const PRODUCER_SCHEDULE_TYPE_CRON: &str = "cron";
pub const PRODUCER_SCHEDULE_TYPE_MANUAL: &str = "manual";

pub const PRODUCER_SCHEDULE_DISPATCH_ASSIGNMENT: &str = "assignment";
pub const PRODUCER_SCHEDULE_DISPATCH_PACKAGE: &str = "package";
pub const PRODUCER_SCHEDULE_DISPATCH_TARGET_QUERY: &str = "target_query";

/// Plugin-package manifest declaration that lets ServiceRadar create
/// operator-managed schedule settings without provider hardcoding in core.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProducerScheduleContract {
    pub schedule_id: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub action_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_cadence_seconds: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_cadence_seconds: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_cadence_seconds: Option<i64>,
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub allow_cron: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cron_expression: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jitter_seconds: Option<i64>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub settings_schema: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub credential_requirements: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub payload_template: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub redaction: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dispatch_scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<i64>,
}

impl ProducerScheduleContract {
    pub fn new(
        schedule_id: impl Into<String>,
        label: impl Into<String>,
        action_id: impl Into<String>,
    ) -> Self {
        Self {
            schedule_id: schedule_id.into(),
            label: label.into(),
            description: None,
            action_id: action_id.into(),
            command_type: Some(PRODUCER_SCHEDULE_COMMAND_PLUGIN_RUN_ACTION.to_string()),
            default_cadence_seconds: Some(86_400),
            min_cadence_seconds: Some(300),
            max_cadence_seconds: Some(2_592_000),
            allow_cron: false,
            schedule_type: Some(PRODUCER_SCHEDULE_TYPE_INTERVAL.to_string()),
            cron_expression: None,
            jitter_seconds: None,
            settings_schema: BTreeMap::new(),
            credential_requirements: BTreeMap::new(),
            payload_template: BTreeMap::new(),
            redaction: BTreeMap::new(),
            dispatch_scope: Some(PRODUCER_SCHEDULE_DISPATCH_ASSIGNMENT.to_string()),
            timeout_seconds: Some(300),
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_cadence(
        mut self,
        default_seconds: i64,
        min_seconds: i64,
        max_seconds: i64,
    ) -> Self {
        self.default_cadence_seconds = Some(default_seconds);
        self.min_cadence_seconds = Some(min_seconds);
        self.max_cadence_seconds = Some(max_seconds);
        self
    }

    pub fn with_cron(mut self, cron_expression: impl Into<String>) -> Self {
        self.allow_cron = true;
        self.schedule_type = Some(PRODUCER_SCHEDULE_TYPE_CRON.to_string());
        self.cron_expression = Some(cron_expression.into());
        self
    }

    pub fn with_jitter_seconds(mut self, jitter_seconds: i64) -> Self {
        self.jitter_seconds = Some(jitter_seconds);
        self
    }

    pub fn with_settings_schema(mut self, schema: BTreeMap<String, Value>) -> Self {
        self.settings_schema = schema;
        self
    }

    pub fn with_credential_requirements(mut self, requirements: BTreeMap<String, Value>) -> Self {
        self.credential_requirements = requirements;
        self
    }

    pub fn with_payload_template(mut self, template: BTreeMap<String, Value>) -> Self {
        self.payload_template = template;
        self
    }

    pub fn with_redaction(mut self, redaction: BTreeMap<String, Value>) -> Self {
        self.redaction = redaction;
        self
    }

    pub fn with_dispatch_scope(mut self, scope: impl Into<String>) -> Self {
        self.dispatch_scope = Some(scope.into());
        self
    }

    pub fn with_timeout_seconds(mut self, timeout_seconds: i64) -> Self {
        self.timeout_seconds = Some(timeout_seconds);
        self
    }
}
