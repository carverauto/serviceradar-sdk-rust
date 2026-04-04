use std::collections::BTreeMap;
use std::str;
use std::time::{Duration, Instant};

use base64::Engine;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::error::{HOST_ERR_TOO_LARGE, HostError, SdkResult};
use crate::host;

pub const MAX_HTTP_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Default)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
    pub body_base64: bool,
    pub timeout_ms: u32,
    pub insecure_skip_verify: bool,
}

impl HttpRequest {
    pub fn new(method: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            url: url.into(),
            ..Self::default()
        }
    }

    pub fn get(url: impl Into<String>) -> Self {
        Self::new("GET", url)
    }

    pub fn post(url: impl Into<String>, body: impl Into<Vec<u8>>) -> Self {
        Self {
            body: body.into(),
            ..Self::new("POST", url)
        }
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout_ms = timeout.as_millis().min(u128::from(u32::MAX)) as u32;
        self
    }

    pub fn with_insecure_tls(mut self, enabled: bool) -> Self {
        self.insecure_skip_verify = enabled;
        self
    }

    pub fn with_binary_body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = body.into();
        self.body_base64 = true;
        self
    }

    pub fn with_text_body(mut self, body: impl Into<String>) -> Self {
        self.body = body.into().into_bytes();
        self.body_base64 = false;
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: i32,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
    pub duration: Duration,
}

impl HttpResponse {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    pub fn text(&self) -> SdkResult<&str> {
        str::from_utf8(&self.body).map_err(|err| crate::error::Error::Message(err.to_string()))
    }

    pub fn json<T>(&self) -> SdkResult<T>
    where
        T: DeserializeOwned,
    {
        Ok(serde_json::from_slice(&self.body)?)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HttpClient {
    pub max_response_bytes: usize,
}

pub const HTTP: HttpClient = HttpClient {
    max_response_bytes: MAX_HTTP_RESPONSE_BYTES,
};

impl Default for HttpClient {
    fn default() -> Self {
        HTTP
    }
}

impl HttpClient {
    pub fn do_request(&self, request: HttpRequest) -> SdkResult<HttpResponse> {
        let payload = HttpRequestPayload::from_request(request);
        let encoded = serde_json::to_vec(&payload)?;
        let mut response_buf = vec![0_u8; self.max_response_bytes.max(1)];
        let start = Instant::now();
        let res = host::http_request(&encoded, &mut response_buf);

        if res < 0 {
            return Err(HostError {
                code: res,
                op: "http_request",
            }
            .into());
        }

        if res == 0 {
            return Ok(HttpResponse {
                duration: start.elapsed(),
                ..HttpResponse::default()
            });
        }

        let len = res as usize;
        if len > response_buf.len() {
            return Err(HostError {
                code: HOST_ERR_TOO_LARGE,
                op: "http_request",
            }
            .into());
        }

        let payload: HttpResponsePayload = serde_json::from_slice(&response_buf[..len])?;
        let body = if payload.body_base64.is_empty() {
            Vec::new()
        } else {
            base64::engine::general_purpose::STANDARD
                .decode(payload.body_base64)
                .map_err(|err| crate::error::Error::Message(err.to_string()))?
        };

        Ok(HttpResponse {
            status: payload.status,
            headers: payload.headers,
            body,
            duration: start.elapsed(),
        })
    }

    pub fn get(&self, url: impl Into<String>) -> SdkResult<HttpResponse> {
        self.do_request(HttpRequest::get(url))
    }

    pub fn post(
        &self,
        url: impl Into<String>,
        body: Vec<u8>,
        content_type: impl Into<String>,
    ) -> SdkResult<HttpResponse> {
        let content_type = content_type.into();
        let request = if content_type.is_empty() {
            HttpRequest::post(url, body)
        } else {
            HttpRequest::post(url, body).with_header("content-type", content_type)
        };
        self.do_request(request)
    }
}

#[derive(Serialize)]
struct HttpRequestPayload {
    method: String,
    url: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    headers: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body_base64: Option<String>,
    #[serde(skip_serializing_if = "is_zero")]
    timeout_ms: u32,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    insecure_skip_verify: bool,
}

impl HttpRequestPayload {
    fn from_request(request: HttpRequest) -> Self {
        let method = if request.method.trim().is_empty() {
            "GET".to_string()
        } else {
            request.method.trim().to_uppercase()
        };

        let mut payload = Self {
            method,
            url: request.url,
            headers: request.headers,
            body: None,
            body_base64: None,
            timeout_ms: request.timeout_ms,
            insecure_skip_verify: request.insecure_skip_verify,
        };

        if !request.body.is_empty() {
            if request.body_base64 {
                payload.body_base64 =
                    Some(base64::engine::general_purpose::STANDARD.encode(request.body));
            } else {
                payload.body = Some(String::from_utf8_lossy(&request.body).into_owned());
            }
        }

        payload
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct HttpResponsePayload {
    status: i32,
    #[serde(default)]
    headers: BTreeMap<String, String>,
    #[serde(default)]
    body_base64: String,
}

const fn is_zero(value: &u32) -> bool {
    *value == 0
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use base64::Engine;

    use crate::host::{TestHostBackend, install_test_backend};

    use std::time::Duration;

    use super::{HttpClient, HttpRequest, HttpRequestPayload, HttpResponse, HttpResponsePayload};

    struct HttpTestHost;

    impl TestHostBackend for HttpTestHost {
        fn http_request(&mut self, req: &[u8], resp: &mut [u8]) -> i32 {
            let payload: serde_json::Value = serde_json::from_slice(req).expect("decode request");
            assert_eq!(payload["method"], "POST");
            assert_eq!(payload["url"], "https://example.com");
            assert_eq!(payload["headers"]["content-type"], "application/json");

            let response = HttpResponsePayload {
                status: 200,
                headers: BTreeMap::from([(
                    "content-type".to_string(),
                    "application/json".to_string(),
                )]),
                body_base64: base64::engine::general_purpose::STANDARD.encode(br#"{"ok":true}"#),
            };
            let encoded = serde_json::to_vec(&response).expect("encode response");
            resp[..encoded.len()].copy_from_slice(&encoded);
            encoded.len() as i32
        }
    }

    #[test]
    fn http_client_encodes_request_and_decodes_response() {
        let _guard = install_test_backend(Box::new(HttpTestHost));

        let response = HttpClient::default()
            .do_request(HttpRequest {
                method: "post".to_string(),
                url: "https://example.com".to_string(),
                headers: BTreeMap::from([(
                    "content-type".to_string(),
                    "application/json".to_string(),
                )]),
                body: br#"{"ping":true}"#.to_vec(),
                body_base64: false,
                timeout_ms: 5_000,
                insecure_skip_verify: false,
            })
            .expect("http request");

        assert_eq!(response.status, 200);
        assert_eq!(response.body, br#"{"ok":true}"#);
    }

    #[test]
    fn http_request_builders_encode_expected_fields() {
        let request = HttpRequest::post("https://example.com", br#"{"ping":true}"#.to_vec())
            .with_header("accept", "application/json")
            .with_timeout(Duration::from_secs(5))
            .with_insecure_tls(true);

        let payload = HttpRequestPayload::from_request(request);
        assert_eq!(payload.method, "POST");
        assert_eq!(payload.url, "https://example.com");
        assert_eq!(
            payload.headers.get("accept").map(String::as_str),
            Some("application/json")
        );
        assert_eq!(payload.timeout_ms, 5_000);
        assert!(payload.insecure_skip_verify);
        assert_eq!(payload.body.as_deref(), Some(r#"{"ping":true}"#));
    }

    #[test]
    fn http_response_helpers_decode_headers_text_and_json() {
        let response = HttpResponse {
            status: 200,
            headers: BTreeMap::from([("content-type".to_string(), "application/json".to_string())]),
            body: br#"{"ok":true}"#.to_vec(),
            duration: Duration::from_millis(12),
        };

        assert_eq!(response.header("Content-Type"), Some("application/json"));
        assert_eq!(response.text().expect("utf8 body"), r#"{"ok":true}"#);

        let value: serde_json::Value = response.json().expect("json body");
        assert_eq!(value["ok"], true);
    }
}
