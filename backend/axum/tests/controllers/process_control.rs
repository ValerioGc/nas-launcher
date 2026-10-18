use super::*;
use axum::body::{to_bytes, Body};
use axum::http::StatusCode;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixListener,
};

#[tokio::test]
async fn legacy_telemetry_is_forwarded_to_the_application_without_collecting_system_data() {
    let socket =
        std::env::temp_dir().join(format!("nas-monitor-forward-{}.sock", std::process::id()));
    let listener = UnixListener::bind(&socket).unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut bytes = [0; 2048];
        let read = stream.read(&mut bytes).await.unwrap();
        let request = String::from_utf8_lossy(&bytes[..read]);
        assert!(request.starts_with("GET /api/telemetry HTTP/1.1"));
        let body = r#"{"metrics":{"physicalCores":4,"cores":8},"processes":[],"sensors":[]}"#;
        stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
    });
    let app = manifest::Manifest {
        id: "nas-system-monitor".to_owned(),
        name: "Monitor".to_owned(),
        version: "0.1.2".to_owned(),
        icon: String::new(),
        category: None,
        system: false,
        protocol: 1,
        provides: Vec::new(),
        paths: Vec::new(),
        network: Vec::new(),
        packages: Vec::new(),
        background: false,
        shares: false,
        cleanup: false,
        socket: socket.clone(),
        exec: String::new(),
    };
    let response = forward_telemetry(
        app,
        Request::builder()
            .uri("/api/system/telemetry")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 2048).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(payload["metrics"]["physicalCores"], 4);
    assert_eq!(payload["metrics"]["cores"], 8);
    server.await.unwrap();
    std::fs::remove_file(socket).unwrap();
}
