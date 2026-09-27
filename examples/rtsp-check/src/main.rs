//! RTSP camera check over the host TCP proxy: OPTIONS + DESCRIBE against an
//! allowlisted `rtsp://` URL and reports the server capabilities. Plain
//! `rtsp://` works without extra features; `rtsps://` URLs need the SDK's
//! `rtsps` feature (TLS inside the plugin).
use std::collections::BTreeMap;
use std::time::Duration;

use serviceradar_sdk_rust as sdk;

#[derive(Debug, serde::Deserialize)]
#[serde(default)]
struct Config {
    url: String,
    username: String,
    password: String,
    timeout_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            url: "rtsp://example.com:8554/live".to_string(),
            username: String::new(),
            password: String::new(),
            timeout_ms: 5000,
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn run_check() {
    let _ = sdk::execute(|| {
        let config = sdk::load_config_or_default::<Config>()?;
        let timeout = Duration::from_millis(config.timeout_ms.max(1));

        let endpoint = sdk::StreamEndpoint::parse(&config.url, &config.username, &config.password)?;
        let conn = sdk::dial_rtsp_transport(&endpoint, timeout, false)?;
        let mut client = sdk::StreamClient::new(conn, timeout, endpoint.clone());

        let options = client.do_request("OPTIONS", &endpoint.request_uri, &BTreeMap::new())?;
        let mut headers = BTreeMap::new();
        headers.insert("Accept".to_string(), "application/sdp".to_string());
        let describe = client.do_request("DESCRIBE", &endpoint.request_uri, &headers)?;

        let mut table = BTreeMap::new();
        table.insert("URL".to_string(), config.url.clone());
        table.insert("OPTIONS".to_string(), options.status_code.to_string());
        table.insert("DESCRIBE".to_string(), describe.status_code.to_string());
        if let Some(public) = options.headers.get("public") {
            table.insert("Public".to_string(), public.clone());
        }
        table.insert("SDP bytes".to_string(), describe.body.len().to_string());

        let ok = options.status_code == 200 && describe.status_code == 200;
        let mut result = sdk::PluginResult::new()
            .with_summary(format!(
                "rtsp {}: options {} describe {}",
                config.url, options.status_code, describe.status_code
            ))
            .with_table(table, "key-value");
        if !ok {
            result = result.with_status(sdk::Status::Warning);
        }
        Ok(result)
    });
}

fn main() {}
