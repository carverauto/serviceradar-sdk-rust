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
mod tests;
