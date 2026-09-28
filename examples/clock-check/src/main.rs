//! Proves the WASI clock and sleep work inside a plugin: it reads wall-clock
//! time, sleeps, and reports the monotonic time that elapsed. Built for
//! `wasm32-wasip1` this runs under the agent's runtime; built for
//! `wasm32-unknown-unknown` the clock calls trap, which is why the SDK targets
//! WASI.
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serviceradar_sdk_rust as sdk;

#[derive(Debug, serde::Deserialize)]
#[serde(default)]
struct Config {
    sleep_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self { sleep_ms: 20 }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn run_check() {
    let _ = sdk::execute(|| {
        let config = sdk::load_config_or_default::<Config>()?;
        let wall = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| sdk::Error::Message(err.to_string()))?;
        let started = Instant::now();
        std::thread::sleep(Duration::from_millis(config.sleep_ms));
        let elapsed_ms = started.elapsed().as_millis();

        Ok(sdk::PluginResult::new()
            .with_summary(format!("slept {elapsed_ms}ms at unix {}", wall.as_secs()))
            .with_label("elapsed_ms", elapsed_ms.to_string()))
    });
}

fn main() {}
