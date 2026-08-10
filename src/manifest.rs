#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    SIGNAL_SCHEMA_PAYLOAD_KIND_OCSF_EVENT, SIGNAL_SCHEMA_PAYLOAD_KIND_OTEL_LOG,
    SIGNAL_SCHEMA_SIGNAL_TYPE_EVENT, SIGNAL_SCHEMA_SIGNAL_TYPE_LOG,
};

pub const RUNTIME_NONE: &str = "none";
pub const RUNTIME_WASI_PREVIEW1: &str = "wasi-preview1";

pub const OUTPUTS_PLUGIN_RESULT: &str = "serviceradar.plugin_result.v1";
pub const OUTPUTS_CAMERA_STREAM: &str = "serviceradar.camera_stream.v1";
pub const OUTPUTS_PROXMOX_CONSOLE: &str = "serviceradar.proxmox_console.v1";

/// Mirrors `@allowed_runtimes` in core.
const ALLOWED_RUNTIMES: &[&str] = &[RUNTIME_NONE, RUNTIME_WASI_PREVIEW1];

/// Mirrors `@allowed_outputs` in core.
const ALLOWED_OUTPUTS: &[&str] = &[
    OUTPUTS_PLUGIN_RESULT,
    OUTPUTS_CAMERA_STREAM,
    OUTPUTS_PROXMOX_CONSOLE,
];

/// Mirrors `@allowed_capabilities` in core.
const ALLOWED_CAPABILITIES: &[&str] = &[
    "get_config",
    "log",
    "submit_result",
    "emit_telemetry",
    "http_request",
    "websocket_connect",
    "websocket_send",
    "websocket_recv",
    "websocket_close",
    "camera_media_stream",
    "proxmox_console_stream",
    "tcp_connect",
    "tcp_read",
    "tcp_write",
    "tcp_close",
    "udp_sendto",
    "artifact-staging:v1",
    "advisory-feed:v1",
    "producer-schedule:v1",
    "action-result-ingest:v1",
    "action-only:v1",
];

/// Mirrors `@max_signal_ref_length` in core.
const MAX_SIGNAL_REF_LENGTH: usize = 160;

/// Mirrors `@max_signal_path_length` in core.
const MAX_SIGNAL_PATH_LENGTH: usize = 240;

/// A single reason a manifest would be rejected, addressed by field path.
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

impl std::fmt::Display for ManifestValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.path, self.message)
    }
}

impl std::error::Error for ManifestValidationError {}

/// The `plugin.yaml` shape accepted by ServiceRadar core.
///
/// Validation mirrors `ServiceRadar.Plugins.Manifest` in
/// `elixir/serviceradar_core/lib/serviceradar/plugins/manifest.ex`. Core checks
/// every uploaded manifest against strict allowlists and rejects unknown keys
/// outright, so a manifest that fails [`PluginManifest::validate`] would also be
/// rejected at upload. Keeping the two in sync is the point: plugin authors
/// should learn about a bad manifest at build time, not from an upload error.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub entrypoint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    /// Reports every way the manifest would be rejected by core, so authors can
    /// fix them in one pass.
    pub fn validate(&self) -> Result<(), Vec<ManifestValidationError>> {
        let mut errors = Vec::new();

        for (path, value) in [
            ("id", &self.id),
            ("name", &self.name),
            ("version", &self.version),
            ("entrypoint", &self.entrypoint),
        ] {
            if value.trim().is_empty() {
                errors.push(ManifestValidationError::new(path, "must be set"));
            }
        }

        if let Some(runtime) = self.runtime.as_deref()
            && !ALLOWED_RUNTIMES.contains(&runtime)
        {
            errors.push(ManifestValidationError::new(
                "runtime",
                format!("must be one of: {}", ALLOWED_RUNTIMES.join(", ")),
            ));
        }

        if !ALLOWED_OUTPUTS.contains(&self.outputs.as_str()) {
            errors.push(ManifestValidationError::new(
                "outputs",
                format!("must be one of: {}", ALLOWED_OUTPUTS.join(", ")),
            ));
        }

        if self.capabilities.is_empty() {
            errors.push(ManifestValidationError::new("capabilities", "must be set"));
        }

        for capability in &self.capabilities {
            if !ALLOWED_CAPABILITIES.contains(&capability.as_str()) {
                errors.push(ManifestValidationError::new(
                    "capabilities",
                    format!("contains unsupported capability {capability}"),
                ));
            }
        }

        if self.resources.is_empty() {
            errors.push(ManifestValidationError::new("resources", "must be set"));
        }

        for (index, schema) in self.signal_schemas.iter().enumerate() {
            schema.validate(&format!("signal_schemas[{index}]"), &mut errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Validates the manifest and returns its JSON encoding.
    pub fn serialize(&self) -> Result<Vec<u8>, Vec<ManifestValidationError>> {
        self.validate()?;

        serde_json::to_vec(self).map_err(|err| {
            vec![ManifestValidationError::new(
                "manifest",
                format!("could not be encoded: {err}"),
            )]
        })
    }
}

/// Declares a package-owned signal schema and display contract shipped inside
/// the plugin bundle. Add-ons that emit logs or events are required to declare
/// one.
///
/// The field set is closed: core validates `signal_schemas` entries against an
/// explicit key allowlist and reports `signal_schemas[i].<key> is not allowed`
/// for anything else.
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ocsf_schema_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class_uid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_uid: Option<u32>,
}

impl SignalSchemaContribution {
    /// Builds a contribution with the conventional bundle paths derived from the
    /// schema id. Any field may be overridden afterwards.
    pub fn new(
        id: impl Into<String>,
        version: impl Into<String>,
        signal_type: impl Into<String>,
        payload_kind: impl Into<String>,
    ) -> Self {
        let id = id.into();
        let version = version.into();
        let short = short_schema_name(&id);

        Self {
            payload_schema: format!("schemas/{short}.schema.json"),
            display_contract: format!("display/{short}.display.json"),
            display_contract_id: format!("{id}.display"),
            display_contract_version: version.clone(),
            id,
            version,
            signal_type: signal_type.into(),
            payload_kind: payload_kind.into(),
            ocsf_schema_version: None,
            class_uid: None,
            type_uid: None,
        }
    }

    /// Sets the OCSF classification fields.
    #[must_use]
    pub fn with_ocsf(
        mut self,
        schema_version: impl Into<String>,
        class_uid: u32,
        type_uid: u32,
    ) -> Self {
        self.ocsf_schema_version = Some(schema_version.into());
        self.class_uid = Some(class_uid);
        self.type_uid = Some(type_uid);
        self
    }

    fn validate(&self, path: &str, errors: &mut Vec<ManifestValidationError>) {
        validate_signal_ref(&self.id, &format!("{path}.id"), errors);
        validate_signal_ref(
            &self.display_contract_id,
            &format!("{path}.display_contract_id"),
            errors,
        );
        validate_semver(&self.version, &format!("{path}.version"), errors);
        validate_semver(
            &self.display_contract_version,
            &format!("{path}.display_contract_version"),
            errors,
        );

        if let Some(ocsf_schema_version) = self.ocsf_schema_version.as_deref() {
            validate_semver(
                ocsf_schema_version,
                &format!("{path}.ocsf_schema_version"),
                errors,
            );
        }

        if self.signal_type != SIGNAL_SCHEMA_SIGNAL_TYPE_EVENT
            && self.signal_type != SIGNAL_SCHEMA_SIGNAL_TYPE_LOG
        {
            errors.push(ManifestValidationError::new(
                format!("{path}.signal_type"),
                format!(
                    "must be one of: {SIGNAL_SCHEMA_SIGNAL_TYPE_EVENT}, {SIGNAL_SCHEMA_SIGNAL_TYPE_LOG}"
                ),
            ));
        }

        if self.payload_kind != SIGNAL_SCHEMA_PAYLOAD_KIND_OCSF_EVENT
            && self.payload_kind != SIGNAL_SCHEMA_PAYLOAD_KIND_OTEL_LOG
        {
            errors.push(ManifestValidationError::new(
                format!("{path}.payload_kind"),
                format!(
                    "must be one of: {SIGNAL_SCHEMA_PAYLOAD_KIND_OCSF_EVENT}, {SIGNAL_SCHEMA_PAYLOAD_KIND_OTEL_LOG}"
                ),
            ));
        }

        validate_bundle_path(
            &self.payload_schema,
            &format!("{path}.payload_schema"),
            errors,
        );
        validate_bundle_path(
            &self.display_contract,
            &format!("{path}.display_contract"),
            errors,
        );
    }
}

/// Mirrors core's lowercase reverse-DNS identifier rule.
fn validate_signal_ref(value: &str, path: &str, errors: &mut Vec<ManifestValidationError>) {
    if value.trim().is_empty() {
        errors.push(ManifestValidationError::new(
            path,
            "must be a non-empty string",
        ));
        return;
    }

    if value.len() > MAX_SIGNAL_REF_LENGTH {
        errors.push(ManifestValidationError::new(path, "exceeds maximum length"));
        return;
    }

    let valid_first = value
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    let valid_rest = value
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '.' | '-'));

    if !valid_first || !valid_rest {
        errors.push(ManifestValidationError::new(
            path,
            "must use lowercase letters, numbers, dots, underscores, or hyphens",
        ));
    }
}

/// Mirrors core's semver rule for schema versions.
fn validate_semver(value: &str, path: &str, errors: &mut Vec<ManifestValidationError>) {
    if !is_semver(value) {
        errors.push(ManifestValidationError::new(
            path,
            "must be a valid semver string",
        ));
    }
}

fn is_semver(value: &str) -> bool {
    let core = match value.find(['-', '+']) {
        Some(index) => {
            let (core, suffix) = value.split_at(index);
            let suffix = &suffix[1..];

            if suffix.is_empty()
                || !suffix
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
            {
                return false;
            }

            core
        }
        None => value,
    };

    let parts: Vec<&str> = core.split('.').collect();

    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
}

/// Mirrors core's relative-bundle-path rule.
fn validate_bundle_path(value: &str, path: &str, errors: &mut Vec<ManifestValidationError>) {
    if value.trim().is_empty() {
        errors.push(ManifestValidationError::new(
            path,
            "must be a non-empty string",
        ));
        return;
    }

    if value.len() > MAX_SIGNAL_PATH_LENGTH {
        errors.push(ManifestValidationError::new(path, "exceeds maximum length"));
        return;
    }

    if value.contains("..") {
        errors.push(ManifestValidationError::new(
            path,
            "must not traverse directories",
        ));
        return;
    }

    if !value.ends_with(".json") {
        errors.push(ManifestValidationError::new(
            path,
            "must reference a JSON file",
        ));
        return;
    }

    let segments_valid = !value.starts_with('/')
        && !value.ends_with('/')
        && value.split('/').all(|segment| {
            let valid_first = segment
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'));
            let valid_rest = segment
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'));

            valid_first && valid_rest
        });

    if !segments_valid {
        errors.push(ManifestValidationError::new(
            path,
            "must be a relative bundle path",
        ));
    }
}

fn short_schema_name(id: &str) -> &str {
    id.rsplit('.').next().unwrap_or(id)
}
