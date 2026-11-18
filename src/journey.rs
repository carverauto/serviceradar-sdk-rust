use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;
use url::Url;

use crate::cookie_jar::CookieJar;
use crate::error::{Error, SdkResult};
use crate::http::{HTTP_RESPONSE_MODE_ENVELOPE, HttpClient, HttpRequest, HttpResponse};
use crate::metric_envelope::{
    METRIC_ENVELOPE_SCHEMA_VERSION, Metric, MetricBatch, MetricIngestIdentity, MetricKind,
    MetricPoint, MetricResource, MetricStringMapEntry,
};
use crate::plugin_inputs::TargetContext;
use crate::result::{Result as PluginResult, Severity, Status};
use crate::telemetry::{TelemetryBatch, TelemetryRecord, TelemetrySource, emit_telemetry};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    Ok,
    Warning,
    Critical,
    Skipped,
}

impl StepStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Warning => "WARNING",
            Self::Critical => "CRITICAL",
            Self::Skipped => "SKIPPED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeaderMatch {
    Exact(String),
    Contains(String),
}

#[derive(Debug, Clone)]
enum StepAssertion {
    Status(Vec<i32>),
    StatusRange(i32, i32),
    Header(String, HeaderMatch),
    BodyContains(String),
    JsonField(String, Value),
    MaxDurationMs(Option<f64>, Option<f64>),
    MinCertDays(Option<i64>, Option<i64>),
}

#[derive(Debug, Clone)]
enum StepExtractor {
    Header { var: String, header: String },
    Cookie { var: String, cookie: String },
    JsonField { var: String, path: String },
}

#[derive(Debug, Clone)]
pub struct JourneyStep {
    pub name: String,
    pub method: String,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
    pub body_base64: bool,
    pub timeout_ms: u32,
    pub follow_redirects: Option<bool>,
    pub insecure_skip_verify: bool,
    assertions: Vec<StepAssertion>,
    extractors: Vec<StepExtractor>,
}

pub type Step = JourneyStep;

impl JourneyStep {
    pub fn new(name: impl Into<String>, method: impl Into<String>, url: impl Into<String>) -> Self {
        let method = method.into().trim().to_ascii_uppercase();
        Self {
            name: name.into().trim().to_string(),
            method: if method.is_empty() {
                "GET".to_string()
            } else {
                method
            },
            url: url.into().trim().to_string(),
            headers: BTreeMap::new(),
            body: Vec::new(),
            body_base64: false,
            timeout_ms: 0,
            follow_redirects: None,
            insecure_skip_verify: false,
            assertions: Vec::new(),
            extractors: Vec::new(),
        }
    }

    pub fn get(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self::new(name, "GET", url)
    }

    pub fn post(name: impl Into<String>, url: impl Into<String>, body: impl Into<Vec<u8>>) -> Self {
        let mut step = Self::new(name, "POST", url);
        step.body = body.into();
        step
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into().trim().to_string();
        self
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        if !key.trim().is_empty() {
            self.headers.insert(key, value.into());
        }
        self
    }

    pub fn with_headers(mut self, headers: BTreeMap<String, String>) -> Self {
        for (k, v) in headers {
            self = self.with_header(k, v);
        }
        self
    }

    pub fn with_text_body(mut self, body: impl Into<String>) -> Self {
        self.body = body.into().into_bytes();
        self.body_base64 = false;
        self
    }

    pub fn with_json_body(mut self, body: impl Into<String>) -> Self {
        self.body = body.into().into_bytes();
        self.body_base64 = false;
        self.with_header("Content-Type", "application/json")
    }

    pub fn with_binary_body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = body.into();
        self.body_base64 = true;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        let ms = timeout.as_millis().min(u128::from(u32::MAX)) as u32;
        self.timeout_ms = if !timeout.is_zero() && ms == 0 { 1 } else { ms };
        self
    }

    pub fn with_timeout_ms(mut self, timeout_ms: u32) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    pub fn with_follow_redirects(mut self, follow: bool) -> Self {
        self.follow_redirects = Some(follow);
        self
    }

    pub fn with_insecure_tls(mut self, enabled: bool) -> Self {
        self.insecure_skip_verify = enabled;
        self
    }

    pub fn expect_status(mut self, codes: impl Into<Vec<i32>>) -> Self {
        self.assertions.push(StepAssertion::Status(codes.into()));
        self
    }

    pub fn expect_status_code(self, code: i32) -> Self {
        self.expect_status(vec![code])
    }

    pub fn expect_status_range(mut self, min_code: i32, max_code: i32) -> Self {
        self.assertions
            .push(StepAssertion::StatusRange(min_code, max_code));
        self
    }

    pub fn expect_header(mut self, name: impl Into<String>, matcher: HeaderMatch) -> Self {
        self.assertions
            .push(StepAssertion::Header(name.into(), matcher));
        self
    }

    pub fn expect_header_exact(self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.expect_header(name, HeaderMatch::Exact(value.into()))
    }

    pub fn expect_header_contains(
        self,
        name: impl Into<String>,
        substr: impl Into<String>,
    ) -> Self {
        self.expect_header(name, HeaderMatch::Contains(substr.into()))
    }

    pub fn expect_body_contains(mut self, substr: impl Into<String>) -> Self {
        self.assertions
            .push(StepAssertion::BodyContains(substr.into()));
        self
    }

    pub fn expect_json_field(mut self, pointer: impl Into<String>, expected: Value) -> Self {
        self.assertions
            .push(StepAssertion::JsonField(pointer.into(), expected));
        self
    }

    pub fn expect_max_duration_ms(mut self, warn_ms: Option<f64>, crit_ms: Option<f64>) -> Self {
        self.assertions
            .push(StepAssertion::MaxDurationMs(warn_ms, crit_ms));
        self
    }

    pub fn expect_min_cert_days(mut self, warn_days: Option<i64>, crit_days: Option<i64>) -> Self {
        self.assertions
            .push(StepAssertion::MinCertDays(warn_days, crit_days));
        self
    }

    pub fn extract_header(
        mut self,
        var_name: impl Into<String>,
        header_name: impl Into<String>,
    ) -> Self {
        self.extractors.push(StepExtractor::Header {
            var: var_name.into(),
            header: header_name.into(),
        });
        self
    }

    pub fn extract_cookie(
        mut self,
        var_name: impl Into<String>,
        cookie_name: impl Into<String>,
    ) -> Self {
        self.extractors.push(StepExtractor::Cookie {
            var: var_name.into(),
            cookie: cookie_name.into(),
        });
        self
    }

    pub fn extract_json_field(
        mut self,
        var_name: impl Into<String>,
        pointer: impl Into<String>,
    ) -> Self {
        self.extractors.push(StepExtractor::JsonField {
            var: var_name.into(),
            path: pointer.into(),
        });
        self
    }
}

#[derive(Debug, Clone)]
pub struct StepOutcome {
    pub name: String,
    pub method: String,
    pub url: String,
    pub status: StepStatus,
    pub http_status: i32,
    pub duration_ms: f64,
    pub dns_ms: f64,
    pub connect_ms: f64,
    pub tls_ms: f64,
    pub ttfb_ms: f64,
    pub transfer_ms: f64,
    pub connection_reused: bool,
    pub redirect_count: u32,
    pub protocol: String,
    pub remote_addr: String,
    pub tls_cert_days: Option<i64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct JourneyReport {
    pub status: Status,
    pub steps: Vec<StepOutcome>,
    pub steps_completed: usize,
    pub total_ms: f64,
    pub plugin_result: PluginResult,
    pub metric_batch: MetricBatch,
    pub telemetry_batch: TelemetryBatch,
}

#[derive(Debug, Clone)]
pub struct Journey {
    name: String,
    steps: Vec<JourneyStep>,
    client: HttpClient,
    web_property_id: String,
    target_context: Option<TargetContext>,
    device_id: String,
    target_device_ip: String,
    producer_id: String,
    emit_telemetry: bool,
    alert_on_failure: bool,
    now_override: Option<SystemTime>,
}

impl Journey {
    pub fn new(name: impl Into<String>) -> Self {
        let trimmed = name.into().trim().to_string();
        Self {
            name: if trimmed.is_empty() {
                "http-synthetic-journey".to_string()
            } else {
                trimmed
            },
            steps: Vec::new(),
            client: HttpClient::default(),
            web_property_id: String::new(),
            target_context: None,
            device_id: String::new(),
            target_device_ip: String::new(),
            producer_id: String::new(),
            emit_telemetry: true,
            alert_on_failure: true,
            now_override: None,
        }
    }

    pub fn with_client(mut self, client: HttpClient) -> Self {
        self.client = client;
        self
    }

    pub fn with_web_property_id(mut self, id: impl Into<String>) -> Self {
        self.web_property_id = id.into().trim().to_string();
        self
    }

    pub fn with_target_context(mut self, ctx: TargetContext) -> Self {
        self.target_context = Some(ctx);
        self
    }

    pub fn with_device_id(mut self, device_id: impl Into<String>) -> Self {
        self.device_id = device_id.into().trim().to_string();
        self
    }

    pub fn with_target_device_ip(mut self, ip: impl Into<String>) -> Self {
        self.target_device_ip = ip.into().trim().to_string();
        self
    }

    pub fn with_producer_id(mut self, producer_id: impl Into<String>) -> Self {
        self.producer_id = producer_id.into().trim().to_string();
        self
    }

    pub fn with_telemetry(mut self, enabled: bool) -> Self {
        self.emit_telemetry = enabled;
        self
    }

    pub fn with_alert_on_failure(mut self, enabled: bool) -> Self {
        self.alert_on_failure = enabled;
        self
    }

    #[doc(hidden)]
    pub fn with_now(mut self, now: SystemTime) -> Self {
        self.now_override = Some(now);
        self
    }

    pub fn with_step(mut self, step: JourneyStep) -> Self {
        self.add_step(step);
        self
    }

    pub fn add_step(&mut self, mut step: JourneyStep) -> &mut Self {
        if step.name.trim().is_empty() {
            step.name = format!("step-{}", self.steps.len() + 1);
        }
        self.steps.push(step);
        self
    }

    pub fn run(&self) -> SdkResult<PluginResult> {
        let report = self.execute()?;
        if self.emit_telemetry && !report.telemetry_batch.records.is_empty() {
            emit_telemetry(report.telemetry_batch)?;
        }
        Ok(report.plugin_result)
    }

    pub fn execute(&self) -> SdkResult<JourneyReport> {
        let now = self.now_override.unwrap_or_else(SystemTime::now);
        let observed_at_nano = now
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_nanos() as u64;

        let mut jar = CookieJar::new();
        let mut vars: BTreeMap<String, String> = BTreeMap::new();

        let mut outcomes = Vec::with_capacity(self.steps.len());
        let mut overall_status = Status::Ok;
        let mut first_critical: Option<(String, String)> = None;
        let mut first_warning: Option<String> = None;
        let mut steps_completed = 0_usize;
        let mut total_ms = 0.0_f64;
        let mut short_circuit = false;
        let mut step_metrics = Vec::with_capacity(self.steps.len() * 7);

        for (idx, step) in self.steps.iter().enumerate() {
            if short_circuit {
                outcomes.push(StepOutcome {
                    name: step.name.clone(),
                    method: step.method.clone(),
                    url: sanitize_url_for_telemetry(&step.url),
                    status: StepStatus::Skipped,
                    http_status: 0,
                    duration_ms: 0.0,
                    dns_ms: 0.0,
                    connect_ms: 0.0,
                    tls_ms: 0.0,
                    ttfb_ms: 0.0,
                    transfer_ms: 0.0,
                    connection_reused: false,
                    redirect_count: 0,
                    protocol: String::new(),
                    remote_addr: String::new(),
                    tls_cert_days: None,
                    error: None,
                });
                continue;
            }

            let (outcome, resp, crit_reason, warn_reason) =
                self.run_step(step, &mut jar, &mut vars, now);
            total_ms += outcome.duration_ms;

            match outcome.status {
                StepStatus::Critical => {
                    overall_status = Status::Critical;
                    short_circuit = true;
                    if first_critical.is_none() {
                        first_critical = Some((
                            step.name.clone(),
                            crit_reason.unwrap_or_else(|| "step failed".to_string()),
                        ));
                    }
                }
                StepStatus::Warning => {
                    steps_completed += 1;
                    if overall_status != Status::Critical {
                        overall_status = Status::Warning;
                        if first_warning.is_none() {
                            first_warning = warn_reason;
                        }
                    }
                }
                StepStatus::Ok => {
                    steps_completed += 1;
                }
                StepStatus::Skipped => {}
            }

            step_metrics.extend(self.build_step_metrics(
                idx,
                &outcome,
                resp.as_ref(),
                observed_at_nano,
            ));
            outcomes.push(outcome);
        }

        let plugin_result = self.build_result(
            overall_status,
            &outcomes,
            steps_completed,
            total_ms,
            first_critical.as_ref(),
            first_warning.as_deref(),
        );
        let metric_batch = self.build_metric_batch(
            overall_status,
            steps_completed,
            total_ms,
            step_metrics,
            observed_at_nano,
        );
        let event_id = format!("journey-{}-{observed_at_nano}", self.name);
        let telemetry_batch =
            TelemetryBatch::new(vec![TelemetryRecord::serviceradar_metric_batch(
                event_id,
                metric_batch.clone(),
            )])
            .with_source(TelemetrySource::new("plugin", &self.name));

        Ok(JourneyReport {
            status: overall_status,
            steps: outcomes,
            steps_completed,
            total_ms,
            plugin_result,
            metric_batch,
            telemetry_batch,
        })
    }

    fn run_step(
        &self,
        step: &JourneyStep,
        jar: &mut CookieJar,
        vars: &mut BTreeMap<String, String>,
        now: SystemTime,
    ) -> (
        StepOutcome,
        Option<HttpResponse>,
        Option<String>,
        Option<String>,
    ) {
        let mut outcome = StepOutcome {
            name: step.name.clone(),
            method: step.method.clone(),
            url: sanitize_url_for_telemetry(&step.url),
            status: StepStatus::Ok,
            http_status: 0,
            duration_ms: 0.0,
            dns_ms: 0.0,
            connect_ms: 0.0,
            tls_ms: 0.0,
            ttfb_ms: 0.0,
            transfer_ms: 0.0,
            connection_reused: false,
            redirect_count: 0,
            protocol: String::new(),
            remote_addr: String::new(),
            tls_cert_days: None,
            error: None,
        };

        let resolved_url = match replace_template_vars(&step.url, vars) {
            Ok(url) => url,
            Err(msg) => {
                outcome.status = StepStatus::Critical;
                outcome.error = Some(msg.clone());
                return (outcome, None, Some(msg), None);
            }
        };
        outcome.url = sanitize_url_for_telemetry(&resolved_url);

        let mut resolved_headers = BTreeMap::new();
        let mut explicit_cookie: Option<String> = None;
        for (key, value) in &step.headers {
            let expanded = match replace_template_vars(value, vars) {
                Ok(v) => v,
                Err(msg) => {
                    outcome.status = StepStatus::Critical;
                    outcome.error = Some(msg.clone());
                    return (outcome, None, Some(msg), None);
                }
            };
            if key.eq_ignore_ascii_case("Cookie") {
                match explicit_cookie.as_mut() {
                    Some(existing) if !expanded.is_empty() => {
                        existing.push_str("; ");
                        existing.push_str(&expanded);
                    }
                    None if !expanded.is_empty() => explicit_cookie = Some(expanded),
                    _ => {}
                }
                continue;
            }
            resolved_headers.insert(key.clone(), expanded);
        }

        let jar_cookie = jar.cookie_header_for_url_at(&resolved_url, now);
        if let Some(merged) =
            merge_cookie_headers(jar_cookie.as_deref(), explicit_cookie.as_deref())
        {
            resolved_headers.insert("Cookie".to_string(), merged);
        }

        let resolved_body = if !step.body_base64 && !step.body.is_empty() {
            let body_text = String::from_utf8_lossy(&step.body);
            match replace_template_vars(&body_text, vars) {
                Ok(expanded) => expanded.into_bytes(),
                Err(msg) => {
                    outcome.status = StepStatus::Critical;
                    outcome.error = Some(msg.clone());
                    return (outcome, None, Some(msg), None);
                }
            }
        } else {
            step.body.clone()
        };

        let request = HttpRequest {
            method: step.method.clone(),
            url: resolved_url.clone(),
            headers: resolved_headers,
            body: resolved_body,
            body_base64: step.body_base64,
            response_mode: HTTP_RESPONSE_MODE_ENVELOPE.to_string(),
            timeout_ms: step.timeout_ms,
            insecure_skip_verify: step.insecure_skip_verify,
            follow_redirects: step.follow_redirects,
        };

        let resp = match self.client.do_request(request) {
            Ok(resp) => resp,
            Err(err) => {
                let msg = sanitize_error_message(&err, vars);
                outcome.status = StepStatus::Critical;
                outcome.error = Some(msg.clone());
                return (outcome, None, Some(msg), None);
            }
        };

        outcome.http_status = resp.status;
        if let Some(trace) = &resp.trace {
            if !trace.final_url.is_empty() {
                outcome.url = sanitize_url_for_telemetry(&trace.final_url);
            }
            outcome.dns_ms = trace.dns_ms();
            outcome.connect_ms = trace.connect_ms();
            outcome.tls_ms = trace.tls_handshake_ms();
            outcome.ttfb_ms = trace.ttfb_ms();
            outcome.transfer_ms = trace.transfer_ms();
            outcome.duration_ms = if trace.total_duration_ns > 0 {
                trace.total_ms()
            } else {
                resp.duration.as_nanos() as f64 / 1_000_000.0
            };
            outcome.connection_reused = trace.connection_reused;
            outcome.redirect_count = trace.redirect_count;
            outcome.protocol = trace.protocol.clone();
            outcome.remote_addr = trace.remote_addr.clone();
        } else {
            outcome.duration_ms = resp.duration.as_nanos() as f64 / 1_000_000.0;
        }
        if let Some(tls) = &resp.tls
            && (tls.leaf_not_after_unix != 0
                || !tls.leaf_subject_cn.is_empty()
                || !tls.version.is_empty())
        {
            outcome.tls_cert_days = Some(tls.leaf_days_remaining);
        }

        let effective_url = resp
            .trace
            .as_ref()
            .filter(|t| !t.final_url.is_empty())
            .map_or(resolved_url.as_str(), |t| t.final_url.as_str());
        let set_cookies = resp.header_values("Set-Cookie");
        jar.store_set_cookies_at(effective_url, &set_cookies, now);

        let mut parsed_json: Option<Result<Value, String>> = None;

        let mut warn_reasons = Vec::new();
        for assertion in &step.assertions {
            match assertion {
                StepAssertion::Status(codes) => {
                    if !codes.contains(&resp.status) {
                        let msg =
                            format!("status {} did not match expected {:?}", resp.status, codes);
                        outcome.status = StepStatus::Critical;
                        outcome.error = Some(msg.clone());
                        return (outcome, Some(resp), Some(msg), None);
                    }
                }
                StepAssertion::StatusRange(min_code, max_code) => {
                    if resp.status < *min_code || resp.status > *max_code {
                        let msg = format!(
                            "status {} outside expected range [{min_code}, {max_code}]",
                            resp.status
                        );
                        outcome.status = StepStatus::Critical;
                        outcome.error = Some(msg.clone());
                        return (outcome, Some(resp), Some(msg), None);
                    }
                }
                StepAssertion::Header(name, matcher) => {
                    let got = resp.header(name);
                    match matcher {
                        HeaderMatch::Exact(expected) => {
                            let ok = if expected.is_empty() {
                                got.is_some_and(|v| !v.is_empty())
                            } else {
                                got == Some(expected.as_str())
                            };
                            if !ok {
                                let msg = format!("header \"{name}\" mismatch");
                                outcome.status = StepStatus::Critical;
                                outcome.error = Some(msg.clone());
                                return (outcome, Some(resp), Some(msg), None);
                            }
                        }
                        HeaderMatch::Contains(substr) => {
                            if !got.is_some_and(|v| v.contains(substr.as_str())) {
                                let msg = format!("header \"{name}\" missing expected substring");
                                outcome.status = StepStatus::Critical;
                                outcome.error = Some(msg.clone());
                                return (outcome, Some(resp), Some(msg), None);
                            }
                        }
                    }
                }
                StepAssertion::BodyContains(substr) => {
                    if !resp
                        .body
                        .windows(substr.len())
                        .any(|w| w == substr.as_bytes())
                        && !substr.is_empty()
                    {
                        let msg = format!("response body missing expected substring \"{substr}\"");
                        outcome.status = StepStatus::Critical;
                        outcome.error = Some(msg.clone());
                        return (outcome, Some(resp), Some(msg), None);
                    }
                }
                StepAssertion::JsonField(pointer, expected) => {
                    let root = match ensure_json(&mut parsed_json, &resp.body) {
                        Ok(v) => v,
                        Err(_) => {
                            let msg = format!("invalid JSON body checking \"{pointer}\"");
                            outcome.status = StepStatus::Critical;
                            outcome.error = Some(msg.clone());
                            return (outcome, Some(resp), Some(msg), None);
                        }
                    };
                    let matched = resolve_json_path(root, pointer)
                        .is_some_and(|actual| json_values_equal(actual, expected));
                    if !matched {
                        let msg = format!("JSON field \"{pointer}\" did not match expected value");
                        outcome.status = StepStatus::Critical;
                        outcome.error = Some(msg.clone());
                        return (outcome, Some(resp), Some(msg), None);
                    }
                }
                StepAssertion::MaxDurationMs(warn_ms, crit_ms) => {
                    if let Some(crit) = crit_ms
                        && *crit > 0.0
                        && outcome.duration_ms >= *crit
                    {
                        let msg = format!(
                            "duration {:.1}ms >= critical {:.1}ms",
                            outcome.duration_ms, crit
                        );
                        outcome.status = StepStatus::Critical;
                        outcome.error = Some(msg.clone());
                        return (outcome, Some(resp), Some(msg), None);
                    }
                    if let Some(warn) = warn_ms
                        && *warn > 0.0
                        && outcome.duration_ms >= *warn
                    {
                        warn_reasons.push(format!(
                            "duration {:.1}ms >= warning {:.1}ms",
                            outcome.duration_ms, warn
                        ));
                    }
                }
                StepAssertion::MinCertDays(warn_days, crit_days) => {
                    if let Some(days) = outcome.tls_cert_days {
                        if let Some(crit) = crit_days
                            && days <= *crit
                        {
                            let msg = format!(
                                "TLS certificate expires in {days} days (<= critical {crit})"
                            );
                            outcome.status = StepStatus::Critical;
                            outcome.error = Some(msg.clone());
                            return (outcome, Some(resp), Some(msg), None);
                        }
                        if let Some(warn) = warn_days
                            && days <= *warn
                        {
                            warn_reasons.push(format!(
                                "TLS certificate expires in {days} days (<= warning {warn})"
                            ));
                        }
                    }
                }
            }
        }

        for extractor in &step.extractors {
            match extractor {
                StepExtractor::Header { var, header } => {
                    let Some(val) = resp.header(header).filter(|v| !v.is_empty()) else {
                        let msg = format!("extractor \"{var}\": missing header \"{header}\"");
                        outcome.status = StepStatus::Critical;
                        outcome.error = Some(msg.clone());
                        return (outcome, Some(resp), Some(msg), None);
                    };
                    vars.insert(var.clone(), val.to_string());
                }
                StepExtractor::Cookie { var, cookie } => {
                    let found = jar
                        .cookie_value_at(effective_url, cookie, now)
                        .or_else(|| {
                            if effective_url != resolved_url {
                                jar.cookie_value_at(&resolved_url, cookie, now)
                            } else {
                                None
                            }
                        })
                        .or_else(|| jar.latest_value_by_name(cookie));
                    let Some(val) = found else {
                        let msg = format!("extractor \"{var}\": missing cookie \"{cookie}\"");
                        outcome.status = StepStatus::Critical;
                        outcome.error = Some(msg.clone());
                        return (outcome, Some(resp), Some(msg), None);
                    };
                    vars.insert(var.clone(), val);
                }
                StepExtractor::JsonField { var, path } => {
                    let root = match ensure_json(&mut parsed_json, &resp.body) {
                        Ok(v) => v,
                        Err(_) => {
                            let msg =
                                format!("extractor \"{var}\": invalid JSON body for \"{path}\"");
                            outcome.status = StepStatus::Critical;
                            outcome.error = Some(msg.clone());
                            return (outcome, Some(resp), Some(msg), None);
                        }
                    };
                    let Some(val) = resolve_json_path(root, path) else {
                        let msg = format!("extractor \"{var}\": missing JSON field \"{path}\"");
                        outcome.status = StepStatus::Critical;
                        outcome.error = Some(msg.clone());
                        return (outcome, Some(resp), Some(msg), None);
                    };
                    let Some(str_val) = json_scalar_to_string(val) else {
                        let msg =
                            format!("extractor \"{var}\": JSON field \"{path}\" is not scalar");
                        outcome.status = StepStatus::Critical;
                        outcome.error = Some(msg.clone());
                        return (outcome, Some(resp), Some(msg), None);
                    };
                    vars.insert(var.clone(), str_val);
                }
            }
        }

        if !warn_reasons.is_empty() {
            let joined = warn_reasons.join("; ");
            outcome.status = StepStatus::Warning;
            outcome.error = Some(joined.clone());
            return (outcome, Some(resp), None, Some(joined));
        }

        (outcome, Some(resp), None, None)
    }

    fn build_result(
        &self,
        overall_status: Status,
        outcomes: &[StepOutcome],
        steps_completed: usize,
        total_ms: f64,
        first_critical: Option<&(String, String)>,
        first_warning: Option<&str>,
    ) -> PluginResult {
        let mut res = PluginResult::new();
        if let Some(ctx) = &self.target_context {
            res = res.for_target(ctx);
        }
        res.set_status(overall_status);
        res.add_label("journey", &self.name);
        if !self.web_property_id.is_empty() {
            res.add_label("web_property_id", &self.web_property_id);
        }

        let summary = match overall_status {
            Status::Critical => {
                let (step, reason) = first_critical.map_or(("unknown", "step failed"), |(s, r)| {
                    (s.as_str(), r.as_str())
                });
                format!(
                    "journey {} CRITICAL at step \"{step}\": {reason}",
                    self.name
                )
            }
            Status::Warning => {
                let reason = first_warning.unwrap_or("warning threshold exceeded");
                format!(
                    "journey {} WARNING ({steps_completed}/{} steps in {total_ms:.1}ms): {reason}",
                    self.name,
                    outcomes.len()
                )
            }
            _ => format!(
                "journey {} OK ({steps_completed}/{} steps in {total_ms:.1}ms)",
                self.name,
                outcomes.len()
            ),
        };
        res.set_summary(&summary);

        match overall_status {
            Status::Critical => {
                res.emit_event(Severity::Critical, &summary, "synthetic_journey_failed");
                if self.alert_on_failure {
                    res.request_immediate_alert("synthetic_journey_failed");
                }
            }
            Status::Warning => {
                res.emit_event(Severity::Warning, &summary, "synthetic_journey_degraded");
            }
            _ => {}
        }

        let tone = tone_for_status(overall_status);
        res.add_stat_card("Journey Status", status_str(overall_status), tone);
        res.add_stat_card("Total Duration", format!("{total_ms:.1}ms"), tone);
        res.add_stat_card(
            "Steps Completed",
            format!("{steps_completed}/{}", outcomes.len()),
            tone,
        );

        if !outcomes.is_empty() {
            let mut table = BTreeMap::new();
            for (idx, step) in outcomes.iter().enumerate() {
                let key = format!("{:02}. {}", idx + 1, step.name);
                if step.status == StepStatus::Skipped {
                    table.insert(key, "SKIPPED".to_string());
                    continue;
                }
                let mut val = format!(
                    "{} | {} | {:.1}ms (dns={:.1} connect={:.1} tls={:.1} ttfb={:.1} xfer={:.1})",
                    step.status.as_str(),
                    step.http_status,
                    step.duration_ms,
                    step.dns_ms,
                    step.connect_ms,
                    step.tls_ms,
                    step.ttfb_ms,
                    step.transfer_ms,
                );
                if !step.remote_addr.is_empty() {
                    val.push_str(" | ");
                    val.push_str(&step.remote_addr);
                }
                if let Some(days) = step.tls_cert_days {
                    val.push_str(&format!(" | tls_days={days}"));
                }
                if let Some(err) = &step.error {
                    val.push_str(" | ");
                    val.push_str(err);
                }
                table.insert(key, val);
            }
            res.add_table(table, "full");
        }

        res
    }

    fn build_step_metrics(
        &self,
        idx: usize,
        outcome: &StepOutcome,
        resp: Option<&HttpResponse>,
        observed_at_nano: u64,
    ) -> Vec<Metric> {
        if outcome.status == StepStatus::Skipped {
            return Vec::new();
        }
        let host = Url::parse(&outcome.url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_string))
            .unwrap_or_default();

        let mut attrs = vec![
            MetricStringMapEntry {
                key: "journey".to_string(),
                value: self.name.clone(),
            },
            MetricStringMapEntry {
                key: "step".to_string(),
                value: outcome.name.clone(),
            },
            MetricStringMapEntry {
                key: "step_index".to_string(),
                value: idx.to_string(),
            },
            MetricStringMapEntry {
                key: "method".to_string(),
                value: outcome.method.clone(),
            },
            MetricStringMapEntry {
                key: "host".to_string(),
                value: host,
            },
            MetricStringMapEntry {
                key: "status_code".to_string(),
                value: outcome.http_status.to_string(),
            },
        ];
        if let Some(trace) = resp.and_then(|r| r.trace.as_ref()) {
            attrs.push(MetricStringMapEntry {
                key: "connection_reused".to_string(),
                value: trace.connection_reused.to_string(),
            });
            if !trace.protocol.is_empty() {
                attrs.push(MetricStringMapEntry {
                    key: "protocol".to_string(),
                    value: trace.protocol.clone(),
                });
            }
        }
        if !self.web_property_id.is_empty() {
            attrs.push(MetricStringMapEntry {
                key: "web_property_id".to_string(),
                value: self.web_property_id.clone(),
            });
        }

        let make_metric = |name: &str, unit: &str, value: f64| Metric {
            name: name.to_string(),
            metric_type: "synthetic".to_string(),
            kind: MetricKind::Gauge,
            unit: unit.to_string(),
            points: vec![MetricPoint {
                value,
                observed_at_unix_nano: observed_at_nano,
                attributes: attrs.clone(),
                ..MetricPoint::default()
            }],
            ..Metric::default()
        };

        let mut out = vec![
            make_metric("synthetic.step.duration_ms", "ms", outcome.duration_ms),
            make_metric("synthetic.step.dns_ms", "ms", outcome.dns_ms),
            make_metric("synthetic.step.connect_ms", "ms", outcome.connect_ms),
            make_metric("synthetic.step.tls_ms", "ms", outcome.tls_ms),
            make_metric("synthetic.step.ttfb_ms", "ms", outcome.ttfb_ms),
            make_metric("synthetic.step.transfer_ms", "ms", outcome.transfer_ms),
        ];
        if let Some(days) = outcome.tls_cert_days {
            out.push(make_metric(
                "synthetic.step.tls_cert_days_remaining",
                "d",
                days as f64,
            ));
        }
        out
    }

    fn build_metric_batch(
        &self,
        overall_status: Status,
        steps_completed: usize,
        total_ms: f64,
        step_metrics: Vec<Metric>,
        observed_at_nano: u64,
    ) -> MetricBatch {
        let device_id = if !self.device_id.is_empty() {
            self.device_id.clone()
        } else {
            self.target_context
                .as_ref()
                .and_then(TargetContext::device_uid)
                .unwrap_or_default()
                .to_string()
        };
        let producer_id = if !self.producer_id.is_empty() {
            self.producer_id.clone()
        } else {
            self.name.clone()
        };

        let mut resource_attrs = vec![MetricStringMapEntry {
            key: "journey".to_string(),
            value: self.name.clone(),
        }];
        let mut journey_attrs = vec![MetricStringMapEntry {
            key: "journey".to_string(),
            value: self.name.clone(),
        }];
        if !self.web_property_id.is_empty() {
            resource_attrs.push(MetricStringMapEntry {
                key: "web_property_id".to_string(),
                value: self.web_property_id.clone(),
            });
            journey_attrs.push(MetricStringMapEntry {
                key: "web_property_id".to_string(),
                value: self.web_property_id.clone(),
            });
        }

        let available = if overall_status == Status::Critical {
            0.0
        } else {
            1.0
        };

        let make_journey_metric = |name: &str, unit: &str, value: f64| Metric {
            name: name.to_string(),
            metric_type: "synthetic".to_string(),
            kind: MetricKind::Gauge,
            unit: unit.to_string(),
            points: vec![MetricPoint {
                value,
                observed_at_unix_nano: observed_at_nano,
                attributes: journey_attrs.clone(),
                ..MetricPoint::default()
            }],
            ..Metric::default()
        };

        let mut metrics = Vec::with_capacity(3 + step_metrics.len());
        metrics.push(make_journey_metric(
            "synthetic.journey.available",
            "1",
            available,
        ));
        metrics.push(make_journey_metric(
            "synthetic.journey.duration_ms",
            "ms",
            total_ms,
        ));
        metrics.push(make_journey_metric(
            "synthetic.journey.steps_completed",
            "{step}",
            steps_completed as f64,
        ));
        metrics.extend(step_metrics);

        MetricBatch {
            schema_version: METRIC_ENVELOPE_SCHEMA_VERSION.to_string(),
            resource: MetricResource {
                service_name: self.name.clone(),
                service_type: "wasm-plugin".to_string(),
                device_id,
                target_device_ip: self.target_device_ip.clone(),
                attributes: resource_attrs,
                ..MetricResource::default()
            },
            ingest_identity: MetricIngestIdentity {
                source: "plugin-metrics".to_string(),
                payload_kind: METRIC_ENVELOPE_SCHEMA_VERSION.to_string(),
                producer_id,
                producer_kind: "wasm-plugin".to_string(),
                ..MetricIngestIdentity::default()
            },
            emitted_at_unix_nano: observed_at_nano,
            metrics,
            ..MetricBatch::default()
        }
    }
}

fn ensure_json<'a>(
    cache: &'a mut Option<Result<Value, String>>,
    body: &[u8],
) -> Result<&'a Value, &'a str> {
    if cache.is_none() {
        *cache = Some(serde_json::from_slice::<Value>(body).map_err(|err| err.to_string()));
    }
    match cache.as_ref().expect("initialized above") {
        Ok(v) => Ok(v),
        Err(e) => Err(e.as_str()),
    }
}

fn status_str(status: Status) -> &'static str {
    match status {
        Status::Ok => "OK",
        Status::Warning => "WARNING",
        Status::Critical => "CRITICAL",
        Status::Unknown => "UNKNOWN",
    }
}

fn tone_for_status(status: Status) -> &'static str {
    match status {
        Status::Ok => "success",
        Status::Warning => "warning",
        Status::Critical | Status::Unknown => "error",
    }
}

fn sanitize_url_for_telemetry(raw_url: &str) -> String {
    let trimmed = raw_url.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    match Url::parse(trimmed) {
        Ok(mut parsed) => {
            let _ = parsed.set_username("");
            let _ = parsed.set_password(None);
            parsed.set_query(None);
            parsed.set_fragment(None);
            parsed.to_string()
        }
        Err(_) => match trimmed.find(['?', '#']) {
            Some(idx) => trimmed[..idx].to_string(),
            None => trimmed.to_string(),
        },
    }
}

fn sanitize_error_message(err: &Error, vars: &BTreeMap<String, String>) -> String {
    let mut msg = err.to_string();
    for secret in vars.values() {
        if !secret.is_empty() && msg.contains(secret.as_str()) {
            msg = msg.replace(secret.as_str(), "[REDACTED]");
        }
    }
    msg
}

fn replace_template_vars(input: &str, vars: &BTreeMap<String, String>) -> Result<String, String> {
    if !input.contains("{{") {
        return Ok(input.to_string());
    }
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find("}}") else {
            out.push_str(&rest[start..]);
            return Ok(out);
        };
        let var_name = after_start[..end].trim();
        let Some(val) = vars.get(var_name) else {
            return Err(format!("missing template variable \"{var_name}\""));
        };
        out.push_str(val);
        rest = &after_start[end + 2..];
    }
    out.push_str(rest);
    Ok(out)
}

fn merge_cookie_headers(jar_header: Option<&str>, explicit_header: Option<&str>) -> Option<String> {
    match (jar_header, explicit_header) {
        (None, None) => None,
        (Some(j), None) => Some(j.to_string()),
        (None, Some(e)) => Some(e.to_string()),
        (Some(j), Some(e)) => {
            let mut pairs: Vec<(String, String)> = Vec::new();
            let mut push_pairs = |raw: &str, override_existing: bool| {
                for segment in raw.split(';') {
                    let part = segment.trim();
                    if part.is_empty() {
                        continue;
                    }
                    let Some((name, val)) = part.split_once('=') else {
                        continue;
                    };
                    let name = name.trim().to_string();
                    let val = val.trim().to_string();
                    if override_existing
                        && let Some(existing) = pairs.iter_mut().find(|(n, _)| *n == name)
                    {
                        existing.1 = val;
                        continue;
                    }
                    pairs.push((name, val));
                }
            };
            push_pairs(j, false);
            push_pairs(e, true);
            if pairs.is_empty() {
                return None;
            }
            Some(
                pairs
                    .into_iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join("; "),
            )
        }
    }
}

fn resolve_json_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() || path == "/" {
        return Some(root);
    }

    let tokens: Vec<String> = if let Some(rest) = path.strip_prefix('/') {
        rest.split('/')
            .map(|tok| tok.replace("~1", "/").replace("~0", "~"))
            .collect()
    } else {
        let mut out = Vec::new();
        for mut part in path.split('.') {
            if part.is_empty() {
                continue;
            }
            while let Some(open) = part.find('[') {
                if open > 0 {
                    out.push(part[..open].to_string());
                }
                let after_open = &part[open + 1..];
                let Some(close) = after_open.find(']') else {
                    out.push(part[open..].to_string());
                    part = "";
                    break;
                };
                out.push(after_open[..close].to_string());
                part = &after_open[close + 1..];
            }
            if !part.is_empty() {
                out.push(part.to_string());
            }
        }
        out
    };

    let mut curr = root;
    for tok in tokens {
        match curr {
            Value::Object(map) => {
                curr = map.get(&tok)?;
            }
            Value::Array(arr) => {
                let idx = tok.parse::<usize>().ok()?;
                curr = arr.get(idx)?;
            }
            _ => return None,
        }
    }
    Some(curr)
}

fn json_values_equal(actual: &Value, expected: &Value) -> bool {
    if actual == expected {
        return true;
    }
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => {
            if let (Some(ai), Some(bi)) = (a.as_i64(), b.as_i64()) {
                ai == bi
            } else if let (Some(af), Some(bf)) = (a.as_f64(), b.as_f64()) {
                af == bf
            } else {
                false
            }
        }
        _ => false,
    }
}

fn json_scalar_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local::{LocalHostOptions, run_local_host};

    #[test]
    fn journey_missing_var_short_circuits_without_http_call() {
        let journey = Journey::new("missing")
            .with_step(JourneyStep::get("bad", "https://example.com/{{tok}}"));
        let called = std::sync::Arc::new(std::sync::Mutex::new(false));
        let called_clone = std::sync::Arc::clone(&called);

        let (_capture, res) = run_local_host(
            LocalHostOptions {
                config_json: b"{}".to_vec(),
                http_handler: Some(Box::new(move |_| {
                    *called_clone.lock().unwrap() = true;
                    Ok(HttpResponse::default())
                })),
                ..LocalHostOptions::default()
            },
            || {
                let report = journey.execute()?;
                assert_eq!(report.status, Status::Critical);
                assert!(
                    report
                        .plugin_result
                        .summary()
                        .unwrap_or_default()
                        .contains("tok")
                );
                Ok(())
            },
        );
        res.unwrap();
        assert!(!*called.lock().unwrap());
    }

    #[test]
    fn journey_dot_path_and_rfc6901_unescaping() {
        let journey = Journey::new("paths").with_step(
            JourneyStep::get("step1", "https://api.example.com/items?secret=1")
                .expect_status_range(200, 299)
                .expect_header_contains("Content-Type", "json")
                .expect_json_field("/data/items/0/slash~1key", Value::String("ok".to_string()))
                .expect_json_field("data.items[0].id", Value::String("42".to_string()))
                .extract_json_field("item_id", "data.items[0].id"),
        );

        let (_capture, res) = run_local_host(
            LocalHostOptions {
                config_json: b"{}".to_vec(),
                http_handler: Some(Box::new(|_| {
                    Ok(HttpResponse {
                        status: 201,
                        headers: BTreeMap::from([(
                            "Content-Type".to_string(),
                            "application/json".to_string(),
                        )]),
                        body: br#"{"data":{"items":[{"id":"42","slash/key":"ok"}]}}"#.to_vec(),
                        ..HttpResponse::default()
                    })
                })),
                ..LocalHostOptions::default()
            },
            || {
                let report = journey.execute()?;
                assert_eq!(report.status, Status::Ok);
                Ok(())
            },
        );
        res.unwrap();
    }
}
