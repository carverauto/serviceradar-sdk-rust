//! Local emulation of the agent's host-mediated `grpc_unary` operation.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use base64::Engine;

use crate::error::{
    Error, HOST_ERR_INTERNAL, HOST_ERR_INVALID, HOST_ERR_NOT_FOUND, HOST_ERR_TIMEOUT,
    HOST_ERR_TOO_LARGE, HostError, SdkResult,
};
use crate::grpc::{
    GRPC_ERR_TRANSPORT, GRPC_TRANSPORT_H2C, GRPC_TRANSPORT_TLS, GrpcRequestPayload,
    GrpcResponsePayload, MAX_GRPC_RESPONSE_BYTES,
};
use crate::{GrpcCode, GrpcRequest, GrpcResponse, GrpcTlsConfig};

/// Serves one host-mediated unary gRPC call during a native local run.
///
/// The request has already passed the shape and metadata checks the agent
/// applies. Return a response with a non-OK `status` to emulate a server-side
/// gRPC error. Return an error to emulate a transport failure, which the plugin
/// sees as `UNAVAILABLE`; return `Error::Host` with code `-6`, or an
/// `std::io::ErrorKind::TimedOut` I/O error, to emulate a timeout. The call is
/// synchronous, so the handler should honor `request.timeout_ms` itself (zero
/// means the host default of 10 seconds).
pub type LocalGrpcHandler = Box<dyn FnMut(GrpcRequest) -> SdkResult<GrpcResponse> + Send>;

const LOCAL_GRPC_DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// Transport-owned headers the host refuses to take from plugin metadata.
const RESERVED_METADATA: &[&str] = &[
    "connection",
    "content-type",
    "host",
    "keep-alive",
    "proxy-connection",
    "te",
    "transfer-encoding",
    "upgrade",
];

pub(super) fn grpc_unary(
    handler: Option<&mut LocalGrpcHandler>,
    encoded: &[u8],
    response_buf: &mut [u8],
) -> i32 {
    let Some(handler) = handler else {
        return HOST_ERR_NOT_FOUND;
    };
    let Ok(request) = decode_local_grpc_request(encoded) else {
        return HOST_ERR_INVALID;
    };

    let timeout = if request.timeout_ms > 0 {
        Duration::from_millis(u64::from(request.timeout_ms))
    } else {
        LOCAL_GRPC_DEFAULT_TIMEOUT
    };
    let limit = match request.max_response_bytes as usize {
        0 => MAX_GRPC_RESPONSE_BYTES,
        requested => requested.min(MAX_GRPC_RESPONSE_BYTES),
    };

    let start = Instant::now();
    let response = match handler(request) {
        Ok(response) => response,
        Err(err) if is_timeout(&err) || start.elapsed() >= timeout => return HOST_ERR_TIMEOUT,
        // Handler errors can carry local secrets; report the class only, the
        // way the agent reports a dial failure.
        Err(_) => GrpcResponse {
            status: GrpcCode::UNAVAILABLE,
            status_message: "local grpc handler failed".to_string(),
            ..GrpcResponse::default()
        },
    };
    if response.message.len() > limit {
        return HOST_ERR_TOO_LARGE;
    }

    let Ok(encoded_response) = serde_json::to_vec(&GrpcResponsePayload::from_response(&response))
    else {
        return HOST_ERR_INTERNAL;
    };
    if encoded_response.len() > response_buf.len() {
        return HOST_ERR_TOO_LARGE;
    }
    response_buf[..encoded_response.len()].copy_from_slice(&encoded_response);
    encoded_response.len() as i32
}

fn is_timeout(err: &Error) -> bool {
    match err {
        Error::Host(HostError { code, .. }) => *code == HOST_ERR_TIMEOUT,
        Error::Io(err) => err.kind() == std::io::ErrorKind::TimedOut,
        _ => false,
    }
}

pub(super) fn decode_local_grpc_request(encoded: &[u8]) -> SdkResult<GrpcRequest> {
    let payload: GrpcRequestPayload = serde_json::from_slice(encoded)?;
    let message = base64::engine::general_purpose::STANDARD
        .decode(&payload.message_base64)
        .map_err(|_| Error::Message("invalid local grpc message".to_string()))?;
    let metadata = normalize_local_grpc_metadata(payload.metadata)?;

    let request = GrpcRequest {
        target_host: payload.target_host,
        target_port: payload.target_port,
        authority: payload.authority,
        method: payload.method,
        metadata,
        message,
        timeout_ms: payload.timeout_ms,
        transport: payload.transport,
        tls: payload.tls.map(|tls| GrpcTlsConfig {
            server_name: tls.server_name,
            insecure_skip_verify: tls.insecure_skip_verify,
        }),
        max_response_bytes: payload.max_response_bytes,
    };
    // Re-run the SDK's own shape checks: the local host must reject what the
    // agent rejects even if a caller bypasses GrpcClient.
    GrpcRequestPayload::from_request(&request)?;
    if request.transport != GRPC_TRANSPORT_TLS && request.transport != GRPC_TRANSPORT_H2C {
        return Err(Error::InvalidGrpcRequest(GRPC_ERR_TRANSPORT));
    }
    Ok(request)
}

fn normalize_local_grpc_metadata(
    metadata: BTreeMap<String, String>,
) -> SdkResult<BTreeMap<String, String>> {
    let mut normalized = BTreeMap::new();
    for (key, value) in metadata {
        let name = key.trim().to_ascii_lowercase();
        if name.is_empty()
            || name.starts_with(':')
            || name.starts_with("grpc-")
            || RESERVED_METADATA.contains(&name.as_str())
        {
            return Err(Error::Message("reserved grpc metadata key".to_string()));
        }
        if normalized.contains_key(&name) {
            return Err(Error::Message("duplicate grpc metadata key".to_string()));
        }
        if name.ends_with("-bin")
            && base64::engine::general_purpose::STANDARD
                .decode(&value)
                .is_err()
        {
            return Err(Error::Message(
                "binary grpc metadata must be base64".to_string(),
            ));
        }
        normalized.insert(name, value);
    }
    Ok(normalized)
}
