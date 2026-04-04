## ADDED Requirements

### Requirement: The Rust SDK SHALL expose the core ServiceRadar plugin host interface
The Rust SDK SHALL provide safe, idiomatic APIs for ServiceRadar plugins to load host config, write logs, execute checker logic, and submit results through the ServiceRadar WASM host interface.

#### Scenario: Plugin code loads config and submits a result
- **WHEN** a Rust plugin uses the SDK to decode host-provided configuration and execute a checker function
- **THEN** the SDK decodes the configuration from the host ABI
- **AND** the SDK allows the checker to return a plugin result or an error
- **AND** the SDK submits a valid serialized plugin result payload back to the host

#### Scenario: Plugin execution returns an error
- **WHEN** a checker function returns an error
- **THEN** the SDK converts that failure into a critical plugin result when needed
- **AND** the error details are surfaced in the submitted payload in a way consistent with the Go SDK's behavior

#### Scenario: Native tests run without a real WASM host
- **WHEN** SDK logic is exercised in native unit tests
- **THEN** non-WASM builds provide stubbed or mockable host behavior sufficient to validate SDK logic without the real runtime

### Requirement: The Rust SDK SHALL support `serviceradar.plugin_result.v1`
The Rust SDK SHALL provide typed support for constructing, defaulting, and serializing `serviceradar.plugin_result.v1` payloads, including status, summary, details, metrics, labels, observed timestamp, display widgets, events, alert hints, and condition IDs.

#### Scenario: A plugin builds a result with metrics and widgets
- **WHEN** plugin code constructs a result with summary text, metrics, thresholds, and display widgets
- **THEN** the SDK serializes those fields into the expected plugin result payload schema
- **AND** the payload remains compatible with the ServiceRadar control plane

#### Scenario: A result is serialized from a minimal value
- **WHEN** a plugin result is serialized with missing optional fields and unset defaults
- **THEN** the SDK applies the documented default values required for a valid payload
- **AND** default application does not require mutating caller-owned state

#### Scenario: A plugin emits events and alert hints
- **WHEN** plugin code attaches OCSF-style events, `alert_hint`, and `condition_id` metadata to a result
- **THEN** the SDK serializes those fields into the result payload
- **AND** older ServiceRadar components can safely ignore those fields if unsupported

### Requirement: The Rust SDK SHALL provide host-proxied network helpers
The Rust SDK SHALL expose helpers for host-mediated HTTP, TCP, UDP, and WebSocket operations using the same underlying ServiceRadar capability model as the Go SDK.

#### Scenario: A plugin performs an HTTP request
- **WHEN** plugin code performs an HTTP request through the Rust SDK
- **THEN** the SDK encodes the request for the host ABI
- **AND** decodes the host response into typed Rust response data

#### Scenario: A plugin performs TCP or UDP operations
- **WHEN** plugin code opens TCP connections, reads or writes TCP data, or sends UDP payloads
- **THEN** the SDK routes those operations through the ServiceRadar host ABI
- **AND** host errors are returned as Rust errors with clear operation context

#### Scenario: A plugin uses WebSocket features
- **WHEN** plugin code opens a WebSocket connection and sends or receives messages
- **THEN** the SDK supports the Go SDK's connection lifecycle capabilities
- **AND** handshake payloads support header-bearing requests where the host contract requires them

### Requirement: The Rust SDK SHALL support policy input payload helpers
The Rust SDK SHALL provide typed parsing and validation helpers for `serviceradar.plugin_inputs.v1`, including flattened iteration and filtered access patterns.

#### Scenario: A plugin receives policy-driven inputs
- **WHEN** plugin code loads a `serviceradar.plugin_inputs.v1` document from the host config payload
- **THEN** the SDK can parse and validate the schema and required fields
- **AND** the plugin can iterate resolved items in deterministic payload order

#### Scenario: A plugin filters items by entity or input name
- **WHEN** plugin code requests only items for a given entity or input name
- **THEN** the SDK returns the matching flattened items
- **AND** empty or invalid filters behave predictably

### Requirement: The Rust SDK SHALL provide camera and media helpers matching current Go SDK support
The Rust SDK SHALL expose camera, RTSP, and media-related helpers required to match the capability surface already present in the Go SDK, subject to the same host ABI constraints.

#### Scenario: A plugin uses camera or media helper APIs
- **WHEN** plugin code uses Rust SDK camera or media helper functions supported by the current Go SDK
- **THEN** the Rust SDK provides equivalent host-mediated functionality
- **AND** errors and lifecycle operations are exposed through safe Rust interfaces

### Requirement: The Rust SDK SHALL include examples and build verification
The Rust SDK SHALL include example plugins and automated verification that the SDK remains usable for ServiceRadar plugin development.

#### Scenario: A developer uses an example plugin
- **WHEN** a developer inspects the repository examples
- **THEN** the repository includes example Rust plugins demonstrating core checker flows such as HTTP, TCP, UDP, and rich result display output

#### Scenario: CI verifies SDK usability
- **WHEN** repository CI runs for the Rust SDK
- **THEN** it executes native tests
- **AND** it verifies that example plugins build for the supported Rust WebAssembly target
