use super::*;
use crate::services::database;

fn configuration() -> FirstConfiguration {
    FirstConfiguration {
        language: Language::En,
        theme: Theme::Dark,
        nas_name: None,
    }
}

#[tokio::test]
async fn the_wizard_keeps_the_choices_and_closes_the_configuration() {
    let db = database::open_in_memory().await.expect("database");

    assert!(!state(&db).await.expect("state").completed);
    let done = complete(&db, configuration()).await.expect("complete");
    assert!(done.completed);
    assert!(done.steps.iter().all(|step| step.kind == "app"));

    assert!(state(&db).await.expect("state").completed);
    let general: General = settings::read(&db, Section::General)
        .await
        .expect("general");
    assert_eq!(general.language, Language::En);
    assert_eq!(general.theme, Theme::Dark);
}

#[tokio::test]
async fn once_configured_nothing_can_run_the_wizard_again() {
    let db = database::open_in_memory().await.expect("database");
    complete(&db, configuration()).await.expect("first");

    let mut again = configuration();
    again.language = Language::It;
    let refused = complete(&db, again).await.expect_err("refused");

    assert_eq!(refused.0, StatusCode::CONFLICT);
    assert!(still_open(&db).await.is_err());
    let general: General = settings::read(&db, Section::General)
        .await
        .expect("general");
    assert_eq!(general.language, Language::En);

    let told: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM notifications WHERE key = 'discoveryOff'")
            .fetch_one(&db)
            .await
            .expect("notifications");
    assert_eq!(told.0, 1);
}

#[tokio::test]
async fn the_name_chosen_in_the_wizard_becomes_the_name_of_the_nas() {
    let db = database::open_in_memory().await.expect("database");
    let mut named = configuration();
    named.nas_name = Some(" attic ".to_owned());

    complete(&db, named).await.expect("complete");

    let general: General = settings::read(&db, Section::General)
        .await
        .expect("general");
    assert_eq!(general.nas_name, "attic");
}

#[tokio::test]
async fn a_name_that_cannot_be_a_hostname_leaves_the_configuration_open() {
    let db = database::open_in_memory().await.expect("database");
    for name in ["-attic", "the attic", ""] {
        let mut named = configuration();
        named.nas_name = Some(name.to_owned());

        let refused = complete(&db, named).await.expect_err("refused");

        assert_eq!(refused.0, StatusCode::BAD_REQUEST, "{name}");
    }
    assert!(still_open(&db).await.is_ok());
}

#[test]
fn reads_the_security_the_wizard_found() {
    assert_eq!(
        parse_security("system=plain\ntpm=present\n"),
        SystemSecurity {
            system_encrypted: Some(false),
            tpm: Some(true),
        }
    );
    let protected = parse_security("system=encrypted\ntpm=present\n");
    assert!(!protected.unprotected());
    assert!(parse_security("system=encrypted\ntpm=missing\n").unprotected());
    assert_eq!(
        parse_security("garbage\nsystem=maybe"),
        SystemSecurity::default()
    );
    assert!(!SystemSecurity::default().unprotected());
}

#[tokio::test]
async fn a_system_without_protection_is_reported_once() {
    let db = database::open_in_memory().await.expect("database");
    let directory = std::env::temp_dir().join(format!("nas-setup-mark-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("directory");
    let state = directory.join("security");
    let warnings = || async {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM notifications WHERE key = 'systemUnprotected'",
        )
        .fetch_one(&db)
        .await
        .expect("notifications")
    };

    report_security_state(&db, &state).await;
    std::fs::write(&state, "system=encrypted\ntpm=present\n").expect("state");
    report_security_state(&db, &state).await;
    assert_eq!(warnings().await, 0);

    std::fs::write(&state, "system=plain\ntpm=missing\n").expect("state");
    report_security_state(&db, &state).await;
    report_security_state(&db, &state).await;
    assert_eq!(warnings().await, 1);

    sqlx::query("UPDATE notifications SET read = 1")
        .execute(&db)
        .await
        .expect("read");
    report_security_state(&db, &state).await;
    assert_eq!(warnings().await, 1);
    std::fs::remove_dir_all(&directory).expect("removed");
}
