use std::collections::BTreeMap;

use serde_json::Value;

use crate::error::{HOST_ERR_NOT_FOUND, HOST_ERR_OK};
use crate::host::{TestHostBackend, install_test_backend};
use crate::result::{
    OcsfEvent, SIGNAL_SCHEMA_PAYLOAD_KIND_OCSF_EVENT, SIGNAL_SCHEMA_PAYLOAD_KIND_OTEL_LOG,
    SIGNAL_SCHEMA_SIGNAL_TYPE_EVENT, Severity, SignalSchemaRef,
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
