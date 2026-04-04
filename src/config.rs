use serde::de::DeserializeOwned;

use crate::error::{HOST_ERR_INVALID, HOST_ERR_TOO_LARGE, HostError, SdkResult};
use crate::host;

pub const MAX_PAYLOAD_BYTES: usize = 2 * 1024 * 1024;

pub fn get_config_bytes() -> SdkResult<Vec<u8>> {
    let sizes = [16 * 1024, 64 * 1024, 256 * 1024, MAX_PAYLOAD_BYTES];

    for (index, size) in sizes.into_iter().enumerate() {
        if size == 0 {
            return Ok(Vec::new());
        }

        let mut buf = vec![0_u8; size];
        let res = host::get_config(&mut buf);

        if res == HOST_ERR_TOO_LARGE && index + 1 < sizes.len() {
            continue;
        }

        if res < 0 {
            return Err(HostError {
                code: res,
                op: "get_config",
            }
            .into());
        }

        if res == 0 {
            return Ok(Vec::new());
        }

        let len = res as usize;
        if len > size {
            return Err(HostError {
                code: HOST_ERR_INVALID,
                op: "get_config",
            }
            .into());
        }

        buf.truncate(len);
        return Ok(buf);
    }

    Err(HostError {
        code: HOST_ERR_TOO_LARGE,
        op: "get_config",
    }
    .into())
}

pub fn load_config<T>(out: &mut T) -> SdkResult<()>
where
    T: DeserializeOwned,
{
    let data = get_config_bytes()?;
    if data.is_empty() {
        return Ok(());
    }

    *out = serde_json::from_slice(&data)?;
    Ok(())
}

pub fn get_config<T>() -> SdkResult<Option<T>>
where
    T: DeserializeOwned,
{
    let data = get_config_bytes()?;
    if data.is_empty() {
        return Ok(None);
    }

    Ok(Some(serde_json::from_slice(&data)?))
}

pub fn load_config_or_default<T>() -> SdkResult<T>
where
    T: DeserializeOwned + Default,
{
    let mut value = T::default();
    load_config(&mut value)?;
    Ok(value)
}

#[cfg(test)]
mod tests;
