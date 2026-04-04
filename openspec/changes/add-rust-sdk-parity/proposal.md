# Change: Add Rust SDK parity with the Go SDK

## Why
`serviceradar-sdk-rust` is currently only a bootstrap repository with documentation. ServiceRadar needs a real Rust SDK so plugin authors can build checkers in Rust with the same practical capabilities already available in `serviceradar-sdk-go`.

Without this change, Rust plugin development has no supported path for host config access, result submission, logging, host-mediated network I/O, policy input decoding, or camera/media helpers.

## What Changes
- Create the initial Rust SDK crate structure and public API surface for ServiceRadar plugins.
- Implement the ServiceRadar WASM host ABI bindings and safe Rust wrappers for config loading, logging, result submission, and network/media operations.
- Add Rust equivalents for the Go SDK's result builder, thresholds, events, alert hints, plugin input helpers, and execution wrapper.
- Add host-proxied HTTP, TCP, UDP, and WebSocket helpers with behavior aligned to the Go SDK.
- Add camera/media and related domain helpers needed for parity with the current Go SDK surface.
- Add examples, tests, and CI/build verification for the Rust SDK.
- Establish the Go SDK at `/Users/mfreeman/src/serviceradar-sdk-go` as the parity reference for behavior and schema compatibility.

## Impact
- Affected specs: `rust-sdk`
- Affected code: Cargo crate, `src/` modules, examples, tests, GitHub Actions, README/docs
