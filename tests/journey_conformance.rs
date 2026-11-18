use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};

use base64::Engine;
use serde::Deserialize;
use serde_json::Value;
use serviceradar_sdk_rust::local::{LocalHostOptions, run_local_host};
use serviceradar_sdk_rust::{
    CookieJar, HttpResponse, HttpTlsInfo, HttpTrace, Journey, JourneyStep, emit_telemetry, execute,
};

#[derive(Deserialize)]
struct ConformanceDoc {
    schema: String,
    now_unix: u64,
    cookie_jar_vectors: Vec<CookieJarVec>,
    journey_scenarios: Vec<JourneyScen>,
}

#[derive(Deserialize)]
struct CookieJarVec {
    name: String,
    steps: Vec<CookieJarStep>,
}

#[derive(Deserialize)]
struct CookieJarStep {
    action: String,
    url: String,
    #[serde(default)]
    set_cookies: Vec<String>,
    #[serde(default)]
    want_header: String,
}

#[derive(Deserialize)]
struct JourneyScen {
    name: String,
    journey_name: String,
    #[serde(default)]
    web_property_id: String,
    #[serde(default)]
    device_id: String,
    #[serde(default)]
    target_ip: String,
    steps: Vec<JourneyScenStep>,
    expected: JourneyExpect,
}

#[derive(Deserialize)]
struct JourneyScenStep {
    name: String,
    method: String,
    url: String,
    #[serde(default)]
    headers: BTreeMap<String, String>,
    follow_redirects: Option<bool>,
    #[serde(default)]
    expect_status: Vec<i32>,
    expect_header: Option<NameValue>,
    #[serde(default)]
    expect_body_contains: String,
    expect_json_field: Option<PointerValue>,
    expect_max_duration_ms: Option<ThresholdF64>,
    expect_min_cert_days: Option<ThresholdI64>,
    extract_cookie: Option<ExtractCookieSpec>,
    extract_json_field: Option<ExtractJsonSpec>,
    #[serde(default)]
    expected_request_headers: BTreeMap<String, String>,
    mock_response: Option<MockResponse>,
}

#[derive(Deserialize)]
struct NameValue {
    name: String,
    value: String,
}

#[derive(Deserialize)]
struct PointerValue {
    pointer: String,
    value: Value,
}

#[derive(Deserialize)]
struct ThresholdF64 {
    warn: f64,
    crit: f64,
}

#[derive(Deserialize)]
struct ThresholdI64 {
    warn: i64,
    crit: i64,
}

#[derive(Deserialize)]
struct ExtractCookieSpec {
    cookie: String,
    var: String,
}

#[derive(Deserialize)]
struct ExtractJsonSpec {
    pointer: String,
    var: String,
}

#[derive(Deserialize)]
struct MockResponse {
    status: i32,
    #[serde(default)]
    headers: BTreeMap<String, String>,
    #[serde(default)]
    set_cookies: Vec<String>,
    #[serde(default)]
    body: String,
    trace: Option<HttpTrace>,
    tls: Option<HttpTlsInfo>,
}

#[derive(Deserialize)]
struct JourneyExpect {
    status: String,
    http_calls: usize,
    steps_completed: usize,
    step_statuses: Vec<String>,
    alert_hint: bool,
    journey_available: f64,
    #[serde(default)]
    forbidden_substrings: Vec<String>,
    #[serde(default)]
    expected_metric_names: Vec<String>,
}

#[test]
fn journey_conformance_vectors_pass() {
    let raw = include_str!("../fixtures/journey_conformance.json");
    let doc: ConformanceDoc = serde_json::from_str(raw).expect("decode conformance fixture");
    assert_eq!(doc.schema, "serviceradar.journey_conformance.v1");
    let now = UNIX_EPOCH + Duration::from_secs(doc.now_unix);

    for vec in doc.cookie_jar_vectors {
        let mut jar = CookieJar::new();
        for (idx, step) in vec.steps.iter().enumerate() {
            match step.action.as_str() {
                "store" => jar.store_set_cookies_at(&step.url, &step.set_cookies, now),
                "query" => {
                    let got = jar
                        .cookie_header_for_url_at(&step.url, now)
                        .unwrap_or_default();
                    assert_eq!(
                        got, step.want_header,
                        "cookie_jar/{} step {idx} ({})",
                        vec.name, step.url
                    );
                }
                other => panic!("unknown action {other}"),
            }
        }
    }

    for scen in doc.journey_scenarios {
        run_journey_scenario(scen, now);
    }
}

fn run_journey_scenario(scen: JourneyScen, now: std::time::SystemTime) {
    let mut journey = Journey::new(&scen.journey_name).with_now(now);
    if !scen.web_property_id.is_empty() {
        journey = journey.with_web_property_id(&scen.web_property_id);
    }
    if !scen.device_id.is_empty() {
        journey = journey.with_device_id(&scen.device_id);
    }
    if !scen.target_ip.is_empty() {
        journey = journey.with_target_device_ip(&scen.target_ip);
    }

    for s in &scen.steps {
        let mut step = JourneyStep::new(&s.name, &s.method, &s.url).with_headers(s.headers.clone());
        if let Some(follow) = s.follow_redirects {
            step = step.with_follow_redirects(follow);
        }
        if !s.expect_status.is_empty() {
            step = step.expect_status(s.expect_status.clone());
        }
        if let Some(h) = &s.expect_header {
            step = step.expect_header_exact(&h.name, &h.value);
        }
        if !s.expect_body_contains.is_empty() {
            step = step.expect_body_contains(&s.expect_body_contains);
        }
        if let Some(jf) = &s.expect_json_field {
            step = step.expect_json_field(&jf.pointer, jf.value.clone());
        }
        if let Some(d) = &s.expect_max_duration_ms {
            step = step.expect_max_duration_ms(
                (d.warn > 0.0).then_some(d.warn),
                (d.crit > 0.0).then_some(d.crit),
            );
        }
        if let Some(c) = &s.expect_min_cert_days {
            step = step.expect_min_cert_days(Some(c.warn), Some(c.crit));
        }
        if let Some(ec) = &s.extract_cookie {
            step = step.extract_cookie(&ec.var, &ec.cookie);
        }
        if let Some(ej) = &s.extract_json_field {
            step = step.extract_json_field(&ej.var, &ej.pointer);
        }
        journey.add_step(step);
    }

    let calls = Arc::new(Mutex::new(0_usize));
    let calls_handler = Arc::clone(&calls);
    let steps_for_handler: Vec<(BTreeMap<String, String>, Option<MockResponse>)> = scen
        .steps
        .into_iter()
        .map(|s| (s.expected_request_headers, s.mock_response))
        .collect();
    let scenario_name = scen.name.clone();
    let handler_name = scenario_name.clone();

    let report_slot = Arc::new(Mutex::new(None));
    let report_clone = Arc::clone(&report_slot);

    let (capture, res) = run_local_host(
        LocalHostOptions {
            config_json: b"{}".to_vec(),
            http_handler: Some(Box::new(move |req| {
                let mut guard = calls_handler.lock().unwrap();
                let idx = *guard;
                *guard += 1;
                let (expected_headers, mock_opt) = &steps_for_handler[idx];
                let mock = mock_opt
                    .as_ref()
                    .unwrap_or_else(|| panic!("{handler_name}: unexpected HTTP call {idx}"));
                assert_eq!(req.response_mode, "envelope");
                for (k, v) in expected_headers {
                    assert_eq!(
                        req.headers.get(k).map(String::as_str),
                        Some(v.as_str()),
                        "{handler_name} step {idx} header {k}"
                    );
                }
                Ok(HttpResponse {
                    status: mock.status,
                    headers: mock.headers.clone(),
                    set_cookies: mock.set_cookies.clone(),
                    body: mock.body.as_bytes().to_vec(),
                    trace: mock.trace.clone(),
                    tls: mock.tls.clone(),
                    ..HttpResponse::default()
                })
            })),
            ..LocalHostOptions::default()
        },
        || {
            let report = journey.execute()?;
            emit_telemetry(report.telemetry_batch.clone())?;
            let result = report.plugin_result.clone();
            *report_clone.lock().unwrap() = Some(report);
            execute(|| Ok(result))
        },
    );
    res.unwrap();

    let calls = *calls.lock().unwrap();
    assert_eq!(
        calls, scen.expected.http_calls,
        "{scenario_name} http_calls"
    );
    let result_json: Value = serde_json::from_slice(&capture.result_json).unwrap();
    assert_eq!(
        result_json["status"].as_str().unwrap_or_default(),
        scen.expected.status,
        "{scenario_name} status"
    );
    assert_eq!(
        result_json["alert_hint"].as_bool().unwrap_or(false),
        scen.expected.alert_hint,
        "{scenario_name} alert_hint"
    );
    let report = report_slot.lock().unwrap().take().expect("report");
    assert_eq!(
        report.steps_completed, scen.expected.steps_completed,
        "{scenario_name} steps_completed"
    );
    assert_eq!(
        report.steps.len(),
        scen.expected.step_statuses.len(),
        "{scenario_name} steps len"
    );
    for (idx, want) in scen.expected.step_statuses.iter().enumerate() {
        assert_eq!(
            report.steps[idx].status.as_str(),
            want,
            "{scenario_name} step[{idx}] status"
        );
    }

    let result_text = String::from_utf8(capture.result_json).unwrap();
    assert_eq!(capture.telemetry_json.len(), 1);
    let telemetry_text = String::from_utf8(capture.telemetry_json[0].clone()).unwrap();
    let tb: Value = serde_json::from_str(&telemetry_text).unwrap();
    let payload_b64 = tb["records"][0]["payload"].as_str().unwrap();
    let proto_bytes = base64::engine::general_purpose::STANDARD
        .decode(payload_b64)
        .unwrap();
    let proto_text = String::from_utf8_lossy(&proto_bytes);

    for secret in &scen.expected.forbidden_substrings {
        assert!(
            !result_text.contains(secret),
            "{scenario_name}: result leaked {secret}: {result_text}"
        );
        assert!(
            !telemetry_text.contains(secret),
            "{scenario_name}: telemetry leaked {secret}"
        );
        assert!(
            !proto_text.contains(secret),
            "{scenario_name}: proto leaked {secret}"
        );
    }

    let seen_metrics: BTreeMap<String, f64> = report
        .metric_batch
        .metrics
        .iter()
        .map(|m| {
            assert_eq!(m.metric_type, "synthetic");
            (m.name.clone(), m.points[0].value)
        })
        .collect();
    for want_name in &scen.expected.expected_metric_names {
        assert!(
            seen_metrics.contains_key(want_name),
            "{scenario_name}: missing metric {want_name} in {seen_metrics:?}"
        );
    }
    assert_eq!(
        seen_metrics.get("synthetic.journey.available").copied(),
        Some(scen.expected.journey_available)
    );
    if !scen.web_property_id.is_empty() {
        assert!(
            report
                .metric_batch
                .resource
                .attributes
                .iter()
                .any(|a| a.key == "web_property_id" && a.value == scen.web_property_id)
        );
    }
}
