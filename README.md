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

Build native examples:

```bash
cargo build --examples
```

Build WebAssembly examples:

```bash
rustup target add wasm32-unknown-unknown
cargo build --examples --target wasm32-unknown-unknown
```

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
3. Create and push a matching tag such as `v0.1.0`.

The publish workflow verifies that the tag matches the crate version and then runs `cargo publish`. Configure the Forgejo repository secret `crates` with a crates.io API token before using the release workflow.
