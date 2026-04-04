use std::time::Duration;

use serviceradar_sdk_rust as sdk;

#[derive(Debug, serde::Deserialize)]
#[serde(default)]
struct Config {
    host: String,
    port: u16,
    payload: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8125,
            payload: "ping".to_string(),
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn run_check() {
    let _ = sdk::execute(|| {
        let config = sdk::load_config_or_default::<Config>()?;

        sdk::udp_send_to(
            &config.host,
            config.port,
            config.payload.as_bytes(),
            Duration::from_secs(5),
        )?;

        Ok(sdk::PluginResult::ok(format!(
            "udp {}:{} sent {} bytes",
            config.host,
            config.port,
            config.payload.len()
        )))
    });
}

fn main() {}
