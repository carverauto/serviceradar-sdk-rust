use std::time::Duration;

use super::{CameraPluginConfig, default_camera_plugin_config};

#[test]
fn camera_defaults_match_go_sdk() {
    let config = default_camera_plugin_config();
    assert_eq!(config.scheme, "http");
    assert!(config.discover_streams);
    assert!(!config.collect_events);
    assert_eq!(config.event_sources, "events");
    assert_eq!(config.timeout, "10s");
}

#[test]
fn camera_scheme_normalizes() {
    let config = CameraPluginConfig {
        scheme: " HTTPS ".to_string(),
        ..default_camera_plugin_config()
    };
    assert_eq!(config.normalized_scheme().expect("scheme"), "https");
}

#[test]
fn camera_timeout_parses() {
    let config = CameraPluginConfig {
        timeout: "15s".to_string(),
        ..default_camera_plugin_config()
    };
    assert_eq!(
        config.parsed_timeout(Duration::from_secs(3)),
        Duration::from_secs(15)
    );
}

#[test]
fn camera_basic_auth_header_encodes_credentials() {
    let config = CameraPluginConfig {
        username: "root".to_string(),
        password: "secret".to_string(),
        ..default_camera_plugin_config()
    };
    assert_eq!(config.basic_auth_header(), "Basic cm9vdDpzZWNyZXQ=");
}
