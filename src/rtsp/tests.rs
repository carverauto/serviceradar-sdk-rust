use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;

use crate::{Error, SdkResult};

use super::{
    RtspAuthState, RtspClient, RtspEndpoint, RtspH264Depacketizer, RtspInterleavedFrame,
    RtspResponse, parse_rtp_packet, parse_rtsp_authenticate_header, parse_rtsp_endpoint,
    parse_rtsp_response,
};

#[derive(Default)]
struct FakeRtspTransport {
    writes: Vec<Vec<u8>>,
    reads: VecDeque<Vec<u8>>,
}

impl super::RtspTransport for FakeRtspTransport {
    fn read(&mut self, buf: &mut [u8], _timeout: Duration) -> SdkResult<usize> {
        let payload = self.reads.pop_front().unwrap_or_default();
        let len = payload.len().min(buf.len());
        buf[..len].copy_from_slice(&payload[..len]);
        Ok(len)
    }

    fn write(&mut self, data: &[u8], _timeout: Duration) -> SdkResult<usize> {
        self.writes.push(data.to_vec());
        Ok(data.len())
    }

    fn close(&mut self) -> SdkResult<()> {
        Ok(())
    }
}

#[test]
fn parse_endpoint() {
    let endpoint = parse_rtsp_endpoint(
        "rtsp://root:secret@10.0.0.5:8554/axis-media/media.amp?stream=1",
        "",
        "",
    )
    .expect("parse endpoint");
    assert_eq!(endpoint.host, "10.0.0.5");
    assert_eq!(endpoint.port, 8554);
    assert_eq!(endpoint.username, "root");
    assert_eq!(endpoint.password, "secret");
    assert_eq!(endpoint.request_uri, "/axis-media/media.amp?stream=1");
    assert_eq!(endpoint.scheme, "rtsp");
    assert_eq!(endpoint.authority(), "10.0.0.5:8554");
}

#[test]
fn build_basic_authorization() {
    let auth = RtspEndpoint {
        raw_url: String::new(),
        scheme: "rtsp".to_string(),
        host: String::new(),
        port: 554,
        request_uri: String::new(),
        base_url: String::new(),
        username: "root".to_string(),
        password: "secret".to_string(),
    }
    .authorization("DESCRIBE", "/axis-media/media.amp", None);
    assert_eq!(auth, "Basic cm9vdDpzZWNyZXQ=");
}

#[test]
fn parse_digest_authenticate_header() {
    let auth = parse_rtsp_authenticate_header(
        r#"Digest realm="AXIS", nonce="abcdef", opaque="opaque-token", qop="auth,auth-int", algorithm=MD5"#,
    )
    .expect("parse auth");
    assert_eq!(auth.scheme, "digest");
    assert_eq!(auth.realm, "AXIS");
    assert_eq!(auth.nonce, "abcdef");
    assert_eq!(auth.qop, "auth");
    assert!(auth.is_digest());
}

#[test]
fn parse_response() {
    let response = RtspResponse::parse(
        b"RTSP/1.0 200 OK\r\nCSeq: 2\r\nContent-Length: 17\r\nSession: 12345;timeout=60\r\n\r\nv=0\r\nm=video 0\r\n",
    )
    .expect("parse response");
    assert_eq!(response.status_code, 200);
    assert_eq!(response.headers["session"], "12345;timeout=60");
    assert_eq!(response.content_length, 17);
    assert!(response.is_success());
    assert_eq!(response.header("Session"), Some("12345;timeout=60"));
    assert_eq!(response.session().as_deref(), Some("12345"));
}

#[test]
fn parse_h264_track() {
    let endpoint = RtspEndpoint {
        raw_url: String::new(),
        scheme: "rtsp".to_string(),
        host: "10.0.0.5".to_string(),
        port: 554,
        request_uri: "/axis-media/media.amp".to_string(),
        base_url: "rtsp://10.0.0.5".to_string(),
        username: String::new(),
        password: String::new(),
    };
    let sdp = b"v=0\nm=video 0 RTP/AVP 96\na=rtpmap:96 H264/90000\na=control:trackID=1\n";
    let track = endpoint.find_h264_track(sdp).expect("parse track");
    assert_eq!(track.payload_type, 96);
    assert_eq!(
        track.control_url,
        "rtsp://10.0.0.5/axis-media/media.amp/trackID=1"
    );
    assert_eq!(
        endpoint.resolve_control_url("trackID=1"),
        "rtsp://10.0.0.5/axis-media/media.amp/trackID=1"
    );
}

#[test]
fn parse_interleaved_and_rtp() {
    let frame = RtspInterleavedFrame::parse(&[b'$', 0x00, 0x00, 0x04, 0xDE, 0xAD, 0xBE, 0xEF])
        .expect("frame");
    assert_eq!(frame.channel, 0);
    assert_eq!(frame.payload, vec![0xDE, 0xAD, 0xBE, 0xEF]);
    assert_eq!(frame.len(), 4);
    assert!(!frame.is_empty());

    let packet = [
        0x80, 0xE0, 0x00, 0x02, 0x00, 0x00, 0x03, 0xE8, 0x12, 0x34, 0x56, 0x78, 0x65, 0x88, 0x84,
    ];
    let (payload, marker, timestamp) = parse_rtp_packet(&packet).expect("rtp");
    assert!(marker);
    assert_eq!(timestamp, 1000);
    assert_eq!(payload, vec![0x65, 0x88, 0x84]);

    let mut depacketizer = RtspH264Depacketizer::default();
    let (unit, keyframe, complete) = depacketizer.push_rtp_packet(&packet).expect("rtp push");
    assert!(complete);
    assert!(keyframe);
    assert_eq!(unit, vec![0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84]);
}

#[test]
fn depacketizes_single_nal_and_fua() {
    let mut depacketizer = RtspH264Depacketizer::default();
    let (unit, keyframe, complete) = depacketizer
        .push(&[0x65, 0x88, 0x84], true, 1000)
        .expect("single nal");
    assert!(complete);
    assert!(keyframe);
    assert_eq!(unit, vec![0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84]);

    let mut depacketizer = RtspH264Depacketizer::default();
    assert!(
        !depacketizer
            .push(&[0x7C, 0x85, 0xAA, 0xBB], false, 1000)
            .expect("start")
            .2
    );
    assert!(
        !depacketizer
            .push(&[0x7C, 0x05, 0xCC], false, 1000)
            .expect("middle")
            .2
    );
    let (unit, keyframe, complete) = depacketizer
        .push(&[0x7C, 0x45, 0xDD, 0xEE], true, 1000)
        .expect("end");
    assert!(complete);
    assert!(keyframe);
    assert_eq!(
        unit,
        vec![0x00, 0x00, 0x00, 0x01, 0x65, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE]
    );
}

#[test]
fn request_includes_digest_authorization() {
    let mut headers = BTreeMap::new();
    headers.insert(
        "Transport".to_string(),
        "RTP/AVP/TCP;unicast;interleaved=0-1".to_string(),
    );

    let request = RtspEndpoint {
        raw_url: String::new(),
        scheme: "rtsp".to_string(),
        host: String::new(),
        port: 554,
        request_uri: "/axis-media/media.amp".to_string(),
        base_url: String::new(),
        username: "root".to_string(),
        password: "secret".to_string(),
    }
    .build_request(
        "SETUP",
        "/axis-media/media.amp/trackID=1",
        3,
        "session-1",
        Some(&mut RtspAuthState {
            scheme: "digest".to_string(),
            realm: "AXIS".to_string(),
            nonce: "abcdef".to_string(),
            opaque: String::new(),
            algorithm: "MD5".to_string(),
            qop: "auth".to_string(),
            cnonce: "serviceradar".to_string(),
            nc: 0,
        }),
        &headers,
    );

    assert!(request.contains("Authorization: Digest "));
    assert!(request.contains("Session: session-1"));
}

#[test]
fn read_rtsp_response_reads_from_transport() {
    let mut transport = FakeRtspTransport {
        writes: Vec::new(),
        reads: VecDeque::from([b"RTSP/1.0 200 OK\r\nCSeq: 1\r\n\r\n".to_vec()]),
    };

    let response = RtspResponse::read_from(&mut transport, Duration::from_secs(1))
        .expect("read rtsp response");
    assert_eq!(response.status_code, 200);
}

#[test]
fn rtsp_client_retries_digest_challenge_and_tracks_session() {
    let transport = FakeRtspTransport {
        writes: Vec::new(),
        reads: VecDeque::from([
            b"RTSP/1.0 401 Unauthorized\r\nCSeq: 1\r\nWWW-Authenticate: Digest realm=\"AXIS\", nonce=\"abcdef\", qop=\"auth\", algorithm=MD5\r\n\r\n".to_vec(),
            b"RTSP/1.0 200 OK\r\nCSeq: 2\r\nSession: session-1;timeout=60\r\n\r\n".to_vec(),
            b"RTSP/1.0 200 OK\r\nCSeq: 3\r\n\r\n".to_vec(),
        ]),
    };

    let endpoint = parse_rtsp_endpoint("rtsp://root:secret@10.0.0.5/axis-media/media.amp", "", "")
        .expect("parse endpoint");
    let mut client = RtspClient::new(transport, Duration::from_secs(1), endpoint);
    let response = client
        .request("DESCRIBE", "/axis-media/media.amp", &BTreeMap::new())
        .expect("describe request");

    assert_eq!(response.status_code, 200);
    assert_eq!(client.session, "session-1");
    assert_eq!(client.session(), Some("session-1"));
    assert_eq!(client.seq, 3);
    assert_eq!(client.conn.writes.len(), 2);
    assert!(String::from_utf8_lossy(&client.conn.writes[1]).contains("Authorization: Digest "));

    client.teardown().expect("teardown");
    assert!(client.session.is_empty());
    assert_eq!(client.conn.writes.len(), 3);
    assert!(String::from_utf8_lossy(&client.conn.writes[2]).starts_with("TEARDOWN "));
}

#[test]
fn parse_rtsp_response_rejects_invalid_payload() {
    let err = parse_rtsp_response(b"not-rtsp").expect_err("invalid response should fail");
    assert!(matches!(err, Error::RtspBadResponse));
}

#[test]
fn domain_entry_points_delegate_to_existing_parsers() {
    let endpoint = RtspEndpoint::parse("rtsp://root:secret@10.0.0.5/axis-media/media.amp", "", "")
        .expect("parse endpoint");
    assert_eq!(endpoint.host, "10.0.0.5");
    assert_eq!(
        endpoint.resolve_control_url("trackID=1"),
        "rtsp://10.0.0.5/axis-media/media.amp/trackID=1"
    );

    let auth = RtspAuthState::parse(r#"Basic realm="cam""#).expect("parse auth");
    assert!(auth.is_basic());

    let frame = RtspInterleavedFrame::parse(&[b'$', 0x00, 0x00, 0x04, 0xDE, 0xAD, 0xBE, 0xEF])
        .expect("parse frame");
    assert_eq!(frame.payload, vec![0xDE, 0xAD, 0xBE, 0xEF]);
}
