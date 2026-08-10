pub const METRIC_ENVELOPE_SCHEMA_VERSION: &str = "serviceradar.metric.v1";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MetricKind {
    #[default]
    Unspecified = 0,
    Gauge = 1,
    Sum = 2,
    Histogram = 3,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MetricTemporality {
    #[default]
    Unspecified = 0,
    Delta = 1,
    Cumulative = 2,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MetricValueType {
    #[default]
    Unspecified = 0,
    Double = 1,
    Int64 = 2,
    Uint64 = 3,
    Bool = 4,
    String = 5,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MetricStringMapEntry {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MetricResource {
    pub agent_id: String,
    pub gateway_id: String,
    pub partition: String,
    pub service_name: String,
    pub service_type: String,
    pub host_id: String,
    pub host_ip: String,
    pub target_device_ip: String,
    pub device_id: String,
    pub kv_store_id: String,
    pub attributes: Vec<MetricStringMapEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MetricIngestIdentity {
    pub source: String,
    pub payload_kind: String,
    pub producer_id: String,
    pub producer_kind: String,
    pub attested_by: String,
    pub attributes: Vec<MetricStringMapEntry>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MetricPoint {
    pub value: f64,
    pub raw_value: String,
    pub raw_value_type: MetricValueType,
    pub observed_at_unix_nano: u64,
    pub start_time_unix_nano: u64,
    pub reset_anchor: String,
    pub if_index: i32,
    pub interface_uid: String,
    pub series_identity_hint: String,
    pub attributes: Vec<MetricStringMapEntry>,
    pub metadata: Vec<MetricStringMapEntry>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Metric {
    pub name: String,
    pub metric_type: String,
    pub kind: MetricKind,
    pub temporality: MetricTemporality,
    pub is_monotonic: bool,
    pub unit: String,
    pub scale: f64,
    pub counter_width: u32,
    pub points: Vec<MetricPoint>,
    pub tags: Vec<MetricStringMapEntry>,
    pub metadata: Vec<MetricStringMapEntry>,
    pub thresholds: Vec<MetricStringMapEntry>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MetricBatch {
    pub schema_version: String,
    pub resource: MetricResource,
    pub ingest_identity: MetricIngestIdentity,
    pub ingress_id: String,
    pub ingress_timestamp_unix_nano: u64,
    pub emitted_at_unix_nano: u64,
    pub metrics: Vec<Metric>,
}

pub fn marshal_metric_batch(mut batch: MetricBatch) -> Vec<u8> {
    if batch.schema_version.is_empty() {
        batch.schema_version = METRIC_ENVELOPE_SCHEMA_VERSION.to_owned();
    }
    if batch.ingest_identity.payload_kind.is_empty() {
        batch.ingest_identity.payload_kind = METRIC_ENVELOPE_SCHEMA_VERSION.to_owned();
    }

    let mut out = Vec::new();
    append_string(&mut out, 1, &batch.schema_version);
    append_message(&mut out, 2, metric_resource_proto(&batch.resource));
    append_message(
        &mut out,
        3,
        metric_ingest_identity_proto(&batch.ingest_identity),
    );
    append_string(&mut out, 4, &batch.ingress_id);
    append_u64(&mut out, 5, batch.ingress_timestamp_unix_nano);
    append_u64(&mut out, 6, batch.emitted_at_unix_nano);
    for metric in &batch.metrics {
        append_message(&mut out, 20, metric_proto(metric));
    }
    out
}

fn metric_resource_proto(resource: &MetricResource) -> Vec<u8> {
    let mut out = Vec::new();
    append_string(&mut out, 1, &resource.agent_id);
    append_string(&mut out, 2, &resource.gateway_id);
    append_string(&mut out, 3, &resource.partition);
    append_string(&mut out, 4, &resource.service_name);
    append_string(&mut out, 5, &resource.service_type);
    append_string(&mut out, 6, &resource.host_id);
    append_string(&mut out, 7, &resource.host_ip);
    append_string(&mut out, 8, &resource.target_device_ip);
    append_string(&mut out, 9, &resource.device_id);
    append_string(&mut out, 10, &resource.kv_store_id);
    append_string_map_entries(&mut out, 20, &resource.attributes);
    out
}

fn metric_ingest_identity_proto(identity: &MetricIngestIdentity) -> Vec<u8> {
    let mut out = Vec::new();
    append_string(&mut out, 1, &identity.source);
    append_string(&mut out, 2, &identity.payload_kind);
    append_string(&mut out, 3, &identity.producer_id);
    append_string(&mut out, 4, &identity.producer_kind);
    append_string(&mut out, 5, &identity.attested_by);
    append_string_map_entries(&mut out, 20, &identity.attributes);
    out
}

fn metric_proto(metric: &Metric) -> Vec<u8> {
    let mut out = Vec::new();
    append_string(&mut out, 1, &metric.name);
    append_string(&mut out, 2, &metric.metric_type);
    append_i32(&mut out, 3, metric.kind as i32);
    append_i32(&mut out, 4, metric.temporality as i32);
    append_bool(&mut out, 5, metric.is_monotonic);
    append_string(&mut out, 6, &metric.unit);
    append_f64(&mut out, 7, metric.scale);
    append_u32(&mut out, 8, metric.counter_width);
    for point in &metric.points {
        append_message(&mut out, 20, metric_point_proto(point));
    }
    append_string_map_entries(&mut out, 30, &metric.tags);
    append_string_map_entries(&mut out, 31, &metric.metadata);
    append_string_map_entries(&mut out, 32, &metric.thresholds);
    out
}

fn metric_point_proto(point: &MetricPoint) -> Vec<u8> {
    let mut out = Vec::new();
    append_f64(&mut out, 1, point.value);
    append_string(&mut out, 2, &point.raw_value);
    append_i32(&mut out, 3, point.raw_value_type as i32);
    append_u64(&mut out, 4, point.observed_at_unix_nano);
    append_u64(&mut out, 5, point.start_time_unix_nano);
    append_string(&mut out, 6, &point.reset_anchor);
    append_i32(&mut out, 7, point.if_index);
    append_string(&mut out, 8, &point.interface_uid);
    append_string(&mut out, 9, &point.series_identity_hint);
    append_string_map_entries(&mut out, 20, &point.attributes);
    append_string_map_entries(&mut out, 21, &point.metadata);
    out
}

fn string_map_entry_proto(entry: &MetricStringMapEntry) -> Vec<u8> {
    let mut out = Vec::new();
    append_string(&mut out, 1, &entry.key);
    append_string(&mut out, 2, &entry.value);
    out
}

fn append_string_map_entries(out: &mut Vec<u8>, field: u32, entries: &[MetricStringMapEntry]) {
    for entry in entries {
        append_message(out, field, string_map_entry_proto(entry));
    }
}

fn append_string(out: &mut Vec<u8>, field: u32, value: &str) {
    if value.is_empty() {
        return;
    }
    append_tag(out, field, 2);
    append_varint(out, value.len() as u64);
    out.extend_from_slice(value.as_bytes());
}

fn append_message(out: &mut Vec<u8>, field: u32, value: Vec<u8>) {
    if value.is_empty() {
        return;
    }
    append_tag(out, field, 2);
    append_varint(out, value.len() as u64);
    out.extend_from_slice(&value);
}

fn append_i32(out: &mut Vec<u8>, field: u32, value: i32) {
    if value == 0 {
        return;
    }
    append_tag(out, field, 0);
    append_varint(out, value as u64);
}

fn append_u32(out: &mut Vec<u8>, field: u32, value: u32) {
    if value == 0 {
        return;
    }
    append_tag(out, field, 0);
    append_varint(out, value as u64);
}

fn append_u64(out: &mut Vec<u8>, field: u32, value: u64) {
    if value == 0 {
        return;
    }
    append_tag(out, field, 0);
    append_varint(out, value);
}

fn append_bool(out: &mut Vec<u8>, field: u32, value: bool) {
    if !value {
        return;
    }
    append_tag(out, field, 0);
    out.push(1);
}

fn append_f64(out: &mut Vec<u8>, field: u32, value: f64) {
    if value == 0.0 {
        return;
    }
    append_tag(out, field, 1);
    out.extend_from_slice(&value.to_bits().to_le_bytes());
}

fn append_tag(out: &mut Vec<u8>, field: u32, wire_type: u32) {
    append_varint(out, ((field << 3) | wire_type) as u64);
}

fn append_varint(out: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_scalar_metric_batch() {
        let payload = marshal_metric_batch(MetricBatch {
            resource: MetricResource {
                agent_id: "agent-1".to_owned(),
                service_name: "rust-plugin".to_owned(),
                service_type: "wasm-plugin".to_owned(),
                attributes: vec![MetricStringMapEntry {
                    key: "rack".to_owned(),
                    value: "rack-7".to_owned(),
                }],
                ..Default::default()
            },
            ingest_identity: MetricIngestIdentity {
                source: "plugin-metrics".to_owned(),
                producer_id: "rust-plugin".to_owned(),
                producer_kind: "wasm-plugin".to_owned(),
                ..Default::default()
            },
            metrics: vec![Metric {
                name: "temperature_c".to_owned(),
                metric_type: "plugin".to_owned(),
                kind: MetricKind::Gauge,
                unit: "Cel".to_owned(),
                points: vec![MetricPoint {
                    value: 42.5,
                    raw_value: "42.5".to_owned(),
                    raw_value_type: MetricValueType::Double,
                    observed_at_unix_nano: 123,
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        });

        let root = parse_fields(&payload);
        assert_eq!(field_string(&root, 1), METRIC_ENVELOPE_SCHEMA_VERSION);
        let resource = parse_fields(field_bytes(&root, 2));
        assert_eq!(field_string(&resource, 1), "agent-1");
        assert_eq!(field_string(&resource, 4), "rust-plugin");
        let identity = parse_fields(field_bytes(&root, 3));
        assert_eq!(field_string(&identity, 2), METRIC_ENVELOPE_SCHEMA_VERSION);
        let metric = parse_fields(field_bytes(&root, 20));
        assert_eq!(field_string(&metric, 1), "temperature_c");
        assert_eq!(field_varint(&metric, 3), MetricKind::Gauge as u64);
        let point = parse_fields(field_bytes(&metric, 20));
        assert_eq!(field_f64(&point, 1), 42.5);
        assert_eq!(field_string(&point, 2), "42.5");
    }

    #[derive(Debug)]
    struct Field {
        number: u32,
        wire: u8,
        varint: u64,
        bytes: Vec<u8>,
        fixed: u64,
    }

    fn parse_fields(mut payload: &[u8]) -> Vec<Field> {
        let mut fields = Vec::new();
        while !payload.is_empty() {
            let (tag, n) = read_varint(payload);
            payload = &payload[n..];
            let number = (tag >> 3) as u32;
            let wire = (tag & 0x7) as u8;
            let mut field = Field {
                number,
                wire,
                varint: 0,
                bytes: Vec::new(),
                fixed: 0,
            };
            match wire {
                0 => {
                    let (value, n) = read_varint(payload);
                    field.varint = value;
                    payload = &payload[n..];
                }
                1 => {
                    field.fixed = u64::from_le_bytes(payload[..8].try_into().unwrap());
                    payload = &payload[8..];
                }
                2 => {
                    let (len, n) = read_varint(payload);
                    payload = &payload[n..];
                    field.bytes = payload[..len as usize].to_vec();
                    payload = &payload[len as usize..];
                }
                _ => panic!("unsupported wire type {wire}"),
            }
            fields.push(field);
        }
        fields
    }

    fn read_varint(payload: &[u8]) -> (u64, usize) {
        let mut value = 0u64;
        let mut shift = 0;
        for (idx, byte) in payload.iter().copied().enumerate() {
            value |= u64::from(byte & 0x7f) << shift;
            if byte < 0x80 {
                return (value, idx + 1);
            }
            shift += 7;
        }
        panic!("unterminated varint");
    }

    fn field_bytes(fields: &[Field], number: u32) -> &[u8] {
        let matches: Vec<_> = fields
            .iter()
            .filter(|field| field.number == number)
            .collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].wire, 2);
        &matches[0].bytes
    }

    fn field_string(fields: &[Field], number: u32) -> String {
        String::from_utf8(field_bytes(fields, number).to_vec()).unwrap()
    }

    fn field_varint(fields: &[Field], number: u32) -> u64 {
        let matches: Vec<_> = fields
            .iter()
            .filter(|field| field.number == number)
            .collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].wire, 0);
        matches[0].varint
    }

    fn field_f64(fields: &[Field], number: u32) -> f64 {
        let matches: Vec<_> = fields
            .iter()
            .filter(|field| field.number == number)
            .collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].wire, 1);
        f64::from_bits(matches[0].fixed)
    }
}
