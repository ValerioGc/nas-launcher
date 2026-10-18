use super::*;
use crate::{
    apps::services::nas_app_manager,
    apps::services::nas_system_settings::{
        access::RuleKind, load_general, save_general, BackgroundPattern, DesktopIconSize,
        PowerAction, PowerDay, PowerRule,
    },
    services::database::open_in_memory,
    services::terminal,
};
use axum::http::StatusCode;

async fn configured() -> SqlitePool {
    let db = open_in_memory().await.expect("database");
    let mut general = General {
        nas_name: "attic".to_owned(),
        ..General::default()
    };
    general.theme = Theme::Dark;
    general.themes.dark = "forest".to_owned();
    general.background.pattern = BackgroundPattern::Rings;
    general.desktop_icon_size = DesktopIconSize::Small;
    save_general(&db, general).await.expect("general");
    access::add(
        &db,
        access::NewRule {
            kind: RuleKind::Range,
            value: "192.168.1.0/24".to_owned(),
            label: "Home".to_owned(),
        },
    )
    .await
    .expect("rule");
    super::super::save_power(
        &db,
        Power {
            enabled: true,
            screen_off_minutes: 5,
            rules: vec![PowerRule {
                day: PowerDay::Weekdays,
                time: "23:30".to_owned(),
                action: PowerAction::Shutdown,
                active: true,
            }],
        },
    )
    .await
    .expect("power");
    db
}

#[tokio::test]
async fn the_export_carries_the_settings_but_not_the_name_of_the_nas() {
    let db = configured().await;

    let config = export(&db, &[]).await.expect("export");
    let text = serde_json::to_string(&config).expect("json");

    assert_eq!(config.format, FORMAT);
    assert_eq!(config.version, VERSION);
    assert!(matches!(config.general.theme, Theme::Dark));
    assert_eq!(config.access_rules.len(), 1);
    assert_eq!(config.power.rules.len(), 1);
    assert!(!text.contains("attic"), "{text}");
    assert!(!text.contains("nasName"), "{text}");
}

#[tokio::test]
async fn an_import_replaces_the_settings_and_keeps_the_name_of_the_nas() {
    let source = configured().await;
    let config = export(&source, &[]).await.expect("export");
    let db = open_in_memory().await.expect("database");
    save_general(
        &db,
        General {
            nas_name: "cellar".to_owned(),
            ..General::default()
        },
    )
    .await
    .expect("general");
    access::add(
        &db,
        access::NewRule {
            kind: RuleKind::Ip,
            value: "10.0.0.9".to_owned(),
            label: String::new(),
        },
    )
    .await
    .expect("rule");

    import(&db, config).await.expect("import");

    let general = load_general(&db).await.expect("general");
    assert_eq!(general.nas_name, "cellar");
    assert!(matches!(general.theme, Theme::Dark));
    assert_eq!(general.themes.dark, "forest");
    assert_eq!(general.themes.light, "default");
    assert_eq!(general.desktop_icon_size, DesktopIconSize::Small);
    let rules = access::list(&db).await.expect("rules");
    assert_eq!(
        rules
            .iter()
            .map(|rule| rule.value.as_str())
            .collect::<Vec<_>>(),
        ["192.168.1.0/24"]
    );
    assert_eq!(
        super::super::load_power(&db)
            .await
            .expect("power")
            .rules
            .len(),
        1
    );
}

#[tokio::test]
async fn a_theme_is_named_like_its_file_and_an_older_save_takes_the_defaults() {
    let db = open_in_memory().await.expect("database");
    for id in ["", "Forest", "../dark", &"a".repeat(33)] {
        let mut general = General::default();
        general.themes.light = id.to_owned();
        assert!(save_general(&db, general).await.is_err(), "{id}");
    }

    let older: General = serde_json::from_value(serde_json::json!({
        "theme": "light",
        "language": "it",
        "createShortcut": false,
        "background": serde_json::to_value(General::default().background).expect("background"),
    }))
    .expect("an older save");
    assert_eq!(older.themes, Themes::default());
    assert_eq!(older.desktop_icon_size, DesktopIconSize::Large);
}

#[tokio::test]
async fn a_refused_file_changes_nothing() {
    let source = configured().await;
    let valid = export(&source, &[]).await.expect("export");
    let db = configured().await;
    let before = serde_json::to_string(&export(&db, &[]).await.expect("export")).expect("json");

    let mut foreign = valid.clone();
    foreign.format = "something else".to_owned();
    let mut newer = valid.clone();
    newer.version = VERSION + 1;
    let mut bad_rule = valid.clone();
    bad_rule.access_rules.push(access::NewRule {
        kind: RuleKind::Ip,
        value: "not an address".to_owned(),
        label: String::new(),
    });
    let mut bad_power = valid;
    bad_power.power.rules[0].time = "25:00".to_owned();

    for config in [foreign, newer, bad_rule, bad_power] {
        let refused = import(&db, config).await.err().expect("refused");
        assert_eq!(refused.0, StatusCode::BAD_REQUEST);
    }
    let after = serde_json::to_string(&export(&db, &[]).await.expect("export")).expect("json");
    let without_time = |text: &str| text.split("\"general\"").nth(1).map(str::to_owned);
    assert_eq!(without_time(&before), without_time(&after));
}

fn ids(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

async fn desktop_of(db: &SqlitePool) -> Vec<(String, Option<String>)> {
    desktop::list(db)
        .await
        .expect("desktop")
        .into_iter()
        .map(|shortcut| (shortcut.target, shortcut.name))
        .collect()
}

#[tokio::test]
async fn the_export_carries_only_the_applications_chosen() {
    let db = configured().await;

    let none = export(&db, &[]).await.expect("export");
    let text = serde_json::to_string(&none).expect("json");
    assert!(!text.contains("nas-system-terminal\":"), "{text}");
    assert!(none.apps.terminal.is_none() && none.apps.app_manager.is_none());

    let chosen = export(&db, &ids(&["nas-system-terminal", "nas-app-manager"]))
        .await
        .expect("export");
    assert!(chosen.apps.terminal.is_some());
    assert!(chosen.apps.notifications.is_none());
    let manager = chosen.apps.app_manager.expect("app manager");
    assert!(
        manager.catalogue.is_none(),
        "the catalogue of the installation"
    );
    assert!(manager
        .installed
        .iter()
        .all(|app| !registry::is_system(&app.id)));

    let refused = export(&db, &ids(&["nas-backup-manager"]))
        .await
        .err()
        .expect("refused");
    assert_eq!(refused.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn the_settings_of_the_applications_and_the_desktop_travel_with_the_file() {
    let source = configured().await;
    let terminal = terminal::TerminalSettings {
        font_size: 18,
        ..Default::default()
    };
    terminal::save_settings(&source, terminal.clone())
        .await
        .expect("terminal");
    let manager = nas_app_manager::ManagerSettings {
        auto_check: true,
        ..Default::default()
    };
    nas_app_manager::save_settings(&source, manager)
        .await
        .expect("manager");
    sqlx::query(
        "INSERT INTO desktop_shortcuts (user_name, kind, target, name, position, created_at)
         VALUES ('local', 'folder', '/Photos', 'Photos', -1, 0)",
    )
    .execute(&source)
    .await
    .expect("folder");
    let config = export(
        &source,
        &ids(&[
            "nas-system-terminal",
            "nas-app-manager",
            "nas-notification-manager",
        ]),
    )
    .await
    .expect("export");

    let db = open_in_memory().await.expect("database");
    import(&db, config).await.expect("import");

    assert_eq!(
        terminal::load_settings(&db).await.expect("terminal"),
        terminal
    );
    assert_eq!(
        nas_app_manager::load_settings(&db).await.expect("manager"),
        manager
    );
    assert_eq!(desktop_of(&db).await, desktop_of(&source).await);
    assert_eq!(
        desktop_of(&db).await.first(),
        Some(&("/Photos".to_owned(), Some("Photos".to_owned())))
    );
}

#[tokio::test]
async fn a_file_of_the_first_version_leaves_the_desktop_and_the_applications_alone() {
    let source = configured().await;
    let mut config = export(&source, &[]).await.expect("export");
    config.version = 1;
    config.desktop = None;
    let db = open_in_memory().await.expect("database");
    sqlx::query("DELETE FROM desktop_shortcuts WHERE target = 'nas-system-terminal'")
        .execute(&db)
        .await
        .expect("shortcut removed");
    let before = desktop_of(&db).await;

    import(&db, config).await.expect("import");

    assert_eq!(desktop_of(&db).await, before);
}

#[tokio::test]
async fn a_file_with_wrong_application_settings_changes_nothing() {
    let source = configured().await;
    let valid = export(&source, &ids(&["nas-system-terminal", "nas-app-manager"]))
        .await
        .expect("export");
    let db = open_in_memory().await.expect("database");

    let mut terminal = valid.clone();
    if let Some(settings) = terminal.apps.terminal.as_mut() {
        settings.font_size = 200;
    }
    let mut system = valid.clone();
    if let Some(manager) = system.apps.app_manager.as_mut() {
        manager.installed.push(InstalledApp {
            id: "nas-system-settings".to_owned(),
            version: String::new(),
        });
    }
    let mut update = valid.clone();
    if let Some(manager) = update.apps.app_manager.as_mut() {
        manager.settings.update_on_open = true;
        manager.settings.check_on_open = false;
    }
    let mut folder = valid;
    folder.desktop = Some(vec![DesktopEntry {
        kind: ShortcutKind::Folder,
        target: "../etc".to_owned(),
        name: Some("etc".to_owned()),
    }]);

    for config in [terminal, system, update, folder] {
        let refused = import(&db, config).await.err().expect("refused");
        assert_eq!(refused.0, StatusCode::BAD_REQUEST);
    }
    assert_eq!(
        terminal::load_settings(&db).await.expect("terminal"),
        terminal::TerminalSettings::default()
    );
}

fn entry(id: &str, preinstall: bool) -> CatalogueApp {
    serde_json::from_value(serde_json::json!({
        "id": id,
        "name": id,
        "provider": "github",
        "repository": format!("owner/{id}"),
        "asset": format!("{id}.tar.gz"),
        "preinstall": preinstall,
    }))
    .expect("entry")
}

#[test]
fn the_import_installs_and_removes_to_match_the_file_but_keeps_what_the_nas_needs() {
    let wanted: Vec<InstalledApp> = ["nas-file-manager", "nas-photos", "nas-unknown"]
        .iter()
        .map(|id| InstalledApp {
            id: (*id).to_owned(),
            version: String::new(),
        })
        .collect();
    let entries = [
        entry("nas-file-manager", false),
        entry("nas-photos", false),
        entry("nas-disk-manager", false),
        entry("nas-security-center", true),
    ];
    let present = ids(&[
        "nas-file-manager",
        "nas-disk-manager",
        "nas-security-center",
    ]);

    assert_eq!(
        plan(&present, &wanted, &entries),
        Plan {
            remove: ids(&["nas-disk-manager"]),
            install: ids(&["nas-photos"]),
            missing: ids(&["nas-unknown"]),
        }
    );
    assert_eq!(plan(&present[..1], &wanted[..1], &entries), Plan::default());
}
