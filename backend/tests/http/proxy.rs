use super::*;
use http_body_util::BodyExt;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;

fn manifest(socket: PathBuf) -> Manifest {
    Manifest {
        id: "fileExplorer".to_owned(),
        name: "File explorer".to_owned(),
        version: "1.0.0".to_owned(),
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
        socket,
        exec: String::new(),
    }
}

fn socket_for(what: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("nas-proxy-{}-{what}.sock", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

fn serve(socket: &Path) -> tokio::task::JoinHandle<()> {
    let listener = UnixListener::bind(socket).expect("a free socket");
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("a connection");
        let mut buffer = [0_u8; 2048];
        let read = stream.read(&mut buffer).await.expect("the request");
        let request = String::from_utf8_lossy(&buffer[..read]).to_ascii_lowercase();
        let user = request
            .lines()
            .find_map(|line| line.strip_prefix("x-nas-user:"))
            .map(str::trim)
            .unwrap_or("nobody")
            .to_owned();

        let body = format!("user={user}");
        let answer = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\n\r\n{body}",
            body.len()
        );
        stream
            .write_all(answer.as_bytes())
            .await
            .expect("the answer");
        stream.flush().await.ok();
    })
}

async fn text(response: Response) -> String {
    let body = response
        .into_body()
        .collect()
        .await
        .expect("a body")
        .to_bytes();
    String::from_utf8_lossy(&body).into_owned()
}

#[tokio::test]
async fn the_application_is_told_the_user_the_gateway_named() {
    for (sent, told) in [
        (Some("valerio"), "valerio"),
        (Some("a b"), "local"),
        (None, "local"),
    ] {
        let socket = socket_for("user");
        let server = serve(&socket);
        let mut request = Request::builder().uri("/");
        if let Some(sent) = sent {
            request = request.header("x-nas-user", sent);
        }
        let response = forward(
            manifest(socket.clone()),
            request.body(Body::empty()).expect("a request"),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(text(response).await, format!("user={told}"));
        server.await.expect("the application finished");
        let _ = std::fs::remove_file(&socket);
    }
}

#[tokio::test]
async fn the_internal_paths_of_an_application_are_not_served_to_a_browser() {
    for path in ["/internal", "/internal/search"] {
        let request = Request::builder()
            .uri(path)
            .body(Body::empty())
            .expect("a request");
        let response = forward(manifest(socket_for("unused")), request).await;

        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{path}");
    }
}

#[tokio::test]
async fn an_upgrade_other_than_a_websocket_is_refused() {
    let request = Request::builder()
        .uri("/socket")
        .header("connection", "Upgrade")
        .header("upgrade", "h2c")
        .body(Body::empty())
        .expect("a request");
    let response = forward(manifest(socket_for("unused")), request).await;

    assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
}

/// An application that switches to a WebSocket and sends back whatever it receives.
fn serve_echo(socket: &Path) -> tokio::task::JoinHandle<()> {
    let listener = UnixListener::bind(socket).expect("a free socket");
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("a connection");
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            let read = stream.read(&mut buffer).await.expect("the request");
            request.extend_from_slice(&buffer[..read]);
        }
        let asked = String::from_utf8_lossy(&request).to_ascii_lowercase();
        assert!(asked.contains("upgrade: websocket"), "{asked}");
        stream
            .write_all(
                b"HTTP/1.1 101 Switching Protocols\r\nupgrade: websocket\r\nconnection: upgrade\r\n\r\n",
            )
            .await
            .expect("the switch");
        let read = stream.read(&mut buffer).await.expect("a frame");
        stream.write_all(&buffer[..read]).await.expect("the echo");
    })
}

#[tokio::test]
async fn a_websocket_reaches_the_application_and_back() {
    let socket = socket_for("websocket");
    let application = serve_echo(&socket);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a free port");
    let address = listener.local_addr().expect("an address");
    let routes = axum::Router::new()
        .fallback(move |request: Request| forward(manifest(socket.clone()), request));
    tokio::spawn(async move { axum::serve(listener, routes).await.expect("the proxy") });

    let mut browser = tokio::net::TcpStream::connect(address)
        .await
        .expect("the proxy");
    browser
        .write_all(
            b"GET /live HTTP/1.1\r\nhost: nas\r\nconnection: Upgrade\r\nupgrade: websocket\r\nsec-websocket-version: 13\r\nsec-websocket-key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n",
        )
        .await
        .expect("the handshake");
    let mut answer = Vec::new();
    let mut buffer = [0_u8; 1024];
    while !answer.windows(4).any(|window| window == b"\r\n\r\n") {
        let read = browser.read(&mut buffer).await.expect("the switch");
        answer.extend_from_slice(&buffer[..read]);
    }
    assert!(String::from_utf8_lossy(&answer).starts_with("HTTP/1.1 101"));

    browser.write_all(b"ping").await.expect("a frame");
    let read = browser.read(&mut buffer).await.expect("the echo");
    assert_eq!(&buffer[..read], b"ping");
    application.await.expect("the application");
}

#[tokio::test]
async fn an_application_that_is_not_listening_says_it_is_not_running() {
    let request = Request::builder()
        .uri("/")
        .body(Body::empty())
        .expect("a request");
    let response = forward(manifest(socket_for("absent")), request).await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(text(response).await.contains("not running"));
}

#[tokio::test]
async fn a_socket_path_longer_than_the_kernel_allows_is_named_as_such() {
    let long = PathBuf::from(format!("/tmp/{}.sock", "d/".repeat(60)));
    let reason = connect(&long).await.expect_err("too long to connect to");

    assert!(reason.contains("longer than"));
}

#[test]
fn an_application_keeps_its_cookies_on_its_own_host() {
    let mut headers = HeaderMap::new();
    for cookie in [
        "theme=dark; Path=/; HttpOnly",
        "nas_session=forged; Domain=nas.internal; Path=/",
        "tracker=1; path=/; DOMAIN=.nas.internal",
    ] {
        headers.append(header::SET_COOKIE, HeaderValue::from_static(cookie));
    }

    host_only_cookies(&mut headers);

    let kept: Vec<_> = headers.get_all(header::SET_COOKIE).iter().collect();
    assert_eq!(kept, ["theme=dark; Path=/; HttpOnly"]);
}

#[test]
fn a_redirect_of_an_application_stays_under_its_prefix() {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::LOCATION,
        HeaderValue::from_static("/login?next=%2F"),
    );
    under_prefix(&mut headers, "/apps/photos");
    assert_eq!(headers[header::LOCATION], "/apps/photos/login?next=%2F");

    for elsewhere in ["https://example.com/", "//example.com/", "page"] {
        let mut headers = HeaderMap::new();
        headers.insert(header::LOCATION, HeaderValue::from_static(elsewhere));
        under_prefix(&mut headers, "/apps/photos");
        assert_eq!(headers[header::LOCATION], elsewhere);
    }
}
