//! Conformance replay (task 5.1): golden host-call transcripts produced by
//! the Go SDK, replayed against the Rust SDK.
//!
//! Goldens live in `conformance/testdata/` (vendored from
//! `serviceradar-sdk-go/sdk/testdata/conformance/`; provenance and the
//! transcript schema are documented in `conformance/README.md`). The single
//! `replay` test walks `manifest.json` and dispatches each capability to a
//! replay function that drives the equivalent Rust SDK calls against a
//! scripted recording host, then compares the recorded calls (and derived
//! outputs) with the golden.
//!
//! Comparison rules, mirroring the Go producer:
//! - JSON-document args (`{"json": ...}`) compare semantically, so key order
//!   never matters.
//! - Read buffer capacities and wall-clock durations are not recorded at all.
//! - Fake handles start at 7 in call order on both sides.
//! - The `rtsp` capability alone allows `timeout_ms` to differ by up to 100:
//!   both RTSP clients derive per-call timeouts from remaining deadlines, so
//!   the exact millisecond is timing-dependent by construction.
//!
//! Adding a capability to the Go SDK without a replay here fails this test
//! and names the missing capability.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use serde_json::{Map, Value};

use crate::host::{TestHostBackend, install_test_backend};

fn testdata_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("conformance/testdata")
}

fn load_json(name: &str) -> Value {
    let path = testdata_dir().join(name);
    let raw = std::fs::read(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    serde_json::from_slice(&raw).unwrap_or_else(|err| panic!("decode {}: {err}", path.display()))
}

fn encode_bytes(raw: &[u8]) -> Value {
    match std::str::from_utf8(raw) {
        Ok(text) => Value::String(text.to_owned()),
        Err(_) => {
            let mut map = Map::new();
            map.insert(
                "base64".to_owned(),
                Value::String(base64::engine::general_purpose::STANDARD.encode(raw)),
            );
            Value::Object(map)
        }
    }
}

fn encode_json_doc(raw: &[u8]) -> Value {
    match serde_json::from_slice::<Value>(raw) {
        Ok(parsed) => {
            let mut map = Map::new();
            map.insert("json".to_owned(), parsed);
            Value::Object(map)
        }
        Err(_) => encode_bytes(raw),
    }
}

struct Shared {
    calls: Vec<Value>,
    next_handle: u32,
    tcp_reads: Vec<Vec<u8>>,
    ws_recvs: Vec<Vec<u8>>,
    http_response: Vec<u8>,
    config: Vec<u8>,
}

struct RecordingBackend {
    shared: Arc<Mutex<Shared>>,
}

impl RecordingBackend {
    fn record(&self, func: &str, args: Map<String, Value>, returns: Option<Map<String, Value>>) {
        let mut call = Map::new();
        call.insert("func".to_owned(), Value::String(func.to_owned()));
        call.insert("args".to_owned(), Value::Object(args));
        if let Some(returns) = returns {
            call.insert("returns".to_owned(), Value::Object(returns));
        }
        self.shared
            .lock()
            .expect("calls mutex")
            .calls
            .push(Value::Object(call));
    }

    fn next_handle(&self) -> u32 {
        let mut shared = self.shared.lock().expect("calls mutex");
        let handle = shared.next_handle;
        shared.next_handle += 1;
        handle
    }

    fn insert_timeout(args: &mut Map<String, Value>, timeout_ms: u32) {
        args.insert("timeout_ms".to_owned(), Value::from(timeout_ms));
    }
}

fn num(value: u32) -> Value {
    Value::from(value)
}

fn handle_num(value: u32) -> Value {
    Value::from(value)
}

impl TestHostBackend for RecordingBackend {
    fn get_config(&mut self, buf: &mut [u8]) -> i32 {
        let config = self.shared.lock().expect("calls mutex").config.clone();
        let n = config.len().min(buf.len());
        buf[..n].copy_from_slice(&config[..n]);
        let args = Map::new();
        let mut returns = Map::new();
        returns.insert("n".to_owned(), num(n as u32));
        self.record("get_config", args, Some(returns));
        n as i32
    }

    fn log(&mut self, level: u32, msg: &[u8]) {
        let mut args = Map::new();
        args.insert("level".to_owned(), num(level));
        args.insert("message".to_owned(), encode_bytes(msg));
        self.record("log", args, None);
    }

    fn submit_result(&mut self, payload: &[u8]) -> i32 {
        let mut args = Map::new();
        args.insert("payload".to_owned(), encode_json_doc(payload));
        let mut returns = Map::new();
        returns.insert("code".to_owned(), num(0));
        self.record("submit_result", args, Some(returns));
        0
    }

    fn emit_telemetry(&mut self, payload: &[u8]) -> i32 {
        let mut args = Map::new();
        args.insert("payload".to_owned(), encode_json_doc(payload));
        let mut returns = Map::new();
        returns.insert("code".to_owned(), num(0));
        self.record("emit_telemetry", args, Some(returns));
        0
    }

    fn http_request(&mut self, req: &[u8], resp: &mut [u8]) -> i32 {
        let reply = self
            .shared
            .lock()
            .expect("calls mutex")
            .http_response
            .clone();
        let n = reply.len().min(resp.len());
        resp[..n].copy_from_slice(&reply[..n]);
        let mut args = Map::new();
        args.insert("request".to_owned(), encode_json_doc(req));
        let mut returns = Map::new();
        returns.insert("n".to_owned(), num(n as u32));
        returns.insert("response".to_owned(), encode_bytes(&reply[..n]));
        self.record("http_request", args, Some(returns));
        n as i32
    }

    fn tcp_connect(&mut self, addr: &[u8], port: u32, timeout_ms: u32) -> i32 {
        let handle = self.next_handle();
        let mut args = Map::new();
        args.insert(
            "addr".to_owned(),
            Value::String(String::from_utf8_lossy(addr).into_owned()),
        );
        args.insert("port".to_owned(), num(port));
        Self::insert_timeout(&mut args, timeout_ms);
        let mut returns = Map::new();
        returns.insert("handle".to_owned(), handle_num(handle));
        self.record("tcp_connect", args, Some(returns));
        handle as i32
    }

    fn tcp_read(&mut self, handle: u32, buf: &mut [u8], timeout_ms: u32) -> i32 {
        let mut shared = self.shared.lock().expect("calls mutex");
        let chunk = if shared.tcp_reads.is_empty() {
            Vec::new()
        } else {
            shared.tcp_reads.remove(0)
        };
        drop(shared);
        let n = chunk.len().min(buf.len());
        buf[..n].copy_from_slice(&chunk[..n]);
        let mut args = Map::new();
        args.insert("handle".to_owned(), handle_num(handle));
        Self::insert_timeout(&mut args, timeout_ms);
        let mut returns = Map::new();
        returns.insert("n".to_owned(), num(n as u32));
        returns.insert("data".to_owned(), encode_bytes(&chunk[..n]));
        self.record("tcp_read", args, Some(returns));
        n as i32
    }

    fn tcp_write(&mut self, handle: u32, buf: &[u8], timeout_ms: u32) -> i32 {
        let mut args = Map::new();
        args.insert("handle".to_owned(), handle_num(handle));
        args.insert("data".to_owned(), encode_bytes(buf));
        Self::insert_timeout(&mut args, timeout_ms);
        let mut returns = Map::new();
        returns.insert("n".to_owned(), num(buf.len() as u32));
        self.record("tcp_write", args, Some(returns));
        buf.len() as i32
    }

    fn tcp_close(&mut self, handle: u32) -> i32 {
        let mut args = Map::new();
        args.insert("handle".to_owned(), handle_num(handle));
        let mut returns = Map::new();
        returns.insert("code".to_owned(), num(0));
        self.record("tcp_close", args, Some(returns));
        0
    }

    fn udp_send_to(&mut self, addr: &[u8], port: u32, buf: &[u8], timeout_ms: u32) -> i32 {
        let mut args = Map::new();
        args.insert(
            "addr".to_owned(),
            Value::String(String::from_utf8_lossy(addr).into_owned()),
        );
        args.insert("port".to_owned(), num(port));
        args.insert("payload".to_owned(), encode_bytes(buf));
        Self::insert_timeout(&mut args, timeout_ms);
        let mut returns = Map::new();
        returns.insert("code".to_owned(), num(0));
        self.record("udp_send_to", args, Some(returns));
        0
    }

    fn websocket_connect(&mut self, req: &[u8], timeout_ms: u32) -> i32 {
        let handle = self.next_handle();
        let mut args = Map::new();
        args.insert("request".to_owned(), encode_json_doc(req));
        Self::insert_timeout(&mut args, timeout_ms);
        let mut returns = Map::new();
        returns.insert("handle".to_owned(), handle_num(handle));
        self.record("websocket_connect", args, Some(returns));
        handle as i32
    }

    fn websocket_send(&mut self, handle: u32, data: &[u8], timeout_ms: u32) -> i32 {
        let mut args = Map::new();
        args.insert("handle".to_owned(), handle_num(handle));
        args.insert("data".to_owned(), encode_bytes(data));
        Self::insert_timeout(&mut args, timeout_ms);
        let mut returns = Map::new();
        returns.insert("n".to_owned(), num(data.len() as u32));
        self.record("websocket_send", args, Some(returns));
        data.len() as i32
    }

    fn websocket_recv(&mut self, handle: u32, buf: &mut [u8], timeout_ms: u32) -> i32 {
        let mut shared = self.shared.lock().expect("calls mutex");
        let chunk = if shared.ws_recvs.is_empty() {
            Vec::new()
        } else {
            shared.ws_recvs.remove(0)
        };
        drop(shared);
        let n = chunk.len().min(buf.len());
        buf[..n].copy_from_slice(&chunk[..n]);
        let mut args = Map::new();
        args.insert("handle".to_owned(), handle_num(handle));
        Self::insert_timeout(&mut args, timeout_ms);
        let mut returns = Map::new();
        returns.insert("n".to_owned(), num(n as u32));
        returns.insert("data".to_owned(), encode_bytes(&chunk[..n]));
        self.record("websocket_recv", args, Some(returns));
        n as i32
    }

    fn websocket_close(&mut self, handle: u32) -> i32 {
        let mut args = Map::new();
        args.insert("handle".to_owned(), handle_num(handle));
        let mut returns = Map::new();
        returns.insert("code".to_owned(), num(0));
        self.record("websocket_close", args, Some(returns));
        0
    }
}

/// Compares golden calls with replayed calls. `timeout_tolerant` (rtsp only)
/// allows `timeout_ms` to differ by up to 100 ms; see the module docs.
fn compare_calls(capability: &str, expected: &[Value], actual: &[Value]) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "conformance {capability}: call count differs (actual {actual:?})"
    );
    for (index, (want, got)) in expected.iter().zip(actual.iter()).enumerate() {
        let path = format!("conformance {capability} call {index}");
        compare_call(&path, capability, want, got);
    }
}

fn compare_call(path: &str, capability: &str, want: &Value, got: &Value) {
    let want_func = want.get("func").and_then(Value::as_str).unwrap_or("?");
    let got_func = got.get("func").and_then(Value::as_str).unwrap_or("?");
    assert_eq!(got_func, want_func, "{path}: func differs");
    compare_value(
        &format!("{path} args"),
        capability,
        want.get("args").unwrap_or(&Value::Null),
        got.get("args").unwrap_or(&Value::Null),
    );
    compare_value(
        &format!("{path} returns"),
        capability,
        want.get("returns").unwrap_or(&Value::Null),
        got.get("returns").unwrap_or(&Value::Null),
    );
}

fn compare_value(path: &str, capability: &str, want: &Value, got: &Value) {
    match (want, got) {
        (Value::Object(want_map), Value::Object(got_map)) => {
            assert_eq!(
                got_map.len(),
                want_map.len(),
                "{path}: arg count differs (want {want} got {got})"
            );
            for (key, want_value) in want_map {
                let got_value = got_map
                    .get(key)
                    .unwrap_or_else(|| panic!("{path}: missing key {key} (got {got})"));
                if capability == "rtsp" && key == "timeout_ms" {
                    compare_timeout(path, want_value, got_value);
                } else {
                    compare_value(&format!("{path}.{key}"), capability, want_value, got_value);
                }
            }
        }
        (Value::Array(want_items), Value::Array(got_items)) => {
            assert_eq!(
                got_items.len(),
                want_items.len(),
                "{path}: array length differs (want {want} got {got})"
            );
            for (index, (want_item, got_item)) in
                want_items.iter().zip(got_items.iter()).enumerate()
            {
                compare_value(&format!("{path}[{index}]"), capability, want_item, got_item);
            }
        }
        _ => assert_eq!(got, want, "{path}: value differs (want {want} got {got})"),
    }
}

fn compare_timeout(path: &str, want: &Value, got: &Value) {
    match (want.as_u64(), got.as_u64()) {
        (Some(want_ms), Some(got_ms)) => {
            let delta = want_ms.abs_diff(got_ms);
            assert!(
                delta <= 100,
                "{path}: timeout_ms differs by {delta} ms (want {want_ms} got {got_ms})"
            );
        }
        _ => assert_eq!(got, want, "{path}: timeout_ms differs"),
    }
}

fn golden_string<'a>(map: &'a Map<String, Value>, key: &str) -> &'a str {
    map.get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("golden scenario lacks string {key}"))
}

fn golden_u64(map: &Map<String, Value>, key: &str) -> u64 {
    map.get(key)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("golden scenario lacks number {key}"))
}

fn golden_timeout(scenario: &Map<String, Value>) -> Duration {
    Duration::from_millis(golden_u64(scenario, "timeout_ms"))
}

fn reply_bytes(replies: &Map<String, Value>, key: &str) -> Vec<Vec<u8>> {
    replies
        .get(key)
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("golden replies lack {key}"))
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .unwrap_or_else(|| panic!("golden reply {key} entry is not a string"))
                .as_bytes()
                .to_vec()
        })
        .collect()
}

fn with_scripted(replies: &Map<String, Value>, run: impl FnOnce(&Arc<Mutex<Shared>>)) {
    let shared = Arc::new(Mutex::new(Shared {
        calls: Vec::new(),
        next_handle: 7,
        tcp_reads: replies
            .get("tcp_reads")
            .map(|_| reply_bytes(replies, "tcp_reads"))
            .unwrap_or_default(),
        ws_recvs: replies
            .get("websocket_recvs")
            .map(|_| reply_bytes(replies, "websocket_recvs"))
            .unwrap_or_default(),
        http_response: replies
            .get("http_response")
            .and_then(Value::as_str)
            .map(str::as_bytes)
            .unwrap_or_default()
            .to_vec(),
        config: Vec::new(),
    }));
    let backend = RecordingBackend {
        shared: Arc::clone(&shared),
    };
    let _guard = install_test_backend(Box::new(backend));
    run(&shared);
}

fn replay_http(scenario: &Map<String, Value>) {
    use crate::{HttpClient, HttpRequest};

    let mut headers = BTreeMap::new();
    if let Some(Value::Object(entries)) = scenario.get("headers") {
        for (key, value) in entries {
            headers.insert(key.clone(), value.as_str().unwrap_or_default().to_owned());
        }
    }
    let request = HttpRequest {
        method: golden_string(scenario, "method").to_owned(),
        url: golden_string(scenario, "url").to_owned(),
        headers,
        body: scenario
            .get("body")
            .and_then(Value::as_str)
            .map(str::as_bytes)
            .unwrap_or_default()
            .to_vec(),
        body_base64: false,
        response_mode: golden_string(scenario, "response_mode").to_owned(),
        timeout_ms: golden_u64(scenario, "timeout_ms") as u32,
        insecure_skip_verify: false,
    };
    let response = HttpClient::default()
        .do_request(request)
        .expect("http replay");
    match golden_string(scenario, "response_mode") {
        "status_body" => {
            assert_eq!(response.status, 200, "conformance http status");
            assert_eq!(response.body, b"{\"ok\":true}", "conformance http body");
        }
        "envelope" => {
            assert_eq!(response.status, 202, "conformance http status");
            assert_eq!(response.body, b"{}", "conformance http body");
            assert_eq!(
                response.headers.get("content-type").map(String::as_str),
                Some("application/json"),
                "conformance http headers"
            );
        }
        mode => panic!("unknown golden response_mode {mode}"),
    }
}

fn replay_tcp(scenario: &Map<String, Value>, replies: &Map<String, Value>) {
    use crate::tcp_dial;

    let timeout = golden_timeout(scenario);
    let port = golden_u64(scenario, "port") as u16;
    let mut conn = tcp_dial(golden_string(scenario, "addr"), port, timeout).expect("tcp dial");
    conn.write(golden_string(scenario, "write").as_bytes(), timeout)
        .expect("tcp write");
    let mut buf = vec![0_u8; 4096];
    let n = conn.read(&mut buf, timeout).expect("tcp read");
    let reply = reply_bytes(replies, "tcp_reads");
    assert_eq!(&buf[..n], reply[0].as_slice(), "conformance tcp reply");
    conn.close().expect("tcp close");
}

fn replay_udp(scenario: &Map<String, Value>) {
    use crate::udp_send_to;

    udp_send_to(
        golden_string(scenario, "addr"),
        golden_u64(scenario, "port") as u16,
        golden_string(scenario, "payload").as_bytes(),
        golden_timeout(scenario),
    )
    .expect("udp send");
}

fn replay_websocket(scenario: &Map<String, Value>, replies: &Map<String, Value>) {
    use crate::websocket_dial;

    let timeout = golden_timeout(scenario);
    let mut conn = websocket_dial(golden_string(scenario, "url"), timeout).expect("websocket dial");
    conn.send(golden_string(scenario, "send").as_bytes(), timeout)
        .expect("websocket send");
    let mut buf = vec![0_u8; 4096];
    let n = conn.recv(&mut buf, timeout).expect("websocket recv");
    let reply = reply_bytes(replies, "websocket_recvs");
    assert_eq!(
        &buf[..n],
        reply[0].as_slice(),
        "conformance websocket reply"
    );
    conn.close().expect("websocket close");
}

fn replay_rtsp(scenario: &Map<String, Value>) {
    use crate::dial_rtsp_transport;
    use crate::rtsp::{RtspClient, RtspEndpoint};

    let timeout = golden_timeout(scenario);
    let endpoint =
        RtspEndpoint::parse(golden_string(scenario, "url"), "", "").expect("rtsp endpoint");
    let conn = dial_rtsp_transport(&endpoint, timeout, false).expect("rtsp dial");
    let mut client = RtspClient::new(conn, timeout, endpoint.clone());
    let options = client
        .do_request("OPTIONS", &endpoint.request_uri, &BTreeMap::new())
        .expect("rtsp options");
    assert_eq!(options.status_code, 200, "conformance rtsp options");
    let mut describe_headers = BTreeMap::new();
    if let Some(Value::Object(entries)) = scenario.get("describe_headers") {
        for (key, value) in entries {
            describe_headers.insert(key.clone(), value.as_str().unwrap_or_default().to_owned());
        }
    }
    let describe = client
        .do_request("DESCRIBE", &endpoint.request_uri, &describe_headers)
        .expect("rtsp describe");
    assert_eq!(describe.status_code, 200, "conformance rtsp describe");
}

fn replay_result() {
    use crate::result::{OcsfEvent, Result};
    use crate::submit_result_payload;

    let mut metadata = BTreeMap::new();
    metadata.insert("version".to_owned(), Value::String("1.7.0".to_owned()));
    let mut table = BTreeMap::new();
    table.insert("latency_ms".to_owned(), "4".to_owned());
    let result = Result::ok("conformance ok")
        .with_observed_at("2026-09-27T12:00:00Z")
        .with_details("golden result payload")
        .with_label("camera", "cam-1")
        .with_table(table, "key-value");
    let mut result = result;
    result.add_ocsf_event(OcsfEvent {
        id: "evt-conformance-1".to_owned(),
        time: "2026-09-27T12:00:00Z".to_owned(),
        class_uid: 1008,
        category_uid: 1,
        type_uid: 100801,
        activity_id: 1,
        activity_name: Some("Create".to_owned()),
        severity_id: 1,
        severity: Some("Informational".to_owned()),
        message: Some("conformance event".to_owned()),
        status_id: None,
        status: None,
        status_code: None,
        status_detail: None,
        metadata,
        observables: Vec::new(),
        trace_id: None,
        span_id: None,
        actor: BTreeMap::new(),
        device: BTreeMap::new(),
        src_endpoint: BTreeMap::new(),
        dst_endpoint: BTreeMap::new(),
        log_name: Some("events.ocsf.processed".to_owned()),
        log_provider: Some("serviceradar-plugin".to_owned()),
        log_level: None,
        log_version: None,
        unmapped: BTreeMap::new(),
        raw_data: None,
    });
    let payload = result.serialize().expect("result serialize");
    submit_result_payload(&payload).expect("submit result");
}

fn replay_telemetry_metric() {
    use crate::result::SIGNAL_SCHEMA_PAYLOAD_KIND_SERVICERADAR_METRICS;
    use crate::{
        Metric, MetricBatch, MetricIngestIdentity, MetricKind, MetricPoint, MetricResource,
        MetricValueType, TelemetryBatch, TelemetryRecord, TelemetrySource, emit_telemetry,
        marshal_metric_batch,
    };

    let observed: i64 = 1_790_510_400_000_000_000;
    let batch = MetricBatch {
        schema_version: String::new(),
        resource: MetricResource {
            agent_id: "agent-1".to_owned(),
            service_name: "conformance".to_owned(),
            service_type: "wasm-plugin".to_owned(),
            ..Default::default()
        },
        ingest_identity: MetricIngestIdentity {
            source: "plugin-metrics".to_owned(),
            producer_id: "conformance".to_owned(),
            producer_kind: "wasm-plugin".to_owned(),
            ..Default::default()
        },
        metrics: vec![Metric {
            name: "conformance.latency_ms".to_owned(),
            metric_type: "plugin".to_owned(),
            kind: MetricKind::Gauge,
            unit: "ms".to_owned(),
            points: vec![MetricPoint {
                value: 4.5,
                raw_value: "4.5".to_owned(),
                raw_value_type: MetricValueType::Double,
                observed_at_unix_nano: observed as u64,
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let proto = marshal_metric_batch(batch);
    let record = TelemetryRecord {
        event_id: Some("evt-conformance-1".to_owned()),
        observed_time_unix_nano: Some(observed),
        event_time_unix_nano: Some(observed),
        payload_kind: SIGNAL_SCHEMA_PAYLOAD_KIND_SERVICERADAR_METRICS.to_owned(),
        payload: Value::String(base64::engine::general_purpose::STANDARD.encode(&proto)),
        metadata: BTreeMap::new(),
    };
    let batch = TelemetryBatch::new(vec![record])
        .with_source(TelemetrySource::new("plugin", "conformance"));
    emit_telemetry(batch).expect("emit telemetry");
}

fn replay_run_overrides(scenario: &Map<String, Value>) -> Value {
    use crate::parse_run_overrides;

    let raw =
        serde_json::to_vec(scenario.get("config").expect("golden config")).expect("encode config");
    let overrides = parse_run_overrides(&raw).expect("parse overrides");
    assert_eq!(overrides.len(), 2, "conformance overrides");
    serde_json::to_value(&overrides).expect("encode overrides")
}

fn replay_action_result() -> Value {
    use crate::ActionResult;

    let mut params = Map::new();
    params.insert("severity".to_owned(), Value::String("critical".to_owned()));
    let result = ActionResult::succeeded("fault injected")
        .set_run_override(
            "fault-jam-7",
            "conveyor_jam",
            Some("conveyor-7".to_owned()),
            300,
            params,
        )
        .end_run_override("fault-old-1");
    let raw = result.serialize().expect("action serialize");
    serde_json::from_slice(&raw).expect("decode action result")
}

fn replay_plugin_inputs(scenario: &Map<String, Value>) -> Value {
    use crate::parse_plugin_inputs_json;

    let raw = serde_json::to_vec(scenario.get("document").expect("golden document"))
        .expect("encode document");
    let payload = parse_plugin_inputs_json(&raw).expect("parse inputs");
    let projected: Vec<Value> = payload
        .flatten_items()
        .iter()
        .map(|item| {
            let mut map = Map::new();
            map.insert("name".to_owned(), Value::String(item.name.clone()));
            map.insert("entity".to_owned(), Value::String(item.entity.clone()));
            map.insert("query".to_owned(), Value::String(item.query.clone()));
            map.insert("chunk_index".to_owned(), Value::from(item.chunk_index));
            map.insert("chunk_total".to_owned(), Value::from(item.chunk_total));
            map.insert(
                "chunk_hash".to_owned(),
                Value::String(item.chunk_hash.clone()),
            );
            map.insert(
                "item".to_owned(),
                Value::Object(
                    item.item
                        .iter()
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect(),
                ),
            );
            Value::Object(map)
        })
        .collect();
    Value::Array(projected)
}

fn replay_capability(capability: &str) {
    let golden = load_json(&format!("{capability}.json"));
    assert_eq!(
        golden.get("format").and_then(Value::as_str),
        Some("serviceradar-sdk-conformance/v1"),
        "conformance {capability}: format"
    );
    let scenario = golden
        .get("scenario")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let replies = golden
        .get("replies")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let want_calls: Vec<Value> = golden
        .get("calls")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    with_scripted(&replies, |shared| {
        let output: Option<Value> = match capability {
            "http_status_body" | "http_envelope" => {
                replay_http(&scenario);
                None
            }
            "tcp" => {
                replay_tcp(&scenario, &replies);
                None
            }
            "udp" => {
                replay_udp(&scenario);
                None
            }
            "websocket" => {
                replay_websocket(&scenario, &replies);
                None
            }
            "rtsp" => {
                replay_rtsp(&scenario);
                None
            }
            "result" => {
                replay_result();
                None
            }
            "telemetry_metric" => {
                replay_telemetry_metric();
                None
            }
            "run_overrides" => Some(replay_run_overrides(&scenario)),
            "action_result" => Some(replay_action_result()),
            "plugin_inputs" => Some(replay_plugin_inputs(&scenario)),
            other => panic!("no Rust replay for conformance capability '{other}'"),
        };
        let recorded = shared.lock().expect("calls mutex").calls.clone();
        compare_calls(capability, &want_calls, &recorded);
        if let Some(want_output) = golden.get("output") {
            compare_value(
                &format!("conformance {capability} output"),
                capability,
                want_output,
                output.as_ref().unwrap_or_else(|| {
                    panic!("conformance {capability}: replay produced no output")
                }),
            );
        }
    });
}

#[test]
fn replay_conformance_transcripts() {
    let manifest = load_json("manifest.json");
    assert_eq!(
        manifest.get("format").and_then(Value::as_str),
        Some("serviceradar-sdk-conformance/v1"),
        "conformance manifest format"
    );
    let capabilities = manifest
        .get("capabilities")
        .and_then(Value::as_array)
        .expect("conformance manifest capabilities");
    assert!(
        !capabilities.is_empty(),
        "conformance manifest lists no capabilities"
    );
    for capability in capabilities {
        let name = capability.as_str().expect("conformance capability name");
        replay_capability(name);
    }
}
