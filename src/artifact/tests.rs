use std::sync::{Arc, Mutex};

use crate::error::{Error, HOST_ERR_NOT_FOUND};
use crate::host::{TestHostBackend, install_test_backend};

use super::{
    ArtifactChunkMetadata, ArtifactCommitRequest, ArtifactCommitResponse, ArtifactOpenRequest,
    ArtifactStream,
};

#[derive(Default)]
struct ArtifactState {
    open_request: Vec<u8>,
    chunks: Vec<Vec<u8>>,
    commit_request: Vec<u8>,
    abort_reason: Vec<u8>,
}

struct ArtifactTestHost {
    state: Arc<Mutex<ArtifactState>>,
}

impl TestHostBackend for ArtifactTestHost {
    fn artifact_open(&mut self, req: &[u8]) -> i32 {
        self.state.lock().unwrap().open_request = req.to_vec();
        17
    }

    fn artifact_write(&mut self, _handle: u32, _meta: &[u8], payload: &[u8]) -> i32 {
        self.state.lock().unwrap().chunks.push(payload.to_vec());
        0
    }

    fn artifact_commit(&mut self, _handle: u32, req: &[u8], resp: &mut [u8]) -> i32 {
        self.state.lock().unwrap().commit_request = req.to_vec();
        let payload = serde_json::to_vec(&ArtifactCommitResponse {
            object_key: "vulnerability-feeds/example/latest.zip".to_string(),
            sha256: Some("abc123".to_string()),
            size_bytes: Some(5),
            storage_backend: Some("agent-gateway".to_string()),
            accepted: true,
            status: Some("accepted".to_string()),
            ..ArtifactCommitResponse::default()
        })
        .unwrap();
        resp[..payload.len()].copy_from_slice(&payload);
        payload.len() as i32
    }

    fn artifact_abort(&mut self, _handle: u32, reason: &[u8]) -> i32 {
        self.state.lock().unwrap().abort_reason = reason.to_vec();
        0
    }
}

#[test]
fn artifact_stream_requires_handle() {
    let mut stream = ArtifactStream::default();
    let err = stream
        .write(ArtifactChunkMetadata::default(), b"payload")
        .unwrap_err();
    assert!(matches!(err, Error::ArtifactStreamNotInitialized));

    let err = stream.commit(ArtifactCommitRequest::default()).unwrap_err();
    assert!(matches!(err, Error::ArtifactStreamNotInitialized));
}

#[test]
fn artifact_stream_uses_host_lifecycle() {
    let state = Arc::new(Mutex::new(ArtifactState::default()));
    let _guard = install_test_backend(Box::new(ArtifactTestHost {
        state: Arc::clone(&state),
    }));

    let mut stream = ArtifactStream::open(
        ArtifactOpenRequest::new("vulnerability-feeds/example/latest.zip")
            .with_type("advisory-feed-snapshot")
            .with_content_type("application/zip")
            .with_sha256("abc123"),
    )
    .unwrap();

    assert_eq!(stream.handle(), Some(17));
    stream
        .write(ArtifactChunkMetadata::new(1), b"hello")
        .unwrap();
    let response = stream
        .commit(
            ArtifactCommitRequest::new()
                .with_sha256("abc123")
                .with_size_bytes(5),
        )
        .unwrap();

    assert!(!stream.is_open());
    assert_eq!(
        response.object_key,
        "vulnerability-feeds/example/latest.zip"
    );
    assert_eq!(response.storage_backend.as_deref(), Some("agent-gateway"));
    assert!(response.accepted);

    let state = state.lock().unwrap();
    assert!(!state.open_request.is_empty());
    assert_eq!(state.chunks, vec![b"hello".to_vec()]);
    assert!(!state.commit_request.is_empty());
}

#[test]
fn artifact_stream_reports_host_error_without_runtime() {
    struct MissingArtifactHost;
    impl TestHostBackend for MissingArtifactHost {}

    let _guard = install_test_backend(Box::new(MissingArtifactHost));
    let err = ArtifactStream::open(ArtifactOpenRequest::new("objects/example")).unwrap_err();
    match err {
        Error::Host(host_err) => {
            assert_eq!(host_err.code, HOST_ERR_NOT_FOUND);
            assert_eq!(host_err.op, "artifact_open");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
