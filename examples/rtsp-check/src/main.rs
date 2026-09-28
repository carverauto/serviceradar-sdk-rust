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
        Ok(check_result(&endpoint, &options, &describe))
    });
}

fn reported_url(endpoint: &sdk::StreamEndpoint) -> String {
    format!("{}{}", endpoint.base_url, endpoint.request_uri)
}

fn check_result(
    endpoint: &sdk::StreamEndpoint,
    options: &sdk::StreamResponse,
    describe: &sdk::StreamResponse,
) -> sdk::PluginResult {
    let url = reported_url(endpoint);
    let mut table = BTreeMap::new();
    table.insert("URL".to_string(), url.clone());
    table.insert("OPTIONS".to_string(), options.status_code.to_string());
    table.insert("DESCRIBE".to_string(), describe.status_code.to_string());
    if let Some(public) = options.headers.get("public") {
        table.insert("Public".to_string(), public.clone());
    }
    table.insert("SDP bytes".to_string(), describe.body.len().to_string());

    let ok = options.status_code == 200 && describe.status_code == 200;
    let mut result = sdk::PluginResult::new()
        .with_summary(format!(
            "rtsp {url}: options {} describe {}",
            options.status_code, describe.status_code
        ))
        .with_table(table, "key-value");
    if !ok {
        result = result.with_status(sdk::Status::Warning);
    }
    result
}

fn main() {}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serviceradar_sdk_rust as sdk;

    use super::{check_result, reported_url};

    #[test]
    fn submitted_result_omits_url_userinfo() {
        let endpoint =
            sdk::StreamEndpoint::parse("rtsp://admin:s3cret-camera@example.com:8554/live", "", "")
                .expect("endpoint");
        assert_eq!(reported_url(&endpoint), "rtsp://example.com:8554/live");

        let options = sdk::StreamResponse {
            status_code: 200,
            status_line: "RTSP/1.0 200 OK".to_string(),
            headers: BTreeMap::new(),
            body: Vec::new(),
            content_length: 0,
        };
        let describe = sdk::StreamResponse {
            status_code: 200,
            status_line: "RTSP/1.0 200 OK".to_string(),
            headers: BTreeMap::new(),
            body: b"v=0".to_vec(),
            content_length: 3,
        };
        let payload = check_result(&endpoint, &options, &describe)
            .serialize()
            .expect("serialize");
        let text = String::from_utf8(payload).expect("utf8");
        assert!(text.contains("rtsp://example.com:8554/live"), "{text}");
        assert!(!text.contains("s3cret-camera"), "{text}");
        assert!(!text.contains("admin"), "{text}");
    }
}
