use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CAPABILITY_ADVISORY_FEED_V1: &str = "advisory-feed:v1";
pub const ADVISORY_FEED_CONTRACT_VERSION: &str = "serviceradar.advisory_feed.contract.v1";

pub const COORDINATE_TYPE_PURL: &str = "purl";
pub const COORDINATE_TYPE_CPE: &str = "cpe";
pub const COORDINATE_TYPE_VENDOR_PRODUCT: &str = "vendor_product";

fn advisory_feed_contract_version() -> String {
    ADVISORY_FEED_CONTRACT_VERSION.to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdvisoryFeedBatch {
    #[serde(default = "advisory_feed_contract_version")]
    pub schema_version: String,
    pub producer_id: String,
    pub source: AdvisorySource,
    pub snapshot: AdvisorySnapshot,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub advisories: Vec<AdvisoryRecord>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
}

impl AdvisoryFeedBatch {
    pub fn new(
        producer_id: impl Into<String>,
        source: AdvisorySource,
        snapshot: AdvisorySnapshot,
    ) -> Self {
        Self {
            schema_version: advisory_feed_contract_version(),
            producer_id: producer_id.into(),
            source,
            snapshot,
            advisories: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }

    pub fn push_advisory(&mut self, advisory: AdvisoryRecord) {
        if self.schema_version.is_empty() {
            self.schema_version = advisory_feed_contract_version();
        }
        self.advisories.push(advisory);
    }

    pub fn with_advisory(mut self, advisory: AdvisoryRecord) -> Self {
        self.push_advisory(advisory);
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AdvisorySource {
    pub provider: String,
    pub feed_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed_type: Option<String>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_interval_seconds: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retention_days: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_ref: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_message: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
}

impl AdvisorySource {
    pub fn new(provider: impl Into<String>, feed_key: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            feed_key: feed_key.into(),
            enabled: true,
            ..Self::default()
        }
    }

    pub fn with_display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = Some(display_name.into());
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AdvisorySnapshot {
    pub object_key: String,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_backend: Option<String>,
    #[serde(default)]
    pub accepted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub validation: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_at: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
}

impl AdvisorySnapshot {
    pub fn accepted(object_key: impl Into<String>, sha256: impl Into<String>) -> Self {
        Self {
            object_key: object_key.into(),
            sha256: sha256.into(),
            accepted: true,
            status: Some("accepted".to_string()),
            ..Self::default()
        }
    }

    pub fn with_size_bytes(mut self, size_bytes: u64) -> Self {
        self.size_bytes = Some(size_bytes);
        self
    }

    pub fn with_validation(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.validation.insert(key.into(), value.into());
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AdvisoryRecord {
    pub source_object_id: String,
    pub advisory_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cve_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cvss_score: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cvss_vector: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_at: Option<String>,
    #[serde(default)]
    pub kev: bool,
    #[serde(default)]
    pub exploit_available: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_coordinates: Vec<AffectedCoordinate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
}

impl AdvisoryRecord {
    pub fn new(source_object_id: impl Into<String>, advisory_id: impl Into<String>) -> Self {
        Self {
            source_object_id: source_object_id.into(),
            advisory_id: advisory_id.into(),
            ..Self::default()
        }
    }

    pub fn with_cve_id(mut self, cve_id: impl Into<String>) -> Self {
        self.cve_id = Some(cve_id.into());
        self
    }

    pub fn with_severity(mut self, severity: impl Into<String>) -> Self {
        self.severity = Some(severity.into());
        self
    }

    pub fn with_affected_coordinate(mut self, coordinate: AffectedCoordinate) -> Self {
        self.affected_coordinates.push(coordinate);
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AffectedCoordinate {
    #[serde(rename = "type")]
    pub coordinate_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub match_semantics: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub version_range: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub version_ranges: Vec<BTreeMap<String, Value>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
}

impl AffectedCoordinate {
    pub fn purl(value: impl Into<String>) -> Self {
        Self {
            coordinate_type: COORDINATE_TYPE_PURL.to_string(),
            value: Some(value.into()),
            ..Self::default()
        }
    }

    pub fn cpe(value: impl Into<String>) -> Self {
        Self {
            coordinate_type: COORDINATE_TYPE_CPE.to_string(),
            value: Some(value.into()),
            ..Self::default()
        }
    }

    pub fn vendor_product(vendor: impl Into<String>, product: impl Into<String>) -> Self {
        Self {
            coordinate_type: COORDINATE_TYPE_VENDOR_PRODUCT.to_string(),
            vendor: Some(vendor.into()),
            product: Some(product.into()),
            ..Self::default()
        }
    }

    pub fn with_match_semantics(mut self, match_semantics: impl Into<String>) -> Self {
        self.match_semantics = Some(match_semantics.into());
        self
    }
}
