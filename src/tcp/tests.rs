use std::collections::VecDeque;
use std::time::Duration;

use crate::host::{TestHostBackend, install_test_backend};
use crate::{Error, tcp_dial};

struct TcpTestHost {
    reads: VecDeque<Vec<u8>>,
}

impl TestHostBackend for TcpTestHost {
    fn tcp_connect(&mut self, _addr: &[u8], _port: u32, _timeout_ms: u32) -> i32 {
        42
    }

    fn tcp_read(&mut self, _handle: u32, buf: &mut [u8], _timeout_ms: u32) -> i32 {
        let payload = self.reads.pop_front().unwrap_or_default();
        let len = payload.len().min(buf.len());
        buf[..len].copy_from_slice(&payload[..len]);
        len as i32
    }

    fn tcp_write(&mut self, _handle: u32, buf: &[u8], _timeout_ms: u32) -> i32 {
        buf.len() as i32
    }

    fn tcp_close(&mut self, _handle: u32) -> i32 {
        0
    }
}

#[test]
fn tcp_connection_requires_handle() {
    let conn = crate::TcpConnection::default();
    let err = conn
        .read(&mut [0_u8; 8], Duration::from_secs(1))
        .expect_err("read should fail");
    assert!(matches!(err, Error::TcpConnectionNotInitialized));
}

#[test]
fn tcp_connection_reads_and_writes() {
    let _guard = install_test_backend(Box::new(TcpTestHost {
        reads: VecDeque::from([b"pong".to_vec()]),
    }));

    let mut conn = tcp_dial("example.com", 443, Duration::from_secs(1)).expect("dial");
    let wrote = conn.write(b"ping", Duration::from_secs(1)).expect("write");
    assert_eq!(wrote, 4);

    let mut buf = [0_u8; 8];
    let read = conn.read(&mut buf, Duration::from_secs(1)).expect("read");
    assert_eq!(&buf[..read], b"pong");

    conn.close().expect("close");
}
