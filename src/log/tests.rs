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
