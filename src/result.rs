use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::device_discovery::DeviceDiscovery;
use crate::error::SdkResult;
use crate::plugin_inputs::TargetContext;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Status {
    Ok,
    Warning,
    Critical,
    #[default]
    Unknown,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Warning => "WARNING",
            Self::Critical => "CRITICAL",
            Self::Unknown => "UNKNOWN",
        }
    }
}

impl Display for Status {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Status {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.trim().to_ascii_uppercase().as_str() {
            "OK" => Ok(Self::Ok),
            "WARNING" | "WARN" => Ok(Self::Warning),
            "CRITICAL" | "CRIT" => Ok(Self::Critical),
            "UNKNOWN" => Ok(Self::Unknown),
            value => Err(format!("invalid status: {value}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Severity {
    Info,
    Warning,
    Critical,
    Error,
}

impl Severity {
    fn as_str(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warning => "WARNING",
            Self::Critical => "CRITICAL",
            Self::Error => "ERROR",
        }
    }
}

impl Display for Severity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Severity {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.trim().to_ascii_uppercase().as_str() {
            "INFO" | "INFORMATIONAL" => Ok(Self::Info),
            "WARNING" | "WARN" => Ok(Self::Warning),
            "CRITICAL" | "CRIT" => Ok(Self::Critical),
            "ERROR" | "ERR" | "HIGH" => Ok(Self::Error),
            value => Err(format!("invalid severity: {value}")),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ThresholdSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warn: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crit: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

impl ThresholdSpec {
    pub fn new(warn: f64, crit: f64) -> Self {
        let mut value = Self::default();
        if warn > 0.0 {
            value.warn = Some(warn);
        }
        if crit > 0.0 {
            value.crit = Some(crit);
        }
        value
    }

    pub fn with_min(mut self, min: f64) -> Self {
        self.min = Some(min);
        self
    }

    pub fn with_max(mut self, max: f64) -> Self {
        self.max = Some(max);
        self
    }

    pub fn with_range(mut self, min: f64, max: f64) -> Self {
        self.min = Some(min);
        self.max = Some(max);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub name: String,
    pub value: f64,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub unit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warn: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crit: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

impl Metric {
    pub fn new(name: impl Into<String>, value: f64) -> Self {
        Self {
            name: name.into(),
            value,
            unit: String::new(),
            warn: None,
            crit: None,
            min: None,
            max: None,
        }
    }

    pub fn with_unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = unit.into();
        self
    }

    pub fn with_thresholds(mut self, thresholds: &ThresholdSpec) -> Self {
        self.warn = thresholds.warn;
        self.crit = thresholds.crit;
        self.min = thresholds.min;
        self.max = thresholds.max;
        self
    }

    pub fn with_bounds(mut self, min: f64, max: f64) -> Self {
        self.min = Some(min);
        self.max = Some(max);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayWidget {
    pub widget: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub data: BTreeMap<String, Value>,
}

impl DisplayWidget {
    pub fn stat_card(
        label: impl Into<String>,
        value: impl Into<String>,
        tone: impl Into<String>,
    ) -> Self {
        Self {
            widget: "stat_card".to_string(),
            label: Some(label.into()),
            value: Some(value.into()),
            tone: Some(tone.into()),
            layout: None,
            data: BTreeMap::new(),
        }
    }

    pub fn table(data: BTreeMap<String, String>, layout: impl Into<String>) -> Option<Self> {
        if data.is_empty() {
            return None;
        }

        Some(Self {
            widget: "table".to_string(),
            label: None,
            value: None,
            tone: None,
            layout: Some(layout.into()),
            data: data
                .into_iter()
                .map(|(key, value)| (key, Value::String(value)))
                .collect(),
        })
    }

    pub fn sparkline(
        label: impl Into<String>,
        points: Vec<f64>,
        tone: impl Into<String>,
    ) -> Option<Self> {
        if points.is_empty() {
            return None;
        }

        let mut data = BTreeMap::new();
        data.insert(
            "values".to_string(),
            Value::Array(points.into_iter().map(Value::from).collect()),
        );

        Some(Self {
            widget: "sparkline".to_string(),
            label: Some(label.into()),
            value: None,
            tone: Some(tone.into()),
            layout: None,
            data,
        })
    }

    pub fn markdown(markdown: impl Into<String>) -> Option<Self> {
        let markdown = markdown.into();
        if markdown.is_empty() {
            return None;
        }

        let mut data = BTreeMap::new();
        data.insert("markdown".to_string(), Value::String(markdown));

        Some(Self {
            widget: "markdown".to_string(),
            label: None,
            value: None,
            tone: None,
            layout: None,
            data,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcsfEvent {
    pub id: String,
    pub time: String,
    pub class_uid: i32,
    pub category_uid: i32,
    pub type_uid: i32,
    pub activity_id: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity_name: Option<String>,
    pub severity_id: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_id: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_detail: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub metadata: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub observables: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_id: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub actor: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub device: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub src_endpoint: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub dst_endpoint: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_version: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub unmapped: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_data: Option<String>,
}

impl OcsfEvent {
    pub fn log_activity(message: &str, severity: Severity) -> Self {
        build_ocsf_event_log_activity(message, severity)
    }
}

pub type Thresholds = ThresholdSpec;
pub type Widget = DisplayWidget;
pub type Event = OcsfEvent;

pub const SIGNAL_SCHEMA_METADATA_SERVICE_RADAR: &str = "service_radar";
pub const SIGNAL_SCHEMA_METADATA_SIGNAL_SCHEMA: &str = "signal_schema";
pub const SIGNAL_SCHEMA_METADATA_PRODUCER_ID: &str = "producer_id";
pub const SIGNAL_SCHEMA_METADATA_PRODUCER_VERSION: &str = "producer_version";
pub const SIGNAL_SCHEMA_METADATA_SCHEMA_ID: &str = "schema_id";
pub const SIGNAL_SCHEMA_METADATA_SCHEMA_VERSION: &str = "schema_version";
pub const SIGNAL_SCHEMA_METADATA_DISPLAY_CONTRACT_ID: &str = "display_contract_id";
pub const SIGNAL_SCHEMA_METADATA_DISPLAY_CONTRACT_VERSION: &str = "display_contract_version";
pub const SIGNAL_SCHEMA_METADATA_DISPLAY_CONTRACT: &str = "display_contract";
pub const SIGNAL_SCHEMA_METADATA_SIGNAL_TYPE: &str = "signal_type";
pub const SIGNAL_SCHEMA_METADATA_PAYLOAD_KIND: &str = "payload_kind";
pub const SIGNAL_SCHEMA_SIGNAL_TYPE_EVENT: &str = "event";
pub const SIGNAL_SCHEMA_SIGNAL_TYPE_LOG: &str = "log";
pub const SIGNAL_SCHEMA_PAYLOAD_KIND_OCSF_EVENT: &str = "ocsf_event";
pub const SIGNAL_SCHEMA_PAYLOAD_KIND_OTEL_LOG: &str = "otel_log";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SignalSchemaRef {
    pub producer_id: String,
    pub producer_version: String,
    pub schema_id: String,
    pub schema_version: String,
    pub display_contract_id: String,
    pub display_contract_version: String,
    pub display_contract: String,
    pub signal_type: String,
    pub payload_kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Result {
    #[serde(skip)]
    status: Option<Status>,
    #[serde(skip)]
    summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    perfdata: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    metrics: Vec<Metric>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    labels: BTreeMap<String, String>,
    #[serde(skip)]
    observed_at: Option<String>,
    #[serde(skip)]
    schema_version: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    display: Vec<DisplayWidget>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    events: Vec<OcsfEvent>,
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    alert_hint: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    condition_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    check_instance_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    monitored_service_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_uid: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    device_discovery: Vec<DeviceDiscovery>,
}

impl Result {
    pub fn new() -> Self {
        Self {
            status: Some(Status::Unknown),
            summary: None,
            details: None,
            perfdata: None,
            metrics: Vec::new(),
            labels: BTreeMap::new(),
            observed_at: None,
            schema_version: Some(1),
            display: Vec::new(),
            events: Vec::new(),
            alert_hint: false,
            condition_id: None,
            check_instance_id: None,
            monitored_service_id: None,
            device_uid: None,
            device_discovery: Vec::new(),
        }
    }

    pub fn ok(summary: impl Into<String>) -> Self {
        Self::new().with_status(Status::Ok).with_summary(summary)
    }

    pub fn warning(summary: impl Into<String>) -> Self {
        Self::new()
            .with_status(Status::Warning)
            .with_summary(summary)
    }

    pub fn critical(summary: impl Into<String>) -> Self {
        Self::new()
            .with_status(Status::Critical)
            .with_summary(summary)
    }

    pub fn unknown(summary: impl Into<String>) -> Self {
        Self::new()
            .with_status(Status::Unknown)
            .with_summary(summary)
    }

    pub fn target(ctx: &TargetContext, status: Status, summary: impl Into<String>) -> Self {
        Self::new()
            .for_target(ctx)
            .with_status(status)
            .with_summary(summary)
    }

    pub fn status(&self) -> Status {
        self.status.unwrap_or(Status::Unknown)
    }

    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    pub fn details(&self) -> Option<&str> {
        self.details.as_deref()
    }

    pub fn metrics(&self) -> &[Metric] {
        &self.metrics
    }

    pub fn labels(&self) -> &BTreeMap<String, String> {
        &self.labels
    }

    pub fn widgets(&self) -> &[DisplayWidget] {
        &self.display
    }

    pub fn events(&self) -> &[OcsfEvent] {
        &self.events
    }

    pub fn device_discovery(&self) -> &[DeviceDiscovery] {
        &self.device_discovery
    }

    pub fn set_status(&mut self, status: Status) {
        self.status = Some(status);
    }

    pub fn with_status(mut self, status: Status) -> Self {
        self.set_status(status);
        self
    }

    pub fn set_summary(&mut self, summary: impl Into<String>) {
        self.summary = Some(summary.into());
    }

    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.set_summary(summary);
        self
    }

    pub fn set_details(&mut self, details: impl Into<String>) {
        self.details = Some(details.into());
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.set_details(details);
        self
    }

    pub fn set_perfdata(&mut self, perfdata: impl Into<String>) {
        self.perfdata = Some(perfdata.into());
    }

    pub fn with_perfdata(mut self, perfdata: impl Into<String>) -> Self {
        self.set_perfdata(perfdata);
        self
    }

    pub fn set_schema_version(&mut self, version: u32) {
        self.schema_version = Some(version);
    }

    pub fn with_schema_version(mut self, version: u32) -> Self {
        self.set_schema_version(version);
        self
    }

    pub fn set_observed_at(&mut self, observed_at: impl Into<String>) {
        self.observed_at = Some(observed_at.into());
    }

    pub fn with_observed_at(mut self, observed_at: impl Into<String>) -> Self {
        self.set_observed_at(observed_at);
        self
    }

    pub fn for_target(mut self, ctx: &TargetContext) -> Self {
        self.check_instance_id = Some(ctx.check_instance_id.clone());
        self.monitored_service_id = ctx.monitored_service_id().map(ToOwned::to_owned);
        self.device_uid = ctx.device_uid().map(ToOwned::to_owned);
        self.add_label("check_instance_id", ctx.check_instance_id.clone());

        if !ctx.descriptor_id.is_empty() {
            self.add_label("descriptor_id", ctx.descriptor_id.clone());
        }
        if !ctx.uid.is_empty() {
            self.add_label("target_uid", ctx.uid.clone());
        }

        self
    }

    pub fn add_label(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        if key.is_empty() {
            return;
        }

        self.labels.insert(key, value.into());
    }

    pub fn with_label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.add_label(key, value);
        self
    }

    pub fn add_metric_spec(&mut self, metric: Metric) {
        self.metrics.push(metric);
    }

    pub fn with_metric_spec(mut self, metric: Metric) -> Self {
        self.add_metric_spec(metric);
        self
    }

    pub fn add_metric(
        &mut self,
        name: impl Into<String>,
        value: f64,
        unit: impl Into<String>,
        thresholds: Option<&ThresholdSpec>,
    ) {
        let metric = match thresholds {
            Some(thresholds) => Metric::new(name, value)
                .with_unit(unit)
                .with_thresholds(thresholds),
            None => Metric::new(name, value).with_unit(unit),
        };

        self.add_metric_spec(metric);
    }

    pub fn with_metric(
        mut self,
        name: impl Into<String>,
        value: f64,
        unit: impl Into<String>,
        thresholds: Option<&ThresholdSpec>,
    ) -> Self {
        self.add_metric(name, value, unit, thresholds);
        self
    }

    pub fn add_display_widget(&mut self, widget: DisplayWidget) {
        self.add_widget(widget);
    }

    pub fn with_display_widget(mut self, widget: DisplayWidget) -> Self {
        self.add_widget(widget);
        self
    }

    pub fn add_widget(&mut self, widget: Widget) {
        self.display.push(widget);
    }

    pub fn with_widget(mut self, widget: Widget) -> Self {
        self.add_widget(widget);
        self
    }

    pub fn add_stat_card(
        &mut self,
        label: impl Into<String>,
        value: impl Into<String>,
        tone: impl Into<String>,
    ) {
        self.add_display_widget(DisplayWidget::stat_card(label, value, tone));
    }

    pub fn with_stat_card(
        mut self,
        label: impl Into<String>,
        value: impl Into<String>,
        tone: impl Into<String>,
    ) -> Self {
        self.add_stat_card(label, value, tone);
        self
    }

    pub fn add_table(&mut self, data: BTreeMap<String, String>, layout: impl Into<String>) {
        if let Some(widget) = DisplayWidget::table(data, layout) {
            self.add_widget(widget);
        }
    }

    pub fn with_table(mut self, data: BTreeMap<String, String>, layout: impl Into<String>) -> Self {
        self.add_table(data, layout);
        self
    }

    pub fn add_sparkline(
        &mut self,
        label: impl Into<String>,
        points: Vec<f64>,
        tone: impl Into<String>,
    ) {
        if let Some(widget) = DisplayWidget::sparkline(label, points, tone) {
            self.add_widget(widget);
        }
    }

    pub fn with_sparkline(
        mut self,
        label: impl Into<String>,
        points: Vec<f64>,
        tone: impl Into<String>,
    ) -> Self {
        self.add_sparkline(label, points, tone);
        self
    }

    pub fn add_markdown(&mut self, markdown: impl Into<String>) {
        if let Some(widget) = DisplayWidget::markdown(markdown) {
            self.add_widget(widget);
        }
    }

    pub fn with_markdown(mut self, markdown: impl Into<String>) -> Self {
        self.add_markdown(markdown);
        self
    }

    pub fn emit_event(
        &mut self,
        severity: Severity,
        summary: impl Into<String>,
        key: impl Into<String>,
    ) {
        let summary = summary.into();
        if summary.is_empty() {
            return;
        }

        let mut event = build_ocsf_event_log_activity(&summary, severity);
        let key = key.into();
        if !key.is_empty() {
            event
                .unmapped
                .insert("condition_key".to_string(), Value::String(key));
        }

        self.events.push(event);
    }

    pub fn with_event(
        mut self,
        severity: Severity,
        summary: impl Into<String>,
        key: impl Into<String>,
    ) -> Self {
        self.emit_event(severity, summary, key);
        self
    }

    pub fn add_ocsf_event(&mut self, mut event: OcsfEvent) {
        if event.id.is_empty() {
            event.id = generate_event_id();
        }
        if event.time.is_empty() {
            event.time = now_rfc3339().expect("formatting current time");
        }
        self.events.push(event);
    }

    pub fn with_ocsf_event(mut self, event: OcsfEvent) -> Self {
        self.add_ocsf_event(event);
        self
    }

    pub fn add_device_discovery(&mut self, discovery: DeviceDiscovery) {
        self.device_discovery.push(discovery);
    }

    pub fn with_device_discovery(mut self, discovery: DeviceDiscovery) -> Self {
        self.add_device_discovery(discovery);
        self
    }

    pub fn request_immediate_alert(&mut self, condition_id: impl Into<String>) {
        self.alert_hint = true;
        self.condition_id = Some(condition_id.into());
    }

    pub fn with_immediate_alert(mut self, condition_id: impl Into<String>) -> Self {
        self.request_immediate_alert(condition_id);
        self
    }

    pub fn apply_thresholds(&mut self, value: f64, warn: Option<f64>, crit: Option<f64>) {
        match (warn, crit) {
            (_, Some(crit)) if value >= crit => self.status = Some(Status::Critical),
            (Some(warn), _) if value >= warn => self.status = Some(Status::Warning),
            _ if matches!(self.status(), Status::Unknown) => self.status = Some(Status::Ok),
            _ => {}
        }
    }

    pub fn with_thresholds(mut self, value: f64, warn: Option<f64>, crit: Option<f64>) -> Self {
        self.apply_thresholds(value, warn, crit);
        self
    }

    pub fn serialize(&self) -> SdkResult<Vec<u8>> {
        serde_json::to_vec(&self.serializable()?).map_err(Into::into)
    }

    fn serializable(&self) -> SdkResult<SerializableResult> {
        let status = self.status();
        let summary = self
            .summary
            .clone()
            .unwrap_or_else(|| status.as_str().to_string());
        let observed_at = self.observed_at.clone().unwrap_or(now_rfc3339()?);

        Ok(SerializableResult {
            status,
            summary,
            details: self.details.clone(),
            perfdata: self.perfdata.clone(),
            metrics: self.metrics.clone(),
            labels: self.labels.clone(),
            observed_at,
            schema_version: self.schema_version.unwrap_or(1),
            display: self.display.clone(),
            events: self.events.clone(),
            alert_hint: self.alert_hint,
            condition_id: self.condition_id.clone(),
            check_instance_id: self.check_instance_id.clone(),
            monitored_service_id: self.monitored_service_id.clone(),
            device_uid: self.device_uid.clone(),
            device_discovery: self.device_discovery.clone(),
        })
    }
}

impl OcsfEvent {
    pub fn attach_signal_schema_ref(&mut self, signal_schema: &SignalSchemaRef) {
        attach_signal_schema_ref(self, signal_schema);
    }

    pub fn with_signal_schema_ref(mut self, signal_schema: &SignalSchemaRef) -> Self {
        self.attach_signal_schema_ref(signal_schema);
        self
    }
}

pub fn attach_signal_schema_ref(event: &mut OcsfEvent, signal_schema: &SignalSchemaRef) {
    let mut ref_metadata = Map::new();
    put_signal_schema_field(
        &mut ref_metadata,
        SIGNAL_SCHEMA_METADATA_PRODUCER_ID,
        &signal_schema.producer_id,
    );
    put_signal_schema_field(
        &mut ref_metadata,
        SIGNAL_SCHEMA_METADATA_PRODUCER_VERSION,
        &signal_schema.producer_version,
    );
    put_signal_schema_field(
        &mut ref_metadata,
        SIGNAL_SCHEMA_METADATA_SCHEMA_ID,
        &signal_schema.schema_id,
    );
    put_signal_schema_field(
        &mut ref_metadata,
        SIGNAL_SCHEMA_METADATA_SCHEMA_VERSION,
        &signal_schema.schema_version,
    );
    put_signal_schema_field(
        &mut ref_metadata,
        SIGNAL_SCHEMA_METADATA_DISPLAY_CONTRACT_ID,
        &signal_schema.display_contract_id,
    );
    put_signal_schema_field(
        &mut ref_metadata,
        SIGNAL_SCHEMA_METADATA_DISPLAY_CONTRACT_VERSION,
        &signal_schema.display_contract_version,
    );
    put_signal_schema_field(
        &mut ref_metadata,
        SIGNAL_SCHEMA_METADATA_DISPLAY_CONTRACT,
        &signal_schema.display_contract,
    );
    put_signal_schema_field(
        &mut ref_metadata,
        SIGNAL_SCHEMA_METADATA_SIGNAL_TYPE,
        &signal_schema.signal_type,
    );
    put_signal_schema_field(
        &mut ref_metadata,
        SIGNAL_SCHEMA_METADATA_PAYLOAD_KIND,
        &signal_schema.payload_kind,
    );

    let service_radar = event
        .metadata
        .entry(SIGNAL_SCHEMA_METADATA_SERVICE_RADAR.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !service_radar.is_object() {
        *service_radar = Value::Object(Map::new());
    }

    if let Some(service_radar) = service_radar.as_object_mut() {
        service_radar.insert(
            SIGNAL_SCHEMA_METADATA_SIGNAL_SCHEMA.to_string(),
            Value::Object(ref_metadata),
        );
    }
}

fn put_signal_schema_field(metadata: &mut Map<String, Value>, key: &str, value: &str) {
    if !value.is_empty() {
        metadata.insert(key.to_string(), Value::String(value.to_string()));
    }
}

impl Extend<DeviceDiscovery> for Result {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = DeviceDiscovery>,
    {
        self.device_discovery.extend(iter);
    }
}

impl Default for Result {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize)]
struct SerializableResult {
    status: Status,
    summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    perfdata: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    metrics: Vec<Metric>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    labels: BTreeMap<String, String>,
    observed_at: String,
    schema_version: u32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    display: Vec<DisplayWidget>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    events: Vec<OcsfEvent>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    alert_hint: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    condition_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    check_instance_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    monitored_service_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_uid: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    device_discovery: Vec<DeviceDiscovery>,
}

const OCSF_CLASS_EVENT_LOG_ACTIVITY: i32 = 1008;
const OCSF_CATEGORY_SYSTEM_ACTIVITY: i32 = 1;
const OCSF_ACTIVITY_LOG_CREATE: i32 = 1;
const OCSF_VERSION: &str = "1.7.0";

fn build_ocsf_event_log_activity(message: &str, severity: Severity) -> OcsfEvent {
    let (severity_id, severity_name) = severity_to_ocsf(severity);
    let now = now_rfc3339().expect("formatting current time");

    let mut metadata = BTreeMap::new();
    metadata.insert(
        "version".to_string(),
        Value::String(OCSF_VERSION.to_string()),
    );
    metadata.insert(
        "product".to_string(),
        serde_json::json!({"vendor_name": "ServiceRadar", "name": "plugin"}),
    );
    metadata.insert("logged_time".to_string(), Value::String(now.clone()));

    OcsfEvent {
        id: generate_event_id(),
        time: now,
        class_uid: OCSF_CLASS_EVENT_LOG_ACTIVITY,
        category_uid: OCSF_CATEGORY_SYSTEM_ACTIVITY,
        type_uid: OCSF_CLASS_EVENT_LOG_ACTIVITY * 100 + OCSF_ACTIVITY_LOG_CREATE,
        activity_id: OCSF_ACTIVITY_LOG_CREATE,
        activity_name: Some("Create".to_string()),
        severity_id,
        severity: Some(severity_name.to_string()),
        message: Some(message.to_string()),
        status_id: None,
        status: None,
        status_code: None,
        status_detail: None,
        metadata,
        observables: Vec::new(),
        trace_id: None,
        span_id: None,
        actor: BTreeMap::new(),
        device: BTreeMap::new(),
        src_endpoint: BTreeMap::new(),
        dst_endpoint: BTreeMap::new(),
        log_name: Some("events.ocsf.processed".to_string()),
        log_provider: Some("serviceradar-plugin".to_string()),
        log_level: None,
        log_version: None,
        unmapped: BTreeMap::new(),
        raw_data: None,
    }
}

fn severity_to_ocsf(severity: Severity) -> (i32, &'static str) {
    match severity {
        Severity::Critical => (5, "Critical"),
        Severity::Error => (6, "High"),
        Severity::Warning => (3, "Medium"),
        Severity::Info => (1, "Informational"),
    }
}

fn now_rfc3339() -> SdkResult<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(Into::into)
}

fn generate_event_id() -> String {
    static EVENT_COUNTER: AtomicU64 = AtomicU64::new(0);
    let counter = EVENT_COUNTER.fetch_add(1, Ordering::SeqCst) + 1;
    format!(
        "plugin-{}-{counter}",
        OffsetDateTime::now_utc().unix_timestamp_nanos()
    )
}

#[cfg(test)]
mod tests;
