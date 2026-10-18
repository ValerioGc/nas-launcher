use super::*;
use crate::services::database;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[test]
fn an_application_is_known_by_the_unit_systemd_started_it_in() {
    assert_eq!(
        application_of(
            "0::/system.slice/system-nas\\x2dapp.slice/nas-app@nas-file-manager.service\n"
        ),
        Some("nas-file-manager".to_owned())
    );
    assert_eq!(
        application_of("12:pids:/elsewhere\n0::/system.slice/system-nas\\x2dapp.slice/nas-app@backup_2.service"),
        Some("backup_2".to_owned())
    );
}

#[test]
fn any_other_process_is_nobody() {
    for cgroup in [
        "",
        "0::/user.slice/user-1000.slice/session-2.scope",
        "0::/system.slice/nas-os.service",
        "0::/system.slice/system-nas\\x2dapp.slice/nas-app@.service",
        "0::/system.slice/system-nas\\x2dapp.slice/nas-app@a\\x2fb.service",
        "1:name=nas-app@fake.service",
    ] {
        assert_eq!(application_of(cgroup), None, "{cgroup}");
    }
}

#[tokio::test]
async fn the_socket_turns_away_a_process_that_is_not_an_application() {
    let directory = std::env::temp_dir().join(format!("nas-channel-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("directory");
    let path = directory.join("apps.sock");
    let listener = UnixListener::bind(&path).expect("socket");
    let db = database::open_in_memory().await.expect("database");
    let router = crate::http::routes::build_app_router(AppState::new(db));
    tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<Peer>(),
        )
        .await
        .expect("served");
    });

    let mut stream = tokio::net::UnixStream::connect(&path)
        .await
        .expect("connected");
    stream
        .write_all(b"GET /api/tasks HTTP/1.1\r\nHost: nas\r\nConnection: close\r\n\r\n")
        .await
        .expect("sent");
    let mut answer = String::new();
    stream.read_to_string(&mut answer).await.expect("read");

    assert!(answer.starts_with("HTTP/1.1 403"), "{answer}");
    assert!(answer.contains("only an application of the NAS"));
    std::fs::remove_dir_all(&directory).expect("removed");
}
