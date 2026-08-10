use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use crate::{
    OUTPUTS_PLUGIN_RESULT, PluginManifest, RUNTIME_WASI_PREVIEW1,
    SIGNAL_SCHEMA_PAYLOAD_KIND_OCSF_EVENT, SIGNAL_SCHEMA_SIGNAL_TYPE_EVENT,
    SignalSchemaContribution,
};

fn security_fixture_manifest() -> PluginManifest {
    let schema = SignalSchemaContribution::new(
        "com.carverauto.security.scan_activity",
        "1.0.0",
        SIGNAL_SCHEMA_SIGNAL_TYPE_EVENT,
        SIGNAL_SCHEMA_PAYLOAD_KIND_OCSF_EVENT,
    )
    .with_ocsf("1.9.0-dev", 6007, 600_701);

    let mut permissions = BTreeMap::new();
    permissions.insert("allowed_domains".to_string(), json!([]));
    permissions.insert("allowed_ports".to_string(), json!([]));

    let mut resources = BTreeMap::new();
    resources.insert("requested_memory_mb".to_string(), json!(32));
    resources.insert("requested_cpu_ms".to_string(), json!(5000));
    resources.insert("max_open_connections".to_string(), json!(4));

    PluginManifest {
        id: "security-sample".to_string(),
        name: "Security Sample".to_string(),
        version: "1.0.0".to_string(),
        description: None,
        entrypoint: "run_check".to_string(),
        runtime: Some(RUNTIME_WASI_PREVIEW1.to_string()),
        capabilities: vec![
            "get_config".to_string(),
            "log".to_string(),
            "submit_result".to_string(),
            "emit_telemetry".to_string(),
        ],
        permissions,
        resources,
        outputs: OUTPUTS_PLUGIN_RESULT.to_string(),
        signal_schemas: vec![schema],
    }
}

fn fixture_path() -> std::path::PathBuf {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    manifest_dir.join("testdata/sdk_generated_security_manifest.json")
}

#[test]
fn plugin_manifest_serializes_sdk_generated_security_fixture() {
    let payload = security_fixture_manifest()
        .serialize()
        .expect("fixture manifest must be valid");
    let got: Value = serde_json::from_slice(&payload).expect("decode generated manifest");

    let expected_raw = std::fs::read(fixture_path()).expect("read fixture");
    let expected: Value = serde_json::from_slice(&expected_raw).expect("decode fixture");

    assert_eq!(got, expected);
}

/// The manifest surface is closed on purpose: core rejects any `signal_schemas`
/// key outside its allowlist. A field added here without a matching change in
/// core would produce manifests that fail at upload, so pin the encoded key set.
#[test]
fn signal_schema_contribution_encodes_only_core_allowed_keys() {
    let allowed: BTreeSet<&str> = [
        "id",
        "version",
        "signal_type",
        "payload_kind",
        "payload_schema",
        "display_contract",
        "display_contract_id",
        "display_contract_version",
        "ocsf_schema_version",
        "class_uid",
        "type_uid",
    ]
    .into_iter()
    .collect();

    let encoded = serde_json::to_value(&security_fixture_manifest().signal_schemas[0])
        .expect("encode contribution");
    let object = encoded.as_object().expect("contribution encodes an object");
    let keys: BTreeSet<&str> = object.keys().map(String::as_str).collect();

    assert_eq!(
        keys, allowed,
        "encoded signal schema keys must match core's allowlist exactly"
    );
}

fn error_paths(manifest: &PluginManifest) -> Vec<String> {
    manifest
        .validate()
        .expect_err("expected manifest to be rejected")
        .into_iter()
        .map(|error| error.path)
        .collect()
}

#[test]
fn plugin_manifest_rejects_payload_kind_core_does_not_accept() {
    let mut manifest = security_fixture_manifest();
    manifest.signal_schemas[0].payload_kind = "json".to_string();

    assert!(
        error_paths(&manifest).contains(&"signal_schemas[0].payload_kind".to_string()),
        "expected payload_kind rejection"
    );
}

#[test]
fn plugin_manifest_rejects_unsupported_capability() {
    let mut manifest = security_fixture_manifest();
    manifest.capabilities.push("exec_shell".to_string());

    let messages: Vec<String> = manifest
        .validate()
        .expect_err("expected manifest to be rejected")
        .into_iter()
        .map(|error| error.message)
        .collect();

    assert!(
        messages
            .iter()
            .any(|message| message.contains("exec_shell")),
        "expected capability rejection, got {messages:?}"
    );
}

#[test]
fn plugin_manifest_rejects_traversing_bundle_path() {
    let mut manifest = security_fixture_manifest();
    manifest.signal_schemas[0].payload_schema = "../../etc/passwd.json".to_string();

    let errors = manifest
        .validate()
        .expect_err("expected manifest to be rejected");

    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("traverse")),
        "expected traversal rejection, got {errors:?}"
    );
}

#[test]
fn plugin_manifest_rejects_non_semver_versions() {
    let mut manifest = security_fixture_manifest();
    manifest.signal_schemas[0].version = "v1".to_string();

    assert!(
        error_paths(&manifest).contains(&"signal_schemas[0].version".to_string()),
        "expected semver rejection"
    );
}

#[test]
fn plugin_manifest_rejects_unsupported_outputs() {
    let mut manifest = security_fixture_manifest();
    manifest.outputs = "serviceradar.anything.v1".to_string();

    assert!(
        error_paths(&manifest).contains(&"outputs".to_string()),
        "expected outputs rejection"
    );
}

#[test]
fn plugin_manifest_rejects_unsupported_runtime() {
    let mut manifest = security_fixture_manifest();
    manifest.runtime = Some("wasi-preview2".to_string());

    assert!(
        error_paths(&manifest).contains(&"runtime".to_string()),
        "expected runtime rejection"
    );
}

#[test]
fn serialize_refuses_invalid_manifest() {
    let mut manifest = security_fixture_manifest();
    manifest.id = String::new();

    assert!(manifest.serialize().is_err());
}

#[test]
fn new_signal_schema_contribution_derives_bundle_paths() {
    let contribution = SignalSchemaContribution::new(
        "com.carverauto.security.scan_activity",
        "1.0.0",
        SIGNAL_SCHEMA_SIGNAL_TYPE_EVENT,
        SIGNAL_SCHEMA_PAYLOAD_KIND_OCSF_EVENT,
    );

    assert_eq!(
        contribution.payload_schema,
        "schemas/scan_activity.schema.json"
    );
    assert_eq!(
        contribution.display_contract,
        "display/scan_activity.display.json"
    );
    assert_eq!(
        contribution.display_contract_id,
        "com.carverauto.security.scan_activity.display"
    );
    assert_eq!(contribution.display_contract_version, "1.0.0");
}

#[test]
fn bundle_path_rule_matches_core_pattern() {
    for valid in [
        "schemas/scan_activity.schema.json",
        "a.json",
        "a/b/c-d_e.json",
    ] {
        let mut manifest = security_fixture_manifest();
        manifest.signal_schemas[0].payload_schema = valid.to_string();

        assert!(
            manifest.validate().is_ok(),
            "{valid} should be accepted as a bundle path"
        );
    }

    for invalid in [
        "/absolute/path.json",
        "schemas/../secret.json",
        "schemas/scan_activity.yaml",
        "schemas//double.json",
        ".hidden.json",
        "",
    ] {
        let mut manifest = security_fixture_manifest();
        manifest.signal_schemas[0].payload_schema = invalid.to_string();

        assert!(
            error_paths(&manifest).contains(&"signal_schemas[0].payload_schema".to_string()),
            "{invalid:?} should be rejected as a bundle path"
        );
    }
}

#[test]
fn signal_ref_rule_matches_core_pattern() {
    for invalid in [
        "Uppercase.id",
        ".leading.dot",
        "-leading-dash",
        "has space",
        "",
    ] {
        let mut manifest = security_fixture_manifest();
        manifest.signal_schemas[0].id = invalid.to_string();

        assert!(
            error_paths(&manifest).contains(&"signal_schemas[0].id".to_string()),
            "{invalid:?} should be rejected as a signal ref"
        );
    }
}

#[test]
fn semver_rule_matches_core_pattern() {
    for valid in ["1.0.0", "1.9.0-dev", "0.0.1", "2.3.4+build.5"] {
        let mut manifest = security_fixture_manifest();
        manifest.signal_schemas[0].version = valid.to_string();
        manifest.signal_schemas[0].display_contract_version = valid.to_string();

        assert!(
            manifest.validate().is_ok(),
            "{valid} should be accepted as semver"
        );
    }

    for invalid in ["1.0", "1.0.0.0", "v1.0.0", "1.0.0-", "1.a.0", ""] {
        let mut manifest = security_fixture_manifest();
        manifest.signal_schemas[0].version = invalid.to_string();

        assert!(
            error_paths(&manifest).contains(&"signal_schemas[0].version".to_string()),
            "{invalid:?} should be rejected as semver"
        );
    }
}
