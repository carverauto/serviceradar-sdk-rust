//! Local emulation of the agent's host-side OAuth2 client-credentials
//! injection.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use url::Url;

use crate::credential_grant::oauth2_client_credentials_form_fields;
use crate::{
    CREDENTIAL_INJECT_KEY_ALLOW_INSECURE_TLS, CREDENTIAL_INJECT_KEY_HOST,
    CREDENTIAL_INJECT_KEY_METHOD, CREDENTIAL_INJECT_KEY_PATH,
    CREDENTIAL_INJECT_OAUTH2_CLIENT_CREDENTIALS, CredentialBrokerAllow, CredentialBrokerGrant,
    HttpRequest,
};

/// Why the local host denied a request a grant covers. The agent answers all
/// of these with host error `-2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GrantDenial {
    Target,
    InsecureTls,
    InvalidSpec,
    CredentialMissing,
}

/// Returns the synthetic access token the local host injects for an
/// `oauth2_client_credentials` grant. It is derived from the grant identity
/// only, never from credential material, so a local HTTP handler can recognize
/// it without a token endpoint. The value is identical to the Go SDK's
/// `LocalOAuth2BearerToken` for the same grant.
pub fn local_oauth2_bearer_token(grant: &CredentialBrokerGrant) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"serviceradar.local_oauth2\x00");
    hasher.update(grant.grant_id.as_bytes());
    hasher.update(b"\x00");
    hasher.update(
        grant
            .credential_secret_ref
            .as_deref()
            .unwrap_or_default()
            .as_bytes(),
    );
    let sum = hasher.finalize();
    let mut token = String::from("local-oauth2.");
    for byte in &sum[..16] {
        token.push_str(&format!("{byte:02x}"));
    }
    token
}

/// Emulates the agent's OAuth2 client-credentials injection. A grant is
/// selected by its allow scope; a selected grant whose inject target, TLS
/// policy or credential fields do not fit the request denies it, as on the
/// agent. Requests no grant covers pass through unchanged.
pub(super) fn apply_local_credential_grants(
    grants: &[CredentialBrokerGrant],
    credentials: &BTreeMap<String, String>,
    request: &mut HttpRequest,
) -> Result<(), GrantDenial> {
    if grants.is_empty() {
        return Ok(());
    }
    let url = Url::parse(request.url.trim()).map_err(|_| GrantDenial::Target)?;
    let Some(host) = url.host_str().filter(|host| !host.is_empty()) else {
        return Err(GrantDenial::Target);
    };
    let target = RequestTarget::new(&url, host, &request.method);

    let now = OffsetDateTime::now_utc();
    for grant in grants {
        if grant.inject_type() != CREDENTIAL_INJECT_OAUTH2_CLIENT_CREDENTIALS
            || grant_expired(grant, now)
            || !grant_allows(grant.allow.as_ref(), &target)
        {
            continue;
        }
        let spec = grant.inject_spec();
        if !inject_targets_request(&spec, &target) {
            return Err(GrantDenial::Target);
        }
        let allow_insecure = spec
            .get(CREDENTIAL_INJECT_KEY_ALLOW_INSECURE_TLS)
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("true"));
        if request.insecure_skip_verify && !allow_insecure {
            return Err(GrantDenial::InsecureTls);
        }
        let fields =
            oauth2_client_credentials_form_fields(&spec).map_err(|_| GrantDenial::InvalidSpec)?;
        for source in fields.values() {
            if local_credential(credentials, source).trim().is_empty() {
                return Err(GrantDenial::CredentialMissing);
            }
        }

        request
            .headers
            .retain(|key, _| !key.eq_ignore_ascii_case("Authorization"));
        request.headers.insert(
            "Authorization".to_string(),
            format!("Bearer {}", local_oauth2_bearer_token(grant)),
        );
        return Ok(());
    }
    Ok(())
}

/// The request attributes the allow scope and inject target compare against.
struct RequestTarget {
    scheme: String,
    method: String,
    /// Host without IPv6 brackets.
    hostname: String,
    /// Host with an explicit non-default port, as written in the URL.
    host_port: String,
    path: String,
    port: Option<u16>,
}

impl RequestTarget {
    fn new(url: &Url, host: &str, method: &str) -> Self {
        let method = match method.trim() {
            "" => "GET".to_string(),
            other => other.to_ascii_uppercase(),
        };
        let hostname = host
            .strip_prefix('[')
            .and_then(|inner| inner.strip_suffix(']'))
            .unwrap_or(host)
            .to_string();
        let host_port = match url.port() {
            Some(port) => format!("{host}:{port}"),
            None => host.to_string(),
        };
        let path = match url.path() {
            "" => "/".to_string(),
            path => path.to_string(),
        };
        Self {
            scheme: url.scheme().to_string(),
            method,
            hostname,
            host_port,
            path,
            port: url.port().or(match url.scheme() {
                "http" => Some(80),
                "https" => Some(443),
                _ => None,
            }),
        }
    }

    fn host_matches(&self, candidate: &str) -> bool {
        let candidate = candidate.trim();
        candidate.eq_ignore_ascii_case(&self.hostname)
            || candidate.eq_ignore_ascii_case(&self.host_port)
    }
}

fn local_credential<'a>(credentials: &'a BTreeMap<String, String>, field: &str) -> &'a str {
    credentials
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(field))
        .map(|(_, value)| value.as_str())
        .unwrap_or("")
}

fn grant_expired(grant: &CredentialBrokerGrant, now: OffsetDateTime) -> bool {
    let Some(raw) = grant
        .expires_at
        .as_deref()
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
    else {
        return false;
    };
    match OffsetDateTime::parse(raw, &Rfc3339) {
        Ok(expires_at) => now >= expires_at,
        Err(_) => true,
    }
}

/// Mirrors the agent's allow-scope check for non-AWX grants: hosts are
/// required, the other lists restrict only when set.
fn grant_allows(allow: Option<&CredentialBrokerAllow>, target: &RequestTarget) -> bool {
    let Some(allow) = allow.filter(|allow| !allow.hosts.is_empty()) else {
        return false;
    };
    if !allow.schemes.is_empty() && !folded_contains(&allow.schemes, &target.scheme) {
        return false;
    }
    if !allow.methods.is_empty() && !folded_contains(&allow.methods, &target.method) {
        return false;
    }
    if !allow.paths.is_empty() && !grant_path_allowed(&allow.paths, &target.path) {
        return false;
    }
    if !allow.hosts.iter().any(|host| target.host_matches(host)) {
        return false;
    }
    if !allow.ports.is_empty() {
        let Some(port) = target.port else {
            return false;
        };
        return allow.ports.contains(&i64::from(port));
    }
    true
}

fn inject_targets_request(spec: &BTreeMap<String, String>, target: &RequestTarget) -> bool {
    let expected = |key: &str| spec.get(key).map(|value| value.trim()).unwrap_or("");
    let method = expected(CREDENTIAL_INJECT_KEY_METHOD);
    if !method.is_empty() && !method.eq_ignore_ascii_case(&target.method) {
        return false;
    }
    let host = expected(CREDENTIAL_INJECT_KEY_HOST);
    if !host.is_empty() && !target.host_matches(host) {
        return false;
    }
    let path = expected(CREDENTIAL_INJECT_KEY_PATH);
    path.is_empty() || path == target.path
}

fn grant_path_allowed(patterns: &[String], requested: &str) -> bool {
    patterns
        .iter()
        .map(|pattern| pattern.trim())
        .any(|pattern| {
            if pattern.is_empty() {
                return false;
            }
            if pattern.strip_prefix('=') == Some(requested) || pattern == requested {
                return true;
            }
            if let Some(prefix) = pattern.strip_suffix('*') {
                if requested.starts_with(prefix) {
                    return true;
                }
            }
            pattern.ends_with('/') && requested.starts_with(pattern)
        })
}

fn folded_contains(values: &[String], value: &str) -> bool {
    values
        .iter()
        .any(|candidate| candidate.trim().eq_ignore_ascii_case(value))
}
