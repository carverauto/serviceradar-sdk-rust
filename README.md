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
- Advisory-feed contract builders and gateway-mediated artifact staging helpers
- First-class metric telemetry helpers for canonical `serviceradar.metric.v1` payloads
- Example plugins for HTTP, TCP, UDP, and widget-rich results

The Go SDK in `/Users/mfreeman/src/serviceradar-sdk-go` remains the behavior reference for parity, but this crate aims for an idiomatic Rust interface rather than a line-for-line Go port.

In practice that means the common path uses concrete Rust domain types like `PluginResult`, `Widget`, `Event`, and `HttpClient`, while Go-specific convenience aliases are intentionally avoided on the public surface.

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

Do not put time-series metrics in `serviceradar.plugin_result.v1`. Result
metrics are no longer serialized by the SDK and are rejected by current
ServiceRadar agents. Emit canonical metric protobuf batches with
`TelemetryRecord::serviceradar_metric_batch` instead:

```rust
let record = sdk::TelemetryRecord::serviceradar_metric_batch(
    "metric-event-1",
    sdk::MetricBatch {
        resource: sdk::MetricResource {
            service_name: "http-check".to_string(),
            service_type: "wasm-plugin".to_string(),
            ..Default::default()
        },
        ingest_identity: sdk::MetricIngestIdentity {
            source: "plugin-metrics".to_string(),
            producer_id: "http-check".to_string(),
            producer_kind: "wasm-plugin".to_string(),
            ..Default::default()
        },
        metrics: vec![sdk::Metric {
            name: "http.response_time_ms".to_string(),
            metric_type: "plugin".to_string(),
            kind: sdk::MetricKind::Gauge,
            unit: "ms".to_string(),
            points: vec![sdk::MetricPoint {
                value: 12.5,
                raw_value: "12.5".to_string(),
                raw_value_type: sdk::MetricValueType::Double,
                observed_at_unix_nano: 123,
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    },
);

sdk::emit_telemetry(
    sdk::TelemetryBatch::new(vec![record])
        .with_source(sdk::TelemetrySource::new("http-check", "default")),
)?;
```

`TelemetryRecord::serviceradar_metric_batch` uses a dependency-free protobuf
encoder so wasm plugins do not need to link a full protobuf runtime. If a plugin
already has encoded protobuf bytes from another generator, use
`TelemetryRecord::serviceradar_metrics`.

## Advisory Feed Producers

Plugins that produce vulnerability intelligence should emit normalized advisory
batches through the standard plugin result payload. Provider-specific download,
schema validation, pointer JSON handling, archive extraction, and feed-specific
normalization stay inside the plugin. ServiceRadar core consumes only the
generic `serviceradar.advisory_feed.contract.v1` contract.

For large feed snapshots, stage the raw or normalized artifact through the
gateway-mediated artifact API before submitting the advisory batch:

```rust
let mut stream = sdk::ArtifactStream::open(
    sdk::ArtifactOpenRequest::new("vulnerability-feeds/example/sha256.json")
        .with_content_type("application/json"),
)?;
stream.write(feed_json.as_bytes())?;
let artifact = stream.commit(
    sdk::ArtifactCommitRequest::new().with_sha256(feed_sha256),
)?;

let batch = sdk::AdvisoryFeedBatch::new(
    "com.example.feed",
    sdk::AdvisorySource::new("example", "normalized"),
    sdk::AdvisorySnapshot::new(artifact.object_key, artifact.sha256),
)
.with_advisory(
    sdk::AdvisoryRecord::new("CVE-2026-1")
        .with_cve("CVE-2026-1")
        .with_severity("high")
        .with_coordinate(
            sdk::AffectedCoordinate::purl("pkg:deb/debian/openssl@3.0.13?arch=amd64"),
        ),
);

let result = sdk::PluginResult::ok("submitted advisory feed")
    .with_advisory_feed(batch);
# Ok::<_, sdk::Error>(())
```

Plugins need the `advisory-feed:v1` capability to submit advisory batches and
`artifact-staging:v1` when using the artifact stream helpers. These APIs are
host and agent-gateway mediated; plugins never receive direct object-store
credentials.

Scheduled feed downloads are declared in the plugin package manifest with
`producer_schedules`. The platform persists the declaration, renders operator
settings, and dispatches runs through `plugin.run_action`; the plugin keeps all
provider-specific fetch, checksum, archive, and normalization logic.

```rust
let mut payload_template = std::collections::BTreeMap::new();
payload_template.insert("feed_key".to_string(), serde_json::json!("primary"));

let schedule = sdk::ProducerScheduleContract::new(
    "daily_advisory_refresh",
    "Refresh advisory feed",
    "advisory.refresh",
)
.with_cadence(86_400, 3_600, 2_592_000)
.with_jitter_seconds(120)
.with_payload_template(payload_template);
```

Plugins that declare schedules should include `producer-schedule:v1` in their
manifest capabilities. The scheduled invocation payload uses
`serviceradar.producer_schedule_run.v1`.

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

let location = sdk::DeviceLocation::at(29.9844, -95.3414)
    .with_site_code("IAH")
    .with_site_name("Houston");

let device = sdk::DiscoveredDevice::named("NIAHAP-MDF001-WAP001")
    .with_serial("CNC3HN77NW")
    .with_device_type("access_point")
    .with_location(location)
    .with_label("site", "IAH")
    .with_metadata("radio_count", 2);

let result = sdk::PluginResult::ok("discovered 1 device").with_device_discovery(
    sdk::DeviceDiscovery::new("ual-network-map").with_device(device),
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
