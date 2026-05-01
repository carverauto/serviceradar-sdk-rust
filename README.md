# serviceradar-sdk-rust

ServiceRadar plugin SDK for Rust.

## Device Discovery

Plugins can emit `serviceradar.device_discovery.v1` envelopes inside the normal
plugin-result payload. Core ingests these records through the device discovery
handler and reconciles them into `ocsf_devices`.
