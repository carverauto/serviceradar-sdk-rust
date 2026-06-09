#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Mutex, OnceLock};

#[cfg(not(target_arch = "wasm32"))]
use crate::error::HOST_ERR_NOT_FOUND;
use crate::error::{HostError, host_error};

#[cfg(not(target_arch = "wasm32"))]
pub(crate) trait HostBackend: Send {
    fn get_config(&mut self, _buf: &mut [u8]) -> i32 {
        HOST_ERR_NOT_FOUND
    }

    fn log(&mut self, _level: u32, _msg: &[u8]) {}

    fn submit_result(&mut self, _payload: &[u8]) -> i32 {
        HOST_ERR_NOT_FOUND
    }

    fn emit_telemetry(&mut self, _payload: &[u8]) -> i32 {
        HOST_ERR_NOT_FOUND
    }

    fn http_request(&mut self, _req: &[u8], _resp: &mut [u8]) -> i32 {
        HOST_ERR_NOT_FOUND
    }

    fn tcp_connect(&mut self, _addr: &[u8], _port: u32, _timeout_ms: u32) -> i32 {
        HOST_ERR_NOT_FOUND
    }

    fn tcp_read(&mut self, _handle: u32, _buf: &mut [u8], _timeout_ms: u32) -> i32 {
        HOST_ERR_NOT_FOUND
    }

    fn tcp_write(&mut self, _handle: u32, _buf: &[u8], _timeout_ms: u32) -> i32 {
        HOST_ERR_NOT_FOUND
    }

    fn tcp_close(&mut self, _handle: u32) -> i32 {
        HOST_ERR_NOT_FOUND
    }

    fn udp_send_to(&mut self, _addr: &[u8], _port: u32, _buf: &[u8], _timeout_ms: u32) -> i32 {
        HOST_ERR_NOT_FOUND
    }

    fn websocket_connect(&mut self, _req: &[u8], _timeout_ms: u32) -> i32 {
        HOST_ERR_NOT_FOUND
    }

    fn websocket_send(&mut self, _handle: u32, _data: &[u8], _timeout_ms: u32) -> i32 {
        HOST_ERR_NOT_FOUND
    }

    fn websocket_recv(&mut self, _handle: u32, _buf: &mut [u8], _timeout_ms: u32) -> i32 {
        HOST_ERR_NOT_FOUND
    }

    fn websocket_close(&mut self, _handle: u32) -> i32 {
        HOST_ERR_NOT_FOUND
    }

    fn camera_media_open(&mut self, _req: &[u8]) -> i32 {
        HOST_ERR_NOT_FOUND
    }

    fn camera_media_write(&mut self, _handle: u32, _meta: &[u8], _payload: &[u8]) -> i32 {
        HOST_ERR_NOT_FOUND
    }

    fn camera_media_heartbeat(&mut self, _handle: u32, _meta: &[u8]) -> i32 {
        HOST_ERR_NOT_FOUND
    }

    fn camera_media_close(&mut self, _handle: u32, _reason: &[u8]) -> i32 {
        HOST_ERR_NOT_FOUND
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct DefaultHostBackend;

#[cfg(not(target_arch = "wasm32"))]
impl HostBackend for DefaultHostBackend {}

#[cfg(not(target_arch = "wasm32"))]
fn backend() -> &'static Mutex<Box<dyn HostBackend>> {
    static BACKEND: OnceLock<Mutex<Box<dyn HostBackend>>> = OnceLock::new();
    BACKEND.get_or_init(|| Mutex::new(Box::new(DefaultHostBackend)))
}

#[cfg(test)]
mod test_support;
#[cfg(test)]
pub(crate) use test_support::{TestHostBackend, install_test_backend};

#[cfg(target_arch = "wasm32")]
mod wasm {
    unsafe extern "C" {
        #[link_name = "get_config"]
        fn raw_get_config(ptr: u32, size: u32) -> i32;
        #[link_name = "log"]
        fn raw_log(level: u32, ptr: u32, size: u32);
        #[link_name = "submit_result"]
        fn raw_submit_result(ptr: u32, size: u32) -> i32;
        #[link_name = "emit_telemetry"]
        fn raw_emit_telemetry(ptr: u32, size: u32) -> i32;
        #[link_name = "http_request"]
        fn raw_http_request(req_ptr: u32, req_len: u32, resp_ptr: u32, resp_len: u32) -> i32;
        #[link_name = "tcp_connect"]
        fn raw_tcp_connect(addr_ptr: u32, addr_len: u32, port: u32, timeout_ms: u32) -> i32;
        #[link_name = "tcp_read"]
        fn raw_tcp_read(handle: u32, ptr: u32, len: u32, timeout_ms: u32) -> i32;
        #[link_name = "tcp_write"]
        fn raw_tcp_write(handle: u32, ptr: u32, len: u32, timeout_ms: u32) -> i32;
        #[link_name = "tcp_close"]
        fn raw_tcp_close(handle: u32) -> i32;
        #[link_name = "udp_sendto"]
        fn raw_udp_sendto(
            addr_ptr: u32,
            addr_len: u32,
            port: u32,
            payload_ptr: u32,
            payload_len: u32,
            timeout_ms: u32,
        ) -> i32;
        #[link_name = "websocket_connect"]
        fn raw_websocket_connect(ptr: u32, len: u32, timeout_ms: u32) -> i32;
        #[link_name = "websocket_send"]
        fn raw_websocket_send(handle: u32, ptr: u32, len: u32, timeout_ms: u32) -> i32;
        #[link_name = "websocket_recv"]
        fn raw_websocket_recv(handle: u32, ptr: u32, len: u32, timeout_ms: u32) -> i32;
        #[link_name = "websocket_close"]
        fn raw_websocket_close(handle: u32) -> i32;
        #[link_name = "camera_media_open"]
        fn raw_camera_media_open(ptr: u32, len: u32) -> i32;
        #[link_name = "camera_media_write"]
        fn raw_camera_media_write(
            handle: u32,
            meta_ptr: u32,
            meta_len: u32,
            payload_ptr: u32,
            payload_len: u32,
        ) -> i32;
        #[link_name = "camera_media_heartbeat"]
        fn raw_camera_media_heartbeat(handle: u32, meta_ptr: u32, meta_len: u32) -> i32;
        #[link_name = "camera_media_close"]
        fn raw_camera_media_close(handle: u32, reason_ptr: u32, reason_len: u32) -> i32;
    }

    fn ptr(data: &[u8]) -> u32 {
        if data.is_empty() {
            0
        } else {
            data.as_ptr() as usize as u32
        }
    }

    fn mut_ptr(data: &mut [u8]) -> u32 {
        if data.is_empty() {
            0
        } else {
            data.as_mut_ptr() as usize as u32
        }
    }

    pub(crate) fn get_config(buf: &mut [u8]) -> i32 {
        unsafe { raw_get_config(mut_ptr(buf), buf.len() as u32) }
    }

    pub(crate) fn log(level: u32, msg: &[u8]) {
        unsafe { raw_log(level, ptr(msg), msg.len() as u32) }
    }

    pub(crate) fn submit_result(payload: &[u8]) -> i32 {
        unsafe { raw_submit_result(ptr(payload), payload.len() as u32) }
    }

    pub(crate) fn emit_telemetry(payload: &[u8]) -> i32 {
        unsafe { raw_emit_telemetry(ptr(payload), payload.len() as u32) }
    }

    pub(crate) fn http_request(req: &[u8], resp: &mut [u8]) -> i32 {
        unsafe { raw_http_request(ptr(req), req.len() as u32, mut_ptr(resp), resp.len() as u32) }
    }

    pub(crate) fn tcp_connect(addr: &[u8], port: u32, timeout_ms: u32) -> i32 {
        unsafe { raw_tcp_connect(ptr(addr), addr.len() as u32, port, timeout_ms) }
    }

    pub(crate) fn tcp_read(handle: u32, buf: &mut [u8], timeout_ms: u32) -> i32 {
        unsafe { raw_tcp_read(handle, mut_ptr(buf), buf.len() as u32, timeout_ms) }
    }

    pub(crate) fn tcp_write(handle: u32, buf: &[u8], timeout_ms: u32) -> i32 {
        unsafe { raw_tcp_write(handle, ptr(buf), buf.len() as u32, timeout_ms) }
    }

    pub(crate) fn tcp_close(handle: u32) -> i32 {
        unsafe { raw_tcp_close(handle) }
    }

    pub(crate) fn udp_send_to(addr: &[u8], port: u32, payload: &[u8], timeout_ms: u32) -> i32 {
        unsafe {
            raw_udp_sendto(
                ptr(addr),
                addr.len() as u32,
                port,
                ptr(payload),
                payload.len() as u32,
                timeout_ms,
            )
        }
    }

    pub(crate) fn websocket_connect(req: &[u8], timeout_ms: u32) -> i32 {
        unsafe { raw_websocket_connect(ptr(req), req.len() as u32, timeout_ms) }
    }

    pub(crate) fn websocket_send(handle: u32, payload: &[u8], timeout_ms: u32) -> i32 {
        unsafe { raw_websocket_send(handle, ptr(payload), payload.len() as u32, timeout_ms) }
    }

    pub(crate) fn websocket_recv(handle: u32, buf: &mut [u8], timeout_ms: u32) -> i32 {
        unsafe { raw_websocket_recv(handle, mut_ptr(buf), buf.len() as u32, timeout_ms) }
    }

    pub(crate) fn websocket_close(handle: u32) -> i32 {
        unsafe { raw_websocket_close(handle) }
    }

    pub(crate) fn camera_media_open(req: &[u8]) -> i32 {
        unsafe { raw_camera_media_open(ptr(req), req.len() as u32) }
    }

    pub(crate) fn camera_media_write(handle: u32, meta: &[u8], payload: &[u8]) -> i32 {
        unsafe {
            raw_camera_media_write(
                handle,
                ptr(meta),
                meta.len() as u32,
                ptr(payload),
                payload.len() as u32,
            )
        }
    }

    pub(crate) fn camera_media_heartbeat(handle: u32, meta: &[u8]) -> i32 {
        unsafe { raw_camera_media_heartbeat(handle, ptr(meta), meta.len() as u32) }
    }

    pub(crate) fn camera_media_close(handle: u32, reason: &[u8]) -> i32 {
        unsafe { raw_camera_media_close(handle, ptr(reason), reason.len() as u32) }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn alloc(size: u32) -> u32 {
        if size == 0 {
            return 0;
        }

        let mut buf = Vec::<u8>::with_capacity(size as usize);
        let ptr = buf.as_mut_ptr() as usize as u32;
        std::mem::forget(buf);
        ptr
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn dealloc(ptr: u32, size: u32) {
        if ptr == 0 || size == 0 {
            return;
        }

        unsafe {
            let _ = Vec::<u8>::from_raw_parts(ptr as usize as *mut u8, 0, size as usize);
        }
    }

    pub(crate) fn guest_alloc(size: u32) -> u32 {
        alloc(size)
    }

    pub(crate) fn guest_dealloc(ptr: u32, size: u32) {
        dealloc(ptr, size)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn get_config(buf: &mut [u8]) -> i32 {
    backend()
        .lock()
        .expect("host mutex poisoned")
        .get_config(buf)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn log(level: u32, msg: &[u8]) {
    backend()
        .lock()
        .expect("host mutex poisoned")
        .log(level, msg);
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn submit_result(payload: &[u8]) -> i32 {
    backend()
        .lock()
        .expect("host mutex poisoned")
        .submit_result(payload)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn emit_telemetry(payload: &[u8]) -> i32 {
    backend()
        .lock()
        .expect("host mutex poisoned")
        .emit_telemetry(payload)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn http_request(req: &[u8], resp: &mut [u8]) -> i32 {
    backend()
        .lock()
        .expect("host mutex poisoned")
        .http_request(req, resp)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn tcp_connect(addr: &[u8], port: u32, timeout_ms: u32) -> i32 {
    backend()
        .lock()
        .expect("host mutex poisoned")
        .tcp_connect(addr, port, timeout_ms)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn tcp_read(handle: u32, buf: &mut [u8], timeout_ms: u32) -> i32 {
    backend()
        .lock()
        .expect("host mutex poisoned")
        .tcp_read(handle, buf, timeout_ms)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn tcp_write(handle: u32, buf: &[u8], timeout_ms: u32) -> i32 {
    backend()
        .lock()
        .expect("host mutex poisoned")
        .tcp_write(handle, buf, timeout_ms)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn tcp_close(handle: u32) -> i32 {
    backend()
        .lock()
        .expect("host mutex poisoned")
        .tcp_close(handle)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn udp_send_to(addr: &[u8], port: u32, payload: &[u8], timeout_ms: u32) -> i32 {
    backend()
        .lock()
        .expect("host mutex poisoned")
        .udp_send_to(addr, port, payload, timeout_ms)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn websocket_connect(req: &[u8], timeout_ms: u32) -> i32 {
    backend()
        .lock()
        .expect("host mutex poisoned")
        .websocket_connect(req, timeout_ms)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn websocket_send(handle: u32, payload: &[u8], timeout_ms: u32) -> i32 {
    backend()
        .lock()
        .expect("host mutex poisoned")
        .websocket_send(handle, payload, timeout_ms)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn websocket_recv(handle: u32, buf: &mut [u8], timeout_ms: u32) -> i32 {
    backend()
        .lock()
        .expect("host mutex poisoned")
        .websocket_recv(handle, buf, timeout_ms)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn websocket_close(handle: u32) -> i32 {
    backend()
        .lock()
        .expect("host mutex poisoned")
        .websocket_close(handle)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn camera_media_open(req: &[u8]) -> i32 {
    backend()
        .lock()
        .expect("host mutex poisoned")
        .camera_media_open(req)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn camera_media_write(handle: u32, meta: &[u8], payload: &[u8]) -> i32 {
    backend()
        .lock()
        .expect("host mutex poisoned")
        .camera_media_write(handle, meta, payload)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn camera_media_heartbeat(handle: u32, meta: &[u8]) -> i32 {
    backend()
        .lock()
        .expect("host mutex poisoned")
        .camera_media_heartbeat(handle, meta)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn camera_media_close(handle: u32, reason: &[u8]) -> i32 {
    backend()
        .lock()
        .expect("host mutex poisoned")
        .camera_media_close(handle, reason)
}

#[cfg(target_arch = "wasm32")]
pub(crate) use wasm::*;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn guest_alloc(size: u32) -> u32 {
    if size == 0 { 0 } else { 1 }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn guest_dealloc(_ptr: u32, _size: u32) {}

pub(crate) fn submit_non_empty_result(payload: &[u8]) -> Result<(), HostError> {
    if payload.is_empty() {
        return Err(HostError {
            code: crate::error::HOST_ERR_INVALID,
            op: "submit_result",
        });
    }

    host_error(submit_result(payload), "submit_result")
}

pub(crate) fn emit_non_empty_telemetry(payload: &[u8]) -> Result<(), HostError> {
    if payload.is_empty() {
        return Err(HostError {
            code: crate::error::HOST_ERR_INVALID,
            op: "emit_telemetry",
        });
    }

    host_error(emit_telemetry(payload), "emit_telemetry")
}
