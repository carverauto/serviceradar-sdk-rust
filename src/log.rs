use crate::host;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug = 0,
    Info = 1,
    Warn = 2,
    Error = 3,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Logger;

pub const LOG: Logger = Logger;

impl Logger {
    pub fn debug(&self, msg: impl AsRef<str>) {
        log_with_level(LogLevel::Debug, msg.as_ref());
    }

    pub fn info(&self, msg: impl AsRef<str>) {
        log_with_level(LogLevel::Info, msg.as_ref());
    }

    pub fn warn(&self, msg: impl AsRef<str>) {
        log_with_level(LogLevel::Warn, msg.as_ref());
    }

    pub fn error(&self, msg: impl AsRef<str>) {
        log_with_level(LogLevel::Error, msg.as_ref());
    }
}

fn log_with_level(level: LogLevel, msg: &str) {
    if msg.is_empty() {
        return;
    }

    host::log(level as u32, msg.as_bytes());
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::host::{TestHostBackend, install_test_backend};

    use super::LOG;

    #[derive(Default)]
    struct LogState {
        entries: Vec<(u32, String)>,
    }

    struct LogHost {
        state: Arc<Mutex<LogState>>,
    }

    impl TestHostBackend for LogHost {
        fn log(&mut self, level: u32, msg: &[u8]) {
            self.state
                .lock()
                .expect("log state mutex")
                .entries
                .push((level, String::from_utf8_lossy(msg).into_owned()));
        }
    }

    #[test]
    fn logger_forwards_levels_to_host() {
        let state = Arc::new(Mutex::new(LogState::default()));
        let _guard = install_test_backend(Box::new(LogHost {
            state: Arc::clone(&state),
        }));

        LOG.debug("debug");
        LOG.info("info");
        LOG.warn("warn");
        LOG.error("error");
        LOG.info("");

        let state = state.lock().expect("log state mutex");
        assert_eq!(
            state.entries,
            vec![
                (0, "debug".to_string()),
                (1, "info".to_string()),
                (2, "warn".to_string()),
                (3, "error".to_string()),
            ]
        );
    }
}
