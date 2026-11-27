use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use base64::{Engine as _, engine::general_purpose};
use serde_json::{Value, json};

use crate::error::{HOST_ERR_NOT_FOUND, HOST_ERR_OK};
use crate::host::{TestHostBackend, install_test_backend};
use crate::metric_envelope::{
    METRIC_ENVELOPE_SCHEMA_VERSION, Metric, MetricBatch, MetricKind, MetricPoint,
};
use crate::result::{
    OcsfEvent, SIGNAL_SCHEMA_PAYLOAD_KIND_OCSF_EVENT, SIGNAL_SCHEMA_PAYLOAD_KIND_OTEL_LOG,
    SIGNAL_SCHEMA_SIGNAL_TYPE_EVENT, Severity, SignalSchemaRef,
};
use crate::{
    SIGNAL_SCHEMA_PAYLOAD_KIND_EXTERNAL_TOPOLOGY_V1,
    SIGNAL_SCHEMA_PAYLOAD_KIND_SERVICERADAR_METRICS,
};

use super::{TelemetryBatch, TelemetryRecord, TelemetrySource, emit_telemetry};

#[derive(Default)]
struct TelemetryHost {
    payloads: Vec<Vec<u8>>,
}

impl TestHostBackend for TelemetryHost {
    fn emit_telemetry(&mut self, payload: &[u8]) -> i32 {
        self.payloads.push(payload.to_vec());
        HOST_ERR_OK
    }
}

struct MissingTelemetryHost;

impl TestHostBackend for MissingTelemetryHost {}

#[test]
fn telemetry_batch_serializes_ocsf_record_with_signal_schema_ref() {
    let event = OcsfEvent::log_activity("camera motion", Severity::Warning);
    let record = TelemetryRecord::ocsf_event(event)
        .expect("build ocsf telemetry record")
        .with_signal_schema_ref(&SignalSchemaRef {
            producer_id: "axis".to_string(),
            producer_version: "0.1.0".to_string(),
            schema_id: "com.carverauto.axis_camera.event_log".to_string(),
            schema_version: "1.0.0".to_string(),
            display_contract_id: "com.carverauto.axis_camera.event_log.display".to_string(),
            display_contract_version: "1.0.0".to_string(),
            display_contract: "display/event_log_activity.display.json".to_string(),
            signal_type: SIGNAL_SCHEMA_SIGNAL_TYPE_EVENT.to_string(),
            payload_kind: SIGNAL_SCHEMA_PAYLOAD_KIND_OCSF_EVENT.to_string(),
        });

    let payload = TelemetryBatch::new(vec![record])
        .with_source(TelemetrySource::new("axis-camera", "front-door"))
        .serialize()
        .expect("serialize telemetry batch");
    let decoded: Value = serde_json::from_slice(&payload).expect("decode telemetry json");
    let record = &decoded["records"][0];

    assert_eq!(
        record["payload_kind"],
        Value::String(SIGNAL_SCHEMA_PAYLOAD_KIND_OCSF_EVENT.to_string())
    );
    assert_eq!(
        record["metadata"]["serviceradar.signal_schema.schema_id"],
        Value::String("com.carverauto.axis_camera.event_log".to_string())
    );
    assert_eq!(
        record["payload"]["message"],
        Value::String("camera motion".to_string())
    );
}

#[test]
fn emit_telemetry_forwards_payload_to_host() {
    let _guard = install_test_backend(Box::new(TelemetryHost::default()));

    emit_telemetry(TelemetryBatch::new(vec![
        TelemetryRecord::otel_log("log-1", BTreeMap::from([("body", "hello")]))
            .expect("build otel log telemetry record"),
    ]))
    .expect("emit telemetry through host");
}

#[test]
fn emit_telemetry_reports_host_error_without_runtime() {
    let _guard = install_test_backend(Box::new(MissingTelemetryHost));

    let err = emit_telemetry(TelemetryBatch::new(vec![
        TelemetryRecord::otel_log("log-1", BTreeMap::from([("body", "hello")]))
            .expect("build otel log telemetry record"),
    ]))
    .expect_err("default host should fail");

    match err {
        crate::Error::Host(host_err) => {
            assert_eq!(host_err.code, HOST_ERR_NOT_FOUND);
            assert_eq!(host_err.op, "emit_telemetry");
        }
        other => panic!("expected host error, got {other:?}"),
    }
}

#[test]
fn otel_log_record_uses_otel_payload_kind() {
    let record = TelemetryRecord::otel_log("log-1", BTreeMap::from([("body", "hello")]))
        .expect("build otel log telemetry record");

    assert_eq!(record.payload_kind, SIGNAL_SCHEMA_PAYLOAD_KIND_OTEL_LOG);
}

#[test]
fn metric_record_serializes_base64_protobuf_payload() {
    let proto_payload = [0x0a, 0x16, b's', b'e', b'r', b'v', b'i', b'c', b'e'];
    let record = TelemetryRecord::serviceradar_metrics("metric-1", &proto_payload);

    let payload = TelemetryBatch::new(vec![record])
        .with_source(TelemetrySource::new("env-sensor", "rack-a"))
        .serialize()
        .expect("serialize telemetry batch");
    let decoded: Value = serde_json::from_slice(&payload).expect("decode telemetry json");
    let record = &decoded["records"][0];

    assert_eq!(
        record["payload_kind"],
        Value::String(SIGNAL_SCHEMA_PAYLOAD_KIND_SERVICERADAR_METRICS.to_string())
    );
    let encoded = record["payload"].as_str().expect("payload string");
    let decoded_payload = general_purpose::STANDARD
        .decode(encoded)
        .expect("payload base64");
    assert_eq!(decoded_payload, proto_payload);
}

#[test]
fn metric_batch_record_serializes_dependency_free_metric_proto_payload() {
    let record = TelemetryRecord::serviceradar_metric_batch(
        "metric-1",
        MetricBatch {
            metrics: vec![Metric {
                name: "plugin.camera_total".to_string(),
                metric_type: "plugin".to_string(),
                kind: MetricKind::Gauge,
                points: vec![MetricPoint {
                    value: 2.0,
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        },
    );

    let payload = TelemetryBatch::new(vec![record])
        .with_source(TelemetrySource::new("camera-plugin", "front-door"))
        .serialize()
        .expect("serialize telemetry batch");
    let decoded: Value = serde_json::from_slice(&payload).expect("decode telemetry json");
    let record = &decoded["records"][0];
    let encoded = record["payload"].as_str().expect("payload string");
    let decoded_payload = general_purpose::STANDARD
        .decode(encoded)
        .expect("payload base64");

    assert_eq!(
        record["payload_kind"],
        Value::String(SIGNAL_SCHEMA_PAYLOAD_KIND_SERVICERADAR_METRICS.to_string())
    );
    assert!(
        decoded_payload
            .windows(METRIC_ENVELOPE_SCHEMA_VERSION.len())
            .any(|window| window == METRIC_ENVELOPE_SCHEMA_VERSION.as_bytes()),
        "metric schema version missing from protobuf payload"
    );
    assert!(
        decoded_payload
            .windows("plugin.camera_total".len())
            .any(|window| window == b"plugin.camera_total"),
        "metric name missing from protobuf payload"
    );
}

#[derive(Default)]
struct RecordingTelemetryHost {
    payloads: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl TestHostBackend for RecordingTelemetryHost {
    fn emit_telemetry(&mut self, payload: &[u8]) -> i32 {
        self.payloads
            .lock()
            .expect("recording telemetry mutex")
            .push(payload.to_vec());
        HOST_ERR_OK
    }
}

#[test]
fn crate_root_reexports_telemetry_payload_kind_constants() {
    assert_eq!(
        SIGNAL_SCHEMA_PAYLOAD_KIND_EXTERNAL_TOPOLOGY_V1,
        "external_topology_v1"
    );
    assert_eq!(
        SIGNAL_SCHEMA_PAYLOAD_KIND_SERVICERADAR_METRICS,
        "serviceradar_metrics"
    );
}

#[test]
fn external_topology_v1_record_serializes_json_snapshot_payload() {
    let snapshot = json!({
        "schema_version": 1,
        "snapshot_id": "snap-0001",
        "nodes": [
            {
                "id": "ext:asn:64512",
                "kind": "asn",
                "asn": 64512,
                "name": "Example Transit"
            },
            {
                "id": "ext:ixp:example-ix",
                "kind": "ixp",
                "port_speed_mbps": 100000
            }
        ],
        "edges": [
            {
                "source": "ext:asn:64512",
                "target": "ext:ixp:example-ix",
                "kind": "ixp_member",
                "port_speed_mbps": 100000
            }
        ]
    });

    let record = TelemetryRecord::external_topology_v1("topo-1", &snapshot)
        .expect("build external_topology_v1 telemetry record");

    assert_eq!(record.event_id.as_deref(), Some("topo-1"));
    assert!(record.observed_time_unix_nano.is_some());
    assert_eq!(record.observed_time_unix_nano, record.event_time_unix_nano);
    assert_eq!(
        record.payload_kind,
        SIGNAL_SCHEMA_PAYLOAD_KIND_EXTERNAL_TOPOLOGY_V1
    );
    assert!(record.metadata.is_empty());
    assert_eq!(record.payload, snapshot);

    let batch_bytes = TelemetryBatch::new(vec![record])
        .with_source(TelemetrySource::new("horizon-feed", "default"))
        .serialize()
        .expect("serialize telemetry batch");
    let decoded: Value = serde_json::from_slice(&batch_bytes).expect("decode telemetry json");
    let encoded_record = &decoded["records"][0];

    assert_eq!(encoded_record["event_id"], json!("topo-1"));
    assert_eq!(
        encoded_record["payload_kind"],
        json!("external_topology_v1")
    );
    assert_eq!(encoded_record["payload"]["schema_version"], json!(1));
    assert_eq!(encoded_record["payload"], snapshot);
    assert!(encoded_record.get("metadata").is_none());
}

#[test]
fn external_topology_v1_omits_empty_event_id_and_emits_to_host() {
    let snapshot = json!({
        "schema_version": 1,
        "nodes": [{ "id": "ext:vantage:pop-1", "kind": "vantage" }],
        "edges": []
    });

    let record = TelemetryRecord::external_topology_v1("", &snapshot)
        .expect("build external_topology_v1 with empty event_id");
    assert_eq!(record.event_id, None);

    let captured = Arc::new(Mutex::new(Vec::new()));
    let _guard = install_test_backend(Box::new(RecordingTelemetryHost {
        payloads: Arc::clone(&captured),
    }));

    emit_telemetry(
        TelemetryBatch::new(vec![record])
            .with_source(TelemetrySource::new("horizon-feed", "pop-1")),
    )
    .expect("emit external_topology_v1 telemetry through host");

    let payloads = captured.lock().expect("captured payloads mutex");
    assert_eq!(payloads.len(), 1);
    let decoded: Value = serde_json::from_slice(&payloads[0]).expect("decode captured batch");
    let emitted = &decoded["records"][0];
    assert!(emitted.get("event_id").is_none());
    assert_eq!(emitted["payload_kind"], json!("external_topology_v1"));
    assert_eq!(emitted["payload"], snapshot);
}
