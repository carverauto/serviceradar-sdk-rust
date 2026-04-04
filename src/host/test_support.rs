use std::sync::{Mutex, MutexGuard, OnceLock};

pub(crate) use super::HostBackend as TestHostBackend;

fn test_backend_lock() -> &'static Mutex<()> {
    static TEST_BACKEND_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    TEST_BACKEND_LOCK.get_or_init(|| Mutex::new(()))
}

pub(crate) struct TestHostGuard {
    previous: Option<Box<dyn TestHostBackend>>,
    _lock: MutexGuard<'static, ()>,
}

impl Drop for TestHostGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            *super::backend().lock().expect("host mutex poisoned") = previous;
        }
    }
}

pub(crate) fn install_test_backend(next: Box<dyn TestHostBackend>) -> TestHostGuard {
    let lock = test_backend_lock()
        .lock()
        .expect("test backend lock poisoned");
    let mut current = super::backend().lock().expect("host mutex poisoned");
    let previous = std::mem::replace(&mut *current, next);
    drop(current);

    TestHostGuard {
        previous: Some(previous),
        _lock: lock,
    }
}
