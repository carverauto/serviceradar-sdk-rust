//! Host-proxied unary gRPC over the `grpc_unary` host import.
//!
//! The SDK carries no protobuf runtime: [`GrpcRequest::message`] is the
//! serialized request protobuf and [`GrpcResponse::message`] the serialized
//! response, so encode and decode with whatever generator suits the plugin.

use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::time::{Duration, Instant};

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::error::{Error, HOST_ERR_TOO_LARGE, HostError, SdkResult};
use crate::host;

/// Manifest capability that grants the `grpc_unary` host import.
pub const CAPABILITY_GRPC_REQUEST: &str = "grpc_request";

/// Dials the target over TLS. It is the SDK default.
pub const GRPC_TRANSPORT_TLS: &str = "tls";
/// Dials cleartext HTTP/2. The host only allows it when the resolved
/// destination is inside the manifest's `allowed_networks`.
pub const GRPC_TRANSPORT_H2C: &str = "h2c";

/// Host cap on a serialized response message.
pub const MAX_GRPC_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

/// Room left in the response buffer for the JSON envelope, status message,
/// headers and trailers.
pub(crate) const GRPC_RESPONSE_ENVELOPE_OVERHEAD: usize = 64 * 1024;

pub(crate) const GRPC_ERR_MISSING_TARGET: &str = "grpc request requires target_host";
pub(crate) const GRPC_ERR_INVALID_PORT: &str =
    "grpc request target_port must be between 1 and 65535";
pub(crate) const GRPC_ERR_INVALID_METHOD: &str =
    "grpc request method must look like /package.Service/Method";
pub(crate) const GRPC_ERR_TRANSPORT: &str = "grpc request transport must be h2c or tls";

/// A gRPC status code as defined by the gRPC protocol. Any `i32` round-trips;
/// the associated constants name the codes the protocol defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct GrpcCode(pub i32);

impl GrpcCode {
    pub const OK: Self = Self(0);
    pub const CANCELED: Self = Self(1);
    pub const UNKNOWN: Self = Self(2);
    pub const INVALID_ARGUMENT: Self = Self(3);
    pub const DEADLINE_EXCEEDED: Self = Self(4);
    pub const NOT_FOUND: Self = Self(5);
    pub const ALREADY_EXISTS: Self = Self(6);
    pub const PERMISSION_DENIED: Self = Self(7);
    pub const RESOURCE_EXHAUSTED: Self = Self(8);
    pub const FAILED_PRECONDITION: Self = Self(9);
    pub const ABORTED: Self = Self(10);
    pub const OUT_OF_RANGE: Self = Self(11);
    pub const UNIMPLEMENTED: Self = Self(12);
    pub const INTERNAL: Self = Self(13);
    pub const UNAVAILABLE: Self = Self(14);
    pub const DATA_LOSS: Self = Self(15);
    pub const UNAUTHENTICATED: Self = Self(16);

    const NAMES: [&'static str; 17] = [
        "OK",
        "CANCELED",
        "UNKNOWN",
        "INVALID_ARGUMENT",
        "DEADLINE_EXCEEDED",
        "NOT_FOUND",
        "ALREADY_EXISTS",
        "PERMISSION_DENIED",
        "RESOURCE_EXHAUSTED",
        "FAILED_PRECONDITION",
        "ABORTED",
        "OUT_OF_RANGE",
        "UNIMPLEMENTED",
        "INTERNAL",
        "UNAVAILABLE",
        "DATA_LOSS",
        "UNAUTHENTICATED",
    ];

    /// Returns the protocol name, such as `NOT_FOUND`, for a defined code.
    pub fn name(self) -> Option<&'static str> {
        usize::try_from(self.0)
            .ok()
            .and_then(|index| Self::NAMES.get(index).copied())
    }

    pub fn is_ok(self) -> bool {
        self == Self::OK
    }
}

impl Display for GrpcCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.name() {
            Some(name) => f.write_str(name),
            None => write!(f, "CODE({})", self.0),
        }
    }
}

impl From<i32> for GrpcCode {
    fn from(value: i32) -> Self {
        Self(value)
    }
}

/// Tunes the TLS transport. It is ignored for h2c.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GrpcTlsConfig {
    pub server_name: String,
    pub insecure_skip_verify: bool,
}

/// One host-proxied unary gRPC call.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GrpcRequest {
    /// Hostname or IP literal.
    pub target_host: String,
    pub target_port: u16,
    /// Overrides the `:authority` pseudo-header when set.
    pub authority: String,
    /// Full method path, for example `/example.v1.Device/Handle`.
    pub method: String,
    /// Keys are lowercased by the host. Reserved and `grpc-*` keys are
    /// rejected; values for keys ending in `-bin` must be base64.
    pub metadata: BTreeMap<String, String>,
    /// Serialized request protobuf (may be empty).
    pub message: Vec<u8>,
    /// Zero uses the host default (10 s).
    pub timeout_ms: u32,
    /// [`GRPC_TRANSPORT_TLS`] or [`GRPC_TRANSPORT_H2C`]. Empty uses TLS.
    pub transport: String,
    pub tls: Option<GrpcTlsConfig>,
    /// Lowers the host response cap when non-zero.
    pub max_response_bytes: u32,
}

impl GrpcRequest {
    pub fn new(
        target_host: impl Into<String>,
        target_port: u16,
        method: impl Into<String>,
    ) -> Self {
        Self {
            target_host: target_host.into(),
            target_port,
            method: method.into(),
            ..Self::default()
        }
    }

    pub fn with_authority(mut self, authority: impl Into<String>) -> Self {
        self.authority = authority.into();
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    pub fn with_message(mut self, message: impl Into<Vec<u8>>) -> Self {
        self.message = message.into();
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout_ms = timeout.as_millis().min(u128::from(u32::MAX)) as u32;
        self
    }

    pub fn with_transport(mut self, transport: impl Into<String>) -> Self {
        self.transport = transport.into();
        self
    }

    pub fn with_tls(mut self, tls: GrpcTlsConfig) -> Self {
        self.tls = Some(tls);
        self
    }

    pub fn with_max_response_bytes(mut self, limit: u32) -> Self {
        self.max_response_bytes = limit;
        self
    }
}

/// A completed RPC. `status` is set for every completed call, including
/// non-OK ones.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GrpcResponse {
    pub status: GrpcCode,
    pub status_message: String,
    pub headers: BTreeMap<String, Vec<String>>,
    pub trailers: BTreeMap<String, Vec<String>>,
    /// Serialized response protobuf.
    pub message: Vec<u8>,
    pub duration: Duration,
}

impl GrpcResponse {
    /// Converts a non-OK status into a [`GrpcStatusError`].
    pub fn into_result(self) -> Result<Self, GrpcStatusError> {
        if self.status.is_ok() {
            Ok(self)
        } else {
            Err(GrpcStatusError {
                code: self.status,
                message: self.status_message,
                headers: self.headers,
                trailers: self.trailers,
            })
        }
    }
}

/// Returned by [`GrpcClient::unary`] when the RPC completed with a non-OK
/// status. Transport failures that never reached the server surface as
/// [`GrpcCode::UNAVAILABLE`]. Host policy failures are `Error::Host` instead.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GrpcStatusError {
    pub code: GrpcCode,
    pub message: String,
    pub headers: BTreeMap<String, Vec<String>>,
    pub trailers: BTreeMap<String, Vec<String>>,
}

impl Display for GrpcStatusError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.message.is_empty() {
            write!(f, "grpc status {}", self.code)
        } else {
            write!(f, "grpc status {}: {}", self.code, self.message)
        }
    }
}

impl std::error::Error for GrpcStatusError {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct GrpcTlsPayload {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) server_name: String,
    #[serde(default)]
    pub(crate) insecure_skip_verify: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct GrpcRequestPayload {
    pub(crate) target_host: String,
    pub(crate) target_port: u16,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) authority: String,
    pub(crate) method: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) metadata: BTreeMap<String, String>,
    #[serde(default)]
    pub(crate) message_base64: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub(crate) timeout_ms: u32,
    #[serde(default)]
    pub(crate) transport: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) tls: Option<GrpcTlsPayload>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub(crate) max_response_bytes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct GrpcResponsePayload {
    #[serde(default)]
    pub(crate) grpc_status: i32,
    #[serde(default)]
    pub(crate) grpc_message: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) headers: BTreeMap<String, Vec<String>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) trailers: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub(crate) message_base64: String,
}

impl GrpcRequestPayload {
    /// Validates the request shape and builds the host ABI payload.
    pub(crate) fn from_request(request: &GrpcRequest) -> SdkResult<Self> {
        let target_host = request.target_host.trim();
        if target_host.is_empty() {
            return Err(Error::InvalidGrpcRequest(GRPC_ERR_MISSING_TARGET));
        }
        if request.target_port == 0 {
            return Err(Error::InvalidGrpcRequest(GRPC_ERR_INVALID_PORT));
        }
        let method = request.method.trim();
        if !valid_grpc_method(method) {
            return Err(Error::InvalidGrpcRequest(GRPC_ERR_INVALID_METHOD));
        }
        let mut transport = request.transport.trim().to_ascii_lowercase();
        if transport.is_empty() {
            transport = GRPC_TRANSPORT_TLS.to_string();
        }
        if transport != GRPC_TRANSPORT_TLS && transport != GRPC_TRANSPORT_H2C {
            return Err(Error::InvalidGrpcRequest(GRPC_ERR_TRANSPORT));
        }

        Ok(Self {
            target_host: target_host.to_string(),
            target_port: request.target_port,
            authority: request.authority.trim().to_string(),
            method: method.to_string(),
            metadata: request.metadata.clone(),
            message_base64: base64::engine::general_purpose::STANDARD.encode(&request.message),
            timeout_ms: request.timeout_ms,
            transport,
            tls: request.tls.as_ref().map(|tls| GrpcTlsPayload {
                server_name: tls.server_name.clone(),
                insecure_skip_verify: tls.insecure_skip_verify,
            }),
            max_response_bytes: request.max_response_bytes,
        })
    }
}

#[cfg(any(test, not(target_arch = "wasm32")))]
impl GrpcResponsePayload {
    /// Encodes a response the way the host does; used by the local host.
    pub(crate) fn from_response(response: &GrpcResponse) -> Self {
        Self {
            grpc_status: response.status.0,
            grpc_message: response.status_message.clone(),
            headers: response.headers.clone(),
            trailers: response.trailers.clone(),
            message_base64: base64::engine::general_purpose::STANDARD.encode(&response.message),
        }
    }
}

/// Accepts `/service/method` with a non-empty service and method.
fn valid_grpc_method(method: &str) -> bool {
    if method.len() < 4 || !method.starts_with('/') {
        return false;
    }
    let Some((service, name)) = method[1..].split_once('/') else {
        return false;
    };
    !service.is_empty() && !name.is_empty() && !name.contains('/')
}

/// Wraps the `grpc_unary` host call.
#[derive(Debug, Clone, Copy)]
pub struct GrpcClient {
    /// Bounds the response message the client is prepared to receive. The
    /// response buffer is sized for its base64 JSON envelope.
    pub max_response_bytes: usize,
}

pub const GRPC: GrpcClient = GrpcClient {
    max_response_bytes: MAX_GRPC_RESPONSE_BYTES,
};

impl Default for GrpcClient {
    fn default() -> Self {
        GRPC
    }
}

impl GrpcClient {
    /// Performs one unary RPC through the host. A non-OK status returns
    /// `Error::GrpcStatus` carrying the status, headers and trailers; host
    /// policy failures return `Error::Host`.
    pub fn unary(&self, request: GrpcRequest) -> SdkResult<GrpcResponse> {
        Ok(self.call(request)?.into_result()?)
    }

    /// Performs one unary RPC and returns every completed call as `Ok`,
    /// including non-OK statuses, for callers that inspect
    /// [`GrpcResponse::status`] themselves.
    pub fn call(&self, request: GrpcRequest) -> SdkResult<GrpcResponse> {
        let payload = GrpcRequestPayload::from_request(&request)?;
        let encoded = serde_json::to_vec(&payload)?;

        let mut response_buf = vec![0_u8; self.response_buffer_size(request.max_response_bytes)];
        let start = Instant::now();
        let res = host::grpc_unary(&encoded, &mut response_buf);
        if res < 0 {
            return Err(HostError {
                code: res,
                op: "grpc_unary",
            }
            .into());
        }
        let len = res as usize;
        if len > response_buf.len() {
            return Err(HostError {
                code: HOST_ERR_TOO_LARGE,
                op: "grpc_unary",
            }
            .into());
        }

        decode_grpc_response(&response_buf[..len], start.elapsed())
    }

    pub(crate) fn response_buffer_size(&self, request_limit: u32) -> usize {
        let mut limit = if self.max_response_bytes == 0 {
            MAX_GRPC_RESPONSE_BYTES
        } else {
            self.max_response_bytes
        };
        let request_limit = request_limit as usize;
        if request_limit > 0 && request_limit < limit {
            limit = request_limit;
        }
        base64_encoded_len(limit) + GRPC_RESPONSE_ENVELOPE_OVERHEAD
    }
}

const fn base64_encoded_len(len: usize) -> usize {
    len.div_ceil(3) * 4
}

pub(crate) fn decode_grpc_response(payload: &[u8], duration: Duration) -> SdkResult<GrpcResponse> {
    let decoded: GrpcResponsePayload = serde_json::from_slice(payload)?;
    let message = base64::engine::general_purpose::STANDARD
        .decode(decoded.message_base64)
        .map_err(|err| Error::Message(err.to_string()))?;

    Ok(GrpcResponse {
        status: GrpcCode(decoded.grpc_status),
        status_message: decoded.grpc_message,
        headers: decoded.headers,
        trailers: decoded.trailers,
        message,
        duration,
    })
}

const fn is_zero(value: &u32) -> bool {
    *value == 0
}

#[cfg(test)]
pub(crate) mod tests;
