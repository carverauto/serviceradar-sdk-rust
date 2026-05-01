# serviceradar-sdk-rust

ServiceRadar plugin SDK for Rust.

## Device Discovery

Plugins can emit `serviceradar.device_discovery.v1` envelopes inside the normal
plugin-result payload. Core ingests these records through the device discovery
handler and reconciles them into `ocsf_devices`.

```rust
use serviceradar_sdk_rust::{
    DeviceDiscovery, DeviceLocation, DiscoveredDevice, PluginResult,
};

let location = DeviceLocation::at(29.9844, -95.3414)
    .with_site_code("IAH")
    .with_site_name("Houston");

let device = DiscoveredDevice::named("NIAHAP-MDF001-WAP001")
    .with_serial("CNC3HN77NW")
    .with_device_type("access_point")
    .with_location(location)
    .with_label("site", "IAH")
    .with_metadata("radio_count", 2);

let result = PluginResult::ok("discovered 1 device").with_device_discovery(
    DeviceDiscovery::new("ual-network-map").with_device(device),
);

let payload = serde_json::to_vec(&result)?;
# Ok::<_, serde_json::Error>(())
```

The structs are public and serde-native, so collectors can also build them with
struct literals or mutate them incrementally with `push_device`,
`push_device_discovery`, and `Extend` when processing streams of discovered
assets.
