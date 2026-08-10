use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::error::HOST_ERR_NOT_FOUND;
use crate::host::{TestHostBackend, install_test_backend};
use crate::{Error, HostError};

use super::{ArtifactCommitRequest, ArtifactOpenRequest, ArtifactStream, ArtifactWriteMetadata};

#[derive(Default)]
struct ArtifactState {
    opens: Vec<Value>,
    writes: Vec<(u32, Value, Vec<u8>)>,
    commits: Vec<(u32, Value)>,
    aborts: Vec<u32>,
}

struct ArtifactHost {
    state: Arc<Mutex<ArtifactState>>,
}

impl TestHostBackend for ArtifactHost {
    fn artifact_open(&mut self, req: &[u8]) -> i32 {
        let payload: Value = serde_json::from_slice(req).expect("decode open payload");
        self.state
            .lock()
            .expect("artifact state")
            .opens
            .push(payload);
        7
    }

    fn artifact_write(&mut self, handle: u32, meta: &[u8], payload: &[u8]) -> i32 {
        let metadata: Value = serde_json::from_slice(meta).expect("decode write metadata");
        self.state.lock().expect("artifact state").writes.push((
            handle,
            metadata,
            payload.to_vec(),
        ));
        payload.len() as i32
    }

    fn artifact_commit(&mut self, handle: u32, req: &[u8], resp: &mut [u8]) -> i32 {
        let request: Value = serde_json::from_slice(req).expect("decode commit payload");
        self.state
            .lock()
            .expect("artifact state")
            .commits
            .push((handle, request));

        let response = br#"{"object_key":"feeds/nvd/snapshot.zip","content_type":"application/zip","sha256":"abc123","size_bytes":5,"attributes":{"feed":"nvd"}}"#;
        resp[..response.len()].copy_from_slice(response);
        response.len() as i32
    }

    fn artifact_abort(&mut self, handle: u32) -> i32 {
        self.state
            .lock()
            .expect("artifact state")
            .aborts
            .push(handle);
        0
    }
}

struct MissingArtifactHost;

impl TestHostBackend for MissingArtifactHost {}

#[test]
fn artifact_requests_serialize_expected_shapes() {
    let open = ArtifactOpenRequest::new("feeds/nvd/snapshot.zip")
        .with_content_type("application/zip")
        .with_sha256("abc123")
        .with_size_bytes(5)
        .with_attribute("feed", "nvd");

    let open_payload: Value =
        serde_json::from_slice(&serde_json::to_vec(&open).expect("serialize open request"))
            .expect("decode open request");
    assert_eq!(open_payload["object_key"], "feeds/nvd/snapshot.zip");
    assert_eq!(open_payload["content_type"], "application/zip");
    assert_eq!(open_payload["sha256"], "abc123");
    assert_eq!(open_payload["size_bytes"], 5);
    assert_eq!(open_payload["attributes"]["feed"], "nvd");

    let write = ArtifactWriteMetadata::indexed(3).final_chunk();
    let write_payload: Value =
        serde_json::from_slice(&serde_json::to_vec(&write).expect("serialize write metadata"))
            .expect("decode write metadata");
    assert_eq!(write_payload["index"], 3);
    assert_eq!(write_payload["final"], true);
    assert!(write_payload.get("final_chunk").is_none());
}

#[test]
fn artifact_stream_uses_host_lifecycle() {
    let state = Arc::new(Mutex::new(ArtifactState::default()));
    let _guard = install_test_backend(Box::new(ArtifactHost {
        state: Arc::clone(&state),
    }));

    let mut stream = ArtifactStream::open(
        ArtifactOpenRequest::new("feeds/nvd/snapshot.zip")
            .with_content_type("application/zip")
            .with_attribute("feed", "nvd"),
    )
    .expect("open artifact stream");
    assert!(stream.is_open());
    assert_eq!(stream.handle(), Some(7));

    let written = stream
        .write_chunk(ArtifactWriteMetadata::indexed(0).final_chunk(), b"chunk")
        .expect("write artifact chunk");
    assert_eq!(written, 5);

    let response = stream
        .commit(
            ArtifactCommitRequest::new()
                .with_sha256("abc123")
                .with_size_bytes(5),
        )
        .expect("commit artifact stream");
    assert!(!stream.is_open());
    assert_eq!(response.object_key, "feeds/nvd/snapshot.zip");
    assert_eq!(response.content_type, "application/zip");
    assert_eq!(response.sha256, "abc123");
    assert_eq!(response.size_bytes, 5);
    assert_eq!(response.attributes["feed"], "nvd");

    let state = state.lock().expect("artifact state");
    assert_eq!(state.opens.len(), 1);
    assert_eq!(state.opens[0]["object_key"], "feeds/nvd/snapshot.zip");
    assert_eq!(state.writes.len(), 1);
    assert_eq!(state.writes[0].0, 7);
    assert_eq!(state.writes[0].1["index"], 0);
    assert_eq!(state.writes[0].1["final"], true);
    assert_eq!(state.writes[0].2, b"chunk");
    assert_eq!(state.commits.len(), 1);
    assert_eq!(state.commits[0].0, 7);
    assert_eq!(state.commits[0].1["sha256"], "abc123");
    assert_eq!(state.commits[0].1["size_bytes"], 5);
    assert!(state.aborts.is_empty());
}

#[test]
fn artifact_stream_can_abort_open_handle() {
    let state = Arc::new(Mutex::new(ArtifactState::default()));
    let _guard = install_test_backend(Box::new(ArtifactHost {
        state: Arc::clone(&state),
    }));

    let mut stream =
        ArtifactStream::open(ArtifactOpenRequest::new("tmp/artifact")).expect("open artifact");
    stream.abort().expect("abort artifact stream");
    assert!(!stream.is_open());

    let state = state.lock().expect("artifact state");
    assert_eq!(state.aborts, vec![7]);
}

#[test]
fn artifact_stream_reports_missing_host() {
    let _guard = install_test_backend(Box::new(MissingArtifactHost));

    let err = ArtifactStream::open(ArtifactOpenRequest::new("tmp/artifact"))
        .expect_err("missing host should fail");
    assert!(matches!(
        err,
        Error::Host(HostError {
            code: HOST_ERR_NOT_FOUND,
            op: "artifact_open"
        })
    ));
}

#[test]
fn artifact_stream_rejects_uninitialized_use() {
    let mut stream = ArtifactStream::default();

    let err = stream.write(b"chunk").expect_err("write should fail");
    assert!(matches!(err, Error::ArtifactStreamNotInitialized));

    let err = stream
        .commit(ArtifactCommitRequest::new())
        .expect_err("commit should fail");
    assert!(matches!(err, Error::ArtifactStreamNotInitialized));

    let err = stream.abort().expect_err("abort should fail");
    assert!(matches!(err, Error::ArtifactStreamNotInitialized));
}
