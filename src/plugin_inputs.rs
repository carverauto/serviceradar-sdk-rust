use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, SdkResult};

pub const PLUGIN_INPUTS_SCHEMA_V1: &str = "serviceradar.plugin_inputs.v1";

pub fn parse_plugin_inputs_json(data: &[u8]) -> SdkResult<PluginInputsPayload> {
    PluginInputsPayload::parse_json(data)
}

pub fn parse_plugin_inputs_map(map: &BTreeMap<String, Value>) -> SdkResult<PluginInputsPayload> {
    PluginInputsPayload::parse_map(map)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginInputsPayload {
    pub schema: String,
    pub policy_id: String,
    pub policy_version: i32,
    pub agent_id: String,
    pub generated_at: String,
    #[serde(default)]
    pub template: BTreeMap<String, Value>,
    pub inputs: Vec<PluginInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginInput {
    pub name: String,
    pub entity: String,
    pub query: String,
    pub chunk_index: i32,
    pub chunk_total: i32,
    pub chunk_hash: String,
    pub items: Vec<BTreeMap<String, Value>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PluginInputItem {
    pub name: String,
    pub entity: String,
    pub query: String,
    pub chunk_index: i32,
    pub chunk_total: i32,
    pub chunk_hash: String,
    pub item: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CredentialBrokerGrant {
    pub grant_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grant_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_secret_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grant_type: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub inject: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CredentialPolicySnapshot {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub credential_brokers: Vec<CredentialBrokerGrant>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub credential_broker_grant_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TargetContext {
    pub uid: String,
    pub check_instance_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monitoring_binding_id: Option<String>,
    pub descriptor_id: String,
    pub descriptor_version: String,
    pub target_kind: String,
    #[serde(default)]
    pub target: BTreeMap<String, Value>,
    #[serde(default)]
    pub credential_policy: CredentialPolicySnapshot,
    #[serde(default)]
    pub event_policy: BTreeMap<String, Value>,
}

#[derive(Debug, Clone)]
pub struct PluginInputItems<'a> {
    inputs: std::slice::Iter<'a, PluginInput>,
    current_input: Option<&'a PluginInput>,
    current_items: Option<std::slice::Iter<'a, BTreeMap<String, Value>>>,
}

impl PluginInputsPayload {
    pub fn parse_json(data: &[u8]) -> SdkResult<Self> {
        let payload: Self = serde_json::from_slice(data)?;
        payload.validate()?;
        Ok(payload)
    }

    pub fn parse_map(map: &BTreeMap<String, Value>) -> SdkResult<Self> {
        Self::parse_json(&serde_json::to_vec(map)?)
    }

    pub fn validate(&self) -> SdkResult<()> {
        if self.schema.trim() != PLUGIN_INPUTS_SCHEMA_V1 {
            return Err(Error::InvalidPluginInputs(format!(
                "schema must be {PLUGIN_INPUTS_SCHEMA_V1}"
            )));
        }
        if self.policy_id.trim().is_empty() {
            return Err(Error::InvalidPluginInputs(
                "policy_id is required".to_string(),
            ));
        }
        if self.policy_version < 1 {
            return Err(Error::InvalidPluginInputs(
                "policy_version must be >= 1".to_string(),
            ));
        }
        if self.agent_id.trim().is_empty() {
            return Err(Error::InvalidPluginInputs(
                "agent_id is required".to_string(),
            ));
        }
        if self.generated_at.trim().is_empty() {
            return Err(Error::InvalidPluginInputs(
                "generated_at is required".to_string(),
            ));
        }
        if self.inputs.is_empty() {
            return Err(Error::InvalidPluginInputs("inputs is required".to_string()));
        }

        for (index, input) in self.inputs.iter().enumerate() {
            if input.name.trim().is_empty() {
                return Err(Error::InvalidPluginInputs(format!(
                    "inputs[{index}].name is required"
                )));
            }
            if input.entity.trim().is_empty() {
                return Err(Error::InvalidPluginInputs(format!(
                    "inputs[{index}].entity is required"
                )));
            }
            if input.query.trim().is_empty() {
                return Err(Error::InvalidPluginInputs(format!(
                    "inputs[{index}].query is required"
                )));
            }
            if input.chunk_index < 0 {
                return Err(Error::InvalidPluginInputs(format!(
                    "inputs[{index}].chunk_index must be >= 0"
                )));
            }
            if input.chunk_total < 1 {
                return Err(Error::InvalidPluginInputs(format!(
                    "inputs[{index}].chunk_total must be >= 1"
                )));
            }
            if input.chunk_hash.trim().is_empty() {
                return Err(Error::InvalidPluginInputs(format!(
                    "inputs[{index}].chunk_hash is required"
                )));
            }
            if input.items.is_empty() {
                return Err(Error::InvalidPluginInputs(format!(
                    "inputs[{index}].items is required"
                )));
            }
        }

        Ok(())
    }

    pub fn flatten_items(&self) -> Vec<PluginInputItem> {
        self.iter_items().collect()
    }

    pub fn iter_items(&self) -> PluginInputItems<'_> {
        PluginInputItems {
            inputs: self.inputs.iter(),
            current_input: None,
            current_items: None,
        }
    }

    pub fn each_item<F>(&self, mut func: F) -> SdkResult<()>
    where
        F: FnMut(PluginInputItem) -> SdkResult<()>,
    {
        for item in self.iter_items() {
            func(item)?;
        }
        Ok(())
    }

    pub fn items_by_entity(&self, entity: &str) -> Vec<PluginInputItem> {
        let entity = entity.trim().to_lowercase();
        if entity.is_empty() {
            return Vec::new();
        }

        self.iter_items()
            .filter(|item| item.entity.trim().eq_ignore_ascii_case(&entity))
            .collect()
    }

    pub fn items_by_name(&self, name: &str) -> Vec<PluginInputItem> {
        let name = name.trim().to_lowercase();
        if name.is_empty() {
            return Vec::new();
        }

        self.iter_items()
            .filter(|item| item.name.trim().eq_ignore_ascii_case(&name))
            .collect()
    }

    pub fn target_contexts(&self) -> SdkResult<Vec<TargetContext>> {
        self.iter_items()
            .enumerate()
            .map(|(index, item)| {
                item.target_context().map_err(|err| {
                    Error::InvalidPluginInputs(format!(
                        "decode target context at flattened item {index}: {err}"
                    ))
                })
            })
            .collect()
    }
}

impl PluginInputItem {
    pub fn target_context(&self) -> SdkResult<TargetContext> {
        let ctx: TargetContext = serde_json::from_value(Value::Object(
            self.item
                .clone()
                .into_iter()
                .collect::<serde_json::Map<String, Value>>(),
        ))?;

        if ctx.check_instance_id.trim().is_empty() {
            return Err(Error::InvalidPluginInputs(
                "target context missing check_instance_id".to_string(),
            ));
        }
        if ctx.descriptor_id.trim().is_empty() {
            return Err(Error::InvalidPluginInputs(
                "target context missing descriptor_id".to_string(),
            ));
        }

        Ok(ctx)
    }
}

impl TargetContext {
    pub fn monitored_service_id(&self) -> Option<&str> {
        string_value(&self.target, "monitored_service_id")
    }

    pub fn device_uid(&self) -> Option<&str> {
        string_value(&self.target, "device_uid")
    }

    pub fn endpoint_url(&self) -> Option<&str> {
        string_value(&self.target, "endpoint_url")
    }

    pub fn host(&self) -> Option<&str> {
        string_value(&self.target, "host")
    }

    pub fn path(&self) -> Option<&str> {
        string_value(&self.target, "path")
    }

    pub fn port(&self) -> Option<u16> {
        match self.target.get("port") {
            Some(Value::Number(value)) => value.as_u64().and_then(|port| u16::try_from(port).ok()),
            Some(Value::String(value)) => value.parse::<u16>().ok(),
            _ => None,
        }
    }

    pub fn credential_grants(&self) -> &[CredentialBrokerGrant] {
        &self.credential_policy.credential_brokers
    }
}

impl Iterator for PluginInputItems<'_> {
    type Item = PluginInputItem;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let (Some(input), Some(items)) = (self.current_input, self.current_items.as_mut()) {
                if let Some(item) = items.next() {
                    return Some(PluginInputItem {
                        name: input.name.clone(),
                        entity: input.entity.clone(),
                        query: input.query.clone(),
                        chunk_index: input.chunk_index,
                        chunk_total: input.chunk_total,
                        chunk_hash: input.chunk_hash.clone(),
                        item: item.clone(),
                    });
                }
            }

            let input = self.inputs.next()?;
            self.current_input = Some(input);
            self.current_items = Some(input.items.iter());
        }
    }
}

impl<'a> IntoIterator for &'a PluginInputsPayload {
    type Item = PluginInputItem;
    type IntoIter = PluginInputItems<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_items()
    }
}

impl TryFrom<&[u8]> for PluginInputsPayload {
    type Error = Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        Self::parse_json(value)
    }
}

impl TryFrom<&BTreeMap<String, Value>> for PluginInputsPayload {
    type Error = Error;

    fn try_from(value: &BTreeMap<String, Value>) -> Result<Self, Self::Error> {
        Self::parse_map(value)
    }
}

fn string_value<'a>(map: &'a BTreeMap<String, Value>, key: &str) -> Option<&'a str> {
    match map.get(key) {
        Some(Value::String(value)) if !value.is_empty() => Some(value.as_str()),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
