## 1. Foundation
- [x] 1.1 Create the Cargo crate structure and initial module layout for the Rust SDK.
- [x] 1.2 Define the host ABI layer for ServiceRadar WASM imports and any required guest memory exports.
- [x] 1.3 Add shared error types, result codes, and target-specific stub behavior for native tests.

## 2. Core Plugin API
- [x] 2.1 Implement config loading helpers for host-provided JSON config.
- [x] 2.2 Implement logging helpers that bridge to the ServiceRadar host runtime.
- [x] 2.3 Implement execution helpers that run user plugin code, normalize failures, and submit plugin results.

## 3. Result Payloads
- [x] 3.1 Implement `serviceradar.plugin_result.v1` types, including status, metrics, labels, display widgets, events, alert hints, and condition IDs.
- [x] 3.2 Implement result builder helpers, threshold helpers, and defaulting behavior aligned with the Go SDK.
- [x] 3.3 Add tests for serialization, status transitions, and zero-value/default behavior.

## 4. Network Helpers
- [x] 4.1 Implement host-proxied HTTP request and response helpers.
- [x] 4.2 Implement host-proxied TCP connect/read/write/close helpers.
- [x] 4.3 Implement host-proxied UDP send helpers.
- [x] 4.4 Implement host-proxied WebSocket connect/send/receive/close helpers, including header-capable handshake payloads where required.
- [x] 4.5 Add request/response and error-path tests for all network helper layers.

## 5. Domain Helpers
- [x] 5.1 Implement `serviceradar.plugin_inputs.v1` parsing, validation, flattening, and filtered iteration helpers.
- [x] 5.2 Implement camera/media and related helper APIs needed to match the current Go SDK capability surface.
- [x] 5.3 Add tests for plugin inputs and camera/media helper behavior.

## 6. Examples and CI
- [x] 6.1 Add example Rust plugins covering at least HTTP, TCP, UDP, and rich result/widget usage.
- [x] 6.2 Add documentation showing how to build Rust plugins for the supported WASM target.
- [x] 6.3 Add CI checks for unit tests and example WASM builds.
