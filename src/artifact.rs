#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{Error, HostError, SdkResult};
use crate::host;

pub const MAX_ARTIFACT_COMMIT_RESPONSE_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactOpenRequest {
    pub object_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<i64>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub attributes: BTreeMap<String, String>,
}

impl ArtifactOpenRequest {
    pub fn new(object_key: impl Into<String>) -> Self {
        Self {
            object_key: object_key.into(),
            ..Self::default()
        }
    }

    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    pub fn with_sha256(mut self, sha256: impl Into<String>) -> Self {
        self.sha256 = Some(sha256.into());
        self
    }

    pub fn with_size_bytes(mut self, size_bytes: i64) -> Self {
        self.size_bytes = Some(size_bytes);
        self
    }

    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactWriteMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<i64>,
    #[serde(rename = "final", skip_serializing_if = "is_false")]
    pub final_chunk: bool,
}

impl ArtifactWriteMetadata {
    pub fn indexed(index: i64) -> Self {
        Self {
            index: Some(index),
            final_chunk: false,
        }
    }

    pub fn final_chunk(mut self) -> Self {
        self.final_chunk = true;
        self
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactCommitRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<i64>,
}

impl ArtifactCommitRequest {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_sha256(mut self, sha256: impl Into<String>) -> Self {
        self.sha256 = Some(sha256.into());
        self
    }

    pub fn with_size_bytes(mut self, size_bytes: i64) -> Self {
        self.size_bytes = Some(size_bytes);
        self
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactCommitResponse {
    pub object_key: String,
    #[serde(default)]
    pub content_type: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub size_bytes: i64,
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ArtifactStream {
    handle: u32,
}

impl ArtifactStream {
    pub fn open(request: ArtifactOpenRequest) -> SdkResult<Self> {
        let payload = serde_json::to_vec(&request)?;
        let res = host::artifact_open(&payload);
        if res < 0 {
            return Err(HostError {
                code: res,
                op: "artifact_open",
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

    pub fn write(&self, payload: &[u8]) -> SdkResult<usize> {
        self.write_chunk(ArtifactWriteMetadata::default(), payload)
    }

    pub fn write_chunk(&self, metadata: ArtifactWriteMetadata, payload: &[u8]) -> SdkResult<usize> {
        if self.handle == 0 {
            return Err(Error::ArtifactStreamNotInitialized);
        }

        let meta = serde_json::to_vec(&metadata)?;
        let res = host::artifact_write(self.handle, &meta, payload);
        if res < 0 {
            return Err(HostError {
                code: res,
                op: "artifact_write",
            }
            .into());
        }

        Ok(res as usize)
    }

    pub fn commit(&mut self, request: ArtifactCommitRequest) -> SdkResult<ArtifactCommitResponse> {
        if self.handle == 0 {
            return Err(Error::ArtifactStreamNotInitialized);
        }

        let payload = serde_json::to_vec(&request)?;
        let mut response = vec![0_u8; MAX_ARTIFACT_COMMIT_RESPONSE_BYTES];
        let res = host::artifact_commit(self.handle, &payload, &mut response);
        if res < 0 {
            return Err(HostError {
                code: res,
                op: "artifact_commit",
            }
            .into());
        }
        self.handle = 0;

        Ok(serde_json::from_slice(&response[..res as usize])?)
    }

    pub fn abort(&mut self) -> SdkResult<()> {
        if self.handle == 0 {
            return Err(Error::ArtifactStreamNotInitialized);
        }

        let res = host::artifact_abort(self.handle);
        if res >= 0 {
            self.handle = 0;
        }
        if res < 0 {
            return Err(HostError {
                code: res,
                op: "artifact_abort",
            }
            .into());
        }

        Ok(())
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}
