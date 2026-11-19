use serviceradar_sdk_rust as sdk;

#[derive(Debug, serde::Deserialize)]
#[serde(default)]
struct Config {
    login_url: String,
    api_url: String,
    warn_ms: f64,
    crit_ms: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            login_url: "https://sso.example.com/session".to_string(),
            api_url: "https://app.example.com/app/me".to_string(),
            warn_ms: 1500.0,
            crit_ms: 5000.0,
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn run_check() {
    let _ = sdk::execute(|| {
        let cfg = sdk::load_config_or_default::<Config>()?;

        let journey = sdk::Journey::new("http-synthetic-journey")
            .with_step(
                sdk::JourneyStep::post("login", &cfg.login_url, b"{}".to_vec())
                    .with_follow_redirects(false)
                    .expect_status_code(302)
                    .extract_cookie("session_tok", "sess")
                    .extract_json_field("csrf_tok", "/csrf"),
            )
            .with_step(
                sdk::JourneyStep::get("api-me", &cfg.api_url)
                    .with_header("X-CSRF", "{{csrf_tok}}")
                    .expect_status_code(200)
                    .expect_json_field("/user/role", serde_json::Value::String("admin".to_string()))
                    .expect_max_duration_ms(
                        (cfg.warn_ms > 0.0).then_some(cfg.warn_ms),
                        (cfg.crit_ms > 0.0).then_some(cfg.crit_ms),
                    ),
            );

        journey.run()
    });
}

fn main() {}
