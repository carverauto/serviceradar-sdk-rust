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
