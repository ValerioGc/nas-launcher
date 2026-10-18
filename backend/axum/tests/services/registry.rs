use super::*;

#[test]
fn every_system_application_carries_the_fields_the_shell_needs() {
    for app in SYSTEM {
        assert!(!app.id.is_empty(), "the shell opens the application by id");
        assert!(!app.name.is_empty());
        assert!(!app.info.is_empty());
        assert!(
            app.version().split('.').count() == 3,
            "{} has no version",
            app.id
        );
    }
}

#[test]
fn identifiers_are_unique() {
    let mut ids: Vec<&str> = SYSTEM.iter().map(|app| app.id).collect();
    let total = ids.len();
    ids.sort_unstable();
    ids.dedup();

    assert_eq!(ids.len(), total, "two applications share the same id");
}

#[test]
fn only_the_shell_applications_are_part_of_the_system() {
    assert!(is_system("nas-system-settings"));
    assert!(!is_system("mediaServer"));
    assert!(!is_system("not-an-application"));
}

fn explorer(version: &str) -> Installed {
    Installed {
        id: "fileExplorer".to_owned(),
        name: "Explorer".to_owned(),
        info: "Files".to_owned(),
        version: version.to_owned(),
    }
}

#[tokio::test]
async fn an_installation_is_recorded_once_and_updated_in_place() {
    let db = crate::services::database::open_in_memory()
        .await
        .expect("in-memory database");

    record(&db, &explorer("1.0.0")).await.expect("recorded");
    record(&db, &explorer("1.1.0")).await.expect("updated");

    assert_eq!(installed(&db).await.expect("read"), vec![explorer("1.1.0")]);
}

#[tokio::test]
async fn a_forgotten_application_leaves_the_register() {
    let db = crate::services::database::open_in_memory()
        .await
        .expect("in-memory database");
    record(&db, &explorer("1.0.0")).await.expect("recorded");

    forget(&db, "fileExplorer").await.expect("removed");

    assert!(installed(&db).await.expect("read").is_empty());
}

#[tokio::test]
async fn an_application_says_whether_it_works_without_its_window() {
    let db = crate::services::database::open_in_memory()
        .await
        .expect("in-memory database");
    assert!(!set_background(&db, "fileExplorer", true)
        .await
        .expect("written"));
    record(&db, &explorer("1.0.0")).await.expect("recorded");
    assert!(in_background(&db).await.expect("read").is_empty());

    assert!(set_background(&db, "fileExplorer", true)
        .await
        .expect("written"));
    record(&db, &explorer("1.1.0")).await.expect("updated");
    assert_eq!(
        in_background(&db).await.expect("read"),
        vec!["fileExplorer"]
    );

    set_background(&db, "fileExplorer", false)
        .await
        .expect("written");
    assert!(in_background(&db).await.expect("read").is_empty());
}
