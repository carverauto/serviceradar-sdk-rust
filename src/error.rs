use std::fmt::{Display, Formatter};

pub type SdkResult<T> = std::result::Result<T, Error>;

pub const HOST_ERR_OK: i32 = 0;
pub const HOST_ERR_INVALID: i32 = -1;
pub const HOST_ERR_DENIED: i32 = -2;
pub const HOST_ERR_TOO_LARGE: i32 = -3;
pub const HOST_ERR_NOT_FOUND: i32 = -4;
pub const HOST_ERR_INTERNAL: i32 = -5;
pub const HOST_ERR_TIMEOUT: i32 = -6;
pub const HOST_ERR_BAD_HANDLE: i32 = -7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostErrorCode {
    Invalid,
    Denied,
    TooLarge,
    NotFound,
    Internal,
    Timeout,
    BadHandle,
    Unknown(i32),
}

impl From<i32> for HostErrorCode {
    fn from(value: i32) -> Self {
        match value {
            HOST_ERR_INVALID => Self::Invalid,
            HOST_ERR_DENIED => Self::Denied,
            HOST_ERR_TOO_LARGE => Self::TooLarge,
            HOST_ERR_NOT_FOUND => Self::NotFound,
            HOST_ERR_INTERNAL => Self::Internal,
            HOST_ERR_TIMEOUT => Self::Timeout,
            HOST_ERR_BAD_HANDLE => Self::BadHandle,
            other => Self::Unknown(other),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostError {
    pub code: i32,
    pub op: &'static str,
}

impl Display for HostError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.op.is_empty() {
            write!(f, "host error {}", self.code)
        } else {
            write!(f, "host error {} ({})", self.code, self.op)
        }
    }
}

impl std::error::Error for HostError {}

pub fn host_error(code: i32, op: &'static str) -> std::result::Result<(), HostError> {
    if code >= 0 {
        Ok(())
    } else {
        Err(HostError { code, op })
    }
}

#[derive(Debug)]
pub enum Error {
    Host(HostError),
    Json(serde_json::Error),
    Url(url::ParseError),
    TimeFormat(time::error::Format),
    Io(std::io::Error),
    Message(String),
    TcpConnectionNotInitialized,
    WebSocketNotInitialized,
    CameraMediaStreamNotInitialized,
    CameraHostRequired,
    InvalidCameraScheme,
    InvalidPluginInputs(String),
    RtspInvalidUrl,
    RtspNoVideoTrack,
    RtspBadResponse,
    RtspBadInterleaved,
    RtpPacketTooShort,
    H264PayloadTooShort,
    H264UnsupportedNal,
    RtspNoSession,
    RtspUnauthorized,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Host(err) => Display::fmt(err, f),
            Self::Json(err) => Display::fmt(err, f),
            Self::Url(err) => Display::fmt(err, f),
            Self::TimeFormat(err) => Display::fmt(err, f),
            Self::Io(err) => Display::fmt(err, f),
            Self::Message(msg) => f.write_str(msg),
            Self::TcpConnectionNotInitialized => f.write_str("tcp connection not initialized"),
            Self::WebSocketNotInitialized => f.write_str("websocket connection not initialized"),
            Self::CameraMediaStreamNotInitialized => {
                f.write_str("camera media stream not initialized")
            }
            Self::CameraHostRequired => f.write_str("host is required"),
            Self::InvalidCameraScheme => f.write_str("scheme must be http or https"),
            Self::InvalidPluginInputs(msg) => write!(f, "invalid plugin inputs payload: {msg}"),
            Self::RtspInvalidUrl => f.write_str("invalid rtsp source url"),
            Self::RtspNoVideoTrack => f.write_str("no h264 video track in sdp"),
            Self::RtspBadResponse => f.write_str("invalid rtsp response"),
            Self::RtspBadInterleaved => f.write_str("invalid interleaved frame"),
            Self::RtpPacketTooShort => f.write_str("rtp packet too short"),
            Self::H264PayloadTooShort => f.write_str("h264 payload too short"),
            Self::H264UnsupportedNal => f.write_str("unsupported h264 packetization"),
            Self::RtspNoSession => f.write_str("rtsp session header missing"),
            Self::RtspUnauthorized => f.write_str("rtsp unauthorized"),
        }
    }
}

impl std::error::Error for Error {}

impl From<HostError> for Error {
    fn from(value: HostError) -> Self {
        Self::Host(value)
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<url::ParseError> for Error {
    fn from(value: url::ParseError) -> Self {
        Self::Url(value)
    }
}

impl From<time::error::Format> for Error {
    fn from(value: time::error::Format) -> Self {
        Self::TimeFormat(value)
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}
