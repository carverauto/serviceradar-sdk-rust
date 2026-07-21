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

## Source-native local development

Native builds expose `serviceradar_sdk_rust::local`, which installs a local
development host for ordinary config, HTTP, logging, telemetry, and result SDK
calls. This path does not require a Wasm build, signature, registry, or cluster.

`LocalInputs` reads public config and optional action-invocation JSON from files
or environment variables. It also reads an optional `.env` file; process
environment values override file values. Credential fields use the
`SERVICERADAR_CREDENTIAL_` prefix and remain separate from runtime config:

```dotenv
SERVICERADAR_PLUGIN_CONFIG_FILE=testdata/config.json
SERVICERADAR_PLUGIN_ACTION_FILE=testdata/action.json
SERVICERADAR_CREDENTIAL_USERNAME=local-user
SERVICERADAR_CREDENTIAL_PASSWORD=local-password
```

```rust
use serviceradar_sdk_rust::local::{
    LocalHostOptions, LocalInputOptions, LocalInputs, run_local_host,
};

let inputs = LocalInputs::load(LocalInputOptions::default())?;
let runtime_config = inputs.runtime_config_json()?;
let credentials = inputs.credentials();

let (capture, result) = run_local_host(
    LocalHostOptions {
        config_json: runtime_config,
        http_handler: Some(new_local_broker(credentials)),
    },
    run_plugin,
);
result?;
```

The caller-supplied HTTP handler is the trusted local host adapter and should
enforce production-equivalent endpoint grants, credential injection, redirects,
TLS, and response bounds. Credentials must not enter plugin config, action
input, logs, or results. Local success confers no production authorization.

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
