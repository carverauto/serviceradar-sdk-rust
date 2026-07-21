pub(crate) use super::HostBackend as TestHostBackend;
pub(crate) type TestHostGuard = super::NativeHostGuard;

pub(crate) fn install_test_backend(next: Box<dyn TestHostBackend>) -> TestHostGuard {
    super::install_native_backend(next)
}
