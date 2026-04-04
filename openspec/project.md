# Project Context

## Purpose
`serviceradar-sdk-rust` is the Rust implementation of the ServiceRadar plugin SDK. Its job is to let plugin authors write ServiceRadar checkers in Rust without dealing directly with low-level WebAssembly host calls.

The near-term goal is feature parity with the Go SDK in `/Users/mfreeman/src/serviceradar-sdk-go`. The Rust SDK should expose the same core capabilities with idiomatic Rust APIs:
- Host-provided config decoding
- Result construction for `serviceradar.plugin_result.v1`
- Logging to the ServiceRadar host
- Host-proxied HTTP, TCP, UDP, and WebSocket I/O
- Policy input payload decoding for `serviceradar.plugin_inputs.v1`
- Event emission and alert-promotion hints
- Camera and media helper support where the Go SDK already provides it

This repository is currently in bootstrap state. `openspec/project.md` should describe both the current reality and the intended shape of the SDK so future changes stay aligned.

## Tech Stack
- Rust crate managed with Cargo
- WebAssembly target for ServiceRadar plugins, mirroring the Go SDK's WASI/TinyGo deployment model
- JSON-based payloads for config, results, requests, and host responses
- Forgejo Actions CI/CD expected for test, packaging, and WASM build verification
- Go SDK at `/Users/mfreeman/src/serviceradar-sdk-go` is the reference implementation for API coverage and behavior

Planned capability areas, based on the Go SDK layout:
- Core SDK module surface: config, execution, logging, result building, error handling, memory/ABI helpers
- Network helpers: HTTP, TCP, UDP, WebSocket
- Domain helpers: plugin inputs, RTSP/camera support, camera media relay helpers
- Example plugins that compile to `.wasm`

## Project Conventions

### Code Style
- Keep the public API ergonomic for plugin authors, but make it idiomatic Rust rather than a line-for-line Go translation.
- Match the Go SDK's behavior and payload semantics unless there is a clear Rust-specific reason not to.
- Prefer strong typing for public APIs: structs, enums, and builders over loosely typed maps, except where schemas intentionally allow arbitrary JSON objects.
- Keep the unsafe/WASM ABI boundary narrow. Unsafe code, pointer handling, and raw host imports should stay isolated behind safe Rust APIs.
- Organize code by capability, similar to the Go SDK's `sdk/` package: config, execute, result, http, tcp, udp, websocket, plugin_inputs, camera/rtsp/media helpers.
- Use clear module and type names that map to ServiceRadar concepts, for example `Result`, `Status`, `ThresholdSpec`, and typed payload structs.
- Preserve deterministic serialization and defaulting behavior. If defaults are applied during serialization, avoid mutating caller-owned data as a side effect.
- Keep comments brief and focused on non-obvious ABI, payload, or host-runtime details.

### Architecture Patterns
- Treat the SDK as a thin, safe wrapper over the ServiceRadar host ABI. The host runtime owns config delivery, logging, result submission, and outbound network/media operations.
- Separate layers clearly:
  1. Public Rust API used by plugin authors
  2. Encoding/decoding and request/response helpers
  3. Host ABI bindings for the `env` import surface
  4. Non-WASM stubs or mocks for local tests
- Maintain parity with the Go SDK's host ABI surface. The Go SDK currently wraps:
  - `get_config`
  - `log`
  - `submit_result`
  - `http_request`
  - `tcp_connect`, `tcp_read`, `tcp_write`, `tcp_close`
  - `udp_sendto`
  - `websocket_connect`, `websocket_send`, `websocket_recv`, `websocket_close`
  - `camera_media_open`, `camera_media_write`, `camera_media_heartbeat`, `camera_media_close`
- Follow the Go SDK's pattern of providing native-test stubs for non-WASM builds so most logic can be validated without a real host runtime.
- Keep example plugins as first-class compatibility checks. Each major capability should have a minimal example that exercises the intended public API and can be compiled to WASM.

### Testing Strategy
- Prefer fast native unit tests for payload encoding/decoding, defaulting behavior, threshold/status logic, validation, and error handling.
- Add focused tests for host request construction and response parsing, especially for HTTP, WebSocket, plugin inputs, and result serialization.
- Mirror the Go SDK's approach of testing zero-value/default behavior and edge cases around invalid input, missing handles, and host boundary failures.
- Add build verification for example plugins targeting WebAssembly so CI catches ABI or target regressions early.
- Keep tests deterministic. JSON payloads, timestamps, and generated identifiers should be validated in stable ways where possible.

### Git Workflow
- Use `main` as the integration branch.
- Prefer short-lived feature branches and small pull requests.
- Keep commits focused by behavior or capability area.
- The Go SDK history uses conventional prefixes regularly, for example `feat:`, `chore:`, and `fix:`. Follow that style when practical, even if it is not enforced rigidly.
- Avoid mixing unrelated parity work in a single change. If adding a new capability, keep docs, examples, and tests with that capability.

## Domain Context
ServiceRadar plugins are checker-style modules that run inside a host agent/runtime. The plugin guest should not perform direct host integration work itself; it talks to the runtime through a defined WASM ABI.

Important domain concepts:
- Plugins receive host-provided configuration, typically JSON-decoded into a typed config struct.
- Plugins submit a `serviceradar.plugin_result.v1` payload describing status, summary, metrics, labels, timestamps, UI display widgets, optional OCSF events, and alert-promotion hints.
- Policy-driven assignments may deliver `serviceradar.plugin_inputs.v1`, which contains resolved entity inputs such as devices or interfaces. The SDK should provide typed parsing and iteration helpers for this payload.
- Outbound network access is host-mediated. HTTP, TCP, UDP, WebSocket, and camera/media interactions are brokered by the ServiceRadar runtime rather than by direct guest sockets.
- Event support includes OCSF Event Log Activity payloads, optional `alert_hint`, and `condition_id` fields used by the control plane for promotion and de-duplication.
- Example plugins are part of the product experience. The Go SDK already includes HTTP, TCP, UDP, and widget examples; the Rust SDK should follow the same model.

## Important Constraints
- Preserve wire compatibility with the existing ServiceRadar host ABI and payload schemas.
- Treat the Go SDK as the behavior reference unless the Rust project intentionally documents a different contract.
- Keep the guest-side API safe and simple for plugin authors; complexity should stay inside the SDK.
- Host calls are synchronous today in the Go SDK. Context-aware APIs may exist primarily to preserve future compatibility rather than to provide full async cancellation semantics right now.
- Network and media operations are subject to host-enforced permissions, allowlists, and capability checks. The SDK should surface those failures clearly.
- Avoid assuming the Rust crate layout, dependency set, or CI pipeline is final until Cargo metadata and source files exist. Update this document as the repository moves from bootstrap to implementation.

## External Dependencies
- ServiceRadar host runtime and its WASM `env` import surface
- ServiceRadar control-plane schemas:
  - `serviceradar.plugin_result.v1`
  - `serviceradar.plugin_inputs.v1`
- OCSF event model used for emitted events
- Reference implementation in `/Users/mfreeman/src/serviceradar-sdk-go`
- Forgejo Actions for CI/CD, including unit tests, crate packaging checks, and WASM example build checks
