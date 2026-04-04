use serde::{Deserialize, Serialize};

use crate::error::{Error, HostError, SdkResult};
use crate::host;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CameraMediaOpenRequest {
    pub track_id: String,
    pub codec: String,
    pub payload_format: String,
}

impl CameraMediaOpenRequest {
    pub fn new(track_id: impl Into<String>) -> Self {
        Self {
            track_id: track_id.into(),
            ..Self::default()
        }
    }

    pub fn with_codec(mut self, codec: impl Into<String>) -> Self {
        self.codec = codec.into();
        self
    }

    pub fn with_payload_format(mut self, payload_format: impl Into<String>) -> Self {
        self.payload_format = payload_format.into();
        self
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CameraMediaChunkMetadata {
    pub track_id: String,
    pub sequence: u64,
    pub pts: i64,
    pub dts: i64,
    pub keyframe: bool,
    pub is_final: bool,
    pub codec: String,
    pub payload_format: String,
}

impl CameraMediaChunkMetadata {
    pub fn frame(sequence: u64) -> Self {
        Self {
            sequence,
            ..Self::default()
        }
    }

    pub fn with_track(mut self, track_id: impl Into<String>) -> Self {
        self.track_id = track_id.into();
        self
    }

    pub fn with_timestamps(mut self, pts: i64, dts: i64) -> Self {
        self.pts = pts;
        self.dts = dts;
        self
    }

    pub fn with_codec(mut self, codec: impl Into<String>) -> Self {
        self.codec = codec.into();
        self
    }

    pub fn with_payload_format(mut self, payload_format: impl Into<String>) -> Self {
        self.payload_format = payload_format.into();
        self
    }

    pub fn with_keyframe(mut self) -> Self {
        self.keyframe = true;
        self
    }

    pub fn with_final_chunk(mut self) -> Self {
        self.is_final = true;
        self
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CameraMediaHeartbeat {
    pub sequence: u64,
    pub timestamp_unix: i64,
}

impl CameraMediaHeartbeat {
    pub fn new(sequence: u64, timestamp_unix: i64) -> Self {
        Self {
            sequence,
            timestamp_unix,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CameraMediaStream {
    handle: u32,
}

pub type MediaOpenRequest = CameraMediaOpenRequest;
pub type MediaChunk = CameraMediaChunkMetadata;
pub type MediaHeartbeat = CameraMediaHeartbeat;
pub type MediaStream = CameraMediaStream;

impl CameraMediaStream {
    pub fn open(request: CameraMediaOpenRequest) -> SdkResult<Self> {
        let payload = serde_json::to_vec(&request)?;
        let res = host::camera_media_open(&payload);
        if res < 0 {
            return Err(HostError {
                code: res,
                op: "camera_media_open",
            }
            .into());
        }

        Ok(Self { handle: res as u32 })
    }

    pub fn is_open(&self) -> bool {
        self.handle != 0
    }

    pub fn handle(&self) -> Option<u32> {
        (self.handle != 0).then_some(self.handle)
    }

    pub fn write(&self, metadata: CameraMediaChunkMetadata, payload: &[u8]) -> SdkResult<()> {
        if self.handle == 0 {
            return Err(Error::CameraMediaStreamNotInitialized);
        }

        let metadata = serde_json::to_vec(&metadata)?;
        let res = host::camera_media_write(self.handle, &metadata, payload);
        if res < 0 {
            return Err(HostError {
                code: res,
                op: "camera_media_write",
            }
            .into());
        }

        Ok(())
    }

    pub fn heartbeat(&self, heartbeat: CameraMediaHeartbeat) -> SdkResult<()> {
        if self.handle == 0 {
            return Err(Error::CameraMediaStreamNotInitialized);
        }

        let metadata = serde_json::to_vec(&heartbeat)?;
        let res = host::camera_media_heartbeat(self.handle, &metadata);
        if res < 0 {
            return Err(HostError {
                code: res,
                op: "camera_media_heartbeat",
            }
            .into());
        }

        Ok(())
    }

    pub fn close(&mut self, reason: impl AsRef<str>) -> SdkResult<()> {
        if self.handle == 0 {
            return Ok(());
        }

        let handle = self.handle;
        self.handle = 0;
        let res = host::camera_media_close(handle, reason.as_ref().as_bytes());
        if res < 0 {
            return Err(HostError {
                code: res,
                op: "camera_media_close",
            }
            .into());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests;
