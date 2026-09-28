use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

use super::{
    GRPC_ERR_INVALID_METHOD, GRPC_ERR_INVALID_PORT, GRPC_ERR_MISSING_TARGET, GRPC_ERR_TRANSPORT,
    GRPC_RESPONSE_ENVELOPE_OVERHEAD, GrpcClient, GrpcCode, GrpcRequest, GrpcRequestPayload,
    GrpcResponse, GrpcResponsePayload, GrpcTlsConfig, MAX_GRPC_RESPONSE_BYTES,
    decode_grpc_response,
};
use crate::error::{Error, HOST_ERR_DENIED, HOST_ERR_TOO_LARGE};
use crate::host::{TestHostBackend, install_test_backend};

pub(crate) const REQUEST_FIXTURE: &str = include_str!("../../fixtures/grpc_unary_request.json");
pub(crate) const RESPONSE_OK_FIXTURE: &str =
    include_str!("../../fixtures/grpc_unary_response_ok.json");
pub(crate) const RESPONSE_ERROR_FIXTURE: &str =
    include_str!("../../fixtures/grpc_unary_response_error.json");

/// The request `fixtures/grpc_unary_request.json` encodes.
pub(crate) fn fixture_request() -> GrpcRequest {
    GrpcRequest::new("device.example.com", 9200, "/example.v1.Device/Handle")
        .with_authority("device.example.com:9200")
        .with_metadata("x-request-id", "req-0001")
        .with_metadata("trace-bin", "AAECAw==")
        .with_message(b"\n\x06dev-01".to_vec())
        .with_timeout(Duration::from_secs(5))
        .with_transport(super::GRPC_TRANSPORT_TLS)
        .with_tls(GrpcTlsConfig {
            server_name: "device.example.com".to_string(),
            insecure_skip_verify: false,
        })
        .with_max_response_bytes(65536)
}

/// Decodes a shared response fixture into the response a handler returns.
pub(crate) fn fixture_response(raw: &str) -> GrpcResponse {
    decode_grpc_response(raw.as_bytes(), Duration::ZERO).expect("decode response fixture")
}

fn invalid_reason(result: crate::SdkResult<GrpcRequestPayload>) -> &'static str {
    match result {
        Err(Error::InvalidGrpcRequest(reason)) => reason,
        other => panic!("expected InvalidGrpcRequest, got {other:?}"),
    }
}

#[test]
fn grpc_request_matches_shared_fixture() {
    let want: Value = serde_json::from_str(REQUEST_FIXTURE).unwrap();
    let payload = GrpcRequestPayload::from_request(&fixture_request()).unwrap();
    assert_eq!(serde_json::to_value(&payload).unwrap(), want);

    // The fixture decodes back into the same payload.
    let decoded: GrpcRequestPayload = serde_json::from_str(REQUEST_FIXTURE).unwrap();
    assert_eq!(decoded, payload);
}

#[test]
fn grpc_request_defaults_to_tls_and_omits_optional_fields() {
    let payload = GrpcRequestPayload::from_request(&GrpcRequest::new(
        "192.0.2.10",
        9200,
        "/example.v1.Device/Handle",
    ))
    .unwrap();
    assert_eq!(
        serde_json::to_string(&payload).unwrap(),
        r#"{"target_host":"192.0.2.10","target_port":9200,"method":"/example.v1.Device/Handle","message_base64":"","transport":"tls"}"#
    );
}

type ShapeCase = (&'static str, fn(&mut GrpcRequest), &'static str);

#[test]
fn grpc_request_rejects_invalid_shapes() {
    let cases: [ShapeCase; 6] = [
        (
            "missing host",
            |r| r.target_host = " ".into(),
            GRPC_ERR_MISSING_TARGET,
        ),
        ("zero port", |r| r.target_port = 0, GRPC_ERR_INVALID_PORT),
        (
            "no slash",
            |r| r.method = "example.v1.Device/Handle".into(),
            GRPC_ERR_INVALID_METHOD,
        ),
        (
            "no method name",
            |r| r.method = "/example.v1.Device/".into(),
            GRPC_ERR_INVALID_METHOD,
        ),
        (
            "extra segment",
            |r| r.method = "/a/b/c".into(),
            GRPC_ERR_INVALID_METHOD,
        ),
        (
            "transport",
            |r| r.transport = "http".into(),
            GRPC_ERR_TRANSPORT,
        ),
    ];
    for (name, mutate, want) in cases {
        let mut request = fixture_request();
        mutate(&mut request);
        assert_eq!(
            invalid_reason(GrpcRequestPayload::from_request(&request)),
            want,
            "{name}"
        );
    }
}

#[test]
fn decode_grpc_response_ok_fixture() {
    let response =
        decode_grpc_response(RESPONSE_OK_FIXTURE.as_bytes(), Duration::from_millis(1)).unwrap();
    assert_eq!(response.status, GrpcCode::OK);
    assert_eq!(response.status_message, "");
    assert_eq!(response.message, b"\n\x02ok");
    assert_eq!(
        response.headers.get("content-type"),
        Some(&vec!["application/grpc".to_string()])
    );
    assert_eq!(
        response.trailers.get("x-handled-by"),
        Some(&vec!["handler-1".to_string()])
    );
    assert_eq!(response.duration, Duration::from_millis(1));

    // Re-encoding reproduces the shared fixture.
    let reencoded = serde_json::to_value(GrpcResponsePayload::from_response(&response)).unwrap();
    assert_eq!(
        reencoded,
        serde_json::from_str::<Value>(RESPONSE_OK_FIXTURE).unwrap()
    );
}

#[test]
fn decode_grpc_response_error_fixture() {
    let response = fixture_response(RESPONSE_ERROR_FIXTURE);
    assert_eq!(response.status, GrpcCode::NOT_FOUND);
    assert!(response.message.is_empty());
    let reencoded = serde_json::to_value(GrpcResponsePayload::from_response(&response)).unwrap();
    assert_eq!(
        reencoded,
        serde_json::from_str::<Value>(RESPONSE_ERROR_FIXTURE).unwrap()
    );

    let status = response.into_result().expect_err("non-OK status");
    assert_eq!(status.code, GrpcCode::NOT_FOUND);
    assert_eq!(status.message, "device dev-01 not found");
    assert_eq!(
        status.trailers.get("x-error-detail"),
        Some(&vec!["unknown-device".to_string()])
    );
    assert_eq!(
        status.headers.get("content-type"),
        Some(&vec!["application/grpc".to_string()])
    );
    assert_eq!(
        status.to_string(),
        "grpc status NOT_FOUND: device dev-01 not found"
    );
}

#[test]
fn grpc_code_display() {
    assert_eq!(GrpcCode::UNAVAILABLE.to_string(), "UNAVAILABLE");
    assert_eq!(GrpcCode::UNAUTHENTICATED.to_string(), "UNAUTHENTICATED");
    assert_eq!(GrpcCode(42).to_string(), "CODE(42)");
    assert_eq!(GrpcCode(-1).to_string(), "CODE(-1)");
}

#[test]
fn grpc_response_buffer_size() {
    let client = GrpcClient {
        max_response_bytes: 3000,
    };
    assert_eq!(
        client.response_buffer_size(0),
        4000 + GRPC_RESPONSE_ENVELOPE_OVERHEAD
    );
    assert_eq!(
        client.response_buffer_size(300),
        400 + GRPC_RESPONSE_ENVELOPE_OVERHEAD
    );
    let default_client = GrpcClient {
        max_response_bytes: 0,
    };
    assert!(default_client.response_buffer_size(0) >= MAX_GRPC_RESPONSE_BYTES);
}

/// Replays one scripted host reply and records the request bytes.
struct ScriptedGrpcHost {
    reply: Result<Vec<u8>, i32>,
    claim_len: Option<i32>,
    seen: Arc<Mutex<Vec<u8>>>,
}

impl TestHostBackend for ScriptedGrpcHost {
    fn grpc_unary(&mut self, req: &[u8], resp: &mut [u8]) -> i32 {
        *self.seen.lock().unwrap() = req.to_vec();
        match &self.reply {
            Ok(body) => {
                resp[..body.len()].copy_from_slice(body);
                self.claim_len.unwrap_or(body.len() as i32)
            }
            Err(code) => *code,
        }
    }
}

fn run_scripted(
    reply: Result<Vec<u8>, i32>,
    claim_len: Option<i32>,
) -> (crate::SdkResult<GrpcResponse>, Vec<u8>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let _guard = install_test_backend(Box::new(ScriptedGrpcHost {
        reply,
        claim_len,
        seen: Arc::clone(&seen),
    }));
    let result = GrpcClient::default().unary(fixture_request());
    let request = seen.lock().unwrap().clone();
    (result, request)
}

#[test]
fn grpc_client_sends_fixture_and_decodes_reply() {
    let (result, request) = run_scripted(Ok(RESPONSE_OK_FIXTURE.as_bytes().to_vec()), None);
    assert_eq!(result.unwrap().message, b"\n\x02ok");
    assert_eq!(
        serde_json::from_slice::<Value>(&request).unwrap(),
        serde_json::from_str::<Value>(REQUEST_FIXTURE).unwrap()
    );
}

#[test]
fn grpc_client_surfaces_status_and_host_errors() {
    let (result, _) = run_scripted(Ok(RESPONSE_ERROR_FIXTURE.as_bytes().to_vec()), None);
    match result {
        Err(Error::GrpcStatus(status)) => assert_eq!(status.code, GrpcCode::NOT_FOUND),
        other => panic!("expected GrpcStatus, got {other:?}"),
    }

    let (result, _) = run_scripted(Err(HOST_ERR_DENIED), None);
    match result {
        Err(Error::Host(err)) => {
            assert_eq!(err.code, HOST_ERR_DENIED);
            assert_eq!(err.op, "grpc_unary");
        }
        other => panic!("expected host error, got {other:?}"),
    }

    // A host that claims more bytes than the buffer holds is reported as -3.
    let (result, _) = run_scripted(Ok(Vec::new()), Some(i32::MAX));
    match result {
        Err(Error::Host(err)) => assert_eq!(err.code, HOST_ERR_TOO_LARGE),
        other => panic!("expected too-large host error, got {other:?}"),
    }
}

#[test]
fn grpc_call_returns_non_ok_status_as_a_response() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let _guard = install_test_backend(Box::new(ScriptedGrpcHost {
        reply: Ok(RESPONSE_ERROR_FIXTURE.as_bytes().to_vec()),
        claim_len: None,
        seen,
    }));
    let response = GrpcClient::default().call(fixture_request()).unwrap();
    assert_eq!(response.status, GrpcCode::NOT_FOUND);
}

#[test]
fn grpc_metadata_is_forwarded_verbatim() {
    let payload =
        GrpcRequestPayload::from_request(&fixture_request().with_metadata("x-extra", "value-1"))
            .unwrap();
    assert_eq!(
        payload.metadata,
        BTreeMap::from([
            ("trace-bin".to_string(), "AAECAw==".to_string()),
            ("x-extra".to_string(), "value-1".to_string()),
            ("x-request-id".to_string(), "req-0001".to_string()),
        ])
    );
}
