//! RTSP and RTSPS transports over the host TCP proxy, matching the Go SDK's
//! `DialRTSPTransport`. TLS for `rtsps://` runs inside the plugin (rustls with
//! a pure-Rust crypto provider) on top of the host TCP connection, and needs
//! the `rtsps` feature.

use std::time::Duration;

use crate::error::{Error, SdkResult};
use crate::rtsp::{RtspEndpoint, RtspTransport};
use crate::tcp::{TcpConnection, tcp_dial};

#[cfg(feature = "rtsps")]
mod tls;

impl RtspTransport for TcpConnection {
    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> SdkResult<usize> {
        TcpConnection::read(self, buf, timeout)
    }

    fn write(&mut self, data: &[u8], timeout: Duration) -> SdkResult<usize> {
        let mut written = 0;
        while written < data.len() {
            let n = TcpConnection::write(self, &data[written..], timeout)?;
            if n == 0 {
                return Err(Error::Message("tcp write made no progress".to_string()));
            }
            written += n;
        }
        Ok(written)
    }

    fn close(&mut self) -> SdkResult<()> {
        TcpConnection::close(self)
    }
}

/// An RTSP transport opened by [`dial_rtsp_transport`].
pub enum RtspConnection {
    Plain(TcpConnection),
    #[cfg(feature = "rtsps")]
    Tls(Box<tls::TlsConnection>),
}

impl std::fmt::Debug for RtspConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Plain(conn) => f.debug_tuple("Plain").field(conn).finish(),
            #[cfg(feature = "rtsps")]
            Self::Tls(_) => f.write_str("Tls(..)"),
        }
    }
}

impl RtspTransport for RtspConnection {
    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> SdkResult<usize> {
        match self {
            Self::Plain(conn) => RtspTransport::read(conn, buf, timeout),
            #[cfg(feature = "rtsps")]
            Self::Tls(conn) => conn.read(buf, timeout),
        }
    }

    fn write(&mut self, data: &[u8], timeout: Duration) -> SdkResult<usize> {
        match self {
            Self::Plain(conn) => RtspTransport::write(conn, data, timeout),
            #[cfg(feature = "rtsps")]
            Self::Tls(conn) => conn.write(data, timeout),
        }
    }

    fn close(&mut self) -> SdkResult<()> {
        match self {
            Self::Plain(conn) => RtspTransport::close(conn),
            #[cfg(feature = "rtsps")]
            Self::Tls(conn) => conn.close(),
        }
    }
}

/// Opens an RTSP (`rtsp://`) or RTSPS (`rtsps://`) transport for a parsed
/// endpoint through the host TCP proxy. `insecure_skip_verify` disables server
/// certificate verification for RTSPS, as cameras commonly present
/// self-signed certificates.
pub fn dial_rtsp_transport(
    endpoint: &RtspEndpoint,
    timeout: Duration,
    insecure_skip_verify: bool,
) -> SdkResult<RtspConnection> {
    let conn = tcp_dial(&endpoint.host, endpoint.port, timeout)?;
    if !endpoint.scheme.eq_ignore_ascii_case("rtsps") {
        let _ = insecure_skip_verify;
        return Ok(RtspConnection::Plain(conn));
    }

    #[cfg(feature = "rtsps")]
    {
        tls::TlsConnection::handshake(conn, &endpoint.host, timeout, insecure_skip_verify)
            .map(|tls| RtspConnection::Tls(Box::new(tls)))
    }

    #[cfg(not(feature = "rtsps"))]
    {
        let mut conn = conn;
        let _ = TcpConnection::close(&mut conn);
        Err(Error::Message(
            "rtsps requires the serviceradar-sdk-rust `rtsps` feature".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests;
