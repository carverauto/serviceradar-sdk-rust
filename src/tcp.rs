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
mod tests;
