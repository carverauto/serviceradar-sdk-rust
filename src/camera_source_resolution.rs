//! Camera source resolution lets ServiceRadar open a camera stream without
//! storing its address. A stream address is a secret: an RTSP path alias works
//! as a bearer token and other addresses carry credentials. When a stream
//! opens, the agent runs the camera inventory plugin that discovered the
//! camera with a resolution request in its config, and the plugin answers with
//! the address. The agent hands that address to its stream reader and keeps it
//! nowhere else.
//!
//! A plugin's entrypoint checks for the request before doing anything else. A
//! resolver never creates or enables a stream on the camera or controller, and
//! never logs the address.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::get_config_bytes;
use crate::error::{Error, SdkResult};
use crate::execute::submit_result_payload;

/// Host-owned config key carrying a resolution request.
pub const CAMERA_SOURCE_RESOLUTION_REQUEST_CONFIG_KEY: &str =
    "_serviceradar_camera_source_resolution_request";
/// Identifies the request document.
pub const CAMERA_SOURCE_RESOLUTION_REQUEST_SCHEMA_V1: &str =
    "serviceradar.camera_source_resolution_request.v1";
/// Identifies the plugin's answer.
pub const CAMERA_SOURCE_RESOLUTION_SCHEMA_V1: &str = "serviceradar.camera_source_resolution.v1";

/// Host-owned config key that tells a camera inventory plugin how to report
/// stream addresses.
pub const CAMERA_STREAM_ADDRESSES_CONFIG_KEY: &str = "_serviceradar_camera_stream_addresses";
/// The value that asks the plugin to leave stream addresses out of inventory
/// results and report only each profile's origin (scheme, host and port).
pub const CAMERA_STREAM_ADDRESSES_OMIT: &str = "omit";

/// A failure a resolver may report. The host reports any other class as
/// unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraSourceFailure {
    /// The camera or profile is unknown to the controller, or streaming is
    /// disabled for it.
    NotFound,
    AuthenticationFailed,
    DnsFailure,
    ConnectRefused,
    ConnectTimeout,
    HostUnreachable,
    ConnectFailed,
    TlsCertificateInvalid,
    TlsHandshakeFailed,
}

impl CameraSourceFailure {
    /// The class name on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotFound => "source_not_found",
            Self::AuthenticationFailed => "authentication_failed",
            Self::DnsFailure => "dns_failure",
            Self::ConnectRefused => "connect_refused",
            Self::ConnectTimeout => "connect_timeout",
            Self::HostUnreachable => "host_unreachable",
            Self::ConnectFailed => "connect_failed",
            Self::TlsCertificateInvalid => "tls_certificate_invalid",
            Self::TlsHandshakeFailed => "tls_handshake_failed",
        }
    }
}

/// Names the camera and stream profile whose address the host needs, by the
/// ids the plugin reported in its inventory.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CameraSourceResolutionRequest {
    pub schema: String,
    pub vendor_camera_id: String,
    #[serde(default)]
    pub vendor_profile_id: String,
}

/// The plugin's answer: an address, or a failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CameraSourceResolution {
    /// The full stream address. It may carry userinfo.
    Url(String),
    /// The detail is shown to operators, so it must not contain the address
    /// or a credential.
    Failure {
        class: CameraSourceFailure,
        detail: String,
    },
}

#[derive(Serialize)]
struct ResolutionDocument<'a> {
    schema: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure_class: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure_detail: Option<&'a str>,
}

impl CameraSourceResolution {
    /// Encodes the resolution document.
    pub fn serialize(&self) -> SdkResult<Vec<u8>> {
        let document = match self {
            Self::Url(url) => {
                let url = url.trim();
                if url.is_empty() {
                    return Err(Error::Message(
                        "camera source resolution url is empty".to_string(),
                    ));
                }
                ResolutionDocument {
                    schema: CAMERA_SOURCE_RESOLUTION_SCHEMA_V1,
                    url: Some(url),
                    failure_class: None,
                    failure_detail: None,
                }
            }
            Self::Failure { class, detail } => ResolutionDocument {
                schema: CAMERA_SOURCE_RESOLUTION_SCHEMA_V1,
                url: None,
                failure_class: Some(class.as_str()),
                failure_detail: Some(detail.as_str()).filter(|detail| !detail.is_empty()),
            },
        };
        Ok(serde_json::to_vec(&document)?)
    }

    /// Answers the resolution request of the current run.
    pub fn submit(&self) -> SdkResult<()> {
        submit_result_payload(&self.serialize()?)
    }
}

/// Returns the resolution request delivered to the current run, or `None` for
/// an ordinary run.
pub fn camera_source_resolution_requested() -> SdkResult<Option<CameraSourceResolutionRequest>> {
    parse_camera_source_resolution_request(&get_config_bytes()?)
}

/// Extracts the resolution request from a run config document. A request the
/// plugin cannot act on is an error, so a plugin never falls through to an
/// ordinary inventory run.
pub fn parse_camera_source_resolution_request(
    config_json: &[u8],
) -> SdkResult<Option<CameraSourceResolutionRequest>> {
    let Some(raw) = host_config_value(config_json, CAMERA_SOURCE_RESOLUTION_REQUEST_CONFIG_KEY)?
    else {
        return Ok(None);
    };

    let request: CameraSourceResolutionRequest = serde_json::from_value(raw)?;
    if request.schema != CAMERA_SOURCE_RESOLUTION_REQUEST_SCHEMA_V1 {
        return Err(Error::Message(format!(
            "unsupported camera source resolution request schema {:?}",
            request.schema
        )));
    }
    if request.vendor_camera_id.trim().is_empty() {
        return Err(Error::Message(
            "camera source resolution request has no vendor_camera_id".to_string(),
        ));
    }
    Ok(Some(request))
}

/// Reports whether the host asked the current run to leave stream addresses
/// out of its inventory result. When it is true, report each profile's origin
/// (scheme, host and port) and no stream URL, path or alias. When it is false,
/// report addresses as before.
pub fn omit_camera_stream_addresses() -> SdkResult<bool> {
    parse_omit_camera_stream_addresses(&get_config_bytes()?)
}

/// Reads [`CAMERA_STREAM_ADDRESSES_CONFIG_KEY`] from a run config document.
pub fn parse_omit_camera_stream_addresses(config_json: &[u8]) -> SdkResult<bool> {
    match host_config_value(config_json, CAMERA_STREAM_ADDRESSES_CONFIG_KEY)? {
        None => Ok(false),
        Some(Value::String(value)) => Ok(value == CAMERA_STREAM_ADDRESSES_OMIT),
        Some(_) => Err(Error::Message(format!(
            "{CAMERA_STREAM_ADDRESSES_CONFIG_KEY} is not a string"
        ))),
    }
}

fn host_config_value(config_json: &[u8], key: &str) -> SdkResult<Option<Value>> {
    if config_json.is_empty() {
        return Ok(None);
    }

    let mut config: serde_json::Map<String, Value> = serde_json::from_slice(config_json)?;
    Ok(config.remove(key))
}

#[cfg(test)]
mod tests;
