## ADDED Requirements

### Requirement: The Rust SDK SHALL provide a source-native local plugin host
The Rust SDK SHALL provide a non-Wasm local host that supplies production-shaped plugin configuration and optional action invocation data through normal SDK APIs, routes host HTTP through a caller-supplied handler, and captures submitted results, telemetry, and logs without requiring a Wasm build or deployment.

#### Scenario: Plugin runs natively through ordinary SDK APIs
- **GIVEN** a public plugin configuration and optional action invocation
- **WHEN** plugin code runs through the local host
- **THEN** ordinary config, HTTP, logging, telemetry, and result APIs SHALL use the installed local host
- **AND** the previous native host SHALL be restored after the run

#### Scenario: Local credentials stay outside plugin inputs
- **GIVEN** credential fields are supplied by `.env` and process environment
- **WHEN** local inputs are assembled
- **THEN** process environment values SHALL override file values
- **AND** credentials SHALL remain available only to the local host adapter
- **AND** credentials SHALL NOT appear in the merged plugin configuration or action invocation

#### Scenario: Local success does not authorize production use
- **WHEN** a native local run succeeds
- **THEN** it SHALL confer no package signature, approval, assignment, or production execution state
