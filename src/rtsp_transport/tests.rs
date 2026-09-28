use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
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

/// Bridges the SDK's host TCP calls to a real loopback socket and records any
/// bytes the plugin writes, for live assertions about what reaches the wire.
#[cfg(not(feature = "rtsps"))]
struct PlainLoopbackHost {
    port: u16,
    stream: Option<TcpStream>,
    writes: Arc<Mutex<Vec<u8>>>,
}

#[cfg(not(feature = "rtsps"))]
impl TestHostBackend for PlainLoopbackHost {
    fn tcp_connect(&mut self, _addr: &[u8], _port: u32, _timeout_ms: u32) -> i32 {
        let stream = TcpStream::connect(("127.0.0.1", self.port)).expect("loopback connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("read timeout");
        self.stream = Some(stream);
        9
    }

    fn tcp_read(&mut self, _handle: u32, buf: &mut [u8], _timeout_ms: u32) -> i32 {
        match self.stream.as_mut().expect("connected").read(buf) {
            Ok(n) => n as i32,
            Err(_) => -1,
        }
    }

    fn tcp_write(&mut self, _handle: u32, buf: &[u8], _timeout_ms: u32) -> i32 {
        self.writes.lock().expect("writes").extend_from_slice(buf);
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

/// Bridges the SDK's host TCP calls to a real IPv6 loopback socket, recording
/// the exact host string handed to the dial so brackets are observable.
struct Ipv6LoopbackHost {
    stream: Option<TcpStream>,
    dialed: Arc<Mutex<Vec<(String, u32)>>>,
}

impl TestHostBackend for Ipv6LoopbackHost {
    fn tcp_connect(&mut self, addr: &[u8], port: u32, _timeout_ms: u32) -> i32 {
        let host = String::from_utf8_lossy(addr).into_owned();
        self.dialed
            .lock()
            .expect("dialed")
            .push((host.clone(), port));
        let stream = TcpStream::connect((host.as_str(), port as u16)).expect("ipv6 connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("read timeout");
        self.stream = Some(stream);
        11
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

struct ShortWriteHost {
    timeouts: Arc<Mutex<Vec<u32>>>,
    writes: u32,
    hold_first: Duration,
    byte_at_a_time: bool,
}

impl TestHostBackend for ShortWriteHost {
    fn tcp_connect(&mut self, _addr: &[u8], _port: u32, _timeout_ms: u32) -> i32 {
        7
    }

    fn tcp_write(&mut self, _handle: u32, buf: &[u8], timeout_ms: u32) -> i32 {
        self.timeouts.lock().expect("timeouts").push(timeout_ms);
        self.writes += 1;
        if self.writes == 1 && !self.hold_first.is_zero() {
            std::thread::sleep(self.hold_first);
        }
        if self.byte_at_a_time {
            return 1;
        }
        buf.len() as i32
    }

    fn tcp_read(&mut self, _handle: u32, buf: &mut [u8], _timeout_ms: u32) -> i32 {
        let msg = b"RTSP/1.0 200 OK\r\nCSeq: 1\r\n\r\n";
        let n = msg.len().min(buf.len());
        buf[..n].copy_from_slice(&msg[..n]);
        n as i32
    }
}

#[test]
fn plain_write_keeps_one_deadline_across_short_writes() {
    let timeouts = Arc::new(Mutex::new(Vec::new()));
    let _guard = install_test_backend(Box::new(ShortWriteHost {
        timeouts: Arc::clone(&timeouts),
        writes: 0,
        hold_first: Duration::from_millis(80),
        byte_at_a_time: true,
    }));

    let endpoint =
        RtspEndpoint::parse("rtsp://camera01.example.com:8554/stream", "", "").expect("endpoint");
    let conn = dial_rtsp_transport(&endpoint, Duration::from_millis(500), false).expect("dial");
    let mut client = RtspClient::new(conn, Duration::from_millis(500), endpoint.clone());
    client
        .do_request("OPTIONS", &endpoint.request_uri, &BTreeMap::new())
        .expect("options");

    let timeouts = timeouts.lock().expect("timeouts").clone();
    assert!(timeouts.len() >= 2, "short writes: {timeouts:?}");
    assert!(
        timeouts.iter().all(|timeout| *timeout >= 1),
        "host write used a zero timeout: {timeouts:?}"
    );
    assert!(
        timeouts[1] + 40 < timeouts[0],
        "later short write kept a fresh timeout: {timeouts:?}"
    );
}

#[test]
fn plain_write_does_not_call_the_host_after_the_deadline() {
    let timeouts = Arc::new(Mutex::new(Vec::new()));
    let _guard = install_test_backend(Box::new(ShortWriteHost {
        timeouts: Arc::clone(&timeouts),
        writes: 0,
        hold_first: Duration::from_millis(80),
        byte_at_a_time: true,
    }));

    let endpoint =
        RtspEndpoint::parse("rtsp://camera01.example.com:8554/stream", "", "").expect("endpoint");
    let conn = dial_rtsp_transport(&endpoint, Duration::from_millis(30), false).expect("dial");
    let mut client = RtspClient::new(conn, Duration::from_millis(30), endpoint.clone());
    let err = client
        .do_request("OPTIONS", &endpoint.request_uri, &BTreeMap::new())
        .expect_err("deadline");
    assert!(err.to_string().contains("deadline"), "{err}");
    assert_eq!(timeouts.lock().expect("timeouts").len(), 1);
}

#[test]
fn ipv6_literal_dials_without_brackets_and_keeps_them_in_urls() {
    // Parse level: the host handed to the dial loses its brackets while the
    // reported/control URLs keep them.
    let endpoint =
        RtspEndpoint::parse("rtsp://[2001:db8::10]:8554/live", "", "").expect("endpoint");
    assert_eq!(endpoint.host, "2001:db8::10");
    assert_eq!(endpoint.port, 8554);
    assert_eq!(endpoint.base_url, "rtsp://[2001:db8::10]:8554");
    assert_eq!(endpoint.authority(), "[2001:db8::10]:8554");
    assert_eq!(
        endpoint.resolve_control_url("trackID=1"),
        "rtsp://[2001:db8::10]:8554/live/trackID=1"
    );

    let implicit = RtspEndpoint::parse("rtsps://[2001:db8::10]/stream", "", "").expect("endpoint");
    assert_eq!(implicit.host, "2001:db8::10");
    assert_eq!(implicit.port, 322);
    assert_eq!(implicit.base_url, "rtsps://[2001:db8::10]");
    assert_eq!(
        format!("{}{}", implicit.base_url, implicit.request_uri),
        "rtsps://[2001:db8::10]/stream"
    );

    // Live: dial a real IPv6 loopback listener with a bracketed literal and
    // confirm the host string handed to the socket has no brackets, while the
    // reported URLs keep them.
    let listener = TcpListener::bind("[::1]:0").expect("bind ipv6");
    let port = listener.local_addr().expect("addr").port();
    let server = thread::spawn(move || {
        let (mut sock, _) = listener.accept().expect("accept");
        sock.set_read_timeout(Some(Duration::from_secs(2)))
            .expect("read timeout");
        let mut buf = [0_u8; 4096];
        let n = sock.read(&mut buf).expect("read request");
        let request = String::from_utf8_lossy(&buf[..n]).into_owned();
        sock.write_all(b"RTSP/1.0 200 OK\r\nCSeq: 1\r\nPublic: OPTIONS\r\n\r\n")
            .expect("write response");
        request
    });

    let dialed = Arc::new(Mutex::new(Vec::new()));
    let _guard = install_test_backend(Box::new(Ipv6LoopbackHost {
        stream: None,
        dialed: Arc::clone(&dialed),
    }));

    let live =
        RtspEndpoint::parse(&format!("rtsp://[::1]:{port}/live"), "", "").expect("endpoint");
    assert_eq!(live.host, "::1");
    assert_eq!(live.base_url, format!("rtsp://[::1]:{port}"));
    assert_eq!(live.authority(), format!("[::1]:{port}"));

    let conn = dial_rtsp_transport(&live, Duration::from_secs(2), false).expect("dial");
    assert!(matches!(conn, RtspConnection::Plain(_)));
    let mut client = RtspClient::new(conn, Duration::from_secs(2), live.clone());
    let response = client
        .do_request("OPTIONS", &live.request_uri, &BTreeMap::new())
        .expect("options");
    assert_eq!(response.status_code, 200);
    client.close().expect("close");

    assert_eq!(
        dialed.lock().expect("dialed").as_slice(),
        &[("::1".to_string(), u32::from(port))]
    );
    let request = server.join().expect("server thread");
    assert!(request.starts_with("OPTIONS /live RTSP/1.0\r\n"), "{request}");
}

#[cfg(not(feature = "rtsps"))]
#[test]
fn rtsps_without_the_feature_fails_instead_of_sending_plaintext() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let received = Arc::new(Mutex::new(Vec::<u8>::new()));
    let server_received = Arc::clone(&received);
    let server = thread::spawn(move || {
        let (mut sock, _) = listener.accept().expect("accept");
        sock.set_read_timeout(Some(Duration::from_millis(500)))
            .expect("read timeout");
        let mut buf = [0_u8; 4096];
        loop {
            match sock.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => server_received
                    .lock()
                    .expect("received")
                    .extend_from_slice(&buf[..n]),
                Err(_) => break,
            }
        }
    });

    let writes = Arc::new(Mutex::new(Vec::<u8>::new()));
    let _guard = install_test_backend(Box::new(PlainLoopbackHost {
        port,
        stream: None,
        writes: Arc::clone(&writes),
    }));

    let endpoint =
        RtspEndpoint::parse(&format!("rtsps://127.0.0.1:{port}/stream"), "", "")
            .expect("endpoint");
    let err = dial_rtsp_transport(&endpoint, Duration::from_secs(2), false)
        .expect_err("rtsps needs the feature");
    assert!(err.to_string().contains("rtsps"), "{err}");

    server.join().expect("server thread");
    assert!(
        writes.lock().expect("writes").is_empty(),
        "no plaintext may be sent to an rtsps endpoint"
    );
    assert!(
        received.lock().expect("received").is_empty(),
        "the rtsps endpoint received plaintext bytes"
    );
}

#[cfg(feature = "rtsps")]
mod rtsps {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::{AtomicBool, Ordering};
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
            write_timeouts: Arc::new(Mutex::new(Vec::new())),
            pending: Vec::new(),
            primed: false,
            first_read_hold: Duration::from_millis(400),
        }));

        let endpoint =
            RtspEndpoint::parse("rtsps://camera01.example.com/stream", "", "").expect("endpoint");
        let _ = dial_rtsp_transport(&endpoint, Duration::from_millis(800), true);

        let reads = read_timeouts.lock().expect("timeouts").clone();
        assert!(reads.len() >= 2, "handshake reads: {reads:?}");
        assert!(
            reads.iter().all(|timeout| *timeout >= 1),
            "host was called with timeout 0: {reads:?}"
        );
        assert!(
            reads[1] + 100 < reads[0],
            "later handshake read kept a fresh timeout {reads:?}"
        );
        let _ = server.join().expect("server thread");
    }

    #[test]
    fn rtsps_handshake_does_not_call_the_host_after_the_deadline() {
        let (port, server) = spawn_server();
        let read_timeouts = Arc::new(Mutex::new(Vec::new()));
        let write_timeouts = Arc::new(Mutex::new(Vec::new()));
        let _guard = install_test_backend(Box::new(StallingHost {
            port,
            stream: None,
            read_timeouts: Arc::clone(&read_timeouts),
            write_timeouts: Arc::clone(&write_timeouts),
            pending: Vec::new(),
            primed: false,
            first_read_hold: Duration::from_millis(500),
        }));

        let endpoint =
            RtspEndpoint::parse("rtsps://camera01.example.com/stream", "", "").expect("endpoint");
        let _ = dial_rtsp_transport(&endpoint, Duration::from_millis(200), true);

        let reads = read_timeouts.lock().expect("timeouts").clone();
        let writes = write_timeouts.lock().expect("timeouts").clone();
        assert!(!reads.is_empty(), "handshake made no reads");
        assert!(
            reads
                .iter()
                .chain(writes.iter())
                .all(|timeout| *timeout >= 1),
            "expired deadline still called the host: reads {reads:?} writes {writes:?}"
        );
        let _ = server.join().expect("server thread");
    }

    #[test]
    fn rtsps_read_keeps_one_deadline_across_record_fragments() {
        let (port, server) = spawn_server();
        let application = Arc::new(AtomicBool::new(false));
        let app_reads = Arc::new(Mutex::new(Vec::new()));
        let _guard = install_test_backend(Box::new(ChunkedResponseHost {
            port,
            stream: None,
            application: Arc::clone(&application),
            app_reads: Arc::clone(&app_reads),
            app_writes: 0,
            held: false,
        }));

        let endpoint =
            RtspEndpoint::parse("rtsps://camera01.example.com/stream", "", "").expect("endpoint");
        let conn =
            dial_rtsp_transport(&endpoint, Duration::from_secs(5), true).expect("rtsps handshake");
        application.store(true, Ordering::SeqCst);
        let mut client = RtspClient::new(conn, Duration::from_millis(500), endpoint.clone());
        let _ = client.do_request("OPTIONS", &endpoint.request_uri, &Default::default());

        let reads = app_reads.lock().expect("timeouts").clone();
        assert!(reads.len() >= 2, "response reads: {reads:?}");
        assert!(
            reads.iter().all(|timeout| *timeout >= 1),
            "host was called with timeout 0: {reads:?}"
        );
        assert!(
            reads[1] + 100 < reads[0],
            "later record fragment kept a fresh timeout {reads:?}"
        );
        let _ = server.join().expect("server thread");
    }

    struct ZeroWriteHost {
        writes: Arc<Mutex<u32>>,
    }

    impl TestHostBackend for ZeroWriteHost {
        fn tcp_connect(&mut self, _addr: &[u8], _port: u32, _timeout_ms: u32) -> i32 {
            4
        }

        fn tcp_write(&mut self, _handle: u32, _buf: &[u8], _timeout_ms: u32) -> i32 {
            *self.writes.lock().expect("writes") += 1;
            0
        }
    }

    #[test]
    fn rtsps_zero_write_fails_without_spinning() {
        let writes = Arc::new(Mutex::new(0_u32));
        let _guard = install_test_backend(Box::new(ZeroWriteHost {
            writes: Arc::clone(&writes),
        }));

        let endpoint =
            RtspEndpoint::parse("rtsps://camera01.example.com/stream", "", "").expect("endpoint");
        let started = std::time::Instant::now();
        let err =
            dial_rtsp_transport(&endpoint, Duration::from_secs(2), true).expect_err("zero write");
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "handshake spun until the deadline"
        );
        assert_eq!(*writes.lock().expect("writes"), 1);
        assert!(err.to_string().contains("no progress"), "{err}");
    }

    struct StallingHost {
        port: u16,
        stream: Option<TcpStream>,
        read_timeouts: Arc<Mutex<Vec<u32>>>,
        write_timeouts: Arc<Mutex<Vec<u32>>>,
        pending: Vec<u8>,
        primed: bool,
        first_read_hold: Duration,
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
                thread::sleep(self.first_read_hold);
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

        fn tcp_write(&mut self, _handle: u32, buf: &[u8], timeout_ms: u32) -> i32 {
            self.write_timeouts
                .lock()
                .expect("timeouts")
                .push(timeout_ms);
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

    struct ChunkedResponseHost {
        port: u16,
        stream: Option<TcpStream>,
        application: Arc<AtomicBool>,
        app_reads: Arc<Mutex<Vec<u32>>>,
        app_writes: u32,
        held: bool,
    }

    impl TestHostBackend for ChunkedResponseHost {
        fn tcp_connect(&mut self, _addr: &[u8], _port: u32, _timeout_ms: u32) -> i32 {
            let stream = TcpStream::connect(("127.0.0.1", self.port)).expect("loopback connect");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("read timeout");
            self.stream = Some(stream);
            3
        }

        fn tcp_read(&mut self, _handle: u32, buf: &mut [u8], timeout_ms: u32) -> i32 {
            if !self.application.load(Ordering::SeqCst) || self.app_writes == 0 {
                return match self.stream.as_mut().expect("connected").read(buf) {
                    Ok(n) => n as i32,
                    Err(_) => -1,
                };
            }
            self.app_reads.lock().expect("timeouts").push(timeout_ms);
            let sock = self.stream.as_mut().expect("connected");
            if !self.held {
                self.held = true;
                thread::sleep(Duration::from_millis(300));
                let mut byte = [0_u8; 1];
                return match sock.read(&mut byte) {
                    Ok(0) => -1,
                    Ok(_) => {
                        buf[0] = byte[0];
                        1
                    }
                    Err(_) => -1,
                };
            }
            let _ = sock.set_nonblocking(true);
            match sock.read(buf) {
                Ok(0) => -1,
                Ok(n) => n as i32,
                Err(_) => -1,
            }
        }

        fn tcp_write(&mut self, _handle: u32, buf: &[u8], _timeout_ms: u32) -> i32 {
            if self.application.load(Ordering::SeqCst) {
                self.app_writes += 1;
            }
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
