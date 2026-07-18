use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    SIGNAL_SCHEMA_PAYLOAD_KIND_JSON, SIGNAL_SCHEMA_PAYLOAD_KIND_OCSF_EVENT,
    SIGNAL_SCHEMA_PAYLOAD_KIND_OTEL_LOG, SIGNAL_SCHEMA_SIGNAL_TYPE_EVENT,
    SIGNAL_SCHEMA_SIGNAL_TYPE_LOG,
};

pub const PROCESSOR_OCSF_PASSTHROUGH: &str = "ocsf_passthrough";
pub const PROCESSOR_OTEL_LOG_PASSTHROUGH: &str = "otel_log_passthrough";
pub const PROCESSOR_JSON_TO_OCSF: &str = "json_to_ocsf";
pub const PROCESSOR_SECURITY_FINDING: &str = "security_finding";
pub const PROCESSOR_SCAN_ACTIVITY: &str = "scan_activity";

pub const CONFLICT_POLICY_REJECT: &str = "reject";
pub const CONFLICT_POLICY_OVERRIDE: &str = "override";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestValidationError {
    pub path: String,
    pub message: String,
}

impl ManifestValidationError {
    fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub entrypoint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub permissions: BTreeMap<String, Value>,
    pub resources: BTreeMap<String, Value>,
    pub outputs: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signal_schemas: Vec<SignalSchemaContribution>,
}

impl PluginManifest {
    pub fn validate(&self) -> Result<(), Vec<ManifestValidationError>> {
        let mut errors = Vec::new();

        validate_non_empty(&self.id, "id", &mut errors);
        validate_non_empty(&self.entrypoint, "entrypoint", &mut errors);
        validate_non_empty(&self.outputs, "outputs", &mut errors);

        if self.resources.is_empty() {
            errors.push(ManifestValidationError::new("resources", "must be set"));
        }

        for (index, schema) in self.signal_schemas.iter().enumerate() {
            if let Err(mut schema_errors) =
                schema.validate_with_path(format!("signal_schemas[{index}]"))
            {
                errors.append(&mut schema_errors);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn serialize(&self) -> Result<Vec<u8>, Vec<ManifestValidationError>> {
        self.validate()?;
        serde_json::to_vec(self).map_err(|error| {
            vec![ManifestValidationError::new(
                "manifest",
                format!("failed to serialize manifest: {error}"),
            )]
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SignalSchemaContribution {
    pub id: String,
    pub version: String,
    pub signal_type: String,
    pub payload_kind: String,
    pub payload_schema: String,
    pub display_contract: String,
    pub display_contract_id: String,
    pub display_contract_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ocsf_schema_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_uid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_uid: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub event_writer: Vec<EventWriterContribution>,
}

impl SignalSchemaContribution {
    pub fn new(
        id: impl Into<String>,
        version: impl Into<String>,
        signal_type: impl Into<String>,
        payload_kind: impl Into<String>,
    ) -> Self {
        let id = id.into();
        let version = version.into();
        let short_name = short_schema_name(&id);

        Self {
            payload_schema: format!("schemas/{short_name}.schema.json"),
            display_contract: format!("display/{short_name}.display.json"),
            display_contract_id: format!("{id}.display"),
            display_contract_version: version.clone(),
            id,
            version,
            signal_type: signal_type.into(),
            payload_kind: payload_kind.into(),
            ..Self::default()
        }
    }

    pub fn with_event_writer(mut self, contribution: EventWriterContribution) -> Self {
        self.event_writer.push(contribution);
        self
    }

    fn validate_with_path(&self, path: String) -> Result<(), Vec<ManifestValidationError>> {
        let mut errors = Vec::new();

        validate_identifier(&self.id, format!("{path}.id"), &mut errors);
        validate_allowed(
            &self.signal_type,
            format!("{path}.signal_type"),
            &[
                SIGNAL_SCHEMA_SIGNAL_TYPE_EVENT,
                SIGNAL_SCHEMA_SIGNAL_TYPE_LOG,
            ],
            &mut errors,
        );
        validate_allowed(
            &self.payload_kind,
            format!("{path}.payload_kind"),
            &[
                SIGNAL_SCHEMA_PAYLOAD_KIND_OCSF_EVENT,
                SIGNAL_SCHEMA_PAYLOAD_KIND_OTEL_LOG,
                SIGNAL_SCHEMA_PAYLOAD_KIND_JSON,
            ],
            &mut errors,
        );
        validate_bundle_json_path(
            &self.payload_schema,
            format!("{path}.payload_schema"),
            &mut errors,
        );
        validate_bundle_json_path(
            &self.display_contract,
            format!("{path}.display_contract"),
            &mut errors,
        );
        validate_identifier(
            &self.display_contract_id,
            format!("{path}.display_contract_id"),
            &mut errors,
        );

        for (index, contribution) in self.event_writer.iter().enumerate() {
            if let Err(mut contribution_errors) =
                contribution.validate_with_path(format!("{path}.event_writer[{index}]"))
            {
                errors.append(&mut contribution_errors);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EventWriterContribution {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_name: Option<String>,
    pub subject: String,
    pub processor_id: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub destination: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ocsf: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub mapping: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub device_correlation: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub limits: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict_policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_timeout: Option<u32>,
}

impl EventWriterContribution {
    pub fn new(
        name: impl Into<String>,
        subject: impl Into<String>,
        processor_id: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            subject: subject.into(),
            processor_id: processor_id.into(),
            conflict_policy: Some(CONFLICT_POLICY_REJECT.to_string()),
            ..Self::default()
        }
    }

    pub fn with_stream_name(mut self, stream_name: impl Into<String>) -> Self {
        self.stream_name = Some(stream_name.into());
        self
    }

    pub fn with_destination(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.destination.insert(key.into(), value.into());
        self
    }

    pub fn with_ocsf(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.ocsf.insert(key.into(), value.into());
        self
    }

    pub fn with_mapping(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.mapping.insert(key.into(), value.into());
        self
    }

    pub fn with_device_correlation(
        mut self,
        key: impl Into<String>,
        value: impl Into<Value>,
    ) -> Self {
        self.device_correlation.insert(key.into(), value.into());
        self
    }

    pub fn with_limit(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.limits.insert(key.into(), value.into());
        self
    }

    pub fn with_batch(mut self, size: u32, timeout: u32) -> Self {
        self.batch_size = Some(size);
        self.batch_timeout = Some(timeout);
        self
    }

    fn validate_with_path(&self, path: String) -> Result<(), Vec<ManifestValidationError>> {
        let mut errors = Vec::new();

        validate_identifier(&self.name, format!("{path}.name"), &mut errors);
        validate_nats_subject(&self.subject, format!("{path}.subject"), &mut errors);
        validate_allowed(
            &self.processor_id,
            format!("{path}.processor_id"),
            &[
                PROCESSOR_OCSF_PASSTHROUGH,
                PROCESSOR_OTEL_LOG_PASSTHROUGH,
                PROCESSOR_JSON_TO_OCSF,
                PROCESSOR_SECURITY_FINDING,
                PROCESSOR_SCAN_ACTIVITY,
            ],
            &mut errors,
        );

        if let Some(policy) = &self.conflict_policy {
            validate_allowed(
                policy,
                format!("{path}.conflict_policy"),
                &[CONFLICT_POLICY_REJECT, CONFLICT_POLICY_OVERRIDE],
                &mut errors,
            );
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

fn validate_non_empty(
    value: &str,
    path: impl Into<String>,
    errors: &mut Vec<ManifestValidationError>,
) {
    if value.trim().is_empty() {
        errors.push(ManifestValidationError::new(
            path,
            "must be a non-empty string",
        ));
    }
}

fn validate_allowed(
    value: &str,
    path: impl Into<String>,
    allowed: &[&str],
    errors: &mut Vec<ManifestValidationError>,
) {
    if !allowed.contains(&value) {
        errors.push(ManifestValidationError::new(
            path,
            format!("must be one of: {}", allowed.join(", ")),
        ));
    }
}

fn validate_identifier(
    value: &str,
    path: impl Into<String>,
    errors: &mut Vec<ManifestValidationError>,
) {
    let path = path.into();
    validate_non_empty(value, path.clone(), errors);

    if value.len() > 160
        || !value.chars().all(|ch| {
            ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '.' | '_' | '-')
        })
        || value
            .chars()
            .next()
            .is_none_or(|ch| !(ch.is_ascii_lowercase() || ch.is_ascii_digit()))
    {
        errors.push(ManifestValidationError::new(
            path,
            "must use lowercase letters, digits, dots, underscores, or hyphens",
        ));
    }
}

fn validate_bundle_json_path(
    value: &str,
    path: impl Into<String>,
    errors: &mut Vec<ManifestValidationError>,
) {
    let path = path.into();
    validate_non_empty(value, path.clone(), errors);

    if value.starts_with('/')
        || value.split('/').any(|part| part == "..")
        || !value.ends_with(".json")
    {
        errors.push(ManifestValidationError::new(
            path,
            "must be a relative JSON bundle path",
        ));
    }
}

fn validate_nats_subject(
    value: &str,
    path: impl Into<String>,
    errors: &mut Vec<ManifestValidationError>,
) {
    let path = path.into();
    validate_non_empty(value, path.clone(), errors);

    let parts: Vec<&str> = value.split('.').collect();
    let valid = !parts.is_empty()
        && parts.iter().enumerate().all(|(index, part)| {
            !part.is_empty()
                && (*part == "*"
                    || (*part == ">" && index + 1 == parts.len())
                    || part
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-')))
        });

    if !valid {
        errors.push(ManifestValidationError::new(
            path,
            "must be a valid NATS subject",
        ));
    }
}

fn short_schema_name(id: &str) -> &str {
    id.rsplit('.').next().unwrap_or(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn plugin_manifest_serializes_sdk_generated_security_fixture() {
        let manifest = security_fixture_manifest();
        let payload = manifest.serialize().expect("serialize manifest");
        let generated: Value = serde_json::from_slice(&payload).expect("decode generated manifest");
        let expected: Value = serde_json::from_str(include_str!(
            "../testdata/sdk_generated_security_manifest.json"
        ))
        .expect("decode fixture");

        assert_eq!(generated, expected);
    }

    #[test]
    fn plugin_manifest_rejects_unsafe_processor_contribution() {
        let mut manifest = security_fixture_manifest();
        manifest.signal_schemas[0].event_writer[0].processor_id = "arbitrary_code".to_string();

        let errors = manifest
            .validate()
            .expect_err("unsafe processor should fail");
        assert!(
            errors
                .iter()
                .any(|error| error.path.ends_with(".processor_id"))
        );
    }

    fn security_fixture_manifest() -> PluginManifest {
        let mut schema = SignalSchemaContribution::new(
            "com.carverauto.security.scan_activity",
            "1.0.0",
            SIGNAL_SCHEMA_SIGNAL_TYPE_EVENT,
            SIGNAL_SCHEMA_PAYLOAD_KIND_JSON,
        );
        schema.ocsf_schema_version = Some("1.9.0-dev".to_string());
        schema.class_uid = Some(6007);
        schema.type_uid = Some(600701);

        let processor = EventWriterContribution::new(
            "security_scan_activity",
            "plugins.security_sample.scan_activity",
            PROCESSOR_SCAN_ACTIVITY,
        )
        .with_stream_name("events")
        .with_destination("table", "ocsf_events")
        .with_ocsf("schema_version", "1.9.0-dev")
        .with_ocsf("class_uid", 6007)
        .with_ocsf("activity_id", 1)
        .with_mapping(
            "fields",
            json!({
                "device.name": {"path": "host.hostname"},
                "message": {"template": "Security scan completed for {{host.hostname}}"}
            }),
        )
        .with_device_correlation(
            "candidates",
            json!(["host.hostname", "metadata.service_radar.agent_id"]),
        )
        .with_limit("max_output_bytes", 131072)
        .with_batch(25, 250);

        let mut permissions = BTreeMap::new();
        permissions.insert("allowed_domains".to_string(), json!([]));
        permissions.insert("allowed_ports".to_string(), json!([]));

        let mut resources = BTreeMap::new();
        resources.insert("requested_memory_mb".to_string(), json!(32));
        resources.insert("requested_cpu_ms".to_string(), json!(5000));
        resources.insert("max_open_connections".to_string(), json!(4));

        PluginManifest {
            id: "security-sample".to_string(),
            name: "Security Sample".to_string(),
            version: "1.0.0".to_string(),
            entrypoint: "run_check".to_string(),
            runtime: Some("wasi-preview1".to_string()),
            capabilities: vec![
                "get_config".to_string(),
                "log".to_string(),
                "submit_result".to_string(),
                "emit_telemetry".to_string(),
            ],
            permissions,
            resources,
            outputs: "serviceradar.plugin_result.v1".to_string(),
            signal_schemas: vec![schema.with_event_writer(processor)],
        }
    }
}
