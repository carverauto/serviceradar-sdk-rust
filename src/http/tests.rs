use std::collections::BTreeMap;
use std::time::Duration;

use base64::Engine;

use crate::host::{TestHostBackend, install_test_backend};

use super::{HttpClient, HttpRequest, HttpRequestPayload, HttpResponse, HttpResponsePayload};

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
