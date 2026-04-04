use crate::host;

pub fn alloc(size: u32) -> u32 {
    host::guest_alloc(size)
}

pub fn dealloc(ptr: u32, size: u32) {
    host::guest_dealloc(ptr, size);
}

#[cfg(test)]
mod tests;
