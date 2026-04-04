use crate::host;

pub fn alloc(size: u32) -> u32 {
    host::guest_alloc(size)
}

pub fn dealloc(ptr: u32, size: u32) {
    host::guest_dealloc(ptr, size);
}

#[cfg(test)]
mod tests {
    use super::{alloc, dealloc};

    #[test]
    fn alloc_stub_matches_go_sdk_behavior() {
        assert_eq!(alloc(0), 0);
        let ptr = alloc(8);
        assert_ne!(ptr, 0);
        dealloc(ptr, 8);
    }
}
