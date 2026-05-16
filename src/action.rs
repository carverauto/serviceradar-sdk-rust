use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{Error, SdkResult, get_config_bytes, submit_result_payload};

pub const ACTION_INVOCATION_SCHEMA_V1: &str = "serviceradar.northbound_action_invocation.v1";
pub const ACTION_RESULT_SCHEMA_V1: &str = "serviceradar.northbound_action_result.v1";

fn action_result_schema_v1() -> String {
    ACTION_RESULT_SCHEMA_V1.to_owned()
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionScope {
    Device,
    Interface,
    Event,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionSafety {
    ReadOnly,
    Standard,
    Destructive,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionStatus {
    #[default]
    Succeeded,
    Failed,
    Skipped,
    Suppressed,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ActionDescriptor {
    pub action_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub scopes: Vec<ActionScope>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_context: Vec<String>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub input_schema: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safety_classification: Option<ActionSafety>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub requires_confirmation: bool,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub credential_requirements: Map<String, Value>,
    #[serde(default = "action_result_schema_v1")]
    pub result_schema_version: String,
}

impl ActionDescriptor {
    pub fn new(
        action_id: impl Into<String>,
        label: impl Into<String>,
        scopes: Vec<ActionScope>,
    ) -> Self {
        Self {
            action_id: action_id.into(),
            version: Some("1.0.0".to_string()),
            label: label.into(),
            description: None,
            scopes,
            required_context: Vec::new(),
            input_schema: Map::new(),
            timeout_seconds: Some(60),
            safety_classification: Some(ActionSafety::Standard),
            requires_confirmation: false,
            credential_requirements: Map::new(),
            result_schema_version: ACTION_RESULT_SCHEMA_V1.to_string(),
        }
    }

    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_required_context<T, S>(mut self, fields: T) -> Self
    where
        T: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.required_context = fields.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_input_schema(mut self, schema: Map<String, Value>) -> Self {
        self.input_schema = schema;
        self
    }

    pub fn with_timeout_seconds(mut self, seconds: u32) -> Self {
        self.timeout_seconds = Some(seconds);
        self
    }

    pub fn with_safety(mut self, safety: ActionSafety) -> Self {
        self.safety_classification = Some(safety);
        self
    }

    pub fn with_confirmation_required(mut self, required: bool) -> Self {
        self.requires_confirmation = required;
        self
    }

    pub fn with_credential_requirements(mut self, requirements: Map<String, Value>) -> Self {
        self.credential_requirements = requirements;
        self
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ActionInvocation {
    #[serde(rename = "schema", default)]
    pub schema_version: Option<String>,
    #[serde(default)]
    pub invocation_id: String,
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub descriptor_id: Option<String>,
    #[serde(default)]
    pub action_id: String,
    #[serde(default)]
    pub action_version: Option<String>,
    #[serde(default)]
    pub descriptor_hash: Option<String>,
    #[serde(default)]
    pub result_schema_version: Option<String>,
    #[serde(default)]
    pub plugin_assignment_id: Option<String>,
    #[serde(default)]
    pub plugin_package_id: Option<String>,
    #[serde(default)]
    pub targets: Vec<ActionTargetSnapshot>,
    #[serde(default)]
    pub input_values: Map<String, Value>,
    #[serde(default)]
    pub redacted_input_values: Map<String, Value>,
    #[serde(default)]
    pub requested_at: Option<String>,
    #[serde(default)]
    pub metadata: Map<String, Value>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ActionTargetSnapshot {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub device_uid: Option<String>,
    #[serde(default)]
    pub device_name: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub hostname: Option<String>,
    #[serde(default)]
    pub device_hostname: Option<String>,
    #[serde(default)]
    pub ip: Option<String>,
    #[serde(default)]
    pub device_ip: Option<String>,
    #[serde(default)]
    pub mac: Option<String>,
    #[serde(default)]
    pub vendor_name: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(rename = "type", default)]
    pub device_type: Option<String>,
    #[serde(default)]
    pub is_available: Option<bool>,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub gateway_id: Option<String>,
    #[serde(default)]
    pub device_agent_id: Option<String>,
    #[serde(default)]
    pub device_gateway_id: Option<String>,
    #[serde(default)]
    pub discovery_sources: Vec<String>,
    #[serde(default)]
    pub interface_uid: Option<String>,
    #[serde(default)]
    pub if_index: Option<i64>,
    #[serde(default)]
    pub if_name: Option<String>,
    #[serde(default)]
    pub if_descr: Option<String>,
    #[serde(default)]
    pub if_alias: Option<String>,
    #[serde(default)]
    pub if_phys_address: Option<String>,
    #[serde(default)]
    pub ip_addresses: Vec<String>,
    #[serde(default)]
    pub if_admin_status: Option<String>,
    #[serde(default)]
    pub if_oper_status: Option<String>,
    #[serde(default)]
    pub if_type_name: Option<String>,
    #[serde(default)]
    pub interface_kind: Option<String>,
    #[serde(default)]
    pub classifications: Vec<String>,
    #[serde(default)]
    pub event_id: Option<String>,
    #[serde(default)]
    pub attributes: Map<String, Value>,
}

impl ActionTargetSnapshot {
    pub fn address(&self) -> Option<&str> {
        self.ip.as_deref().or(self.device_ip.as_deref())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ActionHostConfig {
    pub action_invocation: ActionInvocation,
    #[serde(skip)]
    pub plugin_config: Map<String, Value>,
}

pub fn load_action_config() -> SdkResult<ActionHostConfig> {
    parse_action_config(&get_config_bytes()?)
}

pub fn parse_action_config(data: &[u8]) -> SdkResult<ActionHostConfig> {
    if data.is_empty() {
        return Err(Error::Message("action config is empty".to_string()));
    }

    let mut raw: Map<String, Value> = serde_json::from_slice(data)?;
    let invocation_value = raw
        .remove("action_invocation")
        .ok_or_else(|| Error::Message("action_invocation is required".to_string()))?;
    let action_invocation = serde_json::from_value(invocation_value)?;

    Ok(ActionHostConfig {
        action_invocation,
        plugin_config: raw,
    })
}

impl ActionHostConfig {
    pub fn decode_plugin_config<T>(&self) -> SdkResult<T>
    where
        T: DeserializeOwned,
    {
        serde_json::from_value(Value::Object(self.plugin_config.clone())).map_err(Into::into)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ActionResult {
    #[serde(default = "action_result_schema_v1", rename = "schema")]
    pub schema_version: String,
    pub status: ActionStatus,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub summary: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_correlation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<ActionTargetResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub metadata: Map<String, Value>,
}

impl ActionResult {
    pub fn new(status: ActionStatus) -> Self {
        Self {
            schema_version: ACTION_RESULT_SCHEMA_V1.to_string(),
            status,
            summary: Map::new(),
            external_correlation_id: None,
            targets: Vec::new(),
            error_class: None,
            error_message: None,
            metadata: Map::new(),
        }
    }

    pub fn succeeded(message: impl Into<String>) -> Self {
        Self::new(ActionStatus::Succeeded).with_summary("message", message.into())
    }

    pub fn failed(class: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ActionStatus::Failed).with_error(class, message)
    }

    pub fn with_summary(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        let key = key.into();
        if !key.is_empty() {
            self.summary.insert(key, value.into());
        }
        self
    }

    pub fn with_correlation_id(mut self, id: impl Into<String>) -> Self {
        self.external_correlation_id = Some(id.into());
        self
    }

    pub fn with_error(mut self, class: impl Into<String>, message: impl Into<String>) -> Self {
        self.error_class = Some(class.into());
        self.error_message = Some(message.into());
        self
    }

    pub fn push_target_result(&mut self, result: ActionTargetResult) {
        self.targets.push(result);
    }

    pub fn with_target_result(mut self, result: ActionTargetResult) -> Self {
        self.push_target_result(result);
        self
    }

    pub fn serialize(&self) -> SdkResult<Vec<u8>> {
        serde_json::to_vec(self).map_err(Into::into)
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ActionTargetResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_uid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface_uid: Option<String>,
    pub status: ActionStatus,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub result: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_correlation_id: Option<String>,
}

impl ActionTargetResult {
    pub fn new(status: ActionStatus) -> Self {
        Self {
            status,
            ..Default::default()
        }
    }

    pub fn for_device(mut self, device_uid: impl Into<String>) -> Self {
        self.device_uid = Some(device_uid.into());
        self
    }

    pub fn for_interface(
        mut self,
        device_uid: impl Into<String>,
        interface_uid: impl Into<String>,
    ) -> Self {
        self.device_uid = Some(device_uid.into());
        self.interface_uid = Some(interface_uid.into());
        self
    }

    pub fn with_result(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        let key = key.into();
        if !key.is_empty() {
            self.result.insert(key, value.into());
        }
        self
    }
}

pub fn submit_action_result(result: &ActionResult) -> SdkResult<()> {
    submit_result_payload(&result.serialize()?)
}

#[cfg(test)]
mod tests;
