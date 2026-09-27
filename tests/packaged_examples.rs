//! Task 5.6: every example ships import-ready packaging (`plugin.yaml` plus
//! `config.schema.json`) covering the Go SDK examples plus an RTSP example
//! and a northbound-actions example. Manifests are decoded into
//! `PluginManifest`; config schemas are decoded as JSON objects.
use std::path::PathBuf;

use serviceradar_sdk_rust::{OUTPUTS_PLUGIN_RESULT, PluginManifest, RUNTIME_WASI_PREVIEW1};

const EXAMPLES: &[&str] = &[
    "clock-check",
    "http-check",
    "tcp-check",
    "udp-check",
    "widgets-check",
    "rtsp-check",
    "northbound-actions",
];

fn parse_manifest(name: &str, text: &str) -> PluginManifest {
    let yaml: serde_yaml::Value =
        serde_yaml::from_str(text).unwrap_or_else(|err| panic!("{name}: plugin.yaml: {err}"));
    let json = serde_json::to_value(yaml)
        .unwrap_or_else(|err| panic!("{name}: plugin.yaml as json: {err}"));
    serde_json::from_value(json)
        .unwrap_or_else(|err| panic!("{name}: plugin.yaml is not a plugin manifest: {err}"))
}

fn example_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(name)
}

#[test]
fn every_example_ships_packaging() {
    for name in EXAMPLES {
        let dir = example_dir(name);
        assert!(
            dir.join("src").join("main.rs").is_file(),
            "{name}: src/main.rs missing"
        );
        let manifest_text = std::fs::read_to_string(dir.join("plugin.yaml"))
            .unwrap_or_else(|_| panic!("{name}: plugin.yaml missing"));
        let manifest = parse_manifest(name, &manifest_text);
        assert_eq!(manifest.entrypoint, "run_check", "{name}: entrypoint");
        assert_eq!(manifest.outputs, OUTPUTS_PLUGIN_RESULT, "{name}: outputs");
        assert_eq!(
            manifest.runtime.as_deref(),
            Some(RUNTIME_WASI_PREVIEW1),
            "{name}: runtime"
        );
        manifest
            .validate()
            .unwrap_or_else(|errors| panic!("{name}: manifest rejected: {errors:?}"));

        let schema_text = std::fs::read_to_string(dir.join("config.schema.json"))
            .unwrap_or_else(|_| panic!("{name}: config.schema.json missing"));
        let schema: serde_json::Value = serde_json::from_str(&schema_text)
            .unwrap_or_else(|err| panic!("{name}: config.schema.json: {err}"));
        assert_eq!(schema["type"], "object", "{name}: schema type");
        assert!(
            schema["properties"].is_object(),
            "{name}: schema properties"
        );
    }
}
