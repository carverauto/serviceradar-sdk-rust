use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::host::{TestHostBackend, install_test_backend};
use crate::{Error, HostError, PluginResult};

use super::{execute, execute_partial, submit_result_payload};

#[derive(Default)]
struct ExecuteState {
    logs: Vec<(u32, String)>,
    submitted: Vec<Vec<u8>>,
}

struct ExecuteHost {
    state: Arc<Mutex<ExecuteState>>,
}

impl TestHostBackend for ExecuteHost {
    fn log(&mut self, level: u32, msg: &[u8]) {
        self.state
            .lock()
            .expect("execute state mutex")
            .logs
            .push((level, String::from_utf8_lossy(msg).into_owned()));
    }

    fn submit_result(&mut self, payload: &[u8]) -> i32 {
        self.state
            .lock()
            .expect("execute state mutex")
            .submitted
            .push(payload.to_vec());
        0
    }
}

#[test]
fn submit_result_payload_rejects_empty_payload() {
    let err = submit_result_payload(&[]).expect_err("empty payload should fail");
    assert!(matches!(
        err,
        Error::Host(HostError {
            code: -1,
            op: "submit_result"
        })
    ));
}

#[test]
fn execute_submits_success_result() {
    let state = Arc::new(Mutex::new(ExecuteState::default()));
    let _guard = install_test_backend(Box::new(ExecuteHost {
        state: Arc::clone(&state),
    }));

    execute(|| Ok(PluginResult::ok("ok"))).expect("execute success");

    let state = state.lock().expect("execute state mutex");
    assert!(state.logs.is_empty());
    assert_eq!(state.submitted.len(), 1);

    let payload: Value =
        serde_json::from_slice(&state.submitted[0]).expect("decode submitted payload");
    assert_eq!(payload["status"], "OK");
    assert_eq!(payload["summary"], "ok");
}

#[test]
fn execute_converts_errors_into_critical_results_and_logs() {
    let state = Arc::new(Mutex::new(ExecuteState::default()));
    let _guard = install_test_backend(Box::new(ExecuteHost {
        state: Arc::clone(&state),
    }));

    execute(|| Err(Error::Message("boom".to_string()))).expect("execute error path");

    let state = state.lock().expect("execute state mutex");
    assert_eq!(state.logs, vec![(3, "plugin error".to_string())]);
    assert_eq!(state.submitted.len(), 1);

    let payload: Value =
        serde_json::from_slice(&state.submitted[0]).expect("decode submitted payload");
    assert_eq!(payload["status"], "CRITICAL");
    assert_eq!(payload["summary"], "plugin error: boom");
    assert_eq!(payload["details"], "boom");
}

#[test]
#[allow(clippy::result_large_err)]
fn execute_upgrades_partial_result_on_error() {
    let state = Arc::new(Mutex::new(ExecuteState::default()));
    let _guard = install_test_backend(Box::new(ExecuteHost {
        state: Arc::clone(&state),
    }));

    execute_partial(|| {
        let mut result = PluginResult::new();
        result.set_summary("camera health degraded");
        result.add_label("camera_id", "cam-1");
        Err((Some(result), Error::Message("stream timeout".to_string())))
    })
    .expect("execute partial error path");

    let state = state.lock().expect("execute state mutex");
    assert_eq!(state.logs, vec![(3, "plugin error".to_string())]);
    assert_eq!(state.submitted.len(), 1);

    let payload: Value =
        serde_json::from_slice(&state.submitted[0]).expect("decode submitted payload");
    assert_eq!(payload["status"], "CRITICAL");
    assert_eq!(payload["summary"], "camera health degraded");
    assert_eq!(payload["details"], "stream timeout");
    assert_eq!(payload["labels"]["camera_id"], "cam-1");
}
