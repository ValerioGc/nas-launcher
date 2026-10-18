use super::*;
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
async fn setup_creates_one_shortcut_even_when_interactive_install_shortcuts_are_disabled() {
    let db = database::open_in_memory().await.expect("database");
    registry::record(
        &db,
        &registry::Installed {
            id: "setup-probe".to_owned(),
            name: "Probe".to_owned(),
            info: String::new(),
            version: "1.0.0".to_owned(),
        },
    )
    .await
    .expect("installed");
    let general = crate::apps::services::nas_system_settings::General {
        create_shortcut: false,
        ..Default::default()
    };
    crate::services::settings::write(&db, crate::services::settings::Section::General, &general)
        .await
        .expect("shortcuts disabled");
    assert!(!desktop::shortcut_on_install(&db).await);
    InstallContext::Interactive
        .add_shortcut(&db, "setup-probe")
        .await
        .expect("interactive");
    assert!(desktop::list(&db)
        .await
        .expect("desktop")
        .iter()
        .all(|item| item.target != "setup-probe"));
    for _ in 0..2 {
        InstallContext::Setup
            .add_shortcut(&db, "setup-probe")
            .await
            .expect("setup");
    }
    assert_eq!(
        desktop::list(&db)
            .await
            .expect("desktop")
            .iter()
            .filter(|item| item.target == "setup-probe")
            .count(),
        1
    );
}

fn mode(path: &std::path::Path) -> u32 {
    std::fs::metadata(path)
        .expect("unpacked file")
        .permissions()
        .mode()
        & 0o777
}

#[tokio::test]
async fn an_unpacked_package_can_be_read_and_run_by_the_group_of_the_launcher() {
    let root = std::env::temp_dir().join(format!("nas-os-unpack-{}", std::process::id()));
    let source = root.join("source");
    std::fs::create_dir_all(source.join("bin")).expect("source directory");
    std::fs::write(source.join("nas-app.json"), "{}").expect("manifest");
    std::fs::write(source.join("bin/app"), "#!/bin/sh\n").expect("program");
    std::fs::set_permissions(
        source.join("nas-app.json"),
        PermissionsExt::from_mode(0o600),
    )
    .expect("manifest mode");
    std::fs::set_permissions(source.join("bin/app"), PermissionsExt::from_mode(0o700))
        .expect("program mode");
    std::fs::set_permissions(source.join("bin"), PermissionsExt::from_mode(0o700))
        .expect("directory mode");
    let archive = root.join("package.tar.gz");
    let packed = std::process::Command::new("tar")
        .arg("--create")
        .arg("--gzip")
        .arg("--file")
        .arg(&archive)
        .arg("--directory")
        .arg(&source)
        .arg(".")
        .status()
        .expect("tar");
    assert!(packed.success());

    let target = root.join("target");
    unpack(&archive, &target).await.expect("unpacked");

    assert_eq!(mode(&target.join("nas-app.json")) & 0o070, 0o040);
    assert_eq!(mode(&target.join("bin/app")) & 0o070, 0o050);
    assert_eq!(mode(&target.join("bin")) & 0o070, 0o050);
    std::fs::remove_dir_all(&root).expect("cleanup");
}

#[test]
fn a_missing_asset_names_the_repository_its_release_and_what_it_publishes() {
    let app = CatalogueApp {
        id: "diskManager".to_owned(),
        name: "Disk Manager".to_owned(),
        info: String::new(),
        provider: catalogue::Provider::Github,
        repository: "example/disk-manager".to_owned(),
        asset: "diskManager-{version}-linux-x64.tar.gz".to_owned(),
        private: false,
        system: false,
        preinstall: false,
        logo: String::new(),
        category: None,
    };
    let release = forge::Release {
        version: "1.4.0".to_owned(),
        assets: vec![forge::Asset {
            name: "disk-manager-1.4.0.tar.gz".to_owned(),
            url: "https://example.com/a".to_owned(),
        }],
    };

    assert_eq!(
        missing_asset(&app, &release, "diskManager-1.4.0-linux-x64.tar.gz"),
        "the latest release of example/disk-manager, 1.4.0, has no asset \
         diskManager-1.4.0-linux-x64.tar.gz; it publishes disk-manager-1.4.0.tar.gz"
    );
    let empty = forge::Release {
        assets: Vec::new(),
        ..release
    };
    assert!(missing_asset(&app, &empty, "x").ends_with("it publishes none"));
}

#[tokio::test]
async fn setup_installations_keep_their_outcome_without_notifications() {
    use crate::services::database;

    for context in [InstallContext::Setup, InstallContext::Interactive] {
        for outcome in [Ok("1.2.0".to_owned()), Err("download failed".to_owned())] {
            let db = database::open_in_memory().await.expect("database");
            let mut catalogue: catalogue::Catalogue =
                serde_json::from_str(include_str!("../fixtures/catalogue.json"))
                    .expect("catalogue fixture");
            let app = catalogue.apps.remove(0);
            let task = format!("install-{}", app.id);
            tasks::register(
                &db,
                NewTask {
                    id: task.clone(),
                    source: "nas-app-manager".to_owned(),
                    name: app.name.clone(),
                    detail: String::new(),
                    progress: Some(0),
                    pausable: false,
                    cancellable: false,
                },
            )
            .await
            .expect("registered");
            let succeeded = outcome.is_ok();

            finish(&db, &task, &app, outcome, context).await;

            let saved = tasks::get(&db, &task).await.expect("task");
            assert_eq!(
                saved.state,
                if succeeded {
                    TaskState::Done
                } else {
                    TaskState::Failed
                }
            );
            assert_eq!(
                saved.detail,
                if succeeded {
                    "version 1.2.0"
                } else {
                    "download failed"
                }
            );
            let keys: Vec<(String,)> = sqlx::query_as("SELECT key FROM notifications")
                .fetch_all(&db)
                .await
                .expect("notifications");
            let expected = if context.notify() {
                vec![(if succeeded {
                    "appInstalled"
                } else {
                    "taskFailed"
                }
                .to_owned(),)]
            } else {
                Vec::new()
            };
            assert_eq!(keys, expected);
        }
    }
}
