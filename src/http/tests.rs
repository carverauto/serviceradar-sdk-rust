use std::collections::BTreeMap;
use std::time::Duration;

use base64::Engine;

use crate::error::{Error, HOST_ERR_TOO_LARGE};
use crate::host::{TestHostBackend, install_test_backend};

use super::{
    HTTP_RESPONSE_MODE_ENVELOPE, HttpClient, HttpRequest, HttpRequestPayload, HttpResponse,
    HttpResponsePayload, MAX_HTTP_RESPONSE_BYTES,
};

struct HttpTestHost;

impl TestHostBackend for HttpTestHost {
    fn http_request(&mut self, req: &[u8], resp: &mut [u8]) -> i32 {
        let payload: serde_json::Value = serde_json::from_slice(req).expect("decode request");
        assert_eq!(payload["method"], "POST");
        assert_eq!(payload["url"], "https://example.com");
        assert_eq!(payload["headers"]["content-type"], "application/json");

        let response = HttpResponsePayload {
            status: 200,
            headers: BTreeMap::from([("content-type".to_string(), "application/json".to_string())]),
            body_base64: base64::engine::general_purpose::STANDARD.encode(br#"{"ok":true}"#),
            ..HttpResponsePayload::default()
        };
        let encoded = serde_json::to_vec(&response).expect("encode response");
        resp[..encoded.len()].copy_from_slice(&encoded);
        encoded.len() as i32
    }
}

#[test]
fn http_client_encodes_request_and_decodes_response() {
    let _guard = install_test_backend(Box::new(HttpTestHost));

    let response = HttpClient::default()
        .do_request(HttpRequest {
            method: "post".to_string(),
            url: "https://example.com".to_string(),
            headers: BTreeMap::from([("content-type".to_string(), "application/json".to_string())]),
            body: br#"{"ping":true}"#.to_vec(),
            body_base64: false,
            response_mode: String::new(),
            timeout_ms: 5_000,
            insecure_skip_verify: false,
            follow_redirects: None,
        })
        .expect("http request");

    assert_eq!(response.status, 200);
    assert_eq!(response.body, br#"{"ok":true}"#);
}

#[test]
fn http_request_builders_encode_expected_fields() {
    let request = HttpRequest::post("https://example.com", br#"{"ping":true}"#.to_vec())
        .with_header("accept", "application/json")
        .with_timeout(Duration::from_secs(5))
        .with_insecure_tls(true);

    let payload = HttpRequestPayload::from_request(request);
    assert_eq!(payload.method, "POST");
    assert_eq!(payload.url, "https://example.com");
    assert_eq!(
        payload.headers.get("accept").map(String::as_str),
        Some("application/json")
    );
    assert_eq!(payload.timeout_ms, 5_000);
    assert!(payload.insecure_skip_verify);
    assert_eq!(payload.body.as_deref(), Some(r#"{"ping":true}"#));
}

#[test]
fn http_response_helpers_decode_headers_text_and_json() {
    let response = HttpResponse {
        status: 200,
        headers: BTreeMap::from([("content-type".to_string(), "application/json".to_string())]),
        body: br#"{"ok":true}"#.to_vec(),
        duration: Duration::from_millis(12),
        ..HttpResponse::default()
    };

    assert_eq!(response.header("Content-Type"), Some("application/json"));
    assert_eq!(response.text().expect("utf8 body"), r#"{"ok":true}"#);

    let value: serde_json::Value = response.json().expect("json body");
    assert_eq!(value["ok"], true);
}

struct StatusBodyHost;

impl TestHostBackend for StatusBodyHost {
    fn http_request(&mut self, req: &[u8], resp: &mut [u8]) -> i32 {
        let payload: serde_json::Value = serde_json::from_slice(req).expect("decode request");
        assert_eq!(payload["response_mode"], "status_body");
        let reply = b"201\n{\"created\":true}";
        resp[..reply.len()].copy_from_slice(reply);
        reply.len() as i32
    }
}

#[test]
fn http_client_defaults_to_status_body_and_decodes_raw_reply() {
    let _guard = install_test_backend(Box::new(StatusBodyHost));

    let response = HttpClient::default()
        .get("https://example.com/items")
        .expect("http request");

    assert_eq!(response.status, 201);
    assert_eq!(response.body, br#"{"created":true}"#);
}

#[test]
fn status_body_decoder_falls_back_for_json_envelopes() {
    let decoded =
        super::decode_status_body_response(br#"{"status":200}"#, Duration::ZERO).expect("decode");
    assert!(decoded.is_none());
    let decoded = super::decode_status_body_response(b"\nbody", Duration::ZERO).expect("decode");
    assert!(decoded.is_none());
}

#[test]
fn explicit_response_mode_is_sent_verbatim() {
    let payload = HttpRequestPayload::from_request(
        HttpRequest::get("https://example.com").with_response_mode("json"),
    );
    assert_eq!(payload.response_mode, "json");
}

#[test]
fn http_response_header_is_case_insensitive() {
    let response = HttpResponse {
        headers: BTreeMap::from([
            ("Content-Type".to_string(), "application/json".to_string()),
            ("X-Rate-Limit".to_string(), "10".to_string()),
        ]),
        ..HttpResponse::default()
    };
    assert_eq!(response.header("content-type"), Some("application/json"));
    assert_eq!(response.header("X-RATE-LIMIT"), Some("10"));
    assert_eq!(response.header("missing"), None);
}

#[test]
fn http_response_retry_after() {
    use std::time::UNIX_EPOCH;

    let now_utc = time::PrimitiveDateTime::new(
        time::Date::from_calendar_date(2026, time::Month::January, 2).unwrap(),
        time::Time::from_hms(3, 4, 5).unwrap(),
    )
    .assume_utc();
    let now = UNIX_EPOCH + Duration::from_secs(now_utc.unix_timestamp() as u64);
    let overflow = "9".repeat(20);
    let beyond_go_duration = (i64::MAX as u64 / 1_000_000_000 + 1).to_string();
    let cases = [
        ("absent", "", None),
        ("delta seconds", "120", Some(Duration::from_secs(120))),
        ("zero", "0", Some(Duration::ZERO)),
        (
            "http date",
            "Fri, 02 Jan 2026 03:05:35 GMT",
            Some(Duration::from_secs(90)),
        ),
        (
            "rfc850 date",
            "Friday, 02-Jan-26 03:05:35 GMT",
            Some(Duration::from_secs(90)),
        ),
        (
            "asctime date",
            "Fri Jan  2 03:05:35 2026",
            Some(Duration::from_secs(90)),
        ),
        (
            "past date",
            "Thu, 01 Jan 2026 00:00:00 GMT",
            Some(Duration::ZERO),
        ),
        ("negative", "-5", None),
        ("fractional", "1.5", None),
        ("malformed", "soon", None),
        ("bad month", "Fri, 02 Foo 2026 03:05:35 GMT", None),
        ("overflow", overflow.as_str(), None),
        ("beyond go duration", beyond_go_duration.as_str(), None),
    ];
    for (name, value, want) in cases {
        let mut response = HttpResponse::default();
        if !value.is_empty() {
            response
                .headers
                .insert("retry-after".to_string(), value.to_string());
        }
        assert_eq!(response.retry_after_at(now), want, "{name}");
    }
}

#[test]
fn envelope_response_mode_is_named() {
    let payload = HttpRequestPayload::from_request(
        HttpRequest::get("https://example.com")
            .with_response_mode(super::HTTP_RESPONSE_MODE_ENVELOPE),
    );
    assert_eq!(payload.response_mode, "envelope");
    let payload = HttpRequestPayload::from_request(HttpRequest::get("https://example.com"));
    assert_eq!(payload.response_mode, super::HTTP_RESPONSE_MODE_STATUS_BODY);
}

#[test]
fn follow_redirects_and_envelope_waterfall_decode() {
    let omit = serde_json::to_string(&HttpRequestPayload::from_request(
        HttpRequest::get("https://example.com")
            .with_response_mode(super::HTTP_RESPONSE_MODE_ENVELOPE),
    ))
    .unwrap();
    assert!(!omit.contains("follow_redirects"));

    let disabled = serde_json::to_string(&HttpRequestPayload::from_request(
        HttpRequest::post(
            "https://sso.example.com/session",
            br#"{"grant":"test"}"#.to_vec(),
        )
        .with_header("Content-Type", "application/json")
        .with_response_mode(super::HTTP_RESPONSE_MODE_ENVELOPE)
        .with_timeout(Duration::from_millis(2500))
        .with_follow_redirects(false),
    ))
    .unwrap();
    assert!(disabled.contains(r#""follow_redirects":false"#));

    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../fixtures/journey_conformance.json")).unwrap();
    let vec = &fixture["http_envelope_vectors"][0];
    let resp_bytes = serde_json::to_vec(&vec["response_json"]).unwrap();
    let decoded = super::decode_envelope_response(&resp_bytes, Duration::from_millis(1)).unwrap();
    assert_eq!(decoded.status, 302);
    assert_eq!(decoded.set_cookies.len(), 2);
    assert_eq!(decoded.header_values("Set-Cookie").len(), 2);
    assert_eq!(decoded.header_values("Location"), vec!["/callback"]);
    let trace = decoded.trace.as_ref().expect("trace");
    assert_eq!(trace.hops.len(), 1);
    assert_eq!(trace.total_duration(), Duration::from_nanos(19_200_000));
    assert!((trace.total_ms() - 19.2).abs() < 1e-6);
    let tls = decoded.tls.as_ref().expect("tls");
    assert_eq!(tls.version, "TLS1.3");
    assert_eq!(tls.leaf_not_after_unix, Some(1_790_553_600));
    assert_eq!(tls.leaf_days_remaining, Some(0));

    let no_leaf_payload = br#"{"status":200,"body_base64":"","tls":{"version":"TLS1.3","cipher_suite":"TLS_AES_128_GCM_SHA256"}}"#;
    let no_leaf =
        super::decode_envelope_response(no_leaf_payload, Duration::from_millis(1)).unwrap();
    let no_leaf_tls = no_leaf.tls.as_ref().expect("tls without leaf");
    assert_eq!(no_leaf_tls.version, "TLS1.3");
    assert_eq!(no_leaf_tls.leaf_not_after_unix, None);
    assert_eq!(no_leaf_tls.leaf_days_remaining, None);
}

struct LargeEnvelopeHost {
    encoded_envelope: Vec<u8>,
}

impl TestHostBackend for LargeEnvelopeHost {
    fn http_request(&mut self, req: &[u8], resp: &mut [u8]) -> i32 {
        let payload: serde_json::Value = serde_json::from_slice(req).expect("decode request");
        assert_eq!(payload["response_mode"], HTTP_RESPONSE_MODE_ENVELOPE);
        if self.encoded_envelope.len() > resp.len() {
            return HOST_ERR_TOO_LARGE;
        }
        resp[..self.encoded_envelope.len()].copy_from_slice(&self.encoded_envelope);
        self.encoded_envelope.len() as i32
    }
}

#[test]
fn max_http_response_bytes_is_16_mib_and_decodes_over_4_mib_envelope() {
    assert_eq!(MAX_HTTP_RESPONSE_BYTES, 16 * 1024 * 1024);
    assert_eq!(crate::MAX_HTTP_RESPONSE_BYTES, 16 * 1024 * 1024);
    assert_eq!(HttpClient::default().max_response_bytes, 16 * 1024 * 1024);
    assert_eq!(crate::HTTP.max_response_bytes, 16 * 1024 * 1024);

    // 3.5 MiB raw snapshot expands to ~4.67 MiB in base64 inside the JSON envelope (> 4 MiB).
    let raw_body = vec![b'x'; 3_500_000];
    let envelope = HttpResponsePayload {
        status: 200,
        headers: BTreeMap::from([
            ("content-type".to_string(), "application/json".to_string()),
            ("etag".to_string(), "\"snap-v1\"".to_string()),
        ]),
        body_base64: base64::engine::general_purpose::STANDARD.encode(&raw_body),
        ..HttpResponsePayload::default()
    };
    let encoded_envelope = serde_json::to_vec(&envelope).expect("encode large envelope");
    assert!(
        encoded_envelope.len() > 4 * 1024 * 1024,
        "envelope should exceed old 4 MiB cap (got {} bytes)",
        encoded_envelope.len()
    );

    let _guard = install_test_backend(Box::new(LargeEnvelopeHost {
        encoded_envelope: encoded_envelope.clone(),
    }));

    let response = HttpClient::default()
        .do_request(
            HttpRequest::get("https://api.example.com/v1/topology/snapshot")
                .with_response_mode(HTTP_RESPONSE_MODE_ENVELOPE),
        )
        .expect("decode >4 MiB envelope response with default HttpClient");

    assert_eq!(response.status, 200);
    assert_eq!(response.header("ETag"), Some("\"snap-v1\""));
    assert_eq!(response.body.len(), raw_body.len());
    assert_eq!(response.body, raw_body);

    let small_client = HttpClient {
        max_response_bytes: 4 * 1024 * 1024,
    };
    let err = small_client
        .do_request(
            HttpRequest::get("https://api.example.com/v1/topology/snapshot")
                .with_response_mode(HTTP_RESPONSE_MODE_ENVELOPE),
        )
        .expect_err("4 MiB client should still reject >4 MiB envelope");
    match err {
        Error::Host(host_err) => {
            assert_eq!(host_err.code, HOST_ERR_TOO_LARGE);
            assert_eq!(host_err.op, "http_request");
        }
        other => panic!("expected HOST_ERR_TOO_LARGE, got {other:?}"),
    }
}
