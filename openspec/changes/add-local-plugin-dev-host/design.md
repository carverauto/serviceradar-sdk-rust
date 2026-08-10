## Context
The crate already has a non-Wasm host backend, but it is private and compiled only for crate tests. Plugin binaries cannot install a development host, so normal `get_config`, HTTP, logging, telemetry, and result submission APIs cannot be exercised together from source.

## Goals / Non-Goals
- Goals:
  - Run ordinary plugin code natively with the same SDK entry points used by Wasm.
  - Keep configuration, action invocation, and credential material as separate inputs.
  - Provide behavior equivalent to the Go SDK local host.
  - Capture bounded outputs for assertions and local inspection.
- Non-Goals:
  - Emulate signature verification, package approval, assignment admission, or every production agent policy.
  - Add a native HTTP/TLS dependency to the SDK; callers supply the local host HTTP handler.
  - Make credentials available to Wasm guest code.

## Decisions
The SDK exposes a native-only `local` module. `LocalInputs` reads a public config JSON object, an optional action invocation JSON object, and explicitly prefixed credential fields. Process environment values override `.env` values. `runtime_config_json` inserts the action document under `action_invocation` and never inserts credentials.

`run_local_host` installs one process-scoped host backend for the duration of a closure. A guard serializes runs and restores the previous backend even after an unwind. The backend serves config, routes HTTP through a caller callback, and captures copied result, telemetry, and log bytes. Unsupported host operations retain existing not-found behavior.

The local host is a developer tool, not a security boundary. Provider test harnesses remain responsible for implementing the same exact endpoint grants, credential injection, token exchange, and redaction behavior that their production host grant uses.

## Verification
- Native tests cover `.env` parsing, process overrides, config/action merging, credential exclusion, result/log/telemetry capture, HTTP round trips, restoration, and concurrent-run serialization.
- Existing crate tests and Wasm example builds remain green.
