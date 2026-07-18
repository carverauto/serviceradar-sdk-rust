# serviceradar-sdk-rust

ServiceRadar plugin SDK for Rust and WebAssembly.

## Overview

This crate lets you write ServiceRadar plugin checkers in Rust without dealing directly with low-level host ABI calls. It currently includes:

- Host-provided config loading
- Result construction and serialization for `serviceradar.plugin_result.v1`
- Host logging
- Host-proxied HTTP, TCP, UDP, and WebSocket helpers
- Policy input parsing and validation for `serviceradar.plugin_inputs.v1`
- Camera/media helpers and RTSP parsing/depacketization utilities
- Signal schema/display contract references for package-managed logs and events
- Device discovery/enrichment payload helpers for inventory-producing plugins
- Example plugins for HTTP, TCP, UDP, and widget-rich results

The Go SDK in `/Users/mfreeman/src/serviceradar-sdk-go` remains the behavior reference for parity, but this crate aims for an idiomatic Rust interface rather than a line-for-line Go port.

In practice that means the common path uses concrete Rust domain types like `PluginResult`, `Metric`, `Widget`, `Event`, and `HttpClient`, while Go-specific convenience aliases are intentionally avoided on the public surface.

## Install

```bash
cargo add serviceradar-sdk-rust
```

## Example

```rust
use serviceradar_sdk_rust as sdk;

#[derive(Debug, serde::Deserialize)]
#[serde(default)]
struct Config {
    url: String,
    warn_ms: f64,
    crit_ms: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            url: "https://example.com/health".to_string(),
            warn_ms: 0.0,
            crit_ms: 0.0,
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn run_check() {
    let _ = sdk::execute(|| {
        let cfg = sdk::load_config_or_default::<Config>()?;

        let response = sdk::HttpClient::default().get(&cfg.url)?;
        let latency_ms = response.duration.as_millis() as f64;
        let thresholds = sdk::Thresholds::new(cfg.warn_ms, cfg.crit_ms);

        Ok(sdk::PluginResult::new()
            .with_summary(format!("http {} in {:.0}ms", response.status, latency_ms))
            .with_thresholds(latency_ms, thresholds.warn, thresholds.crit)
            .with_metric_spec(
                sdk::Metric::new("latency_ms", latency_ms)
                    .with_unit("ms")
                    .with_thresholds(&thresholds),
            )
            .with_widget(sdk::Widget::stat_card(
                "Latency",
                format!("{latency_ms:.0}ms"),
                "success",
            )))
    });
}
```

## Examples

- `http-check`
- `tcp-check`
- `udp-check`
- `widgets-check`

## Signal display contracts

When a plugin emits OCSF events or OTEL-style logs that are described by a package manifest, attach the package schema/display reference through the SDK:

```rust
let event = sdk::Event::log_activity("camera motion", sdk::Severity::Warning)
    .with_signal_schema_ref(&sdk::SignalSchemaRef {
        producer_id: "axis-camera".to_string(),
        producer_version: "0.1.0".to_string(),
        schema_id: "com.carverauto.axis_camera.event_log".to_string(),
        schema_version: "1.0.0".to_string(),
        display_contract_id: "com.carverauto.axis_camera.event_log.display".to_string(),
        display_contract_version: "1.0.0".to_string(),
        display_contract: "display/event_log_activity.display.json".to_string(),
        signal_type: sdk::SIGNAL_SCHEMA_SIGNAL_TYPE_EVENT.to_string(),
        payload_kind: sdk::SIGNAL_SCHEMA_PAYLOAD_KIND_OCSF_EVENT.to_string(),
    });
```

The helper writes the ServiceRadar extension metadata under `metadata.service_radar.signal_schema`.

For first-class telemetry that should be routed independently from the plugin result payload, emit a telemetry batch through the host:

```rust
let record = sdk::TelemetryRecord::ocsf_event(event)?
    .with_signal_schema_ref(&schema_ref);

sdk::emit_telemetry(
    sdk::TelemetryBatch::new(vec![record])
        .with_source(sdk::TelemetrySource::new("axis-camera", "front-door")),
)?;
```

`emit_telemetry` serializes the same JSON host ABI payload as the Go SDK and requires the plugin manifest capability `emit_telemetry`.

## Advisory feed batches

Vulnerability and threat-intelligence feed plugins should normalize provider
data inside the plugin and submit `serviceradar.advisory_feed.contract.v1`
batches through the normal plugin result path. Core stores and matches the
generic contract; it does not parse provider-native CISA, NVD, VulnCheck, or
similar feed formats.

```rust
let mut batch = sdk::AdvisoryFeedBatch::new(
    "com.example.vuln-feed",
    sdk::AdvisorySource::new("example", "normalized")
        .with_display_name("Example Advisory Feed"),
    sdk::AdvisorySnapshot::accepted(
        "vulnerability-feeds/example/latest.json",
        "<sha256>",
    ),
)
.with_advisory(
    sdk::AdvisoryRecord::new("example:CVE-2026-1", "CVE-2026-1")
        .with_cve_id("CVE-2026-1")
        .with_severity("high")
        .with_affected_coordinate(
            sdk::AffectedCoordinate::purl("pkg:generic/example/pkg@1.0.0")
                .with_match_semantics("plugin_normalized"),
        ),
);

let result = sdk::PluginResult::ok("accepted advisory batch")
    .with_advisory_feed(batch);
```

Declare `submit_result` and `advisory-feed:v1` in the plugin manifest.

Large archive or pointer feeds do not require native add-ons solely for object
storage. Plugins that need durable snapshots should also declare
`artifact-staging:v1` and use the host-brokered artifact stream. The host sends
object traffic through agent-gateway; plugins and add-ons never talk directly to
JetStream or the object store.

```rust
let mut stream = sdk::ArtifactStream::open(
    sdk::ArtifactOpenRequest::new("vulnerability-feeds/example/latest.zip")
        .with_type("advisory-feed-snapshot")
        .with_content_type("application/zip")
        .with_sha256("<sha256>"),
)?;

stream.write(sdk::ArtifactChunkMetadata::new(1), chunk)?;

let artifact = stream.commit(
    sdk::ArtifactCommitRequest::new()
        .with_sha256("<sha256>")
        .with_size_bytes(total_bytes),
)?;

batch.snapshot.object_key = artifact.object_key;
batch.snapshot.sha256 = artifact.sha256.unwrap_or_default();
batch.snapshot.size_bytes = artifact.size_bytes;
```

## Manifest EventWriter contributions

Packages can declare how emitted telemetry should be routed and normalized by core
without shipping executable EventWriter code. Use the manifest helpers to keep the
contract shape and processor IDs aligned with core validation:

```rust
let mut schema = sdk::SignalSchemaContribution::new(
    "com.carverauto.security.scan_activity",
    "1.0.0",
    sdk::SIGNAL_SCHEMA_SIGNAL_TYPE_EVENT,
    sdk::SIGNAL_SCHEMA_PAYLOAD_KIND_JSON,
);
schema.ocsf_schema_version = Some("1.9.0-dev".to_string());
schema.class_uid = Some(6007);
schema.type_uid = Some(600701);

let processor = sdk::EventWriterContribution::new(
    "security_scan_activity",
    "plugins.security_sample.scan_activity",
    sdk::PROCESSOR_SCAN_ACTIVITY,
)
.with_stream_name("events")
.with_destination("table", "ocsf_events")
.with_ocsf("schema_version", "1.9.0-dev")
.with_ocsf("class_uid", 6007)
.with_batch(25, 250);

let mut resources = std::collections::BTreeMap::new();
resources.insert("requested_memory_mb".to_string(), serde_json::json!(32));

let manifest = sdk::PluginManifest {
    id: "security-sample".to_string(),
    name: "Security Sample".to_string(),
    version: "1.0.0".to_string(),
    entrypoint: "run_check".to_string(),
    runtime: Some("wasi-preview1".to_string()),
    capabilities: vec![
        "get_config".to_string(),
        "log".to_string(),
        "submit_result".to_string(),
        "emit_telemetry".to_string(),
    ],
    resources,
    outputs: "serviceradar.plugin_result.v1".to_string(),
    signal_schemas: vec![schema.with_event_writer(processor)],
    ..Default::default()
};

let payload = manifest.serialize()?;
```

The processor ID must be one of the platform-owned processors, such as
`sdk::PROCESSOR_OCSF_PASSTHROUGH`, `sdk::PROCESSOR_OTEL_LOG_PASSTHROUGH`,
`sdk::PROCESSOR_JSON_TO_OCSF`, `sdk::PROCESSOR_SECURITY_FINDING`, or
`sdk::PROCESSOR_SCAN_ACTIVITY`.

Build native examples:

```bash
cargo build --examples
```

Build WebAssembly examples:

```bash
rustup target add wasm32-unknown-unknown
cargo build --examples --target wasm32-unknown-unknown
```

## Device Discovery

Plugins can emit `serviceradar.device_discovery.v1` envelopes inside the normal
plugin-result payload. Core ingests these records through the device discovery
handler and reconciles them into `ocsf_devices`.

```rust
use serviceradar_sdk_rust as sdk;

let location = sdk::DeviceLocation::at(0.0, 0.0)
    .with_site_code("SITE01")
    .with_site_name("Example City");

let device = sdk::DiscoveredDevice::named("SITE01-MDF001-WAP001")
    .with_serial("SN0000000001")
    .with_device_type("access_point")
    .with_location(location)
    .with_label("site", "SITE01")
    .with_metadata("radio_count", 2);

let result = sdk::PluginResult::ok("discovered 1 device").with_device_discovery(
    sdk::DeviceDiscovery::new("example-network-map").with_device(device),
);

let payload = result.serialize()?;
# Ok::<_, sdk::Error>(())
```

The discovery structs are public and serde-native, so collectors can also build
them with struct literals or mutate them incrementally with `push_device`,
`add_device_discovery`, and `Extend` while processing streams of discovered
assets.

## Verification

Run the unit tests:

```bash
cargo test
```

The repository CI runs `fmt`, `clippy`, tests, native example builds, wasm example builds, and `cargo publish --dry-run`.

## Release

Crate publishing is automated in Forgejo Actions. To publish a release:

1. Update `version` in `Cargo.toml`.
2. Push the commit to `main`.
3. Create and push a matching tag such as `v0.1.4`.

The publish workflow verifies that the tag matches the crate version and then runs `cargo publish`. Configure the Forgejo repository secret `crates` with a crates.io API token before using the release workflow.
