use super::*;
use crate::services::database;
use std::path::PathBuf;

fn on_disk(id: &str, name: &str, version: &str) -> Manifest {
    Manifest {
        id: id.to_owned(),
        name: name.to_owned(),
        version: version.to_owned(),
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
        socket: PathBuf::new(),
        exec: String::new(),
    }
}

fn registered(id: &str, name: &str, version: &str) -> App {
    App {
        id: id.to_owned(),
        name: name.to_owned(),
        info: "from the catalogue".to_owned(),
        system: false,
        version: version.to_owned(),
        running: false,
        icon: String::new(),
        category: None,
    }
}

fn offered(id: &str) -> CatalogueApp {
    CatalogueApp {
        id: id.to_owned(),
        name: "Media server".to_owned(),
        info: "Serves what is on the disks.".to_owned(),
        provider: catalogue::Provider::Github,
        repository: "example/media-server".to_owned(),
        asset: "media-{version}.tar.gz".to_owned(),
        private: false,
        system: false,
        preinstall: false,
        logo: String::new(),
        category: None,
    }
}

#[test]
fn an_external_system_app_keeps_its_category_and_external_lifecycle() {
    let mut manifest = on_disk("disks", "Disks", "1.0.0");
    manifest.system = true;
    manifest.category = Some(Category::Storage);
    let mut apps = merge(Vec::new(), vec![manifest], &[]);
    complete(&mut apps, &[offered("disks")], &HashMap::new());
    assert!(apps[0].system);
    assert!(!registry::is_system(&apps[0].id));
    let mut entry = offered("logs");
    entry.system = true;
    let mut apps = merge(vec![registered("logs", "Logs", "1.0.0")], Vec::new(), &[]);
    complete(&mut apps, &[entry], &HashMap::new());
    assert!(apps[0].system);
}

#[test]
fn a_package_copied_in_by_hand_appears_without_a_row() {
    let apps = merge(
        Vec::new(),
        vec![on_disk("mediaServer", "Media", "2.0.0")],
        &[],
    );

    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].id, "mediaServer");
    assert_eq!(apps[0].name, "Media");
    assert_eq!(apps[0].version, "2.0.0");
    assert!(!apps[0].system);
}

#[test]
fn the_catalogue_describes_what_the_register_does_not_know() {
    let apps = merge(
        Vec::new(),
        vec![on_disk("mediaServer", "Media", "2.0.0")],
        &[offered("mediaServer")],
    );

    assert_eq!(apps[0].name, "Media server");
    assert_eq!(apps[0].info, "Serves what is on the disks.");
}

#[test]
fn the_version_on_disk_wins_over_the_one_the_register_remembers() {
    let apps = merge(
        vec![registered("mediaServer", "Media server", "1.0.0")],
        vec![on_disk("mediaServer", "Media", "2.0.0")],
        &[],
    );

    assert_eq!(apps.len(), 1, "one application, not two");
    assert_eq!(apps[0].version, "2.0.0");
    assert_eq!(apps[0].name, "Media server");
    assert_eq!(apps[0].info, "from the catalogue");
}

#[test]
fn a_row_whose_package_is_gone_stays_so_it_can_still_be_removed() {
    let apps = merge(
        vec![registered("mediaServer", "Media server", "1.0.0")],
        Vec::new(),
        &[],
    );

    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].version, "1.0.0");
}

#[test]
fn a_manifest_without_a_version_does_not_erase_the_one_that_was_recorded() {
    let apps = merge(
        vec![registered("mediaServer", "Media server", "1.0.0")],
        vec![on_disk("mediaServer", "Media", "")],
        &[],
    );

    assert_eq!(apps[0].version, "1.0.0");
}

#[test]
fn an_installed_application_takes_its_folder_and_its_missing_icon_from_the_catalogue() {
    let mut apps = vec![
        registered("photos", "Photos", "1.0.0"),
        App {
            icon: "/api/apps/disks/icon".to_owned(),
            ..registered("disks", "Disks", "1.0.0")
        },
    ];
    let catalogue = vec![CatalogueApp {
        category: Some(Category::Media),
        ..offered("photos")
    }];
    let logos = HashMap::from([
        ("photos".to_owned(), "/api/apps/photos/logo".to_owned()),
        ("disks".to_owned(), "/api/apps/disks/logo".to_owned()),
    ]);

    complete(&mut apps, &catalogue, &logos);

    assert_eq!(apps[0].category, Some(Category::Media));
    assert_eq!(apps[0].icon, "/api/apps/photos/logo");
    assert_eq!(apps[1].category, None);
    assert_eq!(apps[1].icon, "/api/apps/disks/icon");
}

#[tokio::test]
async fn bundled_manifests_are_recorded_and_reruns_keep_one_row() {
    let db = crate::services::database::open_in_memory()
        .await
        .expect("database");
    let manifest = on_disk("mediaServer", "Media", "2.0.0");
    sync_manifests(&db, vec![manifest.clone()], &[offered("mediaServer")])
        .await
        .expect("first sync");
    sync_manifests(&db, vec![manifest], &[offered("mediaServer")])
        .await
        .expect("second sync");
    let rows = registry::installed(&db).await.expect("registered apps");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].version, "2.0.0");
    assert_eq!(rows[0].name, "Media server");
}

#[tokio::test]
async fn a_package_file_is_served_only_from_inside_the_package() {
    let root = std::env::temp_dir().join(format!("nas-os-icon-{}", std::process::id()));
    let package = root.join("mediaServer");
    tokio::fs::create_dir_all(package.join("assets"))
        .await
        .unwrap();
    tokio::fs::write(package.join("assets/icon.svg"), "<svg/>")
        .await
        .unwrap();
    tokio::fs::write(root.join("launcher.db"), "secret")
        .await
        .unwrap();
    std::os::unix::fs::symlink("../launcher.db", package.join("icon.png")).unwrap();

    let found = inside(&package, std::path::Path::new("assets/icon.svg")).await;
    assert!(found.is_some_and(|file| file.is_ok()));
    for escape in ["icon.png", "../launcher.db", "/etc/hostname"] {
        let refused = inside(&package, std::path::Path::new(escape)).await;
        assert!(refused.is_some_and(|file| file.is_err()), "{escape}");
    }
    assert!(inside(&package, std::path::Path::new("missing.svg"))
        .await
        .is_none());

    tokio::fs::remove_dir_all(&root).await.unwrap();
}

#[test]
fn the_first_automatic_check_runs_at_once_then_once_per_period() {
    let start = Instant::now();

    assert!(due(None, start, Frequency::Weekly));
    assert!(!due(
        Some(start),
        start + Duration::from_secs(59 * 60),
        Frequency::Hourly
    ));
    assert!(due(
        Some(start),
        start + Frequency::Hourly.period(),
        Frequency::Hourly
    ));
    assert!(Frequency::Hourly.period() < Frequency::Daily.period());
    assert!(Frequency::Daily.period() < Frequency::Weekly.period());
}

fn offer(id: &str, latest: Option<&str>, update: bool) -> Offer {
    Offer {
        system: false,
        id: id.to_owned(),
        name: id.to_owned(),
        info: String::new(),
        repository: format!("example/{id}"),
        logo: String::new(),
        installed_version: Some("1.0.0".to_owned()),
        latest_version: latest.map(str::to_owned),
        update_available: update,
    }
}

#[test]
fn only_releases_that_are_updates_count_as_news() {
    let found = [
        offer("photos", Some("1.2.0"), true),
        offer("disks", Some("1.0.0"), false),
        offer("music", None, false),
    ];

    assert_eq!(
        newer_releases(&found),
        BTreeSet::from(["photos@1.2.0".to_owned()])
    );
}

#[tokio::test]
async fn the_automatic_check_is_off_until_the_settings_turn_it_on() {
    let db = database::open_in_memory().await.expect("database");
    assert_eq!(
        load_settings(&db).await.expect("defaults"),
        ManagerSettings::default()
    );

    assert!(!ManagerSettings::default().auto_check);

    let wanted = ManagerSettings {
        auto_check: true,
        frequency: Frequency::Weekly,
        check_on_open: true,
        update_on_open: true,
    };
    save_settings(&db, wanted).await.expect("saved");

    assert_eq!(load_settings(&db).await.expect("read"), wanted);
}

#[tokio::test]
async fn updates_on_opening_need_the_check_on_opening() {
    let db = database::open_in_memory().await.expect("database");
    let refused = save_settings(
        &db,
        ManagerSettings {
            update_on_open: true,
            ..ManagerSettings::default()
        },
    )
    .await;

    assert!(refused.is_err());
    assert_eq!(
        load_settings(&db).await.expect("defaults"),
        ManagerSettings::default()
    );
}

#[test]
fn settings_saved_before_the_opening_options_still_read() {
    let stored: ManagerSettings =
        serde_json::from_str(r#"{"autoCheck":true,"frequency":"hourly"}"#).expect("stored");

    assert!(stored.auto_check);
    assert!(!stored.check_on_open && !stored.update_on_open);
}

#[tokio::test]
async fn an_application_is_removed_with_or_without_its_data() {
    let db = database::open_in_memory().await.expect("database");
    for remove_data in [true, false] {
        let row = registry::Installed {
            id: "removedInTest".to_owned(),
            name: "Removed".to_owned(),
            info: String::new(),
            version: "1.0.0".to_owned(),
        };
        registry::record(&db, &row).await.expect("recorded");

        let ack = uninstall(&db, row.id, remove_data).await.expect("removed");

        assert!(ack.accepted);
        assert!(registry::installed(&db).await.expect("rows").is_empty());
    }
}

#[test]
fn the_notice_of_app_updates_names_only_the_apps_that_have_one() {
    assert!(packages::updates_notice(&[offer("disks", Some("1.0.0"), false)]).is_none());

    let notice = packages::updates_notice(&[
        offer("photos", Some("1.2.0"), true),
        offer("disks", Some("1.0.0"), false),
        offer("music", Some("2.0.0"), true),
    ])
    .expect("a notice");
    assert_eq!(notice.key, "appUpdatesAvailable");
    assert_eq!(notice.params["count"], "2");
    assert_eq!(notice.params["apps"], "photos, music");
}

#[tokio::test]
async fn closing_a_session_stops_only_what_is_running() {
    let db = database::open_in_memory().await.expect("database");
    let row = registry::Installed {
        id: "idleInTest".to_owned(),
        name: "Idle".to_owned(),
        info: String::new(),
        version: "1.0.0".to_owned(),
    };
    registry::record(&db, &row).await.expect("recorded");

    assert!(close_all(&db).await.expect("closed").is_empty());
}

#[test]
fn a_manifest_supplies_the_default_category_and_the_catalogue_can_override_it() {
    let mut manifest = on_disk("explorer", "Explorer", "1.0.0");
    manifest.category = Some(Category::Storage);
    let rows = vec![registered("explorer", "Explorer", "1.0.0")];
    for rows in [Vec::new(), rows] {
        let mut apps = merge(rows, vec![manifest.clone()], &[]);
        complete(&mut apps, &[], &HashMap::new());
        assert_eq!(apps[0].category, Some(Category::Storage));
        assert!(!apps[0].system);
        complete(
            &mut apps,
            &[CatalogueApp {
                category: Some(Category::System),
                ..offered("explorer")
            }],
            &HashMap::new(),
        );
        assert_eq!(apps[0].category, Some(Category::System));
        assert!(!apps[0].system);
        complete(&mut apps, &[offered("explorer")], &HashMap::new());
        assert_eq!(apps[0].category, None);
    }
}
