use std::collections::BTreeMap;
use std::time::Duration;

use crate::{CameraPluginConfig, Error, HttpClient, HttpRequest, HttpResponse, SdkResult};

#[derive(Debug, Clone)]
pub struct CameraHttpClient {
    pub base_url: String,
    pub timeout: Duration,
    pub auth_header: String,
    pub insecure_skip_verify: bool,
}

impl CameraHttpClient {
    pub fn new(config: &CameraPluginConfig, fallback_timeout: Duration) -> SdkResult<Self> {
        if config.host.trim().is_empty() {
            return Err(Error::CameraHostRequired);
        }

        let scheme = config.normalized_scheme()?;
        Ok(Self {
            base_url: format!("{scheme}://{}", config.host.trim()),
            timeout: config.parsed_timeout(fallback_timeout),
            auth_header: config.basic_auth_header(),
            insecure_skip_verify: config.insecure_skip_verify,
        })
    }

    pub fn url(&self, path: impl AsRef<str>) -> String {
        format!("{}{}", self.base_url, path.as_ref())
    }

    pub fn do_request(&self, mut request: HttpRequest) -> SdkResult<HttpResponse> {
        if request.url.is_empty() {
            request.url = self.base_url.clone();
        }
        if request.timeout_ms == 0 {
            request.timeout_ms = self.timeout.as_millis().min(u128::from(u32::MAX)) as u32;
        }
        if self.insecure_skip_verify {
            request.insecure_skip_verify = true;
        }
        if !self.auth_header.is_empty() && !request.headers.contains_key("Authorization") {
            request
                .headers
                .insert("Authorization".to_string(), self.auth_header.clone());
        }

        HttpClient::default().do_request(request)
    }

    pub fn get(&self, path: impl AsRef<str>) -> SdkResult<HttpResponse> {
        self.do_request(HttpRequest {
            method: "GET".to_string(),
            url: self.url(path),
            headers: BTreeMap::new(),
            body: Vec::new(),
            body_base64: false,
            timeout_ms: 0,
            insecure_skip_verify: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::time::Duration;

    use crate::CameraPluginConfig;
    use crate::host::{TestHostBackend, install_test_backend};

    use super::{CameraHttpClient, HttpRequest};

    struct CameraHttpTestHost;

    impl TestHostBackend for CameraHttpTestHost {
        fn http_request(&mut self, req: &[u8], resp: &mut [u8]) -> i32 {
            let payload: serde_json::Value = serde_json::from_slice(req).expect("decode request");
            assert_eq!(payload["method"], "GET");
            assert_eq!(
                payload["url"],
                "https://camera.local/axis-cgi/basicdeviceinfo.cgi"
            );
            assert_eq!(
                payload["headers"]["Authorization"],
                "Basic cm9vdDpzZWNyZXQ="
            );
            assert_eq!(payload["timeout_ms"], 15000);
            assert_eq!(payload["insecure_skip_verify"], true);

            let encoded = serde_json::to_vec(&serde_json::json!({
                "status": 200,
                "headers": {"content-type": "application/json"},
                "body_base64": ""
            }))
            .expect("encode response");
            resp[..encoded.len()].copy_from_slice(&encoded);
            encoded.len() as i32
        }
    }

    #[test]
    fn camera_http_client_normalizes_config() {
        let client = CameraHttpClient::new(
            &CameraPluginConfig {
                host: " camera.local ".to_string(),
                scheme: "HTTPS".to_string(),
                timeout: "15s".to_string(),
                username: "root".to_string(),
                password: "secret".to_string(),
                ..CameraPluginConfig::default()
            },
            Duration::from_secs(3),
        )
        .expect("camera http client");

        assert_eq!(client.base_url, "https://camera.local");
        assert_eq!(client.timeout, Duration::from_secs(15));
        assert_eq!(client.auth_header, "Basic cm9vdDpzZWNyZXQ=");
        assert!(!client.insecure_skip_verify);
    }

    #[test]
    fn camera_http_client_propagates_insecure_tls_and_formats_urls() {
        let client = CameraHttpClient::new(
            &CameraPluginConfig {
                host: "camera.local".to_string(),
                scheme: "https".to_string(),
                insecure_skip_verify: true,
                ..CameraPluginConfig::default()
            },
            Duration::from_secs(3),
        )
        .expect("camera http client");

        assert!(client.insecure_skip_verify);
        assert_eq!(
            client.url("/axis-cgi/basicdeviceinfo.cgi"),
            "https://camera.local/axis-cgi/basicdeviceinfo.cgi"
        );
    }

    #[test]
    fn camera_http_client_injects_auth_timeout_and_tls_settings() {
        let _guard = install_test_backend(Box::new(CameraHttpTestHost));

        let client = CameraHttpClient::new(
            &CameraPluginConfig {
                host: "camera.local".to_string(),
                scheme: "https".to_string(),
                timeout: "15s".to_string(),
                username: "root".to_string(),
                password: "secret".to_string(),
                insecure_skip_verify: true,
                ..CameraPluginConfig::default()
            },
            Duration::from_secs(3),
        )
        .expect("camera http client");

        let response = client
            .do_request(HttpRequest {
                method: "GET".to_string(),
                url: client.url("/axis-cgi/basicdeviceinfo.cgi"),
                headers: BTreeMap::new(),
                body: Vec::new(),
                body_base64: false,
                timeout_ms: 0,
                insecure_skip_verify: false,
            })
            .expect("camera request");

        assert_eq!(response.status, 200);
    }
}
