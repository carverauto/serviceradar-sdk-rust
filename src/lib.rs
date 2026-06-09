mod camera_http;
mod camera_media;
mod camera_plugin;
mod camera_relay;
mod check_descriptor;
mod config;
mod error;
mod execute;
mod host;
mod http;
mod log;
mod memory;
mod plugin_inputs;
mod result;
mod rtsp;
mod tcp;
mod udp;
mod websocket;

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
pub use error::{
    Error, HOST_ERR_BAD_HANDLE, HOST_ERR_DENIED, HOST_ERR_INTERNAL, HOST_ERR_INVALID,
    HOST_ERR_NOT_FOUND, HOST_ERR_OK, HOST_ERR_TIMEOUT, HOST_ERR_TOO_LARGE, HostError,
    HostErrorCode, SdkResult, host_error,
};
pub use execute::{ExecuteErrorWithResult, execute, execute_partial, submit_result_payload};
pub use http::{HttpClient, HttpRequest, HttpResponse, MAX_HTTP_RESPONSE_BYTES};
pub use log::{LOG, LogLevel, Logger};
pub use memory::{alloc, dealloc};
pub use plugin_inputs::{
    CredentialBrokerGrant, CredentialPolicySnapshot, PLUGIN_INPUTS_SCHEMA_V1, PluginInput,
    PluginInputItem, PluginInputItems, PluginInputsPayload, TargetContext,
    parse_plugin_inputs_json, parse_plugin_inputs_map,
};
pub use result::{
    Event, Metric, Result as PluginResult, SIGNAL_SCHEMA_METADATA_DISPLAY_CONTRACT,
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
pub use udp::udp_send_to;
pub use websocket::{
    WebSocketConnection, WebSocketDialRequest, encode_websocket_connect_payload,
    encode_websocket_dial_request, websocket_connect, websocket_connect_with_headers,
    websocket_dial, websocket_dial_request, websocket_dial_request_with_insecure_tls,
    websocket_dial_with_headers,
};
