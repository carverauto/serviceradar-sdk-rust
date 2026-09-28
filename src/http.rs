use std::collections::BTreeMap;
use std::str;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::error::{HOST_ERR_TOO_LARGE, HostError, SdkResult};
use crate::host;

pub const MAX_HTTP_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

/// Host response encoding that returns a decimal status line followed by the raw
/// body. Response headers are dropped. It is the default, and hosts that do not
/// support it answer with the JSON envelope, which the client still decodes.
pub const HTTP_RESPONSE_MODE_STATUS_BODY: &str = "status_body";

/// Host response encoding that returns a JSON envelope which also carries the
/// response headers. Multiple values for one header arrive comma-joined.
pub const HTTP_RESPONSE_MODE_ENVELOPE: &str = "envelope";

#[derive(Debug, Clone, Default)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
    pub body_base64: bool,
    /// Host response encoding. Empty selects [`HTTP_RESPONSE_MODE_STATUS_BODY`],
    /// which drops response headers; set [`HTTP_RESPONSE_MODE_ENVELOPE`] to
    /// receive them in [`HttpResponse::headers`].
    pub response_mode: String,
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

    pub fn with_response_mode(mut self, mode: impl Into<String>) -> Self {
        self.response_mode = mode.into();
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

/// Proxied response data. `headers` is populated only in
/// [`HTTP_RESPONSE_MODE_ENVELOPE`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: i32,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
    pub duration: Duration,
}

impl HttpResponse {
    /// Returns the named response header, matched case-insensitively.
    pub fn header(&self, name: &str) -> Option<&str> {
        if let Some(value) = self.headers.get(name) {
            return Some(value.as_str());
        }
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// Parses the `Retry-After` header as delta-seconds or an HTTP-date. A date
    /// in the past yields zero. Returns `None` when the header is absent or
    /// malformed.
    pub fn retry_after(&self) -> Option<Duration> {
        self.retry_after_at(SystemTime::now())
    }

    pub(crate) fn retry_after_at(&self, now: SystemTime) -> Option<Duration> {
        let value = self.header("Retry-After")?.trim();
        let first = *value.as_bytes().first()?;
        if first.is_ascii_digit() {
            // Same ceiling as a Go time.Duration, so both SDKs reject the
            // same inputs.
            const MAX_SECONDS: u64 = (i64::MAX as u64) / 1_000_000_000;
            let seconds = value.parse::<u64>().ok().filter(|s| *s <= MAX_SECONDS)?;
            return Some(Duration::from_secs(seconds));
        }
        let at = parse_http_date(value)?;
        Some(at.duration_since(now).unwrap_or(Duration::ZERO))
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

        let raw = &response_buf[..len];
        if let Some(response) = decode_status_body_response(raw, start.elapsed())? {
            return Ok(response);
        }
        decode_envelope_response(raw, start.elapsed())
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
    #[serde(skip_serializing_if = "String::is_empty")]
    response_mode: String,
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
            response_mode: if request.response_mode.trim().is_empty() {
                HTTP_RESPONSE_MODE_STATUS_BODY.to_string()
            } else {
                request.response_mode.trim().to_string()
            },
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

/// Decodes a `status_body` reply: ASCII digits, a newline, then the raw body.
/// Returns `Ok(None)` when the reply is not in that form, so the caller can fall
/// back to the JSON envelope, matching the Go SDK.
fn decode_status_body_response(
    payload: &[u8],
    duration: Duration,
) -> SdkResult<Option<HttpResponse>> {
    let mut line_end = None;
    for (index, byte) in payload.iter().enumerate() {
        if *byte == b'\n' {
            line_end = Some(index);
            break;
        }
        if !byte.is_ascii_digit() {
            return Ok(None);
        }
    }
    let Some(line_end) = line_end.filter(|end| *end > 0) else {
        return Ok(None);
    };

    let status = payload[..line_end].iter().try_fold(0_i32, |acc, byte| {
        acc.checked_mul(10)
            .and_then(|value| value.checked_add(i32::from(byte - b'0')))
            .ok_or_else(|| crate::error::Error::Message("invalid http status".to_string()))
    })?;

    Ok(Some(HttpResponse {
        status,
        headers: BTreeMap::new(),
        body: payload[line_end + 1..].to_vec(),
        duration,
    }))
}

fn decode_envelope_response(payload: &[u8], duration: Duration) -> SdkResult<HttpResponse> {
    let payload: HttpResponsePayload = serde_json::from_slice(payload)?;
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
        duration,
    })
}

/// Parses the three HTTP-date forms of RFC 9110 section 5.6.7: IMF-fixdate
/// (`Sun, 06 Nov 1994 08:49:37 GMT`), obsolete RFC 850
/// (`Sunday, 06-Nov-94 08:49:37 GMT`) and asctime (`Sun Nov  6 08:49:37 1994`).
fn parse_http_date(value: &str) -> Option<SystemTime> {
    let fields: Vec<&str> = value.split_ascii_whitespace().collect();
    let (weekday, day, month, year, clock) = match fields.as_slice() {
        [weekday, day, month, year, clock, "GMT"] => (
            weekday.strip_suffix(',')?,
            *day,
            *month,
            parse_year(year)?,
            *clock,
        ),
        [weekday, date, clock, "GMT"] => {
            let mut parts = date.split('-');
            let (day, month, year) = (parts.next()?, parts.next()?, parts.next()?);
            if parts.next().is_some() || year.len() != 2 {
                return None;
            }
            // RFC 850 two-digit years pivot the way Go's time package does.
            let short = parse_year(year)?;
            let year = if short >= 69 {
                1900 + short
            } else {
                2000 + short
            };
            (weekday.strip_suffix(',')?, day, month, year, *clock)
        }
        [weekday, month, day, clock, year] => (*weekday, *day, *month, parse_year(year)?, *clock),
        _ => return None,
    };
    if !is_weekday_name(weekday) || day.is_empty() || day.len() > 2 {
        return None;
    }

    let month = time::Month::try_from(month_number(month)?).ok()?;
    let day: u8 = day.parse().ok()?;
    let date = time::Date::from_calendar_date(year, month, day).ok()?;
    let mut clock_parts = clock.split(':');
    let mut next_clock = || -> Option<u8> {
        let part = clock_parts.next()?;
        if part.len() != 2 {
            return None;
        }
        part.parse().ok()
    };
    let (hour, minute, second) = (next_clock()?, next_clock()?, next_clock()?);
    if clock_parts.next().is_some() {
        return None;
    }
    let clock = time::Time::from_hms(hour, minute, second).ok()?;
    let timestamp = time::PrimitiveDateTime::new(date, clock)
        .assume_utc()
        .unix_timestamp();
    if timestamp >= 0 {
        UNIX_EPOCH.checked_add(Duration::from_secs(timestamp as u64))
    } else {
        UNIX_EPOCH.checked_sub(Duration::from_secs(timestamp.unsigned_abs()))
    }
}

fn parse_year(value: &str) -> Option<i32> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

fn is_weekday_name(value: &str) -> bool {
    const DAYS: [&str; 7] = [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ];
    DAYS.iter()
        .any(|day| value == *day || (value.len() == 3 && day.starts_with(value)))
}

fn month_number(value: &str) -> Option<u8> {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    MONTHS
        .iter()
        .position(|month| *month == value)
        .map(|index| index as u8 + 1)
}

const fn is_zero(value: &u32) -> bool {
    *value == 0
}

#[cfg(test)]
mod tests;
