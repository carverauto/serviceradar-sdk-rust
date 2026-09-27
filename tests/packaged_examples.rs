//! Task 5.6: every example ships import-ready packaging (`plugin.yaml` plus
//! `config.schema.json`) covering the Go SDK examples plus an RTSP example
//! and a northbound-actions example. Content validation (capability names,
//! schema/config cross-checks) lives in CI; here we pin the structure.
use std::path::PathBuf;

const EXAMPLES: &[&str] = &[
    "clock-check",
    "http-check",
    "tcp-check",
    "udp-check",
    "widgets-check",
    "rtsp-check",
    "northbound-actions",
];

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
        let manifest = std::fs::read_to_string(dir.join("plugin.yaml"))
            .unwrap_or_else(|_| panic!("{name}: plugin.yaml missing"));
        assert!(
            manifest.contains("entrypoint: run_check"),
            "{name}: plugin.yaml entrypoint"
        );
        assert!(
            manifest.contains("outputs: serviceradar.plugin_result.v1"),
            "{name}: plugin.yaml outputs"
        );
        let schema = std::fs::read_to_string(dir.join("config.schema.json"))
            .unwrap_or_else(|_| panic!("{name}: config.schema.json missing"));
        assert!(
            schema.contains("\"type\""),
            "{name}: config.schema.json is not a JSON schema"
        );
        assert!(
            schema.contains("\"properties\""),
            "{name}: config.schema.json properties"
        );
    }
}
