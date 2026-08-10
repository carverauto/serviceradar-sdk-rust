#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CAPABILITY_ADVISORY_FEED_V1: &str = "advisory-feed:v1";
pub const ADVISORY_FEED_CONTRACT_VERSION: &str = "serviceradar.advisory_feed.contract.v1";

pub const COORDINATE_TYPE_PURL: &str = "purl";
pub const COORDINATE_TYPE_CPE: &str = "cpe";
pub const COORDINATE_TYPE_VENDOR_PRODUCT: &str = "vendor_product";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdvisoryFeedBatch {
    pub schema_version: String,
    pub producer_id: String,
    pub source: AdvisorySource,
    pub snapshot: AdvisorySnapshot,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub advisories: Vec<AdvisoryRecord>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub metadata: BTreeMap<String, Value>,
}

impl AdvisoryFeedBatch {
    pub fn new(
        producer_id: impl Into<String>,
        source: AdvisorySource,
        snapshot: AdvisorySnapshot,
    ) -> Self {
        Self {
            schema_version: ADVISORY_FEED_CONTRACT_VERSION.to_string(),
            producer_id: producer_id.into(),
            source,
            snapshot,
            advisories: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }

    pub fn push_advisory(&mut self, advisory: AdvisoryRecord) {
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdvisorySource {
    pub provider: String,
    pub feed_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_type: Option<String>,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_interval_seconds: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retention_days: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_ref: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub options: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_message: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub metadata: BTreeMap<String, Value>,
}

impl AdvisorySource {
    pub fn new(provider: impl Into<String>, feed_key: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            feed_key: feed_key.into(),
            display_name: None,
            feed_type: None,
            enabled: true,
            url: None,
            schema_url: None,
            refresh_interval_seconds: None,
            retention_days: None,
            credential_ref: None,
            options: BTreeMap::new(),
            last_message: None,
            metadata: BTreeMap::new(),
        }
    }

    pub fn with_display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = Some(display_name.into());
        self
    }

    pub fn with_credential_ref(mut self, credential_ref: impl Into<String>) -> Self {
        self.credential_ref = Some(credential_ref.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdvisorySnapshot {
    pub object_key: String,
    pub sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_backend: Option<String>,
    pub accepted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub validation: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetched_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_at: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub metadata: BTreeMap<String, Value>,
}

impl AdvisorySnapshot {
    pub fn new(object_key: impl Into<String>, sha256: impl Into<String>) -> Self {
        Self {
            object_key: object_key.into(),
            sha256: sha256.into(),
            source_url: None,
            content_type: None,
            format: None,
            size_bytes: None,
            storage_backend: None,
            accepted: true,
            status: Some("accepted".to_string()),
            error: None,
            validation: BTreeMap::new(),
            fetched_at: None,
            accepted_at: None,
            metadata: BTreeMap::new(),
        }
    }

    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    pub fn with_size_bytes(mut self, size_bytes: i64) -> Self {
        self.size_bytes = Some(size_bytes);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdvisoryRecord {
    pub source_object_id: String,
    pub advisory_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cve_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvss_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvss_vector: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_at: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub kev: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub exploit_available: bool,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub affected_coordinates: Vec<AffectedCoordinate>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub references: Vec<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub metadata: BTreeMap<String, Value>,
}

impl AdvisoryRecord {
    pub fn new(advisory_id: impl Into<String>) -> Self {
        let advisory_id = advisory_id.into();
        Self {
            source_object_id: advisory_id.clone(),
            advisory_id,
            cve_id: None,
            title: None,
            description: None,
            severity: None,
            cvss_score: None,
            cvss_vector: None,
            published_at: None,
            modified_at: None,
            kev: false,
            exploit_available: false,
            affected_coordinates: Vec::new(),
            references: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }

    pub fn with_cve(mut self, cve_id: impl Into<String>) -> Self {
        let cve_id = cve_id.into();
        if self.advisory_id.is_empty() {
            self.advisory_id = cve_id.clone();
        }
        if self.source_object_id.is_empty() {
            self.source_object_id = cve_id.clone();
        }
        self.cve_id = Some(cve_id);
        self
    }

    pub fn with_severity(mut self, severity: impl Into<String>) -> Self {
        self.severity = Some(severity.into());
        self
    }

    pub fn with_coordinate(mut self, coordinate: AffectedCoordinate) -> Self {
        self.affected_coordinates.push(coordinate);
        self
    }

    pub fn with_reference(mut self, reference: impl Into<String>) -> Self {
        let reference = reference.into();
        if !reference.is_empty() {
            self.references.push(reference);
        }
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AffectedCoordinate {
    #[serde(rename = "type")]
    pub coordinate_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_semantics: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub version_range: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub version_ranges: Vec<BTreeMap<String, Value>>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub metadata: BTreeMap<String, Value>,
}

impl AffectedCoordinate {
    pub fn purl(value: impl Into<String>) -> Self {
        Self::new(COORDINATE_TYPE_PURL).with_value(value)
    }

    pub fn cpe(value: impl Into<String>) -> Self {
        Self::new(COORDINATE_TYPE_CPE).with_value(value)
    }

    pub fn vendor_product(vendor: impl Into<String>, product: impl Into<String>) -> Self {
        Self::new(COORDINATE_TYPE_VENDOR_PRODUCT)
            .with_vendor(vendor)
            .with_product(product)
    }

    pub fn new(coordinate_type: impl Into<String>) -> Self {
        Self {
            coordinate_type: coordinate_type.into(),
            value: None,
            vendor: None,
            product: None,
            match_semantics: None,
            version_range: BTreeMap::new(),
            version_ranges: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }

    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn with_vendor(mut self, vendor: impl Into<String>) -> Self {
        self.vendor = Some(vendor.into());
        self
    }

    pub fn with_product(mut self, product: impl Into<String>) -> Self {
        self.product = Some(product.into());
        self
    }

    pub fn with_match_semantics(mut self, match_semantics: impl Into<String>) -> Self {
        self.match_semantics = Some(match_semantics.into());
        self
    }

    pub fn with_version_range(mut self, version_range: BTreeMap<String, Value>) -> Self {
        self.version_ranges.push(version_range);
        self
    }
}
