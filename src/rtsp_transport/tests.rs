use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::host::{TestHostBackend, install_test_backend};
use crate::rtsp::{RtspClient, RtspEndpoint};

use super::{RtspConnection, dial_rtsp_transport};

#[derive(Default)]
struct Wire {
    dialed: Vec<(String, u32)>,
    written: Vec<u8>,
    closed: bool,
}

struct PlainRtspHost {
    wire: Arc<Mutex<Wire>>,
    reply: Vec<u8>,
}

impl TestHostBackend for PlainRtspHost {
    fn tcp_connect(&mut self, addr: &[u8], port: u32, _timeout_ms: u32) -> i32 {
        self.wire
            .lock()
            .unwrap()
            .dialed
            .push((String::from_utf8_lossy(addr).into_owned(), port));
        7
    }

    fn tcp_write(&mut self, handle: u32, buf: &[u8], _timeout_ms: u32) -> i32 {
        assert_eq!(handle, 7);
        self.wire.lock().unwrap().written.extend_from_slice(buf);
        buf.len() as i32
    }

    fn tcp_read(&mut self, handle: u32, buf: &mut [u8], _timeout_ms: u32) -> i32 {
        assert_eq!(handle, 7);
        let len = self.reply.len().min(buf.len());
        buf[..len].copy_from_slice(&self.reply[..len]);
        self.reply.drain(..len);
        len as i32
    }

    fn tcp_close(&mut self, handle: u32) -> i32 {
        assert_eq!(handle, 7);
        self.wire.lock().unwrap().closed = true;
        0
    }
}

#[test]
fn plain_rtsp_dials_host_tcp_and_carries_requests() {
    let wire = Arc::new(Mutex::new(Wire::default()));
    let _guard = install_test_backend(Box::new(PlainRtspHost {
        wire: Arc::clone(&wire),
        reply: b"RTSP/1.0 200 OK\r\nCSeq: 1\r\nPublic: OPTIONS, DESCRIBE\r\n\r\n".to_vec(),
    }));

    let endpoint =
        RtspEndpoint::parse("rtsp://camera01.example.com:8554/stream", "", "").expect("endpoint");
    let conn = dial_rtsp_transport(&endpoint, Duration::from_secs(2), false).expect("dial rtsp");
    assert!(matches!(conn, RtspConnection::Plain(_)));

    let mut client = RtspClient::new(conn, Duration::from_secs(2), endpoint.clone());
    let response = client
        .do_request("OPTIONS", &endpoint.request_uri, &BTreeMap::new())
        .expect("options");
    assert_eq!(response.status_code, 200);
    client.close().expect("close");

    let wire = wire.lock().unwrap();
    assert_eq!(
        wire.dialed,
        vec![("camera01.example.com".to_string(), 8554)]
    );
    let request = String::from_utf8_lossy(&wire.written);
    assert!(request.starts_with("OPTIONS /stream RTSP/1.0\r\n"));
    assert!(request.contains("CSeq: 1\r\n"));
    assert!(wire.closed);
}

#[cfg(not(feature = "rtsps"))]
#[test]
fn rtsps_without_the_feature_fails_instead_of_sending_plaintext() {
    let wire = Arc::new(Mutex::new(Wire::default()));
    let _guard = install_test_backend(Box::new(PlainRtspHost {
        wire: Arc::clone(&wire),
        reply: Vec::new(),
    }));

    let endpoint =
        RtspEndpoint::parse("rtsps://camera01.example.com/stream", "", "").expect("endpoint");
    let err = dial_rtsp_transport(&endpoint, Duration::from_secs(2), false)
        .expect_err("rtsps needs the feature");
    assert!(err.to_string().contains("rtsps"));
    let wire = wire.lock().unwrap();
    assert!(
        wire.written.is_empty(),
        "no plaintext may be sent to an rtsps endpoint"
    );
    assert!(wire.closed);
}

#[cfg(feature = "rtsps")]
mod rtsps {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    use rustls::pki_types::pem::PemObject;
    use rustls::pki_types::{CertificateDer, PrivateKeyDer};

    use crate::host::{TestHostBackend, install_test_backend};
    use crate::rtsp::{RtspClient, RtspEndpoint};

    use super::super::{RtspConnection, dial_rtsp_transport};

    const CERT: &[u8] = include_bytes!("../../testdata/rtsps/server.cert.pem");
    const KEY: &[u8] = include_bytes!("../../testdata/rtsps/server.key.pem");

    /// Bridges the SDK's host TCP calls to a real loopback socket.
    struct LoopbackHost {
        port: u16,
        stream: Option<TcpStream>,
    }

    impl TestHostBackend for LoopbackHost {
        fn tcp_connect(&mut self, _addr: &[u8], _port: u32, _timeout_ms: u32) -> i32 {
            let stream = TcpStream::connect(("127.0.0.1", self.port)).expect("loopback connect");
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("read timeout");
            self.stream = Some(stream);
            3
        }

        fn tcp_read(&mut self, _handle: u32, buf: &mut [u8], _timeout_ms: u32) -> i32 {
            match self.stream.as_mut().expect("connected").read(buf) {
                Ok(n) => n as i32,
                Err(_) => -1,
            }
        }

        fn tcp_write(&mut self, _handle: u32, buf: &[u8], _timeout_ms: u32) -> i32 {
            match self.stream.as_mut().expect("connected").write(buf) {
                Ok(n) => n as i32,
                Err(_) => -1,
            }
        }

        fn tcp_close(&mut self, _handle: u32) -> i32 {
            self.stream = None;
            0
        }
    }

    /// Serves one RTSPS connection: TLS handshake, then answers the first
    /// request with `RTSP/1.0 200 OK`. Returns the request text it received.
    fn spawn_server() -> (u16, thread::JoinHandle<Option<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let handle = thread::spawn(move || {
            let certs = vec![CertificateDer::from_pem_slice(CERT).expect("cert")];
            let key = PrivateKeyDer::from_pem_slice(KEY).expect("key");
            let config = rustls::ServerConfig::builder_with_provider(Arc::new(
                rustls_rustcrypto::provider(),
            ))
            .with_safe_default_protocol_versions()
            .expect("versions")
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .expect("server config");

            let (mut socket, _) = listener.accept().ok()?;
            socket.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
            let mut conn = rustls::ServerConnection::new(Arc::new(config)).ok()?;
            let mut stream = rustls::Stream::new(&mut conn, &mut socket);
            let mut buf = [0_u8; 1024];
            let n = stream.read(&mut buf).ok()?;
            let request = String::from_utf8_lossy(&buf[..n]).into_owned();
            stream
                .write_all(b"RTSP/1.0 200 OK\r\nCSeq: 1\r\nPublic: OPTIONS\r\n\r\n")
                .ok()?;
            stream.flush().ok()?;
            Some(request)
        });
        (port, handle)
    }

    #[test]
    fn rtsps_handshakes_over_host_tcp_and_carries_requests() {
        let (port, server) = spawn_server();
        let _guard = install_test_backend(Box::new(LoopbackHost { port, stream: None }));

        let endpoint =
            RtspEndpoint::parse("rtsps://camera01.example.com/stream", "", "").expect("endpoint");
        let conn =
            dial_rtsp_transport(&endpoint, Duration::from_secs(5), true).expect("rtsps handshake");
        assert!(matches!(conn, RtspConnection::Tls(_)));

        let mut client = RtspClient::new(conn, Duration::from_secs(5), endpoint.clone());
        let response = client
            .do_request("OPTIONS", &endpoint.request_uri, &Default::default())
            .expect("options over tls");
        assert_eq!(response.status_code, 200);

        let request = server
            .join()
            .expect("server thread")
            .expect("server saw request");
        assert!(request.starts_with("OPTIONS /stream RTSP/1.0\r\n"));
    }

    #[test]
    fn rtsps_rejects_an_untrusted_certificate_unless_verification_is_skipped() {
        let (port, server) = spawn_server();
        let _guard = install_test_backend(Box::new(LoopbackHost { port, stream: None }));

        let endpoint =
            RtspEndpoint::parse("rtsps://camera01.example.com/stream", "", "").expect("endpoint");
        let err = dial_rtsp_transport(&endpoint, Duration::from_secs(5), false)
            .expect_err("self-signed certificate must be rejected");
        assert!(err.to_string().contains("rtsps tls"), "{err}");
        assert!(server.join().expect("server thread").is_none());
    }

    #[test]
    fn rtsps_handshake_uses_one_deadline_when_a_later_flight_stalls() {
        let (port, server) = spawn_server();
        let read_timeouts = Arc::new(Mutex::new(Vec::new()));
        let _guard = install_test_backend(Box::new(StallingHost {
            port,
            stream: None,
            read_timeouts: Arc::clone(&read_timeouts),
            pending: Vec::new(),
            primed: false,
        }));

        let endpoint =
            RtspEndpoint::parse("rtsps://camera01.example.com/stream", "", "").expect("endpoint");
        let _ = dial_rtsp_transport(&endpoint, Duration::from_millis(800), true);

        let reads = read_timeouts.lock().expect("timeouts").clone();
        assert!(reads.len() >= 2, "handshake reads: {reads:?}");
        assert!(
            reads[1] + 100 < reads[0],
            "later handshake read kept a fresh timeout {reads:?}"
        );
        let _ = server.join().expect("server thread");
    }

    struct StallingHost {
        port: u16,
        stream: Option<TcpStream>,
        read_timeouts: Arc<Mutex<Vec<u32>>>,
        pending: Vec<u8>,
        primed: bool,
    }

    impl TestHostBackend for StallingHost {
        fn tcp_connect(&mut self, _addr: &[u8], _port: u32, _timeout_ms: u32) -> i32 {
            let stream = TcpStream::connect(("127.0.0.1", self.port)).expect("loopback connect");
            stream
                .set_read_timeout(Some(Duration::from_millis(200)))
                .expect("read timeout");
            self.stream = Some(stream);
            3
        }

        fn tcp_read(&mut self, _handle: u32, buf: &mut [u8], timeout_ms: u32) -> i32 {
            self.read_timeouts
                .lock()
                .expect("timeouts")
                .push(timeout_ms);
            if !self.primed {
                self.primed = true;
                thread::sleep(Duration::from_millis(400));
                let sock = self.stream.as_mut().expect("connected");
                let mut tmp = [0_u8; 8192];
                loop {
                    match sock.read(&mut tmp) {
                        Ok(0) => break,
                        Ok(n) => self.pending.extend_from_slice(&tmp[..n]),
                        Err(_) => break,
                    }
                }
            }
            if self.pending.is_empty() || buf.is_empty() {
                return -1;
            }
            let n = buf.len().min(self.pending.len()).min(32);
            buf[..n].copy_from_slice(&self.pending[..n]);
            self.pending.drain(..n);
            n as i32
        }

        fn tcp_write(&mut self, _handle: u32, buf: &[u8], _timeout_ms: u32) -> i32 {
            match self.stream.as_mut().expect("connected").write(buf) {
                Ok(n) => n as i32,
                Err(_) => -1,
            }
        }

        fn tcp_close(&mut self, _handle: u32) -> i32 {
            self.stream = None;
            0
        }
    }
}
