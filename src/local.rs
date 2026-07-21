//! Source-native development host support.
//!
//! This module emulates the SDK host calls needed for local plugin development.
//! It does not emulate package signing, approval, assignment, or production
//! authorization.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::config::MAX_PAYLOAD_BYTES;
use crate::error::{
    Error, HOST_ERR_INTERNAL, HOST_ERR_INVALID, HOST_ERR_NOT_FOUND, HOST_ERR_OK,
    HOST_ERR_TOO_LARGE, SdkResult,
};
use crate::host::{self, HostBackend};
use crate::{HttpRequest, HttpResponse};

pub const LOCAL_ENV_FILE_VARIABLE: &str = "SERVICERADAR_PLUGIN_ENV_FILE";
pub const LOCAL_CONFIG_FILE_VARIABLE: &str = "SERVICERADAR_PLUGIN_CONFIG_FILE";
pub const LOCAL_CONFIG_JSON_VARIABLE: &str = "SERVICERADAR_PLUGIN_CONFIG_JSON";
pub const LOCAL_ACTION_FILE_VARIABLE: &str = "SERVICERADAR_PLUGIN_ACTION_FILE";
pub const LOCAL_ACTION_JSON_VARIABLE: &str = "SERVICERADAR_PLUGIN_ACTION_JSON";
pub const LOCAL_CREDENTIAL_PREFIX: &str = "SERVICERADAR_CREDENTIAL_";

/// Selects source-native plugin inputs. Explicit values take precedence over
/// environment variables. `None` uses the current process environment; an
/// empty `Some` map supplies no process variables.
#[derive(Default)]
pub struct LocalInputOptions {
    pub env_file: Option<PathBuf>,
    pub config_file: Option<PathBuf>,
    pub config_json: Option<Vec<u8>>,
    pub action_file: Option<PathBuf>,
    pub action_json: Option<Vec<u8>>,
    pub environment: Option<BTreeMap<String, String>>,
    pub credential_prefix: Option<String>,
}

/// Public runtime inputs and separately retained local host credentials.
pub struct LocalInputs {
    config_json: Vec<u8>,
    action_json: Option<Vec<u8>>,
    credentials: BTreeMap<String, String>,
}

impl LocalInputs {
    /// Loads public config/action JSON and credential-prefixed local host data.
    pub fn load(options: LocalInputOptions) -> SdkResult<Self> {
        let process_env = options.environment.unwrap_or_else(process_environment);
        let (env_file, explicit_env_file) = local_env_file(options.env_file, &process_env);
        let file_env = match read_local_env_file(&env_file) {
            Ok(values) => values,
            Err(err) if !explicit_env_file && err.kind() == std::io::ErrorKind::NotFound => {
                BTreeMap::new()
            }
            Err(err) => {
                return Err(Error::Message(format!(
                    "failed to read local environment file: {err}"
                )));
            }
        };
        let merged_env = overlay_environment(file_env, process_env);

        let config_json = resolve_local_json(
            "config",
            options.config_json.as_deref(),
            options.config_file.as_deref(),
            merged_env.get(LOCAL_CONFIG_JSON_VARIABLE),
            merged_env.get(LOCAL_CONFIG_FILE_VARIABLE),
            true,
        )?
        .expect("required config must be present");
        let action_json = resolve_local_json(
            "action invocation",
            options.action_json.as_deref(),
            options.action_file.as_deref(),
            merged_env.get(LOCAL_ACTION_JSON_VARIABLE),
            merged_env.get(LOCAL_ACTION_FILE_VARIABLE),
            false,
        )?;

        let prefix = options
            .credential_prefix
            .as_deref()
            .filter(|value| !value.is_empty())
            .unwrap_or(LOCAL_CREDENTIAL_PREFIX);
        let credentials = merged_env
            .into_iter()
            .filter_map(|(key, value)| {
                key.strip_prefix(prefix)
                    .filter(|field| !field.is_empty())
                    .map(|field| (field.trim().to_ascii_lowercase(), value))
            })
            .collect();

        Ok(Self {
            config_json,
            action_json,
            credentials,
        })
    }

    /// Returns the production-shaped host config. Credentials are never added.
    pub fn runtime_config_json(&self) -> SdkResult<Vec<u8>> {
        let mut config = decode_json_object(&self.config_json, "local plugin config")?;
        if let Some(action_json) = &self.action_json {
            let action = decode_json_object(action_json, "local action invocation")?;
            config.insert("action_invocation".to_string(), Value::Object(action));
        }
        Ok(serde_json::to_vec(&Value::Object(config))?)
    }

    /// Returns one local host credential field by case-insensitive name.
    pub fn credential(&self, name: &str) -> Option<&str> {
        self.credentials
            .get(&name.trim().to_ascii_lowercase())
            .map(String::as_str)
    }

    /// Returns a copy for construction of a trusted local host adapter.
    pub fn credentials(&self) -> BTreeMap<String, String> {
        self.credentials.clone()
    }
}

/// Handles one host-mediated HTTP request during a native local run.
pub type LocalHttpHandler = Box<dyn FnMut(HttpRequest) -> SdkResult<HttpResponse> + Send>;

/// Configures one source-native plugin execution.
#[derive(Default)]
pub struct LocalHostOptions {
    pub config_json: Vec<u8>,
    pub http_handler: Option<LocalHttpHandler>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalHostLog {
    pub level: u32,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalHostCapture {
    pub result_json: Vec<u8>,
    pub telemetry_json: Vec<Vec<u8>>,
    pub logs: Vec<LocalHostLog>,
}

#[derive(Default)]
struct LocalHostState {
    result_json: Vec<u8>,
    telemetry_json: Vec<Vec<u8>>,
    logs: Vec<LocalHostLog>,
}

struct LocalHostBackend {
    config_json: Vec<u8>,
    http_handler: Option<LocalHttpHandler>,
    state: Arc<Mutex<LocalHostState>>,
}

impl HostBackend for LocalHostBackend {
    fn get_config(&mut self, buf: &mut [u8]) -> i32 {
        if self.config_json.len() > buf.len() {
            return HOST_ERR_TOO_LARGE;
        }
        buf[..self.config_json.len()].copy_from_slice(&self.config_json);
        self.config_json.len() as i32
    }

    fn log(&mut self, level: u32, msg: &[u8]) {
        if msg.is_empty() {
            return;
        }
        self.state
            .lock()
            .expect("local host state mutex poisoned")
            .logs
            .push(LocalHostLog {
                level,
                message: String::from_utf8_lossy(msg).into_owned(),
            });
    }

    fn submit_result(&mut self, payload: &[u8]) -> i32 {
        if payload.is_empty() {
            return HOST_ERR_INVALID;
        }
        if payload.len() > MAX_PAYLOAD_BYTES {
            return HOST_ERR_TOO_LARGE;
        }
        self.state
            .lock()
            .expect("local host state mutex poisoned")
            .result_json = payload.to_vec();
        HOST_ERR_OK
    }

    fn emit_telemetry(&mut self, payload: &[u8]) -> i32 {
        if payload.is_empty() {
            return HOST_ERR_INVALID;
        }
        if payload.len() > MAX_PAYLOAD_BYTES {
            return HOST_ERR_TOO_LARGE;
        }
        self.state
            .lock()
            .expect("local host state mutex poisoned")
            .telemetry_json
            .push(payload.to_vec());
        HOST_ERR_OK
    }

    fn http_request(&mut self, request: &[u8], response: &mut [u8]) -> i32 {
        let Some(handler) = self.http_handler.as_mut() else {
            return HOST_ERR_NOT_FOUND;
        };
        let request = match decode_local_http_request(request) {
            Ok(request) => request,
            Err(_) => return HOST_ERR_INVALID,
        };
        let response_payload = match handler(request)
            .and_then(|response| encode_local_http_response(&response).map_err(Error::from))
        {
            Ok(payload) => payload,
            Err(_) => return HOST_ERR_INTERNAL,
        };
        if response_payload.len() > response.len() {
            return HOST_ERR_TOO_LARGE;
        }
        response[..response_payload.len()].copy_from_slice(&response_payload);
        response_payload.len() as i32
    }
}

/// Runs ordinary SDK calls against one process-scoped native host. The tuple
/// retains captured output even when the plugin callback returns an error.
pub fn run_local_host<F>(options: LocalHostOptions, run: F) -> (LocalHostCapture, SdkResult<()>)
where
    F: FnOnce() -> SdkResult<()>,
{
    if options.config_json.len() > MAX_PAYLOAD_BYTES {
        return (
            LocalHostCapture::default(),
            Err(Error::Message(
                "local host config exceeds the SDK payload limit".to_string(),
            )),
        );
    }

    let state = Arc::new(Mutex::new(LocalHostState::default()));
    let backend = LocalHostBackend {
        config_json: options.config_json,
        http_handler: options.http_handler,
        state: Arc::clone(&state),
    };
    let _guard = host::install_native_backend(Box::new(backend));
    let result = run();
    let capture = {
        let state = state.lock().expect("local host state mutex poisoned");
        LocalHostCapture {
            result_json: state.result_json.clone(),
            telemetry_json: state.telemetry_json.clone(),
            logs: state.logs.clone(),
        }
    };
    (capture, result)
}

#[derive(Deserialize)]
struct LocalHttpRequestPayload {
    #[serde(default)]
    method: String,
    url: String,
    #[serde(default)]
    headers: BTreeMap<String, String>,
    body: Option<String>,
    body_base64: Option<String>,
    #[serde(default)]
    timeout_ms: u32,
    #[serde(default)]
    insecure_skip_verify: bool,
}

#[derive(Serialize)]
struct LocalHttpResponsePayload {
    status: i32,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    headers: BTreeMap<String, String>,
    body_base64: String,
    body_encoding: &'static str,
}

fn decode_local_http_request(payload: &[u8]) -> SdkResult<HttpRequest> {
    let payload: LocalHttpRequestPayload = serde_json::from_slice(payload)?;
    if payload.url.trim().is_empty() || (payload.body.is_some() && payload.body_base64.is_some()) {
        return Err(Error::Message("invalid local HTTP request".to_string()));
    }
    let (body, body_base64) = if let Some(value) = payload.body_base64 {
        (
            base64::engine::general_purpose::STANDARD
                .decode(value)
                .map_err(|_| Error::Message("invalid local HTTP body".to_string()))?,
            true,
        )
    } else {
        (payload.body.unwrap_or_default().into_bytes(), false)
    };
    Ok(HttpRequest {
        method: payload.method,
        url: payload.url,
        headers: payload.headers,
        body,
        body_base64,
        timeout_ms: payload.timeout_ms,
        insecure_skip_verify: payload.insecure_skip_verify,
    })
}

fn encode_local_http_response(response: &HttpResponse) -> serde_json::Result<Vec<u8>> {
    serde_json::to_vec(&LocalHttpResponsePayload {
        status: response.status,
        headers: response.headers.clone(),
        body_base64: base64::engine::general_purpose::STANDARD.encode(&response.body),
        body_encoding: "base64",
    })
}

fn process_environment() -> BTreeMap<String, String> {
    env::vars_os()
        .filter_map(|(key, value)| Some((key.into_string().ok()?, value.into_string().ok()?)))
        .collect()
}

fn local_env_file(
    explicit: Option<PathBuf>,
    process_env: &BTreeMap<String, String>,
) -> (PathBuf, bool) {
    if let Some(path) = explicit {
        return (path, true);
    }
    if let Some(path) = process_env
        .get(LOCAL_ENV_FILE_VARIABLE)
        .filter(|value| !value.trim().is_empty())
    {
        return (PathBuf::from(path), true);
    }
    (PathBuf::from(".env"), false)
}

fn overlay_environment(
    mut base: BTreeMap<String, String>,
    process: BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    base.extend(process);
    base
}

fn resolve_local_json(
    label: &str,
    explicit_json: Option<&[u8]>,
    explicit_file: Option<&Path>,
    environment_json: Option<&String>,
    environment_file: Option<&String>,
    required: bool,
) -> SdkResult<Option<Vec<u8>>> {
    let mut payload = explicit_json
        .map(trimmed_bytes)
        .filter(|value| !value.is_empty());
    if payload.is_none() {
        if let Some(path) = explicit_file {
            payload = Some(trimmed_bytes(&fs::read(path)?));
        }
    }
    if payload.is_none() {
        payload = environment_json
            .map(|value| trimmed_bytes(value.as_bytes()))
            .filter(|value| !value.is_empty());
        if payload.is_none() {
            if let Some(path) = environment_file
                .filter(|value| !value.trim().is_empty())
                .map(PathBuf::from)
            {
                payload = Some(trimmed_bytes(&fs::read(path)?));
            }
        }
    }
    let Some(payload) = payload.filter(|value| !value.is_empty()) else {
        if required {
            return Err(Error::Message(format!(
                "local {label} JSON or file is required"
            )));
        }
        return Ok(None);
    };
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err(Error::Message(format!(
            "local {label} exceeds the SDK payload limit"
        )));
    }
    decode_json_object(&payload, label)?;
    Ok(Some(payload))
}

fn trimmed_bytes(value: &[u8]) -> Vec<u8> {
    let Some(start) = value.iter().position(|byte| !byte.is_ascii_whitespace()) else {
        return Vec::new();
    };
    let end = value
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .expect("start proves an ending byte exists");
    value[start..=end].to_vec()
}

fn decode_json_object(payload: &[u8], label: &str) -> SdkResult<Map<String, Value>> {
    match serde_json::from_slice(payload)? {
        Value::Object(object) => Ok(object),
        _ => Err(Error::Message(format!("{label} must be one JSON object"))),
    }
}

fn read_local_env_file(path: &Path) -> std::io::Result<BTreeMap<String, String>> {
    let content = fs::read_to_string(path)?;
    let mut result = BTreeMap::new();
    for (index, source_line) in content.lines().enumerate() {
        let mut line = source_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(value) = line.strip_prefix("export ") {
            line = value.trim();
        }
        let Some((key, raw_value)) = line.split_once('=') else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid environment entry at line {}", index + 1),
            ));
        };
        let key = key.trim();
        if !valid_local_env_key(key) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid environment key at line {}", index + 1),
            ));
        }
        let value = parse_local_env_value(raw_value.trim()).map_err(|()| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid environment value for {key} at line {}", index + 1),
            )
        })?;
        result.insert(key.to_string(), value);
    }
    Ok(result)
}

fn valid_local_env_key(value: &str) -> bool {
    let mut chars = value.bytes();
    let Some(first) = chars.next() else {
        return false;
    };
    is_local_env_key_start(first)
        && chars.all(|byte| is_local_env_key_start(byte) || byte.is_ascii_digit())
}

fn is_local_env_key_start(value: u8) -> bool {
    value == b'_' || value.is_ascii_alphabetic()
}

fn parse_local_env_value(value: &str) -> Result<String, ()> {
    if value.is_empty() {
        return Ok(String::new());
    }
    if value.starts_with('\'') {
        return value
            .strip_prefix('\'')
            .and_then(|inner| inner.strip_suffix('\''))
            .map(str::to_string)
            .ok_or(());
    }
    if value.starts_with('"') {
        return serde_json::from_str(value).map_err(|_| ());
    }
    Ok(value
        .split_once(" #")
        .map_or(value, |(before, _)| before)
        .trim()
        .to_string())
}

#[cfg(test)]
mod tests;
