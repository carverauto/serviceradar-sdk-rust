use std::time::Duration;

use crate::error::{HostError, SdkResult};
use crate::host;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TcpConnection {
    handle: u32,
}

pub fn tcp_dial(
    hostname: impl AsRef<str>,
    port: u16,
    timeout: Duration,
) -> SdkResult<TcpConnection> {
    let res = host::tcp_connect(
        hostname.as_ref().as_bytes(),
        u32::from(port),
        timeout.as_millis().min(u128::from(u32::MAX)) as u32,
    );
    if res < 0 {
        return Err(HostError {
            code: res,
            op: "tcp_connect",
        }
        .into());
    }

    Ok(TcpConnection { handle: res as u32 })
}

impl TcpConnection {
    pub fn read(&self, buf: &mut [u8], timeout: Duration) -> SdkResult<usize> {
        if self.handle == 0 {
            return Err(crate::error::Error::TcpConnectionNotInitialized);
        }
        if buf.is_empty() {
            return Ok(0);
        }

        let res = host::tcp_read(
            self.handle,
            buf,
            timeout.as_millis().min(u128::from(u32::MAX)) as u32,
        );
        if res < 0 {
            return Err(HostError {
                code: res,
                op: "tcp_read",
            }
            .into());
        }

        Ok(res as usize)
    }

    pub fn write(&self, data: &[u8], timeout: Duration) -> SdkResult<usize> {
        if self.handle == 0 {
            return Err(crate::error::Error::TcpConnectionNotInitialized);
        }
        if data.is_empty() {
            return Ok(0);
        }

        let res = host::tcp_write(
            self.handle,
            data,
            timeout.as_millis().min(u128::from(u32::MAX)) as u32,
        );
        if res < 0 {
            return Err(HostError {
                code: res,
                op: "tcp_write",
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
        let res = host::tcp_close(handle);
        if res < 0 {
            return Err(HostError {
                code: res,
                op: "tcp_close",
            }
            .into());
        }

        Ok(())
    }

    pub fn handle(&self) -> u32 {
        self.handle
    }
}

#[cfg(test)]
mod tests {
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
}
