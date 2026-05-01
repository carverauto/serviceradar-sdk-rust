//! Rust helpers for emitting ServiceRadar plugin results.
//!
//! The types in this crate model the JSON contracts that ServiceRadar core
//! ingests from plugins. They intentionally stay serde-friendly so plugin
//! authors can use normal Rust struct literals, builders, or mutation based on
//! whichever style fits their collector.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const DEVICE_DISCOVERY_SCHEMA_V1: &str = "serviceradar.device_discovery.v1";

fn device_discovery_schema_v1() -> String {
    DEVICE_DISCOVERY_SCHEMA_V1.to_owned()
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct PluginResult {
    pub status: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub device_discovery: Vec<DeviceDiscovery>,
}

impl PluginResult {
    pub fn new(status: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            status: status.into(),
            summary: summary.into(),
            device_discovery: Vec::new(),
        }
    }

    pub fn ok(summary: impl Into<String>) -> Self {
        Self::new("OK", summary)
    }

    pub fn push_device_discovery(&mut self, discovery: DeviceDiscovery) {
        self.device_discovery.push(discovery);
    }

    pub fn with_device_discovery(mut self, discovery: DeviceDiscovery) -> Self {
        self.push_device_discovery(discovery);
        self
    }
}

impl Extend<DeviceDiscovery> for PluginResult {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = DeviceDiscovery>,
    {
        self.device_discovery.extend(iter);
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct DeviceDiscovery {
    #[serde(default = "device_discovery_schema_v1")]
    pub schema: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<DiscoveredDevice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_hash: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
}

impl Default for DeviceDiscovery {
    fn default() -> Self {
        Self {
            schema: device_discovery_schema_v1(),
            collection_id: None,
            source: None,
            observed_at: None,
            devices: Vec::new(),
            reference_hash: None,
            metadata: BTreeMap::new(),
        }
    }
}

impl DeviceDiscovery {
    pub fn new(source: impl Into<String>) -> Self {
        Self::default().with_source(source)
    }

    pub fn with_collection_id(mut self, collection_id: impl Into<String>) -> Self {
        self.collection_id = Some(collection_id.into());
        self
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn with_observed_at(mut self, observed_at: impl Into<String>) -> Self {
        self.observed_at = Some(observed_at.into());
        self
    }

    pub fn with_reference_hash(mut self, reference_hash: impl Into<String>) -> Self {
        self.reference_hash = Some(reference_hash.into());
        self
    }

    pub fn insert_metadata(&mut self, key: impl Into<String>, value: impl Into<Value>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.insert_metadata(key, value);
        self
    }

    pub fn push_device(&mut self, device: DiscoveredDevice) {
        self.devices.push(device);
    }

    pub fn with_device(mut self, device: DiscoveredDevice) -> Self {
        self.push_device(device);
        self
    }

    pub fn with_devices<T>(mut self, devices: T) -> Self
    where
        T: IntoIterator<Item = DiscoveredDevice>,
    {
        self.devices.extend(devices);
        self
    }
}

impl Extend<DiscoveredDevice> for DeviceDiscovery {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = DiscoveredDevice>,
    {
        self.devices.extend(iter);
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct DiscoveredDevice {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub device_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_available: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<DeviceLocation>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
}

impl DiscoveredDevice {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn named(hostname: impl Into<String>) -> Self {
        Self::new().with_hostname(hostname)
    }

    pub fn with_device_id(mut self, device_id: impl Into<String>) -> Self {
        self.device_id = Some(device_id.into());
        self
    }

    pub fn with_hostname(mut self, hostname: impl Into<String>) -> Self {
        self.hostname = Some(hostname.into());
        self
    }

    pub fn with_ip(mut self, ip: impl Into<String>) -> Self {
        self.ip = Some(ip.into());
        self
    }

    pub fn with_mac(mut self, mac: impl Into<String>) -> Self {
        self.mac = Some(mac.into());
        self
    }

    pub fn with_serial(mut self, serial: impl Into<String>) -> Self {
        self.serial = Some(serial.into());
        self
    }

    pub fn with_vendor_name(mut self, vendor_name: impl Into<String>) -> Self {
        self.vendor_name = Some(vendor_name.into());
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn with_device_type(mut self, device_type: impl Into<String>) -> Self {
        self.device_type = Some(device_type.into());
        self
    }

    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.role = Some(role.into());
        self
    }

    pub fn with_status(mut self, status: impl Into<String>) -> Self {
        self.status = Some(status.into());
        self
    }

    pub fn with_availability(mut self, is_available: bool) -> Self {
        self.is_available = Some(is_available);
        self
    }

    pub fn with_location(mut self, location: DeviceLocation) -> Self {
        self.location = Some(location);
        self
    }

    pub fn insert_label(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.labels.insert(key.into(), value.into());
    }

    pub fn with_label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.insert_label(key, value);
        self
    }

    pub fn insert_metadata(&mut self, key: impl Into<String>, value: impl Into<Value>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.insert_metadata(key, value);
        self
    }
}

impl DeviceLocation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn at(latitude: f64, longitude: f64) -> Self {
        Self::new().with_coordinates(latitude, longitude)
    }

    pub fn with_site_code(mut self, site_code: impl Into<String>) -> Self {
        self.site_code = Some(site_code.into());
        self
    }

    pub fn with_site_name(mut self, site_name: impl Into<String>) -> Self {
        self.site_name = Some(site_name.into());
        self
    }

    pub fn with_coordinates(mut self, latitude: f64, longitude: f64) -> Self {
        self.latitude = Some(latitude);
        self.longitude = Some(longitude);
        self
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct DeviceLocation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latitude: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub longitude: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_device_discovery() {
        let result = PluginResult::ok("discovered 1 device").with_device_discovery(
            DeviceDiscovery::new("ual-network-map").with_device(DiscoveredDevice {
                hostname: Some("NIAHAP-MDF001-WAP001".to_string()),
                serial: Some("CNC3HN77NW".to_string()),
                device_type: Some("access_point".to_string()),
                ..DiscoveredDevice::default()
            }),
        );

        let encoded = serde_json::to_value(result).expect("serialize");
        assert_eq!(
            encoded["device_discovery"][0]["schema"],
            DEVICE_DISCOVERY_SCHEMA_V1
        );
        assert_eq!(
            encoded["device_discovery"][0]["devices"][0]["hostname"],
            "NIAHAP-MDF001-WAP001"
        );
        assert_eq!(
            encoded["device_discovery"][0]["devices"][0]["type"],
            "access_point"
        );
        assert!(encoded["device_discovery"][0]["devices"][0]
            .get("device_type")
            .is_none());
    }

    #[test]
    fn builders_keep_wire_contract_stable() {
        let location = DeviceLocation::at(29.9844, -95.3414)
            .with_site_code("IAH")
            .with_site_name("Houston");

        let device = DiscoveredDevice::named("NIAHAP-MDF001-WAP001")
            .with_serial("CNC3HN77NW")
            .with_device_type("access_point")
            .with_location(location)
            .with_label("site", "IAH")
            .with_metadata("radio_count", 2);

        let result = PluginResult::ok("discovered 1 device")
            .with_device_discovery(DeviceDiscovery::new("ual-network-map").with_device(device));

        let encoded = serde_json::to_value(result).expect("serialize");
        assert_eq!(
            encoded["device_discovery"][0]["devices"][0]["type"],
            "access_point"
        );
        assert_eq!(
            encoded["device_discovery"][0]["devices"][0]["location"]["site_code"],
            "IAH"
        );
        assert_eq!(
            encoded["device_discovery"][0]["devices"][0]["metadata"]["radio_count"],
            2
        );
    }

    #[test]
    fn deserializes_missing_schema_to_current_version() {
        let discovery: DeviceDiscovery =
            serde_json::from_value(serde_json::json!({"devices": []})).expect("deserialize");

        assert_eq!(discovery.schema, DEVICE_DISCOVERY_SCHEMA_V1);
    }

    #[test]
    fn supports_mutation_and_extend_for_collectors() {
        let mut discovery = DeviceDiscovery::new("collector");
        discovery.push_device(DiscoveredDevice::named("ap-1"));
        discovery.extend([DiscoveredDevice::named("ap-2")]);

        let mut result = PluginResult::ok("done");
        result.push_device_discovery(discovery);

        assert_eq!(result.device_discovery[0].devices.len(), 2);
    }
}
