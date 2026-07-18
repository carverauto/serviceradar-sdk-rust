use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, HOST_ERR_TOO_LARGE, HostError, SdkResult};
use crate::host;

pub const CAPABILITY_ARTIFACT_STAGING_V1: &str = "artifact-staging:v1";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ArtifactOpenRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub artifact_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
}

impl ArtifactOpenRequest {
    pub fn new(object_key: impl Into<String>) -> Self {
        Self {
            object_key: Some(object_key.into()),
            ..Self::default()
        }
    }

    pub fn with_type(mut self, artifact_type: impl Into<String>) -> Self {
        self.artifact_type = Some(artifact_type.into());
        self
    }

    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    pub fn with_sha256(mut self, sha256: impl Into<String>) -> Self {
        self.sha256 = Some(sha256.into());
        self
    }

    pub fn with_size_bytes(mut self, size_bytes: u64) -> Self {
        self.size_bytes = Some(size_bytes);
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ArtifactChunkMetadata {
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub sequence: u64,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub offset: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default)]
    pub is_final: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
}

impl ArtifactChunkMetadata {
    pub fn new(sequence: u64) -> Self {
        Self {
            sequence,
            ..Self::default()
        }
    }

    pub fn with_offset(mut self, offset: u64) -> Self {
        self.offset = offset;
        self
    }

    pub fn with_final_chunk(mut self) -> Self {
        self.is_final = true;
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ArtifactCommitRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
}

impl ArtifactCommitRequest {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_sha256(mut self, sha256: impl Into<String>) -> Self {
        self.sha256 = Some(sha256.into());
        self
    }

    pub fn with_size_bytes(mut self, size_bytes: u64) -> Self {
        self.size_bytes = Some(size_bytes);
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ArtifactCommitResponse {
    pub object_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_backend: Option<String>,
    #[serde(default)]
    pub accepted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
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

    pub fn write(&self, metadata: ArtifactChunkMetadata, payload: &[u8]) -> SdkResult<()> {
        if self.handle == 0 {
            return Err(Error::ArtifactStreamNotInitialized);
        }

        let metadata = serde_json::to_vec(&metadata)?;
        let res = host::artifact_write(self.handle, &metadata, payload);
        if res < 0 {
            return Err(HostError {
                code: res,
                op: "artifact_write",
            }
            .into());
        }

        Ok(())
    }

    pub fn commit(&mut self, request: ArtifactCommitRequest) -> SdkResult<ArtifactCommitResponse> {
        if self.handle == 0 {
            return Err(Error::ArtifactStreamNotInitialized);
        }

        let request = serde_json::to_vec(&request)?;

        for size in [4096usize, 16384, 65536] {
            let mut response = vec![0u8; size];
            let res = host::artifact_commit(self.handle, &request, &mut response);
            if res == HOST_ERR_TOO_LARGE {
                continue;
            }
            if res < 0 {
                return Err(HostError {
                    code: res,
                    op: "artifact_commit",
                }
                .into());
            }

            self.handle = 0;
            response.truncate(res as usize);
            return Ok(serde_json::from_slice(&response)?);
        }

        Err(HostError {
            code: HOST_ERR_TOO_LARGE,
            op: "artifact_commit",
        }
        .into())
    }

    pub fn abort(&mut self, reason: impl AsRef<str>) -> SdkResult<()> {
        if self.handle == 0 {
            return Ok(());
        }

        let handle = self.handle;
        self.handle = 0;
        let res = host::artifact_abort(handle, reason.as_ref().as_bytes());
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

fn is_zero_u64(value: &u64) -> bool {
    *value == 0
}

#[cfg(test)]
mod tests;
