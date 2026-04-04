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
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::Error;
    use crate::host::{TestHostBackend, install_test_backend};

    use super::{
        CameraMediaChunkMetadata, CameraMediaHeartbeat, CameraMediaOpenRequest, CameraMediaStream,
    };

    #[derive(Default)]
    struct CameraMediaState {
        opens: usize,
        writes: usize,
        heartbeats: usize,
        closes: usize,
    }

    struct CameraMediaTestHost {
        state: Arc<Mutex<CameraMediaState>>,
    }

    impl TestHostBackend for CameraMediaTestHost {
        fn camera_media_open(&mut self, req: &[u8]) -> i32 {
            let payload: serde_json::Value =
                serde_json::from_slice(req).expect("decode open payload");
            assert_eq!(payload["track_id"], "video-main");
            self.state.lock().expect("state mutex").opens += 1;
            7
        }

        fn camera_media_write(&mut self, handle: u32, meta: &[u8], payload: &[u8]) -> i32 {
            assert_eq!(handle, 7);
            let metadata: serde_json::Value =
                serde_json::from_slice(meta).expect("decode metadata");
            assert_eq!(metadata["sequence"], 1);
            assert_eq!(payload, b"frame");
            self.state.lock().expect("state mutex").writes += 1;
            0
        }

        fn camera_media_heartbeat(&mut self, handle: u32, meta: &[u8]) -> i32 {
            assert_eq!(handle, 7);
            let metadata: serde_json::Value =
                serde_json::from_slice(meta).expect("decode heartbeat");
            assert_eq!(metadata["sequence"], 1);
            self.state.lock().expect("state mutex").heartbeats += 1;
            0
        }

        fn camera_media_close(&mut self, handle: u32, reason: &[u8]) -> i32 {
            assert_eq!(handle, 7);
            assert_eq!(reason, b"done");
            self.state.lock().expect("state mutex").closes += 1;
            0
        }
    }

    #[test]
    fn camera_media_requires_handle() {
        let stream = CameraMediaStream::default();
        let err = stream
            .write(CameraMediaChunkMetadata::default(), b"frame")
            .expect_err("write should fail");
        assert!(matches!(err, Error::CameraMediaStreamNotInitialized));

        let err = stream
            .heartbeat(CameraMediaHeartbeat::default())
            .expect_err("heartbeat should fail");
        assert!(matches!(err, Error::CameraMediaStreamNotInitialized));
    }

    #[test]
    fn camera_media_stream_uses_host_lifecycle() {
        let state = Arc::new(Mutex::new(CameraMediaState::default()));
        let _guard = install_test_backend(Box::new(CameraMediaTestHost {
            state: Arc::clone(&state),
        }));

        let mut stream = CameraMediaStream::open(CameraMediaOpenRequest {
            track_id: "video-main".to_string(),
            codec: "h264".to_string(),
            payload_format: "annexb".to_string(),
        })
        .expect("open camera media stream");
        assert!(stream.is_open());
        assert_eq!(stream.handle(), Some(7));

        stream
            .write(
                CameraMediaChunkMetadata {
                    sequence: 1,
                    ..CameraMediaChunkMetadata::default()
                },
                b"frame",
            )
            .expect("write frame");
        stream
            .heartbeat(CameraMediaHeartbeat {
                sequence: 1,
                timestamp_unix: 123,
            })
            .expect("heartbeat");
        stream.close("done").expect("close stream");

        let state = state.lock().expect("state mutex");
        assert_eq!(state.opens, 1);
        assert_eq!(state.writes, 1);
        assert_eq!(state.heartbeats, 1);
        assert_eq!(state.closes, 1);
    }

    #[test]
    fn camera_media_domain_builders_construct_expected_shapes() {
        let request = CameraMediaOpenRequest::new("video-main")
            .with_codec("h264")
            .with_payload_format("annexb");
        assert_eq!(request.track_id, "video-main");
        assert_eq!(request.codec, "h264");

        let chunk = CameraMediaChunkMetadata::frame(1)
            .with_track("video-main")
            .with_timestamps(100, 90)
            .with_codec("h264")
            .with_payload_format("annexb")
            .with_keyframe()
            .with_final_chunk();
        assert_eq!(chunk.sequence, 1);
        assert!(chunk.keyframe);
        assert!(chunk.is_final);

        let heartbeat = CameraMediaHeartbeat::new(2, 123);
        assert_eq!(heartbeat.sequence, 2);
        assert_eq!(heartbeat.timestamp_unix, 123);
    }
}
