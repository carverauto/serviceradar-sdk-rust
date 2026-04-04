use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::time::Duration;

use crate::Error;
use crate::host::{TestHostBackend, install_test_backend};

use super::{
    WebSocketConnection, WebSocketDialRequest, encode_websocket_connect_payload,
    encode_websocket_dial_request, websocket_dial, websocket_dial_request_with_insecure_tls,
};

struct WebSocketTestHost {
    recv_payloads: VecDeque<Vec<u8>>,
}

impl TestHostBackend for WebSocketTestHost {
    fn websocket_connect(&mut self, req: &[u8], timeout_ms: u32) -> i32 {
        assert_eq!(String::from_utf8_lossy(req), "ws://camera.local/ws");
        assert_eq!(timeout_ms, 1000);
        99
    }

    fn websocket_send(&mut self, handle: u32, data: &[u8], timeout_ms: u32) -> i32 {
        assert_eq!(handle, 99);
        assert_eq!(data, b"hello");
        assert_eq!(timeout_ms, 1000);
        0
    }

    fn websocket_recv(&mut self, handle: u32, buf: &mut [u8], timeout_ms: u32) -> i32 {
        assert_eq!(handle, 99);
        assert_eq!(timeout_ms, 1000);
        let payload = self.recv_payloads.pop_front().unwrap_or_default();
        buf[..payload.len()].copy_from_slice(&payload);
        payload.len() as i32
    }

    fn websocket_close(&mut self, handle: u32) -> i32 {
        assert_eq!(handle, 99);
        0
    }
}

#[test]
fn websocket_requires_handle() {
    let conn = WebSocketConnection::default();
    let err = conn
        .send(b"hello", Duration::from_secs(1))
        .expect_err("send should fail");
    assert!(matches!(err, Error::WebSocketNotInitialized));

    let err = conn
        .recv(&mut [0_u8; 8], Duration::from_secs(1))
        .expect_err("recv should fail");
    assert!(matches!(err, Error::WebSocketNotInitialized));
}

#[test]
fn websocket_payload_without_headers_uses_raw_url() {
    let payload = encode_websocket_dial_request(WebSocketDialRequest {
        url: " wss://protect.local/ws ".to_string(),
        ..WebSocketDialRequest::default()
    })
    .expect("encode websocket payload");
    assert_eq!(
        String::from_utf8(payload).expect("utf8"),
        "wss://protect.local/ws"
    );
}

#[test]
fn websocket_payload_with_headers_uses_json() {
    let mut headers = BTreeMap::new();
    headers.insert("Cookie".to_string(), " TOKEN=abc ".to_string());
    headers.insert("X-API-Key".to_string(), " secret ".to_string());
    headers.insert(String::new(), "ignored".to_string());

    let payload = encode_websocket_dial_request(WebSocketDialRequest {
        url: "wss://protect.local/ws".to_string(),
        headers,
        insecure_skip_verify: false,
    })
    .expect("encode websocket payload");

    let decoded: WebSocketDialRequest =
        serde_json::from_slice(&payload).expect("decode websocket payload");
    assert_eq!(decoded.url, "wss://protect.local/ws");
    assert_eq!(decoded.headers["Cookie"], "TOKEN=abc");
    assert_eq!(decoded.headers["X-API-Key"], "secret");
    assert!(!decoded.headers.contains_key(""));
}

#[test]
fn websocket_connect_payload_with_headers_is_json() {
    let mut headers = BTreeMap::new();
    headers.insert("Authorization".to_string(), "Basic abc".to_string());
    let payload =
        encode_websocket_connect_payload("wss://camera.local/ws", headers).expect("encode payload");
    let body = String::from_utf8(payload).expect("utf8");
    assert!(body.contains("\"url\":\"wss://camera.local/ws\""));
    assert!(body.contains("\"Authorization\":\"Basic abc\""));
}

#[test]
fn websocket_payload_with_insecure_tls_uses_json() {
    let payload = encode_websocket_dial_request(WebSocketDialRequest {
        url: "wss://protect.local/ws".to_string(),
        headers: BTreeMap::new(),
        insecure_skip_verify: true,
    })
    .expect("encode websocket payload");
    let decoded: WebSocketDialRequest =
        serde_json::from_slice(&payload).expect("decode websocket payload");
    assert_eq!(decoded.url, "wss://protect.local/ws");
    assert!(decoded.insecure_skip_verify);
}

#[test]
fn websocket_connection_uses_host_proxy() {
    let _guard = install_test_backend(Box::new(WebSocketTestHost {
        recv_payloads: VecDeque::from([b"world".to_vec()]),
    }));

    let mut conn =
        websocket_dial("ws://camera.local/ws", Duration::from_secs(1)).expect("dial websocket");
    conn.send(b"hello", Duration::from_secs(1)).expect("send");
    let mut buf = [0_u8; 8];
    let read = conn.recv(&mut buf, Duration::from_secs(1)).expect("recv");
    assert_eq!(&buf[..read], b"world");
    conn.close().expect("close");
}

#[test]
fn websocket_insecure_tls_helper_uses_request_path() {
    struct InsecureHost;

    impl TestHostBackend for InsecureHost {
        fn websocket_connect(&mut self, req: &[u8], _timeout_ms: u32) -> i32 {
            let decoded: WebSocketDialRequest =
                serde_json::from_slice(req).expect("decode websocket connect request");
            assert!(decoded.insecure_skip_verify);
            1
        }

        fn websocket_close(&mut self, _handle: u32) -> i32 {
            0
        }
    }

    let _guard = install_test_backend(Box::new(InsecureHost));
    let mut conn =
        websocket_dial_request_with_insecure_tls("wss://protect.local/ws", Duration::from_secs(1))
            .expect("dial insecure websocket");
    conn.close().expect("close");
}
