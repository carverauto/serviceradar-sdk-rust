use super::{alloc, dealloc};

#[test]
fn alloc_stub_matches_go_sdk_behavior() {
    assert_eq!(alloc(0), 0);
    let ptr = alloc(8);
    assert_ne!(ptr, 0);
    dealloc(ptr, 8);
}
