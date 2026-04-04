use std::time::Duration;

use crate::host::{TestHostBackend, install_test_backend};

use super::udp_send_to;

struct UdpTestHost;

impl TestHostBackend for UdpTestHost {
    fn udp_send_to(&mut self, addr: &[u8], port: u32, buf: &[u8], timeout_ms: u32) -> i32 {
        assert_eq!(addr, b"127.0.0.1");
        assert_eq!(port, 8125);
        assert_eq!(buf, b"ping");
        assert_eq!(timeout_ms, 5_000);
        0
    }
}

#[test]
fn udp_send_uses_host_proxy() {
    let _guard = install_test_backend(Box::new(UdpTestHost));
    udp_send_to("127.0.0.1", 8125, b"ping", Duration::from_secs(5)).expect("udp send");
}
