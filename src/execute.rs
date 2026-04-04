use crate::error::{Error, SdkResult};
use crate::host;
use crate::log::LOG;
use crate::result::{Result, Status};

pub type ExecuteErrorWithResult = (Option<Result>, Error);

pub fn execute<F>(func: F) -> SdkResult<()>
where
    F: FnOnce() -> SdkResult<Result>,
{
    let (result, err) = match func() {
        Ok(result) => (Some(result), None),
        Err(err) => (None, Some(err)),
    };
    finish_execute(result, err)
}

pub fn execute_partial<F>(func: F) -> SdkResult<()>
where
    F: FnOnce() -> std::result::Result<Result, ExecuteErrorWithResult>,
{
    let (result, err) = match func() {
        Ok(result) => (Some(result), None),
        Err((result, err)) => (result, Some(err)),
    };
    finish_execute(result, err)
}

fn finish_execute(mut result: Option<Result>, err: Option<Error>) -> SdkResult<()> {
    if let Some(err) = err {
        LOG.error("plugin error");
        result = Some(match result {
            Some(mut result) => {
                result.set_status(Status::Critical);
                if result.summary().is_none_or(str::is_empty) {
                    result.set_summary(format!("plugin error: {err}"));
                }
                if result.details().is_none_or(str::is_empty) {
                    result.set_details(err.to_string());
                }
                result
            }
            None => Result::critical(format!("plugin error: {err}")).with_details(err.to_string()),
        });
    }

    let result = result.unwrap_or_default();
    let payload = result.serialize()?;
    submit_result_payload(&payload)
}

pub fn submit_result_payload(payload: &[u8]) -> SdkResult<()> {
    host::submit_non_empty_result(payload)?;
    Ok(())
}

#[cfg(test)]
mod tests {
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
}
