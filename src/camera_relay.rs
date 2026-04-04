use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct CameraRelayConfig {
    pub relay_session_id: String,
    pub agent_id: String,
    pub gateway_id: String,
    pub camera_source_id: String,
    pub stream_profile_id: String,
    pub lease_token: String,
    pub plugin_assignment_id: String,
    pub source_url: String,
    pub rtsp_transport: String,
    pub codec_hint: String,
    pub container_hint: String,
}

pub fn with_url_user_info(
    raw_url: impl AsRef<str>,
    username: impl AsRef<str>,
    password: impl AsRef<str>,
) -> String {
    let username = username.as_ref().trim();
    let password = password.as_ref().trim();
    if username.is_empty() && password.is_empty() {
        return raw_url.as_ref().to_string();
    }

    let Ok(mut parsed) = Url::parse(raw_url.as_ref()) else {
        return raw_url.as_ref().to_string();
    };

    let _ = parsed.set_username(username);
    let _ = parsed.set_password(Some(password));
    parsed.to_string()
}

#[cfg(test)]
mod tests {
    use super::with_url_user_info;

    #[test]
    fn injects_url_user_info() {
        let url = with_url_user_info(
            "wss://camera.local/vapix/ws-data-stream?sources=events",
            "root",
            "secret",
        );
        assert_eq!(
            url,
            "wss://root:secret@camera.local/vapix/ws-data-stream?sources=events"
        );
    }
}
