use super::*;

#[test]
fn the_standard_catalogue_keeps_categories_and_declares_initial_installation_separately() {
    let standard: Catalogue = serde_json::from_str(include_str!(
        "../../../../../system/packages/ui_installation/files/catalogue.json"
    ))
    .expect("catalogue");
    for app in standard.apps {
        let optional = matches!(
            app.id.as_str(),
            "nas-backup-manager"
                | "nas-picture-manager"
                | "nas-mail-client"
                | "nas-cleanup-manager"
                | "nas-sync-manager"
        );
        assert_eq!(app.system, !optional, "{} classification", app.id);
        assert_eq!(app.preinstall, !optional, "{} initial installation", app.id);
        let expected = match app.id.as_str() {
            "nas-picture-manager" => Category::Media,
            "nas-backup-manager" | "nas-disk-manager" | "nas-file-manager" | "nas-sync-manager" => {
                Category::Storage
            }
            _ => Category::System,
        };
        assert_eq!(app.category, Some(expected), "{} category", app.id);
    }
}

#[test]
fn an_older_catalogue_still_reads_and_its_token_fields_are_not_handed_on() {
    let json = r#"{"version":1,"credential":"nas-os.launcher-token","apps":[
      {"id":"private","name":"Private","provider":"github","repository":"example/private","asset":"private.tar.gz","private":true}
    ]}"#;
    let catalogue: Catalogue = serde_json::from_str(json).expect("catalogue");

    let written = serde_json::to_value(&catalogue).expect("json");
    assert!(written.get("credential").is_none());
    assert!(written["apps"][0].get("private").is_none());
    assert_eq!(written["apps"][0]["repository"], "example/private");
}

fn entry(id: &str) -> CatalogueApp {
    CatalogueApp {
        id: id.to_owned(),
        name: "Photos".to_owned(),
        info: String::new(),
        provider: Provider::Github,
        repository: "example/photos".to_owned(),
        asset: "photos-{version}.tar.gz".to_owned(),
        private: false,
        system: false,
        preinstall: false,
        logo: String::new(),
        category: None,
    }
}

#[test]
fn an_entry_with_the_address_of_a_logo_still_reads_and_does_not_keep_it() {
    let app: CatalogueApp = serde_json::from_str(
        r#"{"id":"photos","name":"Photos","provider":"github","repository":"a/b","asset":"x.tar.gz","logo":"https://example.com/photos.svg"}"#,
    )
    .expect("entry");

    assert!(app.valid());
    assert!(serde_json::to_value(&app)
        .expect("json")
        .get("logo")
        .is_none());
}

#[test]
fn entries_that_could_escape_where_they_end_up_are_refused() {
    let invalid = [
        CatalogueApp {
            id: "../photos".to_owned(),
            ..entry("photos")
        },
        CatalogueApp {
            id: "nas-app-manager".to_owned(),
            ..entry("photos")
        },
        CatalogueApp {
            repository: "no-owner".to_owned(),
            ..entry("photos")
        },
        CatalogueApp {
            repository: "example/../photos".to_owned(),
            ..entry("photos")
        },
        CatalogueApp {
            asset: "../photos.tar.gz".to_owned(),
            ..entry("photos")
        },
        CatalogueApp {
            name: "   ".to_owned(),
            ..entry("photos")
        },
    ];
    for app in invalid {
        assert!(!app.valid(), "{app:?}");
    }
    assert!(entry("photos").valid());
    assert!(CatalogueApp {
        repository: "group/subgroup/photos".to_owned(),
        ..entry("photos")
    }
    .valid());
}

#[tokio::test]
async fn the_saved_catalogue_takes_the_place_of_the_file_and_keeps_its_credential() {
    let db = crate::services::database::open_in_memory()
        .await
        .expect("database");
    let credential = read_config(&db).await.credential;

    let saved = save(&db, vec![entry("photos"), entry("disks")])
        .await
        .expect("saved");

    assert_eq!(saved.credential, credential);
    let read = read(&db).await;
    assert_eq!(read, vec![entry("photos"), entry("disks")]);
    assert_eq!(find(&db, "disks").await, Some(entry("disks")));
}

#[tokio::test]
async fn a_catalogue_with_one_bad_entry_or_a_repeated_id_is_not_saved_at_all() {
    let db = crate::services::database::open_in_memory()
        .await
        .expect("database");
    let before = read(&db).await;

    let repeated = save(&db, vec![entry("photos"), entry("photos")]).await;
    let bad = save(
        &db,
        vec![
            entry("photos"),
            CatalogueApp {
                id: "bad id".to_owned(),
                ..entry("photos")
            },
        ],
    )
    .await;

    assert!(repeated.is_err());
    assert!(bad.is_err());
    assert_eq!(read(&db).await, before);
}

#[tokio::test]
async fn an_entry_keeps_its_category_and_one_outside_the_list_is_refused() {
    let db = crate::services::database::open_in_memory()
        .await
        .expect("database");
    let photos = CatalogueApp {
        category: Some(Category::Media),
        ..entry("photos")
    };
    save(&db, vec![photos.clone()]).await.expect("saved");
    assert_eq!(find(&db, "photos").await, Some(photos));

    let refused = serde_json::from_str::<CatalogueApp>(
        r#"{"id":"photos","name":"Photos","provider":"github","repository":"a/b","asset":"x.tar.gz","category":"games"}"#,
    );
    assert!(refused.is_err());
}

#[tokio::test]
async fn retired_categories_in_a_saved_catalogue_are_migrated_without_losing_entries() {
    let db = crate::services::database::open_in_memory()
        .await
        .expect("database");
    let mut apps = Vec::new();
    for (id, category) in [
        ("nas-file-manager", "files"),
        ("other-files", "files"),
        ("security-app", "security"),
        ("utility-app", "utilities"),
        ("custom-explorer", "system"),
    ] {
        let mut app = serde_json::to_value(entry(id)).expect("entry");
        app["category"] = serde_json::Value::String(category.to_owned());
        apps.push(app);
    }
    let legacy = serde_json::json!({"version":1,"apps":apps});
    settings::write(&db, Section::Catalogue, &Some(legacy))
        .await
        .expect("saved");
    let found = read(&db).await;
    assert_eq!(found.len(), 5);
    assert_eq!(found[0].category, Some(Category::Storage));
    for app in &found[1..] {
        assert_eq!(app.category, Some(Category::System));
        assert_eq!(app.repository, "example/photos");
    }
    save(&db, found.clone())
        .await
        .expect("save migrated catalogue");
    assert_eq!(read(&db).await, found);
}
