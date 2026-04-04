use std::time::Duration;

use serviceradar_sdk_rust as sdk;

#[derive(Debug, serde::Deserialize)]
#[serde(default)]
struct Config {
    host: String,
    port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 80,
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn run_check() {
    let _ = sdk::execute(|| {
        let config = sdk::load_config_or_default::<Config>()?;

        let mut conn = sdk::tcp_dial(&config.host, config.port, Duration::from_secs(5))?;
        conn.close()?;

        Ok(sdk::PluginResult::ok(format!(
            "tcp {}:{} connected",
            config.host, config.port
        )))
    });
}

fn main() {}
