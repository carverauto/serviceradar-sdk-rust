use std::io::{self, Read, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{
    ClientConfig, ClientConnection, DigitallySignedStruct, RootCertStore, SignatureScheme,
};

use crate::error::{Error, SdkResult};
use crate::tcp::TcpConnection;

/// Adapts a host TCP connection to `std::io` for rustls.
struct HostSocket {
    conn: TcpConnection,
    timeout: Duration,
    deadline: Option<Instant>,
}

impl HostSocket {
    fn call_timeout(&self) -> Duration {
        match self.deadline {
            Some(deadline) => deadline.saturating_duration_since(Instant::now()),
            None => self.timeout,
        }
    }
}

impl Read for HostSocket {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.conn
            .read(buf, self.call_timeout())
            .map_err(|err| io::Error::other(err.to_string()))
    }
}

impl Write for HostSocket {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.conn
            .write(buf, self.call_timeout())
            .map_err(|err| io::Error::other(err.to_string()))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A TLS session carried over a host TCP connection.
pub struct TlsConnection {
    tls: ClientConnection,
    sock: HostSocket,
}

impl TlsConnection {
    pub(super) fn handshake(
        conn: TcpConnection,
        host: &str,
        timeout: Duration,
        insecure_skip_verify: bool,
    ) -> SdkResult<Self> {
        let mut sock = HostSocket {
            conn,
            timeout,
            deadline: Instant::now().checked_add(timeout),
        };
        let result = Self::start(host, insecure_skip_verify).and_then(|mut tls| {
            while tls.is_handshaking() {
                tls.complete_io(&mut sock).map_err(tls_io_error)?;
            }
            Ok(tls)
        });
        match result {
            Ok(tls) => {
                sock.deadline = None;
                Ok(Self { tls, sock })
            }
            Err(err) => {
                let _ = sock.conn.close();
                Err(err)
            }
        }
    }

    fn start(host: &str, insecure_skip_verify: bool) -> SdkResult<ClientConnection> {
        let config = client_config(insecure_skip_verify)?;
        let name = ServerName::try_from(host.to_string())
            .map_err(|err| Error::Message(format!("invalid rtsps server name {host:?}: {err}")))?;
        ClientConnection::new(Arc::new(config), name)
            .map_err(|err| Error::Message(format!("rtsps tls setup failed: {err}")))
    }

    pub(super) fn read(&mut self, buf: &mut [u8], timeout: Duration) -> SdkResult<usize> {
        self.sock.timeout = timeout;
        self.sock.deadline = None;
        rustls::Stream::new(&mut self.tls, &mut self.sock)
            .read(buf)
            .map_err(tls_io_error)
    }

    pub(super) fn write(&mut self, data: &[u8], timeout: Duration) -> SdkResult<usize> {
        self.sock.timeout = timeout;
        self.sock.deadline = None;
        let mut stream = rustls::Stream::new(&mut self.tls, &mut self.sock);
        stream.write_all(data).map_err(tls_io_error)?;
        stream.flush().map_err(tls_io_error)?;
        Ok(data.len())
    }

    pub(super) fn close(&mut self) -> SdkResult<()> {
        self.tls.send_close_notify();
        let _ = self.tls.write_tls(&mut self.sock);
        self.sock.conn.close()
    }
}

fn tls_io_error(err: io::Error) -> Error {
    Error::Message(format!("rtsps tls: {err}"))
}

fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls_rustcrypto::provider())
}

fn client_config(insecure_skip_verify: bool) -> SdkResult<ClientConfig> {
    let provider = provider();
    let builder = ClientConfig::builder_with_provider(Arc::clone(&provider))
        .with_safe_default_protocol_versions()
        .map_err(|err| Error::Message(format!("rtsps tls versions: {err}")))?;

    let config = if insecure_skip_verify {
        builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(SkipChainVerification { provider }))
            .with_no_client_auth()
    } else {
        let mut roots = RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        builder.with_root_certificates(roots).with_no_client_auth()
    };
    Ok(config)
}

/// Accepts any server certificate chain, as Go's `InsecureSkipVerify` does, but
/// still verifies the handshake signatures with the provider's algorithms.
#[derive(Debug)]
struct SkipChainVerification {
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for SkipChainVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}
