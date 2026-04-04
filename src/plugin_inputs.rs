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
}

impl<'a> Iterator for PluginInputItems<'a> {
    type Item = PluginInputItem;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let (Some(input), Some(items)) = (self.current_input, self.current_items.as_mut())
                && let Some(item) = items.next()
            {
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

#[cfg(test)]
mod tests;
