use std::collections::BTreeMap;
use std::time::Duration;

use base64::Engine;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{Error, SdkResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RtspEndpoint {
    pub raw_url: String,
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub request_uri: String,
    pub base_url: String,
    pub username: String,
    pub password: String,
}

impl RtspEndpoint {
    pub fn parse(raw_url: &str, username: &str, password: &str) -> SdkResult<Self> {
        parse_rtsp_endpoint(raw_url, username, password)
    }

    pub fn authority(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn build_request(
        &self,
        method: &str,
        request_uri: &str,
        cseq: i32,
        session: &str,
        auth: Option<&mut RtspAuthState>,
        extra_headers: &BTreeMap<String, String>,
    ) -> String {
        build_rtsp_request(
            self,
            method,
            request_uri,
            cseq,
            session,
            auth,
            extra_headers,
        )
    }

    pub fn authorization(
        &self,
        method: &str,
        request_uri: &str,
        auth: Option<&mut RtspAuthState>,
    ) -> String {
        build_rtsp_authorization(self, method, request_uri, auth)
    }

    pub fn resolve_control_url(&self, control: &str) -> String {
        resolve_rtsp_control_url(self, control)
    }

    pub fn find_h264_track(&self, sdp: &[u8]) -> SdkResult<RtspH264Track> {
        parse_h264_track_from_sdp(self, sdp)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtspResponse {
    pub status_code: i32,
    pub status_line: String,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
    pub content_length: usize,
}

impl RtspResponse {
    pub fn parse(data: &[u8]) -> SdkResult<Self> {
        parse_rtsp_response(data)
    }

    pub fn read_from<T>(conn: &mut T, timeout: Duration) -> SdkResult<Self>
    where
        T: RtspTransport,
    {
        read_rtsp_response(conn, timeout)
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.trim().to_ascii_lowercase())
            .map(String::as_str)
    }

    pub fn is_success(&self) -> bool {
        (200..400).contains(&self.status_code)
    }

    pub fn session(&self) -> Option<String> {
        self.header("session").map(parse_session_header)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtspH264Track {
    pub control_url: String,
    pub payload_type: i32,
}

impl RtspH264Track {
    pub fn new(control_url: impl Into<String>, payload_type: i32) -> Self {
        Self {
            control_url: control_url.into(),
            payload_type,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtspInterleavedFrame {
    pub channel: u8,
    pub payload: Vec<u8>,
}

impl RtspInterleavedFrame {
    pub fn parse(data: &[u8]) -> SdkResult<Self> {
        parse_interleaved_frame(data)
    }

    pub fn len(&self) -> usize {
        self.payload.len()
    }

    pub fn is_empty(&self) -> bool {
        self.payload.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtspAuthState {
    pub scheme: String,
    pub realm: String,
    pub nonce: String,
    pub opaque: String,
    pub algorithm: String,
    pub qop: String,
    pub cnonce: String,
    pub nc: u32,
}

impl RtspAuthState {
    pub fn parse(header: &str) -> SdkResult<Self> {
        parse_rtsp_authenticate_header(header)
    }

    pub fn is_digest(&self) -> bool {
        self.scheme.eq_ignore_ascii_case("digest")
    }

    pub fn is_basic(&self) -> bool {
        self.scheme.eq_ignore_ascii_case("basic")
    }
}

#[derive(Debug, Default)]
pub struct RtspH264Depacketizer {
    timestamp: u32,
    assembling: bool,
    fragments: Vec<Vec<u8>>,
    keyframe: bool,
}

pub type StreamEndpoint = RtspEndpoint;
pub type StreamResponse = RtspResponse;
pub type StreamClient<T> = RtspClient<T>;
pub type VideoTrack = RtspH264Track;
pub type InterleavedFrame = RtspInterleavedFrame;
pub type AuthChallenge = RtspAuthState;
pub type H264Depacketizer = RtspH264Depacketizer;

pub trait RtspTransport {
    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> SdkResult<usize>;
    fn write(&mut self, data: &[u8], timeout: Duration) -> SdkResult<usize>;
    fn close(&mut self) -> SdkResult<()>;
}

pub struct RtspClient<T> {
    pub conn: T,
    pub timeout: Duration,
    pub endpoint: RtspEndpoint,
    pub seq: i32,
    pub session: String,
    pub auth: Option<RtspAuthState>,
}

impl<T> RtspClient<T>
where
    T: RtspTransport,
{
    pub fn new(conn: T, timeout: Duration, endpoint: RtspEndpoint) -> Self {
        Self {
            conn,
            timeout,
            endpoint,
            seq: 1,
            session: String::new(),
            auth: None,
        }
    }

    pub fn do_request(
        &mut self,
        method: &str,
        request_uri: &str,
        extra_headers: &BTreeMap<String, String>,
    ) -> SdkResult<RtspResponse> {
        let request = self.endpoint.build_request(
            method,
            request_uri,
            self.seq,
            &self.session,
            self.auth.as_mut(),
            extra_headers,
        );
        self.seq += 1;
        self.conn.write(request.as_bytes(), self.timeout)?;

        let mut response = RtspResponse::read_from(&mut self.conn, self.timeout)?;
        if response.status_code == 401 && !self.endpoint.username.trim().is_empty() {
            let challenge = response
                .headers
                .get("www-authenticate")
                .cloned()
                .unwrap_or_default();
            self.auth = Some(RtspAuthState::parse(&challenge)?);

            let retry = self.endpoint.build_request(
                method,
                request_uri,
                self.seq,
                &self.session,
                self.auth.as_mut(),
                extra_headers,
            );
            self.seq += 1;
            self.conn.write(retry.as_bytes(), self.timeout)?;
            response = RtspResponse::read_from(&mut self.conn, self.timeout)?;
        }

        if self.session.is_empty() {
            if let Some(session) = response.headers.get("session") {
                self.session = parse_session_header(session);
            }
        }

        if response.status_code >= 400 {
            return Err(Error::Message(format!(
                "invalid rtsp response: {}",
                response.status_line
            )));
        }

        Ok(response)
    }

    pub fn request(
        &mut self,
        method: &str,
        request_uri: &str,
        extra_headers: &BTreeMap<String, String>,
    ) -> SdkResult<RtspResponse> {
        self.do_request(method, request_uri, extra_headers)
    }

    pub fn session(&self) -> Option<&str> {
        (!self.session.is_empty()).then_some(self.session.as_str())
    }

    pub fn teardown(&mut self) -> SdkResult<()> {
        if self.session.is_empty() {
            return Ok(());
        }

        let request_uri = self.endpoint.request_uri.clone();
        let _ = self.do_request("TEARDOWN", &request_uri, &BTreeMap::new())?;
        self.session.clear();
        Ok(())
    }

    pub fn close(&mut self) -> SdkResult<()> {
        self.teardown()?;
        self.conn.close()
    }
}

pub fn parse_rtsp_endpoint(
    raw_url: &str,
    username: &str,
    password: &str,
) -> SdkResult<RtspEndpoint> {
    let parsed = Url::parse(raw_url.trim()).map_err(|_| Error::RtspInvalidUrl)?;
    let scheme = parsed.scheme().trim().to_ascii_lowercase();
    if parsed.host_str().unwrap_or_default().is_empty() || (scheme != "rtsp" && scheme != "rtsps") {
        return Err(Error::RtspInvalidUrl);
    }

    let host = parsed.host_str().unwrap_or_default().to_string();
    let port = if let Some(port) = parsed.port() {
        port
    } else if scheme == "rtsps" {
        322
    } else {
        554
    };

    let mut username = username.to_string();
    let mut password = password.to_string();
    if !parsed.username().is_empty() {
        username = parsed.username().to_string();
    }
    if let Some(parsed_password) = parsed.password() {
        password = parsed_password.to_string();
    }

    let request_uri = if parsed.path().is_empty() {
        "/".to_string()
    } else {
        match parsed.query() {
            Some(query) => format!("{}?{query}", parsed.path()),
            None => parsed.path().to_string(),
        }
    };

    let authority = match parsed.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.clone(),
    };

    Ok(RtspEndpoint {
        raw_url: raw_url.to_string(),
        scheme: scheme.clone(),
        host,
        port,
        request_uri,
        base_url: format!("{scheme}://{authority}"),
        username,
        password,
    })
}

pub fn build_rtsp_request(
    endpoint: &RtspEndpoint,
    method: &str,
    request_uri: &str,
    cseq: i32,
    session: &str,
    auth: Option<&mut RtspAuthState>,
    extra_headers: &BTreeMap<String, String>,
) -> String {
    let mut request = String::new();
    request.push_str(method);
    request.push(' ');
    request.push_str(request_uri);
    request.push_str(" RTSP/1.0\r\n");
    request.push_str(&format!("CSeq: {cseq}\r\n"));
    request.push_str("User-Agent: ServiceRadar-Camera-WASM/0.1\r\n");

    if !session.trim().is_empty() {
        request.push_str("Session: ");
        request.push_str(session);
        request.push_str("\r\n");
    }

    let auth_header = build_rtsp_authorization(endpoint, method, request_uri, auth);
    if !auth_header.is_empty() {
        request.push_str("Authorization: ");
        request.push_str(&auth_header);
        request.push_str("\r\n");
    }

    for (key, value) in extra_headers {
        if key.trim().is_empty() || value.trim().is_empty() {
            continue;
        }
        request.push_str(key);
        request.push_str(": ");
        request.push_str(value);
        request.push_str("\r\n");
    }

    request.push_str("\r\n");
    request
}

pub fn build_rtsp_authorization(
    endpoint: &RtspEndpoint,
    method: &str,
    request_uri: &str,
    auth: Option<&mut RtspAuthState>,
) -> String {
    if endpoint.username.trim().is_empty() && endpoint.password.trim().is_empty() {
        return String::new();
    }

    if let Some(auth) = auth {
        if !auth.scheme.eq_ignore_ascii_case("digest") {
            return String::new();
        }

        auth.nc += 1;
        let nc = format!("{:08x}", auth.nc);
        let cnonce = if auth.cnonce.is_empty() {
            "serviceradar"
        } else {
            &auth.cnonce
        };
        let ha1 = md5_hex(format!(
            "{}:{}:{}",
            endpoint.username, auth.realm, endpoint.password
        ));
        let ha2 = md5_hex(format!("{method}:{request_uri}"));
        let response = if auth.qop.is_empty() {
            md5_hex(format!("{ha1}:{}:{ha2}", auth.nonce))
        } else {
            md5_hex(format!(
                "{ha1}:{}:{nc}:{cnonce}:{}:{ha2}",
                auth.nonce, auth.qop
            ))
        };

        let mut parts = vec![
            format!("username=\"{}\"", endpoint.username),
            format!("realm=\"{}\"", auth.realm),
            format!("nonce=\"{}\"", auth.nonce),
            format!("uri=\"{request_uri}\""),
            format!("response=\"{response}\""),
        ];
        if !auth.algorithm.is_empty() {
            parts.push(format!("algorithm={}", auth.algorithm));
        }
        if !auth.opaque.is_empty() {
            parts.push(format!("opaque=\"{}\"", auth.opaque));
        }
        if !auth.qop.is_empty() {
            parts.push(format!("qop={}", auth.qop));
            parts.push(format!("nc={nc}"));
            parts.push(format!("cnonce=\"{cnonce}\""));
        }
        return format!("Digest {}", parts.join(", "));
    }

    let token = base64::engine::general_purpose::STANDARD
        .encode(format!("{}:{}", endpoint.username, endpoint.password));
    format!("Basic {token}")
}

pub fn parse_rtsp_authenticate_header(header: &str) -> SdkResult<RtspAuthState> {
    let header = header.trim();
    if header.is_empty() {
        return Err(Error::RtspUnauthorized);
    }

    if header.to_ascii_lowercase().starts_with("digest ") {
        let params = parse_auth_params(header[7..].trim());
        let realm = params.get("realm").cloned().unwrap_or_default();
        let nonce = params.get("nonce").cloned().unwrap_or_default();
        if realm.is_empty() || nonce.is_empty() {
            return Err(Error::RtspUnauthorized);
        }

        let qop = params
            .get("qop")
            .map(|value| {
                value
                    .split(',')
                    .map(|part| part.trim().trim_matches('"'))
                    .find(|part| *part == "auth")
                    .or_else(|| {
                        value
                            .split(',')
                            .map(|part| part.trim().trim_matches('"'))
                            .find(|part| !part.is_empty())
                    })
                    .unwrap_or("")
                    .to_string()
            })
            .unwrap_or_default();

        return Ok(RtspAuthState {
            scheme: "digest".to_string(),
            realm,
            nonce,
            opaque: params.get("opaque").cloned().unwrap_or_default(),
            algorithm: first_non_blank([params.get("algorithm").cloned(), Some("MD5".to_string())]),
            qop,
            cnonce: "serviceradar".to_string(),
            nc: 0,
        });
    }

    if header.to_ascii_lowercase().starts_with("basic") {
        return Ok(RtspAuthState {
            scheme: "basic".to_string(),
            realm: String::new(),
            nonce: String::new(),
            opaque: String::new(),
            algorithm: String::new(),
            qop: String::new(),
            cnonce: String::new(),
            nc: 0,
        });
    }

    Err(Error::RtspUnauthorized)
}

pub fn parse_rtsp_response(data: &[u8]) -> SdkResult<RtspResponse> {
    let Some((head, body)) = data.split_once_str(b"\r\n\r\n") else {
        return Err(Error::RtspBadResponse);
    };

    let head = String::from_utf8_lossy(head);
    let mut lines = head.split("\r\n");
    let status_line = lines.next().unwrap_or_default().to_string();
    if !status_line.starts_with("RTSP/1.0 ") {
        return Err(Error::RtspBadResponse);
    }

    let status_code = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<i32>().ok())
        .ok_or(Error::RtspBadResponse)?;

    let mut headers = BTreeMap::new();
    let mut content_length = 0_usize;
    for line in lines {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim().to_string();
        if key == "content-length" {
            content_length = value.parse::<usize>().unwrap_or(0);
        }
        headers.insert(key, value);
    }

    let mut body = body.to_vec();
    if content_length > 0 && body.len() > content_length {
        body.truncate(content_length);
    }

    Ok(RtspResponse {
        status_code,
        status_line,
        headers,
        body,
        content_length,
    })
}

pub fn read_rtsp_response<T>(conn: &mut T, timeout: Duration) -> SdkResult<RtspResponse>
where
    T: RtspTransport,
{
    let mut buf = vec![0_u8; 64 * 1024];
    let len = conn.read(&mut buf, timeout)?;
    parse_rtsp_response(&buf[..len])
}

pub fn parse_h264_track_from_sdp(endpoint: &RtspEndpoint, body: &[u8]) -> SdkResult<RtspH264Track> {
    let mut in_video = false;
    let mut payload_type = 0_i32;
    let mut control = String::new();

    for line in String::from_utf8_lossy(body).lines().map(str::trim) {
        if line.starts_with("m=video ") {
            in_video = true;
            payload_type = 0;
            control.clear();
        } else if line.starts_with("m=") {
            in_video = false;
        } else if in_video && line.starts_with("a=rtpmap:") && line.contains("H264/90000") {
            if let Some(value) = line
                .trim_start_matches("a=rtpmap:")
                .split_whitespace()
                .next()
            {
                payload_type = value.parse().unwrap_or(0);
            }
        } else if in_video && line.starts_with("a=control:") {
            control = line.trim_start_matches("a=control:").trim().to_string();
        }

        if in_video && payload_type != 0 && !control.is_empty() {
            return Ok(RtspH264Track::new(
                resolve_rtsp_control_url(endpoint, &control),
                payload_type,
            ));
        }
    }

    Err(Error::RtspNoVideoTrack)
}

pub fn resolve_rtsp_control_url(endpoint: &RtspEndpoint, control: &str) -> String {
    let control = control.trim();
    if control.is_empty() {
        return endpoint.request_uri.clone();
    }
    if control.starts_with("rtsp://") || control.starts_with("rtsps://") {
        return control.to_string();
    }
    if control.starts_with('/') {
        return format!("{}{}", endpoint.base_url, control);
    }

    let base = format!(
        "{}{}",
        endpoint.base_url,
        endpoint.request_uri.trim_end_matches('/')
    );
    format!("{base}/{control}")
}

pub fn parse_session_header(value: &str) -> String {
    value
        .trim()
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}

pub fn parse_interleaved_frame(data: &[u8]) -> SdkResult<RtspInterleavedFrame> {
    if data.len() < 4 || data[0] != b'$' {
        return Err(Error::RtspBadInterleaved);
    }

    let size = u16::from_be_bytes([data[2], data[3]]) as usize;
    if data.len() < 4 + size {
        return Err(Error::RtspBadInterleaved);
    }

    Ok(RtspInterleavedFrame {
        channel: data[1],
        payload: data[4..4 + size].to_vec(),
    })
}

pub fn parse_rtp_packet(data: &[u8]) -> SdkResult<(Vec<u8>, bool, u32)> {
    if data.len() < 12 {
        return Err(Error::RtpPacketTooShort);
    }

    let cc = (data[0] & 0x0F) as usize;
    let extension = (data[0] & 0x10) != 0;
    let marker = (data[1] & 0x80) != 0;
    let mut offset = 12 + cc * 4;
    if data.len() < offset {
        return Err(Error::RtpPacketTooShort);
    }

    if extension {
        if data.len() < offset + 4 {
            return Err(Error::RtpPacketTooShort);
        }
        let ext_len = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize * 4;
        offset += 4 + ext_len;
        if data.len() < offset {
            return Err(Error::RtpPacketTooShort);
        }
    }

    let timestamp = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    Ok((data[offset..].to_vec(), marker, timestamp))
}

impl RtspH264Depacketizer {
    pub fn push(
        &mut self,
        payload: &[u8],
        marker: bool,
        timestamp: u32,
    ) -> SdkResult<(Vec<u8>, bool, bool)> {
        if payload.is_empty() {
            return Err(Error::H264PayloadTooShort);
        }

        if !self.assembling || self.timestamp != timestamp {
            self.fragments.clear();
            self.keyframe = false;
            self.timestamp = timestamp;
            self.assembling = true;
        }

        let nal_type = payload[0] & 0x1F;
        match nal_type {
            1..=23 => {
                self.fragments.push(annex_b_unit(payload));
                self.keyframe |= nal_type == 5;
            }
            24 => {
                let mut offset = 1;
                while offset + 2 <= payload.len() {
                    let size = u16::from_be_bytes([payload[offset], payload[offset + 1]]) as usize;
                    offset += 2;
                    if size == 0 || offset + size > payload.len() {
                        return Err(Error::H264PayloadTooShort);
                    }
                    let nal = &payload[offset..offset + size];
                    offset += size;
                    if nal.is_empty() {
                        continue;
                    }
                    self.keyframe |= (nal[0] & 0x1F) == 5;
                    self.fragments.push(annex_b_unit(nal));
                }
            }
            28 => {
                if payload.len() < 2 {
                    return Err(Error::H264PayloadTooShort);
                }
                let fu_indicator = payload[0];
                let fu_header = payload[1];
                let start = (fu_header & 0x80) != 0;
                let end = (fu_header & 0x40) != 0;
                let reconstructed = [(fu_indicator & 0xE0) | (fu_header & 0x1F)]
                    .into_iter()
                    .chain(payload[2..].iter().copied())
                    .collect::<Vec<_>>();

                if start {
                    self.fragments.push(annex_b_unit(&reconstructed));
                } else if let Some(last) = self.fragments.last_mut() {
                    last.extend_from_slice(&reconstructed[1..]);
                } else {
                    return Err(Error::H264PayloadTooShort);
                }

                self.keyframe |= (fu_header & 0x1F) == 5;
                if !end && !marker {
                    return Ok((Vec::new(), false, false));
                }
            }
            _ => return Err(Error::H264UnsupportedNal),
        }

        if !marker {
            return Ok((Vec::new(), false, false));
        }

        let access_unit = join_fragments(&self.fragments);
        let keyframe = self.keyframe;
        self.fragments.clear();
        self.keyframe = false;
        self.assembling = false;
        Ok((access_unit, keyframe, true))
    }

    pub fn push_rtp_packet(&mut self, packet: &[u8]) -> SdkResult<(Vec<u8>, bool, bool)> {
        let (payload, marker, timestamp) = parse_rtp_packet(packet)?;
        self.push(&payload, marker, timestamp)
    }
}

fn annex_b_unit(nal: &[u8]) -> Vec<u8> {
    [0_u8, 0, 0, 1]
        .into_iter()
        .chain(nal.iter().copied())
        .collect()
}

fn join_fragments(parts: &[Vec<u8>]) -> Vec<u8> {
    parts.iter().flat_map(|part| part.iter().copied()).collect()
}

fn parse_auth_params(raw: &str) -> BTreeMap<String, String> {
    split_comma_separated(raw)
        .into_iter()
        .filter_map(|part| {
            let (key, value) = part.split_once('=')?;
            Some((
                key.trim().to_ascii_lowercase(),
                value.trim().trim_matches('"').to_string(),
            ))
        })
        .collect()
}

fn split_comma_separated(raw: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in raw.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                current.push(ch);
            }
            ',' if !in_quotes => {
                parts.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        parts.push(current.trim().to_string());
    }

    parts
}

fn md5_hex(value: String) -> String {
    format!("{:x}", md5::compute(value.as_bytes()))
}

fn first_non_blank(values: [Option<String>; 2]) -> String {
    values
        .into_iter()
        .flatten()
        .map(|value| value.trim().to_string())
        .find(|value| !value.is_empty())
        .unwrap_or_default()
}

trait SplitOnceBytes {
    fn split_once_str<'a>(&'a self, needle: &[u8]) -> Option<(&'a [u8], &'a [u8])>;
}

impl SplitOnceBytes for [u8] {
    fn split_once_str<'a>(&'a self, needle: &[u8]) -> Option<(&'a [u8], &'a [u8])> {
        self.windows(needle.len())
            .position(|window| window == needle)
            .map(|index| (&self[..index], &self[index + needle.len()..]))
    }
}

#[cfg(test)]
mod tests;
