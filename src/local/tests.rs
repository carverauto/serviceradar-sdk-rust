use std::collections::BTreeMap;
use std::fs;
use std::sync::{Arc, Mutex};

use serde_json::Value;

use super::{
    LOCAL_ACTION_FILE_VARIABLE, LOCAL_CONFIG_FILE_VARIABLE, LOCAL_CREDENTIAL_PREFIX,
    LocalHostOptions, LocalInputOptions, LocalInputs, run_local_host,
};
use crate::{
    Error, HttpClient, HttpResponse, LOG, PluginResult, TelemetryBatch, emit_telemetry, execute,
    get_config,
};

#[test]
fn local_inputs_merge_action_and_keep_credentials_separate() {
    let directory = tempfile_directory("inputs");
    let config_path = directory.join("config.json");
    let action_path = directory.join("action.json");
    let env_path = directory.join(".env");
    fs::write(&config_path, br#"{"instance_id":"test","page_size":100}"#).unwrap();
    fs::write(
        &action_path,
        br#"{"schema":"serviceradar.northbound_action_invocation.v1","invocation_id":"run-1","action_id":"collect","input_values":{"type":"Switch"}}"#,
    )
    .unwrap();
    fs::write(
        &env_path,
        format!(
            "{LOCAL_CONFIG_FILE_VARIABLE}={}\n{LOCAL_ACTION_FILE_VARIABLE}={}\n{LOCAL_CREDENTIAL_PREFIX}USERNAME=file-user\n{LOCAL_CREDENTIAL_PREFIX}PASSWORD=file-password\n",
            config_path.display(),
            action_path.display()
        ),
    )
    .unwrap();

    let inputs = LocalInputs::load(LocalInputOptions {
        env_file: Some(env_path),
        environment: Some(BTreeMap::from([(
            format!("{LOCAL_CREDENTIAL_PREFIX}PASSWORD"),
            "process-password".to_string(),
        )])),
        ..LocalInputOptions::default()
    })
    .unwrap();
    assert_eq!(inputs.credential("username"), Some("file-user"));
    assert_eq!(inputs.credential("PASSWORD"), Some("process-password"));

    let runtime = inputs.runtime_config_json().unwrap();
    let runtime_text = String::from_utf8(runtime.clone()).unwrap();
    assert!(!runtime_text.contains("file-user"));
    assert!(!runtime_text.contains("process-password"));
    let decoded: Value = serde_json::from_slice(&runtime).unwrap();
    assert_eq!(decoded["page_size"], 100);
    assert!(decoded["action_invocation"].is_object());

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn local_host_exercises_normal_sdk_calls_and_restores_backend() {
    let credentials = Arc::new(BTreeMap::from([
        ("username".to_string(), "local-user".to_string()),
        ("password".to_string(), "local-password".to_string()),
    ]));
    let calls = Arc::new(Mutex::new(0_u32));
    let handler_credentials = Arc::clone(&credentials);
    let handler_calls = Arc::clone(&calls);

    let (capture, result) = run_local_host(
        LocalHostOptions {
            config_json: br#"{"url":"https://inventory.example.test/devices"}"#.to_vec(),
            http_handler: Some(Box::new(move |request| {
                *handler_calls.lock().unwrap() += 1;
                assert_eq!(request.url, "https://inventory.example.test/devices");
                assert_eq!(handler_credentials["username"], "local-user");
                Ok(HttpResponse {
                    status: 200,
                    body: br#"{"devices":[]}"#.to_vec(),
                    ..HttpResponse::default()
                })
            })),
        },
        || {
            let config: Value =
                get_config()?.ok_or_else(|| Error::Message("missing config".into()))?;
            let response = HttpClient::default().get(config["url"].as_str().unwrap())?;
            assert_eq!(response.status, 200);
            assert_eq!(response.body, br#"{"devices":[]}"#);
            LOG.info("local collection complete");
            emit_telemetry(TelemetryBatch::default())?;
            execute(|| Ok(PluginResult::ok("local run complete")))
        },
    );

    result.unwrap();
    assert_eq!(*calls.lock().unwrap(), 1);
    assert!(!capture.result_json.is_empty());
    assert_eq!(capture.telemetry_json.len(), 1);
    assert_eq!(capture.logs.len(), 1);
    let result: Value = serde_json::from_slice(&capture.result_json).unwrap();
    assert_eq!(result["status"], "OK");
    assert_eq!(result["summary"], "local run complete");
    assert!(get_config::<Value>().is_err());
}

#[test]
fn local_http_handler_errors_are_reduced_to_host_errors() {
    let (_capture, result) = run_local_host(
        LocalHostOptions {
            config_json: b"{}".to_vec(),
            http_handler: Some(Box::new(|_| {
                Err(Error::Message(
                    "dial local-password@example.test".to_string(),
                ))
            })),
        },
        || {
            HttpClient::default()
                .get("https://example.test")
                .map(|_| ())
        },
    );
    let error = result.expect_err("local HTTP request should fail");
    assert!(!error.to_string().contains("local-password"));
}

fn tempfile_directory(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "serviceradar-sdk-rust-{name}-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}
