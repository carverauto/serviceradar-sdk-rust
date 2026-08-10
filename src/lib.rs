mod action;
mod advisory;
mod artifact;
mod camera_http;
mod camera_media;
mod camera_plugin;
mod camera_relay;
mod check_descriptor;
mod config;
mod device_discovery;
mod error;
mod execute;
mod host;
mod http;
#[cfg(not(target_arch = "wasm32"))]
pub mod local;
mod log;
mod manifest;
mod memory;
mod metric_envelope;
mod plugin_inputs;
mod producer_schedule;
mod result;
mod rtsp;
mod tcp;
mod telemetry;
mod udp;
mod websocket;

pub use action::{
    ACTION_INVOCATION_SCHEMA_V1, ACTION_RESULT_SCHEMA_V1, ActionCallback, ActionDescriptor,
    ActionHostConfig, ActionInvocation, ActionPollMode, ActionResult, ActionSafety, ActionScope,
    ActionStatus, ActionTargetResult, ActionTargetSnapshot, load_action_config,
    parse_action_config, submit_action_result,
};
pub use advisory::{
    ADVISORY_FEED_CONTRACT_VERSION, AdvisoryFeedBatch, AdvisoryRecord, AdvisorySnapshot,
    AdvisorySource, AffectedCoordinate, CAPABILITY_ADVISORY_FEED_V1, COORDINATE_TYPE_CPE,
    COORDINATE_TYPE_PURL, COORDINATE_TYPE_VENDOR_PRODUCT,
};
pub use artifact::{
    ArtifactCommitRequest, ArtifactCommitResponse, ArtifactOpenRequest, ArtifactStream,
    ArtifactWriteMetadata, MAX_ARTIFACT_COMMIT_RESPONSE_BYTES,
};
pub use camera_http::CameraHttpClient;
pub use camera_media::{MediaChunk, MediaHeartbeat, MediaOpenRequest, MediaStream};
pub use camera_plugin::{
    CameraPluginConfig, CameraStreamingConfig, default_camera_plugin_config,
    default_camera_streaming_config, load_camera_plugin_config, load_camera_streaming_config,
};
pub use camera_relay::{CameraRelayConfig, with_url_user_info};
pub use check_descriptor::{
    CheckDescriptor, RESULT_SCHEMA_TARGET_CHECK_V1, TARGET_KIND_DEVICE, TARGET_KIND_SERVICE,
};
pub use config::{get_config, get_config_bytes, load_config, load_config_or_default};
pub use device_discovery::{
    DEVICE_DISCOVERY_SCHEMA_V1, DeviceDiscovery, DeviceLocation, DiscoveredDevice,
};
pub use error::{
    Error, HOST_ERR_BAD_HANDLE, HOST_ERR_DENIED, HOST_ERR_INTERNAL, HOST_ERR_INVALID,
    HOST_ERR_NOT_FOUND, HOST_ERR_OK, HOST_ERR_TIMEOUT, HOST_ERR_TOO_LARGE, HostError,
    HostErrorCode, SdkResult, host_error,
};
pub use execute::{ExecuteErrorWithResult, execute, execute_partial, submit_result_payload};
pub use http::{HttpClient, HttpRequest, HttpResponse, MAX_HTTP_RESPONSE_BYTES};
pub use log::{LOG, LogLevel, Logger};
pub use manifest::{
    ManifestValidationError, OUTPUTS_CAMERA_STREAM, OUTPUTS_PLUGIN_RESULT, OUTPUTS_PROXMOX_CONSOLE,
    PluginManifest, RUNTIME_NONE, RUNTIME_WASI_PREVIEW1, SignalSchemaContribution,
};
pub use memory::{alloc, dealloc};
pub use metric_envelope::{
    METRIC_ENVELOPE_SCHEMA_VERSION, Metric, MetricBatch, MetricIngestIdentity, MetricKind,
    MetricPoint, MetricResource, MetricStringMapEntry, MetricTemporality, MetricValueType,
    marshal_metric_batch,
};
pub use plugin_inputs::{
    CredentialBrokerGrant, CredentialPolicySnapshot, PLUGIN_INPUTS_SCHEMA_V1, PluginInput,
    PluginInputItem, PluginInputItems, PluginInputsPayload, TargetContext,
    parse_plugin_inputs_json, parse_plugin_inputs_map,
};
pub use producer_schedule::{
    CAPABILITY_PRODUCER_SCHEDULE_V1, PRODUCER_SCHEDULE_COMMAND_PLUGIN_RUN_ACTION,
    PRODUCER_SCHEDULE_DISPATCH_ASSIGNMENT, PRODUCER_SCHEDULE_DISPATCH_PACKAGE,
    PRODUCER_SCHEDULE_DISPATCH_TARGET_QUERY, PRODUCER_SCHEDULE_RUN_SCHEMA_V1,
    PRODUCER_SCHEDULE_TYPE_CRON, PRODUCER_SCHEDULE_TYPE_INTERVAL, PRODUCER_SCHEDULE_TYPE_MANUAL,
    ProducerScheduleContract,
};
pub use result::{
    Event, Result as PluginResult, SIGNAL_SCHEMA_METADATA_DISPLAY_CONTRACT,
    SIGNAL_SCHEMA_METADATA_DISPLAY_CONTRACT_ID, SIGNAL_SCHEMA_METADATA_DISPLAY_CONTRACT_VERSION,
    SIGNAL_SCHEMA_METADATA_PAYLOAD_KIND, SIGNAL_SCHEMA_METADATA_PRODUCER_ID,
    SIGNAL_SCHEMA_METADATA_PRODUCER_VERSION, SIGNAL_SCHEMA_METADATA_SCHEMA_ID,
    SIGNAL_SCHEMA_METADATA_SCHEMA_VERSION, SIGNAL_SCHEMA_METADATA_SERVICE_RADAR,
    SIGNAL_SCHEMA_METADATA_SIGNAL_SCHEMA, SIGNAL_SCHEMA_METADATA_SIGNAL_TYPE,
    SIGNAL_SCHEMA_PAYLOAD_KIND_OCSF_EVENT, SIGNAL_SCHEMA_PAYLOAD_KIND_OTEL_LOG,
    SIGNAL_SCHEMA_SIGNAL_TYPE_EVENT, SIGNAL_SCHEMA_SIGNAL_TYPE_LOG, Severity, SignalSchemaRef,
    Status, Thresholds, Widget, attach_signal_schema_ref,
};
pub use rtsp::RtspTransport as StreamTransport;
pub use rtsp::{
    AuthChallenge, H264Depacketizer, InterleavedFrame, StreamClient, StreamEndpoint,
    StreamResponse, VideoTrack,
};
pub use tcp::{TcpConnection, tcp_dial};
pub use telemetry::{TelemetryBatch, TelemetryRecord, TelemetrySource, emit_telemetry};
pub use udp::udp_send_to;
pub use websocket::{
    WebSocketConnection, WebSocketDialRequest, encode_websocket_connect_payload,
    encode_websocket_dial_request, websocket_connect, websocket_connect_with_headers,
    websocket_dial, websocket_dial_request, websocket_dial_request_with_insecure_tls,
    websocket_dial_with_headers,
};
