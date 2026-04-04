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
mod tests {
    use std::sync::{Arc, Mutex};

    use serde::Deserialize;

    use crate::host::{TestHostBackend, install_test_backend};

    use super::{get_config, get_config_bytes, load_config, load_config_or_default};

    #[derive(Debug, Default, Deserialize, PartialEq)]
    #[serde(default)]
    struct TestConfig {
        url: String,
        warn_ms: f64,
    }

    struct ConfigHost {
        payload: Vec<u8>,
        calls: Arc<Mutex<usize>>,
    }

    impl TestHostBackend for ConfigHost {
        fn get_config(&mut self, buf: &mut [u8]) -> i32 {
            *self.calls.lock().expect("calls mutex") += 1;
            let len = self.payload.len().min(buf.len());
            buf[..len].copy_from_slice(&self.payload[..len]);
            len as i32
        }
    }

    #[test]
    fn config_helpers_decode_host_payload() {
        let calls = Arc::new(Mutex::new(0));
        let _guard = install_test_backend(Box::new(ConfigHost {
            payload: br#"{"url":"https://example.com","warn_ms":50}"#.to_vec(),
            calls: Arc::clone(&calls),
        }));

        let bytes = get_config_bytes().expect("get config bytes");
        assert!(!bytes.is_empty());

        let mut config = TestConfig::default();
        load_config(&mut config).expect("load config");
        assert_eq!(
            config,
            TestConfig {
                url: "https://example.com".to_string(),
                warn_ms: 50.0,
            }
        );

        let loaded = get_config::<TestConfig>()
            .expect("get config")
            .expect("config should be present");
        assert_eq!(loaded, config);
        assert!(*calls.lock().expect("calls mutex") >= 2);
    }

    #[test]
    fn load_config_or_default_preserves_default_on_empty_payload() {
        let _guard = install_test_backend(Box::new(ConfigHost {
            payload: Vec::new(),
            calls: Arc::new(Mutex::new(0)),
        }));

        let config = load_config_or_default::<TestConfig>().expect("load default config");
        assert_eq!(config, TestConfig::default());
    }
}
