use super::with_url_user_info;

#[test]
fn injects_url_user_info() {
    let url = with_url_user_info(
        "wss://camera.local/vapix/ws-data-stream?sources=events",
        "root",
        "secret",
    );
    assert_eq!(
        url,
        "wss://root:secret@camera.local/vapix/ws-data-stream?sources=events"
    );
}
