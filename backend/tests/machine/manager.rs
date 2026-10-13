//! The application manager: what it says of an application, its icon, and installing, starting
//! and removing it.

use crate::support::{package, Machine, Release};
use nas_os_axum::{
    apps::services::nas_app_manager as manager,
    services::{database, manifest, packages},
};

#[tokio::test]
async fn describes_serves_the_icon_of_and_removes_an_application() {
    let machine = Machine::get();
    let db = database::open_in_memory().await.expect("database");
    machine.publish(
        "tests/iconic",
        Release {
            files: vec![
                (
                    "nas-app.json",
                    r#"{"manifest_version":1,"app":{"id":"iconic","name":"Iconic","icon":"icon.svg"}}"#
                        .to_owned(),
                ),
                ("icon.svg", "<svg/>".to_owned()),
            ],
            ..package("iconic", "1.0.0", &[])
        },
    );
    packages::install_for_setup(&db, "iconic")
        .await
        .expect("installed");

    let detail = manager::detail(&db, "iconic").await.expect("detail");
    assert_eq!(detail.name, "iconic");
    assert_eq!(detail.version, "1.0.0");
    assert_eq!(detail.url_repo, "https://github.com/tests/iconic");
    assert!(manager::detail(&db, "nas-app-manager").await.is_ok());
    assert!(manager::detail(&db, "labprobe").await.is_ok());
    assert!(manager::detail(&db, "nowhere").await.is_err());

    let (mime, bytes) = manager::icon("iconic").await.expect("icon");
    assert_eq!(mime, "image/svg+xml");
    assert_eq!(bytes, b"<svg/>");
    assert!(manager::icon("nowhere").await.is_err());

    let socket = manifest::socket_of("iconic");
    let _ = std::fs::remove_file(&socket);
    let listener = tokio::net::UnixListener::bind(&socket).expect("socket");
    manager::control(&db, "iconic".to_owned(), "start")
        .await
        .expect("started");
    manager::control(&db, "iconic".to_owned(), "stop")
        .await
        .expect("stopped");
    drop(listener);
    for (id, action) in [
        ("iconic", "dance"),
        ("nas-app-manager", "stop"),
        ("nowhere", "start"),
    ] {
        assert!(
            manager::control(&db, id.to_owned(), action).await.is_err(),
            "{id} {action}"
        );
    }
    assert!(manager::install(&db, "nas-app-manager".to_owned())
        .await
        .is_err());

    manager::uninstall(&db, "iconic".to_owned(), true)
        .await
        .expect("removed");
    assert!(manager::uninstall(&db, "iconic".to_owned(), false)
        .await
        .is_err());
    assert!(manager::uninstall(&db, "nas-app-manager".to_owned(), false)
        .await
        .is_err());
    assert!(!machine.root.join("apps/iconic").exists());
    assert!(!machine
        .calls("systemctl clean --what=state nas-app@iconic.service")
        .is_empty());
}

#[tokio::test]
async fn serves_only_an_icon_of_a_known_format_inside_its_package() {
    let machine = Machine::get();
    for (id, icon) in [
        ("badicon", "icon.bmp"),
        ("escaped", "../mailer/nas-app.json"),
        ("bare", ""),
    ] {
        let folder = machine.root.join("apps").join(id);
        std::fs::create_dir_all(&folder).expect("package");
        std::fs::write(
            folder.join("nas-app.json"),
            format!(
                r#"{{"manifest_version":1,"app":{{"id":"{id}","name":"{id}","icon":"{icon}"}}}}"#
            ),
        )
        .expect("manifest");
        std::fs::write(folder.join("icon.bmp"), "bitmap").expect("icon");
        assert!(manager::icon(id).await.is_err(), "{id}");
    }
}

#[tokio::test]
async fn reads_only_a_manifest_it_understands_and_prepares_the_sockets() {
    let machine = Machine::get();
    for (id, text) in [
        ("unreadable", "{not json".to_owned()),
        (
            "future",
            r#"{"manifest_version":2,"app":{"id":"future","name":"Future"}}"#.to_owned(),
        ),
        (
            "impostor",
            r#"{"manifest_version":1,"app":{"id":"someone","name":"Else"}}"#.to_owned(),
        ),
    ] {
        let folder = machine.root.join("apps").join(id);
        std::fs::create_dir_all(&folder).expect("package");
        std::fs::write(folder.join("nas-app.json"), text).expect("manifest");
        assert!(manifest::read(id).await.is_none(), "{id}");
    }
    assert!(manifest::read("nothing-here").await.is_none());

    manifest::ensure_sockets_directory().await;
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(manifest::sockets_directory())
        .expect("sockets")
        .permissions()
        .mode();
    assert_eq!(mode & 0o7777, 0o3770);
    manifest::ensure_sockets_directory().await;
}
