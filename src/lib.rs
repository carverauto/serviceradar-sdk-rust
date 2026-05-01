use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const DEVICE_DISCOVERY_SCHEMA_V1: &str = "serviceradar.device_discovery.v1";

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct PluginResult {
    pub status: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub device_discovery: Vec<DeviceDiscovery>,
}

impl PluginResult {
    pub fn ok(summary: impl Into<String>) -> Self {
        Self {
            status: "OK".to_string(),
            summary: summary.into(),
            device_discovery: Vec::new(),
        }
    }

    pub fn with_device_discovery(mut self, discovery: DeviceDiscovery) -> Self {
        self.device_discovery.push(discovery);
        self
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct DeviceDiscovery {
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

impl DeviceDiscovery {
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            schema: DEVICE_DISCOVERY_SCHEMA_V1.to_string(),
            collection_id: None,
            source: Some(source.into()),
            observed_at: None,
            devices: Vec::new(),
            reference_hash: None,
            metadata: BTreeMap::new(),
        }
    }

    pub fn with_device(mut self, device: DiscoveredDevice) -> Self {
        self.devices.push(device);
        self
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
    pub fn named(hostname: impl Into<String>) -> Self {
        Self {
            hostname: Some(hostname.into()),
            ..Self::default()
        }
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
        assert_eq!(encoded["device_discovery"][0]["schema"], DEVICE_DISCOVERY_SCHEMA_V1);
        assert_eq!(
            encoded["device_discovery"][0]["devices"][0]["hostname"],
            "NIAHAP-MDF001-WAP001"
        );
    }
}
