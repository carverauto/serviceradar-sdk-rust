use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::SdkResult;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct CameraPluginConfig {
    pub host: String,
    pub scheme: String,
    pub username: String,
    pub password: String,
    pub timeout: String,
    pub insecure_skip_verify: bool,
    pub discover_streams: bool,
    pub collect_events: bool,
    pub event_sources: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct CameraStreamingConfig {
    #[serde(flatten)]
    pub camera: CameraPluginConfig,
    pub relay: crate::CameraRelayConfig,
}

pub fn default_camera_plugin_config() -> CameraPluginConfig {
    CameraPluginConfig {
        host: String::new(),
        scheme: "http".to_string(),
        username: String::new(),
        password: String::new(),
        timeout: "10s".to_string(),
        insecure_skip_verify: false,
        discover_streams: true,
        collect_events: false,
        event_sources: "events".to_string(),
    }
}

pub fn default_camera_streaming_config() -> CameraStreamingConfig {
    CameraStreamingConfig {
        camera: default_camera_plugin_config(),
        relay: crate::CameraRelayConfig::default(),
    }
}

pub fn load_camera_plugin_config() -> SdkResult<CameraPluginConfig> {
    crate::load_config_or_default()
}

pub fn load_camera_streaming_config() -> SdkResult<CameraStreamingConfig> {
    crate::load_config_or_default()
}

impl CameraPluginConfig {
    pub fn normalized_scheme(&self) -> SdkResult<String> {
        let scheme = if self.scheme.trim().is_empty() {
            "http".to_string()
        } else {
            self.scheme.trim().to_ascii_lowercase()
        };

        if scheme == "http" || scheme == "https" {
            Ok(scheme)
        } else {
            Err(crate::Error::InvalidCameraScheme)
        }
    }

    pub fn parsed_timeout(&self, fallback: std::time::Duration) -> std::time::Duration {
        if self.timeout.trim().is_empty() {
            return fallback;
        }

        humantime::parse_duration(&self.timeout)
            .ok()
            .filter(|value| *value > std::time::Duration::ZERO)
            .unwrap_or(fallback)
    }

    pub fn basic_auth_header(&self) -> String {
        if self.username.is_empty() && self.password.is_empty() {
            return String::new();
        }

        let token = base64::engine::general_purpose::STANDARD
            .encode(format!("{}:{}", self.username, self.password));
        format!("Basic {token}")
    }
}

impl Default for CameraPluginConfig {
    fn default() -> Self {
        default_camera_plugin_config()
    }
}

impl Default for CameraStreamingConfig {
    fn default() -> Self {
        default_camera_streaming_config()
    }
}

#[cfg(test)]
mod tests;
