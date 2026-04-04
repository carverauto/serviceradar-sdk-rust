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
mod tests;
