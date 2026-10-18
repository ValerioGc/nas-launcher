use super::*;
use crate::services::database;

fn targets(shortcuts: &[Shortcut]) -> Vec<&str> {
    shortcuts
        .iter()
        .map(|shortcut| shortcut.target.as_str())
        .collect()
}

fn order(ids: &[&str]) -> Order {
    Order {
        order: ids
            .iter()
            .map(|id| ShortcutOrder::App((*id).to_owned()))
            .collect(),
    }
}

#[tokio::test]
async fn shortcut_creation_reads_the_saved_general_settings_and_defaults_to_on() {
    let db = database::open_in_memory().await.expect("database");
    assert!(shortcut_on_install(&db).await);
    settings::write(
        &db,
        Section::General,
        &serde_json::json!({ "theme": "dark" }),
    )
    .await
    .expect("an older save");
    assert!(shortcut_on_install(&db).await);
    let mut general = crate::apps::services::nas_system_settings::General::default();
    for enabled in [true, false] {
        general.create_shortcut = enabled;
        settings::write(&db, Section::General, &general)
            .await
            .expect("settings saved");
        assert_eq!(shortcut_on_install(&db).await, enabled);
    }
}

#[tokio::test]
async fn the_desktop_keeps_the_order_it_is_given() {
    let db = database::open_in_memory().await.expect("database");
    let start = list(&db).await.expect("list");
    assert_eq!(
        targets(&start),
        [
            "nas-app-manager",
            "nas-global-search",
            "nas-notification-manager",
            "nas-system-settings"
        ]
    );

    crate::services::registry::record(
        &db,
        &crate::services::registry::Installed {
            id: "nas-system-terminal".to_owned(),
            name: "Terminal".to_owned(),
            info: String::new(),
            version: "0.1.0".to_owned(),
        },
    )
    .await
    .expect("external terminal installed");
    add_app(&db, "nas-system-terminal")
        .await
        .expect("manual shortcut");
    let moved = reorder(&db, order(&["nas-system-terminal", "nas-app-manager"]))
        .await
        .expect("reorder");

    assert_eq!(
        targets(&moved),
        [
            "nas-system-terminal",
            "nas-app-manager",
            "nas-global-search",
            "nas-notification-manager",
            "nas-system-settings"
        ]
    );
    assert_eq!(targets(&list(&db).await.expect("list")), targets(&moved));
}

#[tokio::test]
async fn an_order_ignores_what_is_not_on_the_desktop_and_names_nothing_twice() {
    let db = database::open_in_memory().await.expect("database");

    let moved = reorder(
        &db,
        order(&[
            "nas-system-settings",
            "photos",
            "nas-system-settings",
            "nas-app-manager",
        ]),
    )
    .await
    .expect("reorder");

    assert_eq!(moved.len(), 4);
    assert_eq!(
        targets(&moved)[..2],
        ["nas-system-settings", "nas-app-manager"]
    );
    let positions: Vec<i64> = moved.iter().map(|shortcut| shortcut.position).collect();
    assert_eq!(positions, [0, 1, 2, 3]);
}

#[tokio::test]
async fn an_order_that_is_not_a_list_of_identifiers_is_refused() {
    let db = database::open_in_memory().await.expect("database");

    assert!(reorder(&db, order(&[""])).await.is_err());
    assert!(reorder(&db, order(&[&"x".repeat(65)])).await.is_err());
}

#[tokio::test]
async fn folder_shortcuts_are_idempotent_and_survive_mixed_reordering() {
    let db = database::open_in_memory().await.expect("database");
    sqlx::query("INSERT INTO installed_apps (id, name, info, version, installed_at) VALUES ('nas-file-manager', 'Explorer', '', '0.1.0', 0)")
        .execute(&db).await.expect("installed explorer");
    let input = || FolderShortcut {
        path: "/Documents/Été".to_owned(),
        name: "Été".to_owned(),
    };
    add_folder(&db, input()).await.expect("add folder");
    let repeated = add_folder(&db, input()).await.expect("repeat add");
    assert_eq!(
        repeated
            .iter()
            .filter(|item| item.kind == ShortcutKind::Folder)
            .count(),
        1
    );
    let moved = reorder(
        &db,
        Order {
            order: vec![ShortcutOrder::Key(ShortcutKey {
                kind: ShortcutKind::Folder,
                target: input().path,
            })],
        },
    )
    .await
    .expect("mixed reorder");
    assert_eq!(moved[0].kind, ShortcutKind::Folder);
    assert_eq!(moved[0].name.as_deref(), Some("Été"));
    let removed = remove(&db, ShortcutKind::Folder, "/Documents/Été")
        .await
        .expect("remove folder shortcut");
    assert!(removed.iter().all(|item| item.kind == ShortcutKind::App));
}

#[tokio::test]
async fn folder_shortcuts_reject_untrusted_paths_and_missing_explorer() {
    let db = database::open_in_memory().await.expect("database");
    let input = FolderShortcut {
        path: "/Documents".to_owned(),
        name: "Documents".to_owned(),
    };
    assert!(add_folder(&db, input).await.is_err());
    assert!(validate_folder_path("/Documents/../private").is_err());
    assert!(validate_folder_path("relative").is_err());
}
