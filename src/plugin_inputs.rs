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
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use crate::Error;

    use super::{PluginInputsPayload, parse_plugin_inputs_json, parse_plugin_inputs_map};

    #[test]
    fn plugin_inputs_validate_and_flatten() {
        let payload = PluginInputsPayload::parse_json(
            json!({
                "schema": "serviceradar.plugin_inputs.v1",
                "policy_id": "policy-1",
                "policy_version": 1,
                "agent_id": "agent-1",
                "generated_at": "2026-04-03T00:00:00Z",
                "inputs": [{
                    "name": "devices",
                    "entity": "devices",
                    "query": "kind=device",
                    "chunk_index": 0,
                    "chunk_total": 1,
                    "chunk_hash": "abc",
                    "items": [
                        {"uid": "dev-1"},
                        {"uid": "dev-2"}
                    ]
                }]
            })
            .to_string()
            .as_bytes(),
        )
        .expect("parse plugin inputs");

        assert_eq!(payload.flatten_items().len(), 2);
        assert_eq!(payload.items_by_entity("devices").len(), 2);
        assert_eq!(payload.items_by_name("devices").len(), 2);
    }

    #[test]
    fn parse_plugin_inputs_rejects_invalid_schema() {
        let err = parse_plugin_inputs_json(
            json!({
                "schema": "nope",
                "policy_id": "policy-1",
                "policy_version": 1,
                "agent_id": "agent-1",
                "generated_at": "2026-04-03T00:00:00Z",
                "inputs": [{
                    "name": "devices",
                    "entity": "devices",
                    "query": "kind=device",
                    "chunk_index": 0,
                    "chunk_total": 1,
                    "chunk_hash": "abc",
                    "items": [{"uid": "dev-1"}]
                }]
            })
            .to_string()
            .as_bytes(),
        )
        .expect_err("invalid schema should fail");

        assert!(matches!(err, Error::InvalidPluginInputs(_)));
    }

    #[test]
    fn each_item_stops_on_error_and_map_parser_works() {
        let raw = json!({
            "schema": "serviceradar.plugin_inputs.v1",
            "policy_id": "policy-1",
            "policy_version": 1,
            "agent_id": "agent-1",
            "generated_at": "2026-04-03T00:00:00Z",
            "inputs": [{
                "name": "devices",
                "entity": "devices",
                "query": "kind=device",
                "chunk_index": 0,
                "chunk_total": 1,
                "chunk_hash": "abc",
                "items": [{"uid": "dev-1"}, {"uid": "dev-2"}]
            }]
        });

        let map = serde_json::from_value::<BTreeMap<String, serde_json::Value>>(raw)
            .expect("map payload");
        let payload = parse_plugin_inputs_map(&map).expect("parse inputs map");

        let mut seen = 0;
        let err = payload.each_item(|_| {
            seen += 1;
            if seen == 2 {
                return Err(Error::Message("stop".to_string()));
            }
            Ok(())
        });

        assert!(matches!(err, Err(Error::Message(_))));
        assert_eq!(seen, 2);
    }

    #[test]
    fn plugin_inputs_support_try_from_and_iteration() {
        let raw = json!({
            "schema": "serviceradar.plugin_inputs.v1",
            "policy_id": "policy-1",
            "policy_version": 1,
            "agent_id": "agent-1",
            "generated_at": "2026-04-03T00:00:00Z",
            "inputs": [{
                "name": "devices",
                "entity": "devices",
                "query": "kind=device",
                "chunk_index": 0,
                "chunk_total": 1,
                "chunk_hash": "abc",
                "items": [{"uid": "dev-1"}, {"uid": "dev-2"}]
            }]
        });

        let payload =
            PluginInputsPayload::try_from(raw.to_string().as_bytes()).expect("try_from bytes");
        let items: Vec<_> = payload.iter_items().collect();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, "devices");

        let from_iter: Vec<_> = (&payload).into_iter().collect();
        assert_eq!(from_iter.len(), 2);

        let map = serde_json::from_value::<BTreeMap<String, serde_json::Value>>(raw)
            .expect("map payload");
        let from_map = PluginInputsPayload::try_from(&map).expect("try_from map");
        assert_eq!(from_map.flatten_items().len(), 2);
    }
}
