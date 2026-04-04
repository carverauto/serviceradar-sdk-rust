## Context
The Rust repository does not yet contain a functional SDK implementation. The Go SDK already defines the expected capability surface, payload behavior, example layout, and CI checks. This change establishes the Rust SDK as a safe, idiomatic wrapper over the same ServiceRadar host contracts.

Because this is a cross-cutting, foundational change, design constraints need to be documented before implementation starts.

## Goals / Non-Goals
- Goals:
  - Provide a stable Rust API for writing ServiceRadar plugins.
  - Match the Go SDK's host ABI coverage and payload behavior.
  - Keep unsafe/WASM details isolated behind safe Rust interfaces.
  - Support native unit tests with non-WASM stubs where possible.
  - Ship example plugins and CI checks that verify both unit behavior and WASM build viability.
- Non-Goals:
  - Introduce new ServiceRadar payload schemas or host functions.
  - Redesign the SDK around Rust-only concepts that would break behavioral parity.
  - Add functionality that does not already exist in the Go SDK unless separately proposed.

## Proposed Architecture
The crate should separate concerns into four layers:

1. Public API layer
- Idiomatic Rust types and helper functions used by plugin authors.
- Examples: config loading helpers, `Result` builder APIs, thresholds, status enums, event helpers, network client wrappers.

2. Payload/model layer
- `serde`-backed structs/enums for plugin result payloads, plugin input payloads, network request/response payloads, and camera/media metadata.
- Defaulting and validation behavior should follow the Go SDK's observable behavior.

3. Host ABI layer
- WASM imports for the ServiceRadar `env` module.
- Narrow unsafe boundary for pointer passing, memory ownership, and host call return-code translation.
- Export guest memory helpers required by the runtime if the host contract expects them.

4. Test and non-WASM support layer
- Stub or mock host implementations for native testing.
- Focus tests on payload behavior, request encoding, response decoding, and error mapping without requiring a real runtime.

## Module Shape
The initial Rust crate should be organized by capability rather than by transport or schema internals only. A likely shape is:

- `src/lib.rs`
- `src/config.rs`
- `src/execute.rs`
- `src/result.rs`
- `src/log.rs`
- `src/error.rs`
- `src/host/` for ABI imports, pointer helpers, and target-specific implementations
- `src/http.rs`
- `src/tcp.rs`
- `src/udp.rs`
- `src/websocket.rs`
- `src/plugin_inputs.rs`
- `src/camera_*.rs` and related media/RTSP helpers as needed for parity

Exact file names may change during implementation, but the public API should remain capability-oriented and discoverable.

## Parity Strategy
Parity should be evaluated against the current Go SDK behavior, not only by matching names:

- The same host ABI operations must be supported.
- Result payload fields and defaults must serialize compatibly.
- Error paths should be translated into clear Rust errors while preserving host failure semantics.
- Network helper APIs should expose the same underlying capability set, even if Rust naming differs.
- Policy input helpers should support parse, validate, flatten, and filtered iteration behavior.
- Camera/media features present in the Go SDK should be represented in the Rust SDK unless the host contract makes a direct port impossible.

## Verification Strategy
- Native unit tests for serialization, defaulting, thresholds, validation, and request/response encoding.
- Native tests for error conditions such as missing handles, invalid payloads, and host return-code failures.
- Example plugin builds for the supported Rust WASM target used by ServiceRadar.
- CI should fail if the crate tests fail or if example plugins stop building.
