use std::collections::BTreeMap;

use serviceradar_sdk_rust as sdk;

#[derive(Debug, serde::Deserialize)]
#[serde(default)]
struct Config {
    url: String,
    warn_ms: f64,
    crit_ms: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            url: "https://example.com/health".to_string(),
            warn_ms: 0.0,
            crit_ms: 0.0,
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn run_check() {
    let _ = sdk::execute(|| {
        let config = sdk::load_config_or_default::<Config>()?;

        let response = sdk::HttpClient::default().get(&config.url)?;
        let latency_ms = response.duration.as_millis() as f64;
        let thresholds = sdk::Thresholds::new(config.warn_ms, config.crit_ms);
        let status = status_for_latency(latency_ms, &thresholds);

        let mut table = BTreeMap::new();
        table.insert("Status".to_string(), response.status.to_string());
        table.insert("URL".to_string(), config.url.clone());

        Ok(sdk::PluginResult::new()
            .with_summary(format!("http {} in {:.0}ms", response.status, latency_ms))
            .with_thresholds(latency_ms, thresholds.warn, thresholds.crit)
            .with_metric_spec(
                sdk::Metric::new("latency_ms", latency_ms)
                    .with_unit("ms")
                    .with_thresholds(&thresholds),
            )
            .with_widget(sdk::Widget::stat_card(
                "Latency",
                format!("{latency_ms:.0}ms"),
                tone_for_status(status),
            ))
            .with_widget(sdk::Widget::table(table, "full").expect("table widget"))
            .with_widget(
                sdk::Widget::sparkline(
                    "Latency (ms)",
                    sparkline_series(latency_ms),
                    tone_for_status(status),
                )
                .expect("sparkline widget"),
            )
            .with_widget(
                sdk::Widget::markdown(format!(
                    "**Health Check**\n\n- URL: `{}`\n- Status: `{}`\n- Latency: `{latency_ms:.0}ms`",
                    config.url, response.status
                ))
                .expect("markdown widget"),
            ))
    });
}

fn main() {}

fn tone_for_status(status: sdk::Status) -> &'static str {
    match status {
        sdk::Status::Ok => "success",
        sdk::Status::Critical => "critical",
        sdk::Status::Warning => "warning",
        sdk::Status::Unknown => "neutral",
    }
}

fn sparkline_series(latency: f64) -> Vec<f64> {
    let seed = if latency <= 0.0 { 10.0 } else { latency };
    (0..8)
        .map(|index| seed + (index as f64 - 3.0) * 2.0)
        .collect()
}

fn status_for_latency(latency_ms: f64, thresholds: &sdk::Thresholds) -> sdk::Status {
    let mut result = sdk::PluginResult::new();
    result.apply_thresholds(latency_ms, thresholds.warn, thresholds.crit);
    result.status()
}
