## 1. Local host contract
- [x] 1.1 Expose a guarded native host backend installation path without changing the Wasm ABI.
- [x] 1.2 Add production-shaped config/action loading and `.env` plus process-environment credential resolution.
- [x] 1.3 Add host HTTP callbacks and captured result, telemetry, and log outputs.

## 2. Parity and verification
- [x] 2.1 Match the Go SDK local-host names, environment variables, precedence, and observable behavior.
- [x] 2.2 Add focused native tests for input merging, secret separation, host calls, restoration, and errors.
- [x] 2.3 Document source-native usage and the boundary between local testing and production admission.
- [x] 2.4 Run formatting, tests, clippy, package checks, and supported Wasm example builds.
