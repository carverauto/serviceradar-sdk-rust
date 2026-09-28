//! Typed credential broker grants.
//!
//! The host injects the credential into the outbound request; the plugin never
//! reads the secret. These types name the inject types and keys the host
//! accepts and build specs with the exact key names it reads.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, SdkResult};
use crate::plugin_inputs::CredentialBrokerGrant;

// Credential broker inject types accepted by ServiceRadar core's integration
// descriptor validator and applied by the agent host.
pub const CREDENTIAL_INJECT_HTTP_HEADER: &str = "http_header";
/// Alias of [`CREDENTIAL_INJECT_HTTP_HEADER`].
pub const CREDENTIAL_INJECT_HEADER: &str = "header";
pub const CREDENTIAL_INJECT_BEARER_TOKEN: &str = "bearer_token";
pub const CREDENTIAL_INJECT_BASIC_AUTH: &str = "basic_auth";
/// Alias of [`CREDENTIAL_INJECT_BASIC_AUTH`].
pub const CREDENTIAL_INJECT_HTTP_BASIC_AUTH: &str = "http_basic_auth";
pub const CREDENTIAL_INJECT_QUERY: &str = "query";
/// Alias of [`CREDENTIAL_INJECT_QUERY`].
pub const CREDENTIAL_INJECT_QUERY_PARAM: &str = "query_param";
/// Alias of [`CREDENTIAL_INJECT_QUERY`].
pub const CREDENTIAL_INJECT_HTTP_QUERY: &str = "http_query";
pub const CREDENTIAL_INJECT_FORM_URLENCODED: &str = "form_urlencoded";
pub const CREDENTIAL_INJECT_OAUTH2_PASSWORD_BEARER: &str = "oauth2_password_bearer";
pub const CREDENTIAL_INJECT_OAUTH2_CLIENT_CREDENTIALS: &str = "oauth2_client_credentials";

// Inject spec keys the host reads. `field_<credential field>` maps a stored
// credential field to a form field; `fixed_<form field>` sends a literal value.
pub const CREDENTIAL_INJECT_KEY_TYPE: &str = "type";
pub const CREDENTIAL_INJECT_KEY_NAME: &str = "name";
pub const CREDENTIAL_INJECT_KEY_SCHEME: &str = "scheme";
pub const CREDENTIAL_INJECT_KEY_METHOD: &str = "method";
pub const CREDENTIAL_INJECT_KEY_HOST: &str = "host";
pub const CREDENTIAL_INJECT_KEY_PATH: &str = "path";
pub const CREDENTIAL_INJECT_KEY_ALLOW_INSECURE_TLS: &str = "allow_insecure_tls";
pub const CREDENTIAL_INJECT_KEY_TOKEN_METHOD: &str = "token_method";
pub const CREDENTIAL_INJECT_KEY_TOKEN_HOST: &str = "token_host";
pub const CREDENTIAL_INJECT_KEY_TOKEN_PORT: &str = "token_port";
pub const CREDENTIAL_INJECT_KEY_TOKEN_PATH: &str = "token_path";
pub const CREDENTIAL_INJECT_FIELD_PREFIX: &str = "field_";
pub const CREDENTIAL_INJECT_FIXED_PREFIX: &str = "fixed_";

const OAUTH2_FORM_GRANT_TYPE: &str = "grant_type";
const OAUTH2_FORM_CLIENT_ID: &str = "client_id";
const OAUTH2_FORM_CLIENT_SECRET: &str = "client_secret";
const OAUTH2_GRANT_CLIENT_CREDENTIALS: &str = "client_credentials";

/// The request scope a credential broker grant may be applied to. The host
/// selects a grant only when the request matches it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CredentialBrokerAllow {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub schemes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub methods: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hosts: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<i64>,
}

impl CredentialBrokerGrant {
    /// Returns the normalized (trimmed, lowercased) inject type, or `""` when
    /// none is set.
    pub fn inject_type(&self) -> String {
        self.inject_spec()
            .get(CREDENTIAL_INJECT_KEY_TYPE)
            .map(|value| value.trim().to_ascii_lowercase())
            .unwrap_or_default()
    }

    /// Returns the inject map as the host reads it: every value is a string.
    /// Numbers and booleans are formatted; `null` entries are dropped.
    pub fn inject_spec(&self) -> BTreeMap<String, String> {
        self.inject
            .iter()
            .filter_map(|(key, value)| {
                let value = match value {
                    Value::Null => return None,
                    Value::String(text) => text.clone(),
                    other => other.to_string(),
                };
                Some((key.clone(), value))
            })
            .collect()
    }
}

/// Typed builder for an `oauth2_client_credentials` inject spec. The host
/// exchanges the stored client credentials at the token endpoint and sends the
/// resulting access token as `Authorization: Bearer` on requests the grant
/// covers.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OAuth2ClientCredentialsInject {
    /// Must be `POST`, the only method the host exchanges with.
    pub token_method: String,
    /// `token_host`, `token_port` and `token_path` locate the https token
    /// endpoint.
    pub token_host: String,
    pub token_port: u16,
    pub token_path: String,
    /// Maps a stored credential field to the form field it fills.
    pub fields: BTreeMap<String, String>,
    /// Maps a form field to a literal value, such as `grant_type` or `scope`.
    pub fixed: BTreeMap<String, String>,
    /// When set, `method`, `host` and `path` restrict injection to one exact
    /// request.
    pub method: String,
    pub host: String,
    pub path: String,
    /// Permits the grant on requests that skip TLS verification.
    pub allow_insecure_tls: bool,
}

impl OAuth2ClientCredentialsInject {
    /// Returns a spec that posts `client_id` and `client_secret` from the
    /// stored credential with `grant_type=client_credentials`.
    pub fn new(
        token_host: impl Into<String>,
        token_port: u16,
        token_path: impl Into<String>,
    ) -> Self {
        Self {
            token_method: "POST".to_string(),
            token_host: token_host.into(),
            token_port,
            token_path: token_path.into(),
            fields: BTreeMap::from([
                (
                    OAUTH2_FORM_CLIENT_ID.to_string(),
                    OAUTH2_FORM_CLIENT_ID.to_string(),
                ),
                (
                    OAUTH2_FORM_CLIENT_SECRET.to_string(),
                    OAUTH2_FORM_CLIENT_SECRET.to_string(),
                ),
            ]),
            fixed: BTreeMap::from([(
                OAUTH2_FORM_GRANT_TYPE.to_string(),
                OAUTH2_GRANT_CLIENT_CREDENTIALS.to_string(),
            )]),
            ..Self::default()
        }
    }

    /// Maps a stored credential field onto a token form field.
    pub fn with_field(
        mut self,
        credential_field: impl Into<String>,
        form_field: impl Into<String>,
    ) -> Self {
        self.fields
            .insert(credential_field.into(), form_field.into());
        self
    }

    /// Sends a literal token form field.
    pub fn with_fixed(mut self, form_field: impl Into<String>, value: impl Into<String>) -> Self {
        self.fixed.insert(form_field.into(), value.into());
        self
    }

    /// Sends a fixed OAuth2 scope with the token request.
    pub fn with_scope(self, scope: impl Into<String>) -> Self {
        self.with_fixed("scope", scope)
    }

    /// Restricts injection to one exact method, host and path.
    pub fn with_request_target(
        mut self,
        method: impl Into<String>,
        host: impl Into<String>,
        path: impl Into<String>,
    ) -> Self {
        self.method = method.into();
        self.host = host.into();
        self.path = path.into();
        self
    }

    /// Permits the grant on requests that skip TLS verification.
    pub fn with_allow_insecure_tls(mut self, allow: bool) -> Self {
        self.allow_insecure_tls = allow;
        self
    }

    /// Applies the checks the host runs before a token exchange.
    pub fn validate(&self) -> SdkResult<()> {
        self.spec().map(|_| ())
    }

    /// Returns the validated inject map with the exact keys the host reads.
    pub fn spec(&self) -> SdkResult<BTreeMap<String, String>> {
        let mut spec = BTreeMap::from([
            (
                CREDENTIAL_INJECT_KEY_TYPE.to_string(),
                CREDENTIAL_INJECT_OAUTH2_CLIENT_CREDENTIALS.to_string(),
            ),
            (
                CREDENTIAL_INJECT_KEY_TOKEN_METHOD.to_string(),
                self.token_method.trim().to_ascii_uppercase(),
            ),
            (
                CREDENTIAL_INJECT_KEY_TOKEN_HOST.to_string(),
                self.token_host.trim().to_string(),
            ),
            (
                CREDENTIAL_INJECT_KEY_TOKEN_PORT.to_string(),
                self.token_port.to_string(),
            ),
            (
                CREDENTIAL_INJECT_KEY_TOKEN_PATH.to_string(),
                self.token_path.trim().to_string(),
            ),
        ]);
        for (key, value) in [
            (
                CREDENTIAL_INJECT_KEY_METHOD,
                self.method.trim().to_ascii_uppercase(),
            ),
            (CREDENTIAL_INJECT_KEY_HOST, self.host.trim().to_string()),
            (CREDENTIAL_INJECT_KEY_PATH, self.path.trim().to_string()),
        ] {
            if !value.is_empty() {
                spec.insert(key.to_string(), value);
            }
        }
        if self.allow_insecure_tls {
            spec.insert(
                CREDENTIAL_INJECT_KEY_ALLOW_INSECURE_TLS.to_string(),
                "true".to_string(),
            );
        }
        for (source, target) in &self.fields {
            spec.insert(
                format!("{CREDENTIAL_INJECT_FIELD_PREFIX}{source}"),
                target.clone(),
            );
        }
        for (field, value) in &self.fixed {
            spec.insert(
                format!("{CREDENTIAL_INJECT_FIXED_PREFIX}{field}"),
                value.clone(),
            );
        }

        oauth2_client_credentials_form_fields(&spec)?;
        Ok(spec)
    }

    /// Returns the spec in the shape of [`CredentialBrokerGrant::inject`].
    pub fn inject(&self) -> SdkResult<BTreeMap<String, Value>> {
        Ok(self
            .spec()?
            .into_iter()
            .map(|(key, value)| (key, Value::String(value)))
            .collect())
    }
}

/// Mirrors the host's token URL and form checks. Returns form field to
/// credential field for the `field_*` entries.
pub(crate) fn oauth2_client_credentials_form_fields(
    spec: &BTreeMap<String, String>,
) -> SdkResult<BTreeMap<String, String>> {
    let invalid = |reason: &str| {
        Err(Error::InvalidCredentialInject(format!(
            "invalid {CREDENTIAL_INJECT_OAUTH2_CLIENT_CREDENTIALS} inject spec: {reason}"
        )))
    };
    let get = |key: &str| spec.get(key).map(|value| value.trim()).unwrap_or("");

    if !get(CREDENTIAL_INJECT_KEY_TYPE)
        .eq_ignore_ascii_case(CREDENTIAL_INJECT_OAUTH2_CLIENT_CREDENTIALS)
    {
        return invalid(&format!(
            "type must be {CREDENTIAL_INJECT_OAUTH2_CLIENT_CREDENTIALS}"
        ));
    }
    if !get(CREDENTIAL_INJECT_KEY_TOKEN_METHOD).eq_ignore_ascii_case("POST") {
        return invalid("token_method must be POST");
    }
    let host = get(CREDENTIAL_INJECT_KEY_TOKEN_HOST);
    let path = get(CREDENTIAL_INJECT_KEY_TOKEN_PATH);
    let port = get(CREDENTIAL_INJECT_KEY_TOKEN_PORT).parse::<i64>().ok();
    if host.is_empty() || host.contains(['/', '@', '?', '#']) {
        return invalid("token_host must be a bare host");
    }
    if !matches!(port, Some(1..=65535)) {
        return invalid("token_port must be between 1 and 65535");
    }
    if !path.starts_with('/') || path.contains(['?', '#']) {
        return invalid("token_path must be an absolute path without query or fragment");
    }

    let mut fields = BTreeMap::new();
    for (key, target) in spec {
        let Some(source) = key.strip_prefix(CREDENTIAL_INJECT_FIELD_PREFIX) else {
            continue;
        };
        let target = target.trim();
        if !valid_inject_suffix(source) || target.is_empty() {
            return invalid("field_* entries need a credential field and a form field");
        }
        if fields.contains_key(target) {
            return invalid(&format!("form field {target} is set twice"));
        }
        fields.insert(target.to_string(), source.to_string());
    }
    let mut fixed = BTreeMap::new();
    for (key, value) in spec {
        let Some(field) = key.strip_prefix(CREDENTIAL_INJECT_FIXED_PREFIX) else {
            continue;
        };
        if !valid_inject_suffix(field) {
            return invalid("fixed_* entries need a form field");
        }
        if fields.contains_key(field) {
            return invalid(&format!("form field {field} is set twice"));
        }
        fixed.insert(field.to_string(), value.clone());
    }
    if fixed.get(OAUTH2_FORM_GRANT_TYPE).map(String::as_str)
        != Some(OAUTH2_GRANT_CLIENT_CREDENTIALS)
    {
        return invalid("fixed_grant_type must be client_credentials");
    }
    for required in [OAUTH2_FORM_CLIENT_ID, OAUTH2_FORM_CLIENT_SECRET] {
        let fixed_value = fixed.get(required).map(String::as_str).unwrap_or("");
        if !fields.contains_key(required) && fixed_value.is_empty() {
            return invalid(&format!(
                "{required} must be mapped from a credential field"
            ));
        }
    }
    Ok(fields)
}

/// Mirrors core's `^(field|fixed)_[a-z0-9_.-]+$` key rule.
fn valid_inject_suffix(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-'))
}

#[cfg(test)]
pub(crate) mod tests;
