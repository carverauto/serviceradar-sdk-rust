// The wire documents below are written out by hand from the host contract, so
// a renamed key or schema fails here.

use serde_json::{Value, json};

use super::{
    CameraSourceFailure, CameraSourceResolution, parse_camera_source_resolution_request,
    parse_omit_camera_stream_addresses,
};

#[test]
fn parses_a_resolution_request_and_ignores_ordinary_configs() {
    let config = br#"{
        "host": "192.0.2.30",
        "_serviceradar_camera_source_resolution_request": {
            "schema": "serviceradar.camera_source_resolution_request.v1",
            "vendor_camera_id": "camera-vendor-1",
            "vendor_profile_id": "high"
        }
    }"#;

    let request = parse_camera_source_resolution_request(config)
        .expect("parse")
        .expect("request present");
    assert_eq!(request.vendor_camera_id, "camera-vendor-1");
    assert_eq!(request.vendor_profile_id, "high");

    for ordinary in ["", "{}", r#"{"host":"192.0.2.30"}"#] {
        assert!(
            parse_camera_source_resolution_request(ordinary.as_bytes())
                .expect("ordinary config parses")
                .is_none(),
            "ordinary config {ordinary:?} produced a request"
        );
    }
}

#[test]
fn rejects_requests_the_plugin_cannot_act_on() {
    for (name, config) in [
        (
            "unknown schema",
            r#"{"_serviceradar_camera_source_resolution_request":{"schema":"serviceradar.camera_source_resolution_request.v2","vendor_camera_id":"camera-vendor-1"}}"#,
        ),
        (
            "no camera id",
            r#"{"_serviceradar_camera_source_resolution_request":{"schema":"serviceradar.camera_source_resolution_request.v1","vendor_camera_id":" "}}"#,
        ),
        (
            "not an object",
            r#"{"_serviceradar_camera_source_resolution_request":"high"}"#,
        ),
        ("config invalid", "["),
    ] {
        assert!(
            parse_camera_source_resolution_request(config.as_bytes()).is_err(),
            "{name}: expected an error"
        );
    }
}

fn document(resolution: &CameraSourceResolution) -> Value {
    serde_json::from_slice(&resolution.serialize().expect("serialize")).expect("decode")
}

#[test]
fn serializes_an_address_or_a_failure() {
    assert_eq!(
        document(&CameraSourceResolution::Url(
            " rtsps://192.0.2.10:7441/aliasInvented0001?enableSrtp ".to_string()
        )),
        json!({
            "schema": "serviceradar.camera_source_resolution.v1",
            "url": "rtsps://192.0.2.10:7441/aliasInvented0001?enableSrtp"
        })
    );

    assert_eq!(
        document(&CameraSourceResolution::Failure {
            class: CameraSourceFailure::NotFound,
            detail: "streaming is disabled for this camera".to_string(),
        }),
        json!({
            "schema": "serviceradar.camera_source_resolution.v1",
            "failure_class": "source_not_found",
            "failure_detail": "streaming is disabled for this camera"
        })
    );

    assert!(
        CameraSourceResolution::Url("  ".to_string())
            .serialize()
            .is_err()
    );
}

#[test]
fn failure_classes_use_the_host_names() {
    for (class, name) in [
        (CameraSourceFailure::NotFound, "source_not_found"),
        (
            CameraSourceFailure::AuthenticationFailed,
            "authentication_failed",
        ),
        (CameraSourceFailure::DnsFailure, "dns_failure"),
        (CameraSourceFailure::ConnectRefused, "connect_refused"),
        (CameraSourceFailure::ConnectTimeout, "connect_timeout"),
        (CameraSourceFailure::HostUnreachable, "host_unreachable"),
        (CameraSourceFailure::ConnectFailed, "connect_failed"),
        (
            CameraSourceFailure::TlsCertificateInvalid,
            "tls_certificate_invalid",
        ),
        (
            CameraSourceFailure::TlsHandshakeFailed,
            "tls_handshake_failed",
        ),
    ] {
        assert_eq!(class.as_str(), name);
    }
}

#[test]
fn reads_the_address_omit_key() {
    for (config, want) in [
        (
            r#"{"_serviceradar_camera_stream_addresses":"omit","host":"192.0.2.30"}"#,
            true,
        ),
        (
            r#"{"_serviceradar_camera_stream_addresses":"report"}"#,
            false,
        ),
        (r#"{"host":"192.0.2.30"}"#, false),
        ("", false),
    ] {
        assert_eq!(
            parse_omit_camera_stream_addresses(config.as_bytes()).expect("parse"),
            want,
            "config {config:?}"
        );
    }

    assert!(
        parse_omit_camera_stream_addresses(br#"{"_serviceradar_camera_stream_addresses":true}"#)
            .is_err()
    );
}
