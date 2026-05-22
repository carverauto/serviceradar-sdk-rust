use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const TARGET_KIND_DEVICE: &str = "device";
pub const TARGET_KIND_SERVICE: &str = "service";
pub const RESULT_SCHEMA_TARGET_CHECK_V1: &str = "serviceradar.target_check_result.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckDescriptor {
    pub descriptor_id: String,
    pub version: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub target_kinds: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub service_kinds: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub protocols: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub required_target_fields: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub optional_target_fields: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub required_capabilities: Vec<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub credential_requirements: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub schedule_bounds: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub timeout_bounds: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub threshold_schema: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub allowlist_policy: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_contract_ref: Option<String>,
    pub result_schema_version: String,
}

impl CheckDescriptor {
    pub fn new(
        descriptor_id: impl Into<String>,
        version: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        Self {
            descriptor_id: descriptor_id.into(),
            version: version.into(),
            label: label.into(),
            description: None,
            target_kinds: Vec::new(),
            service_kinds: Vec::new(),
            protocols: Vec::new(),
            required_target_fields: Vec::new(),
            optional_target_fields: Vec::new(),
            required_capabilities: Vec::new(),
            credential_requirements: BTreeMap::new(),
            schedule_bounds: BTreeMap::new(),
            timeout_bounds: BTreeMap::new(),
            threshold_schema: BTreeMap::new(),
            allowlist_policy: BTreeMap::new(),
            display_contract_ref: None,
            result_schema_version: RESULT_SCHEMA_TARGET_CHECK_V1.to_string(),
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_target_kinds(mut self, kinds: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.target_kinds = kinds.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_service_kinds(
        mut self,
        kinds: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.service_kinds = kinds.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_protocols(
        mut self,
        protocols: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.protocols = protocols.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_required_target_fields(
        mut self,
        fields: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.required_target_fields = fields.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_required_capabilities(
        mut self,
        capabilities: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.required_capabilities = capabilities.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_credential_requirements(mut self, requirements: BTreeMap<String, Value>) -> Self {
        self.credential_requirements = requirements;
        self
    }

    pub fn with_timeout_bounds(mut self, bounds: BTreeMap<String, Value>) -> Self {
        self.timeout_bounds = bounds;
        self
    }

    pub fn with_allowlist_policy(mut self, policy: BTreeMap<String, Value>) -> Self {
        self.allowlist_policy = policy;
        self
    }
}

#[cfg(test)]
mod tests;
