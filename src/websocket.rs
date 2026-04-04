use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{Error, HostError, SdkResult};
use crate::host;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebSocketDialRequest {
    pub url: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub headers: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub insecure_skip_verify: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WebSocketConnection {
    handle: u32,
}

pub fn websocket_dial(url: impl Into<String>, timeout: Duration) -> SdkResult<WebSocketConnection> {
    websocket_dial_request(
        WebSocketDialRequest {
            url: url.into(),
            ..WebSocketDialRequest::default()
        },
        timeout,
    )
}

pub fn websocket_dial_with_headers(
    url: impl Into<String>,
    headers: BTreeMap<String, String>,
    timeout: Duration,
) -> SdkResult<WebSocketConnection> {
    websocket_dial_request(
        WebSocketDialRequest {
            url: url.into(),
            headers,
            insecure_skip_verify: false,
        },
        timeout,
    )
}

pub fn websocket_connect(
    url: impl Into<String>,
    timeout: Duration,
) -> SdkResult<WebSocketConnection> {
    websocket_dial(url, timeout)
}

pub fn websocket_connect_with_headers(
    url: impl Into<String>,
    headers: BTreeMap<String, String>,
    timeout: Duration,
) -> SdkResult<WebSocketConnection> {
    websocket_dial_with_headers(url, headers, timeout)
}

pub fn websocket_dial_request(
    request: WebSocketDialRequest,
    timeout: Duration,
) -> SdkResult<WebSocketConnection> {
    let data = encode_websocket_dial_request(request)?;
    let res = host::websocket_connect(&data, timeout.as_millis().min(u128::from(u32::MAX)) as u32);
    if res < 0 {
        return Err(HostError {
            code: res,
            op: "websocket_connect",
        }
        .into());
    }

    Ok(WebSocketConnection { handle: res as u32 })
}

pub fn websocket_dial_request_with_insecure_tls(
    url: impl Into<String>,
    timeout: Duration,
) -> SdkResult<WebSocketConnection> {
    websocket_dial_request(
        WebSocketDialRequest {
            url: url.into(),
            headers: BTreeMap::new(),
            insecure_skip_verify: true,
        },
        timeout,
    )
}

pub fn encode_websocket_dial_request(request: WebSocketDialRequest) -> SdkResult<Vec<u8>> {
    let url = request.url.trim().to_string();
    if request.headers.is_empty() && !request.insecure_skip_verify {
        return Ok(url.into_bytes());
    }

    let headers = request
        .headers
        .into_iter()
        .filter_map(|(key, value)| {
            let key = key.trim().to_string();
            let value = value.trim().to_string();
            if key.is_empty() || value.is_empty() {
                None
            } else {
                Some((key, value))
            }
        })
        .collect::<BTreeMap<_, _>>();

    if headers.is_empty() && !request.insecure_skip_verify {
        return Ok(url.into_bytes());
    }

    Ok(serde_json::to_vec(&WebSocketDialRequest {
        url,
        headers,
        insecure_skip_verify: request.insecure_skip_verify,
    })?)
}

pub fn encode_websocket_connect_payload(
    url: impl Into<String>,
    headers: BTreeMap<String, String>,
) -> SdkResult<Vec<u8>> {
    encode_websocket_dial_request(WebSocketDialRequest {
        url: url.into(),
        headers,
        insecure_skip_verify: false,
    })
}

impl WebSocketConnection {
    pub fn send(&self, data: &[u8], timeout: Duration) -> SdkResult<()> {
        if self.handle == 0 {
            return Err(Error::WebSocketNotInitialized);
        }
        if data.is_empty() {
            return Ok(());
        }

        let res = host::websocket_send(
            self.handle,
            data,
            timeout.as_millis().min(u128::from(u32::MAX)) as u32,
        );
        if res < 0 {
            return Err(HostError {
                code: res,
                op: "websocket_send",
            }
            .into());
        }
        Ok(())
    }

    pub fn recv(&self, buf: &mut [u8], timeout: Duration) -> SdkResult<usize> {
        if self.handle == 0 {
            return Err(Error::WebSocketNotInitialized);
        }
        if buf.is_empty() {
            return Ok(0);
        }

        let res = host::websocket_recv(
            self.handle,
            buf,
            timeout.as_millis().min(u128::from(u32::MAX)) as u32,
        );
        if res < 0 {
            return Err(HostError {
                code: res,
                op: "websocket_recv",
            }
            .into());
        }

        Ok(res as usize)
    }

    pub fn close(&mut self) -> SdkResult<()> {
        if self.handle == 0 {
            return Ok(());
        }

        let handle = self.handle;
        self.handle = 0;
        let res = host::websocket_close(handle);
        if res < 0 {
            return Err(HostError {
                code: res,
                op: "websocket_close",
            }
            .into());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests;
