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
            ..LocalHostOptions::default()
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
    // Asserted under the install lock: the guard inside `run_local_host` has
    // already dropped, so without the lock a concurrent test that installs its
    // own backend makes this call succeed and the assertion flaps.
    crate::host::with_install_lock(|| assert!(get_config::<Value>().is_err()));
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
            ..LocalHostOptions::default()
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

mod grpc_host {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use super::super::grpc::decode_local_grpc_request;
    use super::super::{LocalGrpcHandler, LocalHostOptions, run_local_host};
    use crate::error::{
        HOST_ERR_INVALID, HOST_ERR_NOT_FOUND, HOST_ERR_TIMEOUT, HOST_ERR_TOO_LARGE, HostError,
    };
    use crate::grpc::tests::{
        REQUEST_FIXTURE, RESPONSE_ERROR_FIXTURE, RESPONSE_OK_FIXTURE, fixture_request,
        fixture_response,
    };
    use crate::{Error, GRPC, GrpcCode, GrpcRequest, GrpcResponse, SdkResult};

    fn run_local_grpc(
        handler: Option<LocalGrpcHandler>,
        request: GrpcRequest,
    ) -> SdkResult<GrpcResponse> {
        let mut outcome = None;
        let (_capture, result) = run_local_host(
            LocalHostOptions {
                config_json: b"{}".to_vec(),
                grpc_handler: handler,
                ..LocalHostOptions::default()
            },
            || {
                outcome = Some(GRPC.unary(request));
                Ok(())
            },
        );
        result.unwrap();
        outcome.expect("plugin ran")
    }

    fn host_code(result: SdkResult<GrpcResponse>) -> HostError {
        match result {
            Err(Error::Host(err)) => err,
            other => panic!("expected host error, got {other:?}"),
        }
    }

    #[test]
    fn local_host_grpc_round_trips_shared_fixtures() {
        let want = decode_local_grpc_request(REQUEST_FIXTURE.as_bytes()).unwrap();
        assert_eq!(want, fixture_request());
        let ok = fixture_response(RESPONSE_OK_FIXTURE);
        let calls = Arc::new(Mutex::new(0_u32));
        let handler_calls = Arc::clone(&calls);
        let handler_ok = ok.clone();

        let response = run_local_grpc(
            Some(Box::new(move |got| {
                *handler_calls.lock().unwrap() += 1;
                assert_eq!(got, want);
                Ok(handler_ok.clone())
            })),
            fixture_request(),
        )
        .unwrap();
        assert_eq!(*calls.lock().unwrap(), 1);
        assert_eq!(response.status, GrpcCode::OK);
        assert_eq!(response.message, b"\n\x02ok");
        assert_eq!(response.trailers, ok.trailers);
    }

    #[test]
    fn local_host_grpc_non_ok_status() {
        let error_response = fixture_response(RESPONSE_ERROR_FIXTURE);
        let headers = error_response.headers.clone();
        let result = run_local_grpc(
            Some(Box::new(move |_| Ok(error_response.clone()))),
            fixture_request(),
        );
        match result {
            Err(Error::GrpcStatus(status)) => {
                assert_eq!(status.code, GrpcCode::NOT_FOUND);
                assert_eq!(status.headers, headers);
            }
            other => panic!("expected NOT_FOUND status, got {other:?}"),
        }
    }

    #[test]
    fn local_host_grpc_without_handler_is_unsupported() {
        let err = host_code(run_local_grpc(None, fixture_request()));
        assert_eq!(err.code, HOST_ERR_NOT_FOUND);
        assert_eq!(err.op, "grpc_unary");
    }

    #[test]
    fn local_host_grpc_handler_errors_become_unavailable() {
        let secret = "dial local-secret@192.0.2.10 refused";
        let result = run_local_grpc(
            Some(Box::new(move |_| Err(Error::Message(secret.to_string())))),
            fixture_request(),
        );
        match result {
            Err(Error::GrpcStatus(status)) => {
                assert_eq!(status.code, GrpcCode::UNAVAILABLE);
                assert!(!status.message.contains("local-secret"));
            }
            other => panic!("expected UNAVAILABLE, got {other:?}"),
        }
    }

    #[test]
    fn local_host_grpc_timeout() {
        let mut request = fixture_request();
        request.timeout_ms = 1;
        let err = host_code(run_local_grpc(
            Some(Box::new(|request| {
                std::thread::sleep(Duration::from_millis(u64::from(request.timeout_ms) + 5));
                Err(Error::Message("deadline passed".to_string()))
            })),
            request,
        ));
        assert_eq!(err.code, HOST_ERR_TIMEOUT);

        let err = host_code(run_local_grpc(
            Some(Box::new(|_| {
                Err(Error::Host(HostError {
                    code: HOST_ERR_TIMEOUT,
                    op: "local",
                }))
            })),
            fixture_request(),
        ));
        assert_eq!(err.code, HOST_ERR_TIMEOUT);
    }

    #[test]
    fn local_host_grpc_enforces_response_cap() {
        let mut request = fixture_request();
        request.max_response_bytes = 4;
        let err = host_code(run_local_grpc(
            Some(Box::new(|_| {
                Ok(GrpcResponse {
                    message: b"12345".to_vec(),
                    ..GrpcResponse::default()
                })
            })),
            request,
        ));
        assert_eq!(err.code, HOST_ERR_TOO_LARGE);
    }

    type MetadataCase = (&'static str, &'static [(&'static str, &'static str)], bool);

    #[test]
    fn local_host_grpc_metadata_rules() {
        let cases: [MetadataCase; 6] = [
            ("lowercased", &[("X-Request-ID", "req-0002")], false),
            ("grpc prefix", &[("grpc-timeout", "1S")], true),
            ("pseudo header", &[(":authority", "api.example.com")], true),
            (
                "reserved header",
                &[("Content-Type", "application/grpc")],
                true,
            ),
            ("bad binary", &[("trace-bin", "not base64!")], true),
            ("case duplicate", &[("x-a", "1"), ("X-A", "2")], true),
        ];
        for (name, metadata, want_err) in cases {
            let mut request = fixture_request();
            request.metadata = metadata
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect::<BTreeMap<_, _>>();
            let seen = Arc::new(Mutex::new(BTreeMap::new()));
            let handler_seen = Arc::clone(&seen);
            let result = run_local_grpc(
                Some(Box::new(move |got| {
                    *handler_seen.lock().unwrap() = got.metadata;
                    Ok(GrpcResponse::default())
                })),
                request,
            );
            if want_err {
                assert_eq!(host_code(result).code, HOST_ERR_INVALID, "{name}");
                continue;
            }
            result.unwrap_or_else(|err| panic!("{name}: {err}"));
            assert_eq!(
                seen.lock().unwrap().get("x-request-id").map(String::as_str),
                Some("req-0002"),
                "{name}"
            );
        }
    }

    #[test]
    fn local_host_grpc_rejects_unnormalized_transport() {
        let mut raw: serde_json::Value = serde_json::from_str(REQUEST_FIXTURE).unwrap();
        raw["transport"] = serde_json::Value::String("TLS".to_string());
        assert!(decode_local_grpc_request(&serde_json::to_vec(&raw).unwrap()).is_err());
        raw["transport"] = serde_json::Value::String(String::new());
        assert!(decode_local_grpc_request(&serde_json::to_vec(&raw).unwrap()).is_err());
    }
}

mod credential_grants {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use serde_json::json;

    use super::super::{
        LocalHostOptions, LocalInputOptions, LocalInputs, local_oauth2_bearer_token, run_local_host,
    };
    use crate::credential_grant::tests::fixture_oauth2_grant;
    use crate::error::HOST_ERR_DENIED;
    use crate::{
        CREDENTIAL_INJECT_HTTP_HEADER, CredentialBrokerGrant, Error, HTTP,
        HTTP_RESPONSE_MODE_ENVELOPE, HTTP_RESPONSE_MODE_STATUS_BODY, HttpRequest, HttpResponse,
        SdkResult,
    };

    type Calls = Arc<Mutex<Vec<HttpRequest>>>;

    fn run_grant_requests(
        credentials: BTreeMap<String, String>,
        grants: Vec<CredentialBrokerGrant>,
        requests: Vec<HttpRequest>,
    ) -> (Vec<HttpRequest>, Vec<SdkResult<HttpResponse>>) {
        let calls: Calls = Arc::new(Mutex::new(Vec::new()));
        let handler_calls = Arc::clone(&calls);
        let mut results = Vec::new();
        let (_capture, result) = run_local_host(
            LocalHostOptions {
                config_json: b"{}".to_vec(),
                http_handler: Some(Box::new(move |request| {
                    handler_calls.lock().unwrap().push(request);
                    Ok(HttpResponse {
                        status: 200,
                        ..HttpResponse::default()
                    })
                })),
                credential_grants: grants,
                credentials,
                ..LocalHostOptions::default()
            },
            || {
                for request in requests {
                    results.push(HTTP.do_request(request));
                }
                Ok(())
            },
        );
        result.unwrap();
        let calls = calls.lock().unwrap().clone();
        (calls, results)
    }

    fn local_oauth2_credentials() -> BTreeMap<String, String> {
        let directory = super::tempfile_directory("oauth2-credentials");
        let env_path = directory.join(".env");
        std::fs::write(
            &env_path,
            "SERVICERADAR_CREDENTIAL_CLIENT_ID=local-client\n\
             SERVICERADAR_CREDENTIAL_CLIENT_SECRET=local-client-secret\n",
        )
        .unwrap();
        let credentials = LocalInputs::load(LocalInputOptions {
            config_json: Some(b"{}".to_vec()),
            env_file: Some(env_path),
            environment: Some(BTreeMap::new()),
            ..LocalInputOptions::default()
        })
        .unwrap()
        .credentials();
        std::fs::remove_dir_all(directory).unwrap();
        assert_eq!(
            credentials.get("client_secret").map(String::as_str),
            Some("local-client-secret")
        );
        credentials
    }

    #[test]
    fn local_oauth2_bearer_token_matches_go_sdk_derivation() {
        // Expected value computed independently as
        // sha256("serviceradar.local_oauth2\0" + grant_id + "\0" + credential_secret_ref),
        // first 16 bytes in hex, which is how the Go SDK derives it.
        let grant = CredentialBrokerGrant {
            grant_id: "grant-b".to_string(),
            credential_secret_ref: Some("credentialref:secret-a".to_string()),
            ..CredentialBrokerGrant::default()
        };
        assert_eq!(
            local_oauth2_bearer_token(&grant),
            "local-oauth2.c9d150e5d81aa5d69a2b1dbcdbda9c42"
        );
    }

    #[test]
    fn local_host_injects_oauth2_client_credentials_bearer() {
        let grant = fixture_oauth2_grant();
        let token = local_oauth2_bearer_token(&grant);
        let (calls, results) = run_grant_requests(
            local_oauth2_credentials(),
            vec![grant],
            vec![
                HttpRequest::get("https://api.example.com/v1/devices")
                    .with_header("authorization", "Bearer guest-supplied")
                    .with_header("Accept", "application/json"),
                HttpRequest::get("https://other.example.com/v1/devices"),
                HttpRequest::new("POST", "https://api.example.com/v1/devices"),
            ],
        );
        for (index, result) in results.iter().enumerate() {
            assert!(result.is_ok(), "request {index}: {result:?}");
        }
        assert_eq!(calls.len(), 3);

        let covered = &calls[0].headers;
        assert_eq!(
            covered.get("Authorization").map(String::as_str),
            Some(format!("Bearer {token}").as_str())
        );
        assert!(!covered.contains_key("authorization"));
        assert_eq!(
            covered.get("Accept").map(String::as_str),
            Some("application/json")
        );
        assert!(
            covered
                .values()
                .all(|value| !value.contains("local-client-secret"))
        );
        assert!(!token.contains("local-client"));
        for call in &calls[1..] {
            assert!(
                !call.headers.contains_key("Authorization"),
                "request outside the allow scope got a bearer: {}",
                call.url
            );
        }
    }

    #[test]
    fn local_host_denies_covered_requests_it_cannot_authorize() {
        let mut targeted = fixture_oauth2_grant();
        targeted
            .inject
            .insert("path".to_string(), json!("/v1/devices"));
        let cases = [
            (
                "missing secret",
                BTreeMap::from([("client_id".to_string(), "local-client".to_string())]),
                fixture_oauth2_grant(),
                HttpRequest::get("https://api.example.com/v1/devices"),
            ),
            (
                "outside inject target",
                local_oauth2_credentials(),
                targeted,
                HttpRequest::get("https://api.example.com/v1/sites"),
            ),
            (
                "insecure tls",
                local_oauth2_credentials(),
                fixture_oauth2_grant(),
                HttpRequest::get("https://api.example.com/v1/devices").with_insecure_tls(true),
            ),
        ];
        for (name, credentials, grant, request) in cases {
            let (calls, results) = run_grant_requests(credentials, vec![grant], vec![request]);
            match &results[0] {
                Err(Error::Host(err)) => assert_eq!(err.code, HOST_ERR_DENIED, "{name}"),
                other => panic!("{name}: expected host error -2, got {other:?}"),
            }
            assert!(
                calls.is_empty(),
                "{name}: denied request reached the handler"
            );
        }
    }

    #[test]
    fn local_host_skips_expired_and_other_grant_types() {
        let mut expired = fixture_oauth2_grant();
        expired.expires_at = Some("2020-01-01T00:00:00Z".to_string());
        let mut header = fixture_oauth2_grant();
        header.inject = BTreeMap::from([
            ("type".to_string(), json!(CREDENTIAL_INJECT_HTTP_HEADER)),
            ("name".to_string(), json!("X-Api-Key")),
        ]);
        let (calls, results) = run_grant_requests(
            local_oauth2_credentials(),
            vec![expired, header],
            vec![HttpRequest::get("https://api.example.com/v1/devices")],
        );
        assert!(results[0].is_ok());
        assert_eq!(calls.len(), 1);
        assert!(calls[0].headers.is_empty(), "{:?}", calls[0].headers);
    }

    #[test]
    fn local_host_http_response_modes() {
        let modes = Arc::new(Mutex::new(Vec::new()));
        let handler_modes = Arc::clone(&modes);
        let mut responses = Vec::new();
        let (_capture, result) = run_local_host(
            LocalHostOptions {
                config_json: b"{}".to_vec(),
                http_handler: Some(Box::new(move |request| {
                    handler_modes.lock().unwrap().push(request.response_mode);
                    Ok(HttpResponse {
                        status: 429,
                        headers: BTreeMap::from([
                            ("Retry-After".to_string(), "30".to_string()),
                            ("Content-Type".to_string(), "application/json".to_string()),
                        ]),
                        body: b"{}".to_vec(),
                        ..HttpResponse::default()
                    })
                })),
                ..LocalHostOptions::default()
            },
            || {
                responses.push(
                    HTTP.do_request(
                        HttpRequest::get("https://api.example.com/v1/items")
                            .with_response_mode(HTTP_RESPONSE_MODE_ENVELOPE),
                    )?,
                );
                responses.push(HTTP.get("https://api.example.com/v1/items")?);
                Ok(())
            },
        );
        result.unwrap();
        assert_eq!(
            *modes.lock().unwrap(),
            vec![HTTP_RESPONSE_MODE_ENVELOPE, HTTP_RESPONSE_MODE_STATUS_BODY]
        );
        let envelope = &responses[0];
        assert_eq!(envelope.status, 429);
        assert_eq!(envelope.header("content-type"), Some("application/json"));
        assert_eq!(envelope.retry_after(), Some(Duration::from_secs(30)));
        let status_body = &responses[1];
        assert_eq!(status_body.status, 429);
        assert!(status_body.headers.is_empty());
        assert_eq!(status_body.body, b"{}");
    }
}
