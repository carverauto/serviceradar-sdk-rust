# Change: Add a source-native local plugin development host

## Why
Plugin authors currently need a compiled Wasm artifact and a runtime harness, or provider-specific test fakes, to exercise host configuration and I/O. That makes real integration testing unnecessarily dependent on build, signing, publication, and deployment steps.

## What Changes
- Add a non-Wasm local host that supplies production-shaped configuration and optional action invocation payloads to ordinary SDK APIs.
- Load local credential fields from an optional `.env` file and process environment without merging them into guest configuration.
- Capture submitted results, telemetry, and logs and route HTTP through a caller-supplied host handler.
- Keep the public behavior equivalent to the Go SDK while using idiomatic Rust types.

## Impact
- Affected specs: `rust-sdk`
- Affected code: native host backend, local development module, tests, and README
