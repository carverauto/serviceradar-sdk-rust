use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::error::SdkResult;
use crate::host;
use crate::result::{
    OcsfEvent, SIGNAL_SCHEMA_METADATA_DISPLAY_CONTRACT, SIGNAL_SCHEMA_METADATA_DISPLAY_CONTRACT_ID,
    SIGNAL_SCHEMA_METADATA_DISPLAY_CONTRACT_VERSION, SIGNAL_SCHEMA_METADATA_PAYLOAD_KIND,
    SIGNAL_SCHEMA_METADATA_PRODUCER_ID, SIGNAL_SCHEMA_METADATA_PRODUCER_VERSION,
    SIGNAL_SCHEMA_METADATA_SCHEMA_ID, SIGNAL_SCHEMA_METADATA_SCHEMA_VERSION,
    SIGNAL_SCHEMA_METADATA_SIGNAL_TYPE, SIGNAL_SCHEMA_PAYLOAD_KIND_OCSF_EVENT,
    SIGNAL_SCHEMA_PAYLOAD_KIND_OTEL_LOG, SignalSchemaRef,
};

const TELEMETRY_METADATA_PREFIX: &str = "serviceradar.signal_schema.";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelemetrySource {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_instance: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub metadata: BTreeMap<String, String>,
}

impl TelemetrySource {
    pub fn new(source_type: impl Into<String>, source_instance: impl Into<String>) -> Self {
        Self {
            source_type: Some(source_type.into()),
            source_instance: Some(source_instance.into()),
            metadata: BTreeMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TelemetryRecord {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_time_unix_nano: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_time_unix_nano: Option<i64>,
    pub payload_kind: String,
    pub payload: Value,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub metadata: BTreeMap<String, String>,
}

impl TelemetryRecord {
    pub fn ocsf_event(mut event: OcsfEvent) -> SdkResult<Self> {
        let now = now_unix_nanos();
        if event.time.is_empty() {
            event.time = now_rfc3339();
        }

        Ok(Self {
            event_id: empty_string_to_none(event.id.clone()),
            observed_time_unix_nano: Some(now),
            event_time_unix_nano: Some(parse_rfc3339_unix_nanos(&event.time).unwrap_or(now)),
            payload_kind: SIGNAL_SCHEMA_PAYLOAD_KIND_OCSF_EVENT.to_string(),
            payload: serde_json::to_value(event)?,
            metadata: BTreeMap::new(),
        })
    }

    pub fn otel_log(event_id: impl Into<String>, log: impl Serialize) -> SdkResult<Self> {
        let now = now_unix_nanos();

        Ok(Self {
            event_id: empty_string_to_none(event_id.into()),
            observed_time_unix_nano: Some(now),
            event_time_unix_nano: Some(now),
            payload_kind: SIGNAL_SCHEMA_PAYLOAD_KIND_OTEL_LOG.to_string(),
            payload: serde_json::to_value(log)?,
            metadata: BTreeMap::new(),
        })
    }

    pub fn attach_signal_schema_ref(&mut self, signal_schema: &SignalSchemaRef) {
        put_signal_schema_field(
            &mut self.metadata,
            SIGNAL_SCHEMA_METADATA_PRODUCER_ID,
            &signal_schema.producer_id,
        );
        put_signal_schema_field(
            &mut self.metadata,
            SIGNAL_SCHEMA_METADATA_PRODUCER_VERSION,
            &signal_schema.producer_version,
        );
        put_signal_schema_field(
            &mut self.metadata,
            SIGNAL_SCHEMA_METADATA_SCHEMA_ID,
            &signal_schema.schema_id,
        );
        put_signal_schema_field(
            &mut self.metadata,
            SIGNAL_SCHEMA_METADATA_SCHEMA_VERSION,
            &signal_schema.schema_version,
        );
        put_signal_schema_field(
            &mut self.metadata,
            SIGNAL_SCHEMA_METADATA_DISPLAY_CONTRACT_ID,
            &signal_schema.display_contract_id,
        );
        put_signal_schema_field(
            &mut self.metadata,
            SIGNAL_SCHEMA_METADATA_DISPLAY_CONTRACT_VERSION,
            &signal_schema.display_contract_version,
        );
        put_signal_schema_field(
            &mut self.metadata,
            SIGNAL_SCHEMA_METADATA_DISPLAY_CONTRACT,
            &signal_schema.display_contract,
        );
        put_signal_schema_field(
            &mut self.metadata,
            SIGNAL_SCHEMA_METADATA_SIGNAL_TYPE,
            &signal_schema.signal_type,
        );
        put_signal_schema_field(
            &mut self.metadata,
            SIGNAL_SCHEMA_METADATA_PAYLOAD_KIND,
            &signal_schema.payload_kind,
        );
    }

    pub fn with_signal_schema_ref(mut self, signal_schema: &SignalSchemaRef) -> Self {
        self.attach_signal_schema_ref(signal_schema);
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TelemetryBatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<TelemetrySource>,
    pub records: Vec<TelemetryRecord>,
}

impl TelemetryBatch {
    pub fn new(records: impl Into<Vec<TelemetryRecord>>) -> Self {
        Self {
            source: None,
            records: records.into(),
        }
    }

    pub fn with_source(mut self, source: TelemetrySource) -> Self {
        self.source = Some(source);
        self
    }

    pub fn serialize(&self) -> SdkResult<Vec<u8>> {
        serde_json::to_vec(self).map_err(Into::into)
    }
}

pub fn emit_telemetry(batch: TelemetryBatch) -> SdkResult<()> {
    let payload = batch.serialize()?;
    host::emit_non_empty_telemetry(&payload)?;
    Ok(())
}

fn put_signal_schema_field(metadata: &mut BTreeMap<String, String>, key: &str, value: &str) {
    if !value.is_empty() {
        metadata.insert(
            format!("{TELEMETRY_METADATA_PREFIX}{key}"),
            value.to_string(),
        );
    }
}

fn empty_string_to_none(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}

fn parse_rfc3339_unix_nanos(value: &str) -> Option<i64> {
    OffsetDateTime::parse(value, &Rfc3339)
        .ok()
        .map(|time| time.unix_timestamp_nanos() as i64)
}

fn now_unix_nanos() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp_nanos() as i64
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("formatting current time")
}

#[cfg(test)]
mod tests;
