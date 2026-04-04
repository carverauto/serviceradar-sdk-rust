use std::time::Duration;

use crate::error::{HostError, SdkResult};
use crate::host;

pub fn udp_send_to(
    hostname: impl AsRef<str>,
    port: u16,
    payload: &[u8],
    timeout: Duration,
) -> SdkResult<()> {
    let res = host::udp_send_to(
        hostname.as_ref().as_bytes(),
        u32::from(port),
        payload,
        timeout.as_millis().min(u128::from(u32::MAX)) as u32,
    );
    if res < 0 {
        return Err(HostError {
            code: res,
            op: "udp_sendto",
        }
        .into());
    }

    Ok(())
}

#[cfg(test)]
mod tests;
