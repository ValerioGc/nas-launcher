use super::*;
use crate::services::{database, manifest::Capability};

fn offering(id: &str, capability: &str, endpoint: &str) -> Manifest {
    Manifest {
        id: id.to_owned(),
        name: id.to_owned(),
        system: false,
        version: "1.0.0".to_owned(),
        icon: String::new(),
        category: None,
        protocol: 1,
        provides: vec![Capability {
            capability: capability.to_owned(),
            version: 1,
            endpoint: endpoint.to_owned(),
        }],
        paths: Vec::new(),
        network: Vec::new(),
        packages: Vec::new(),
        background: false,
        shares: false,
        cleanup: false,
        socket: std::path::PathBuf::new(),
        exec: String::new(),
    }
}

#[test]
fn only_an_internal_endpoint_of_the_cache_capability_holds_caches() {
    let manifests = [
        offering("explorer", CAPABILITY, "/internal/cache"),
        offering("public", CAPABILITY, "/api/cache"),
        offering("other", "search", "/internal/cache"),
    ];
    let found: Vec<(&str, &str)> = holders(&manifests)
        .into_iter()
        .map(|(manifest, endpoint)| (manifest.id.as_str(), endpoint))
        .collect();
    assert_eq!(found, [("explorer", "/internal/cache")]);
}

#[test]
fn an_answer_is_taken_under_the_name_of_who_gave_it() {
    let body = br#"{"caches":[
        {"app":"someone-else","id":"previews","label":"Miniature","bytes":10,"items":2},
        {"id":"../etc","label":"x","bytes":1,"items":1}
    ]}"#;
    assert_eq!(
        declared("nas-file-manager", body),
        [Cache {
            app: "nas-file-manager".to_owned(),
            id: "previews".to_owned(),
            label: "Miniature".to_owned(),
            bytes: 10,
            items: 2,
            kind: Kind::Cache,
        }]
    );
    assert!(declared("nas-file-manager", b"not json").is_empty());
}

#[test]
fn a_trash_is_listed_beside_the_caches_as_what_it_is() {
    let manifests = [
        offering("explorer", TRASH, "/internal/trash"),
        offering("public", TRASH, "/api/trash"),
    ];
    assert_eq!(holders_of(&manifests, TRASH).len(), 1);
    let trash = trash_of(
        "nas-file-manager",
        br#"{"label":"Cestino di Explorer","bytes":5,"items":1}"#,
    )
    .expect("trash");
    assert_eq!(
        (trash.id.as_str(), trash.kind, trash.bytes),
        ("trash", Kind::Trash, 5)
    );
    assert!(trash_of("nas-file-manager", b"[]").is_none());
}

#[test]
fn the_archive_of_the_antivirus_shows_only_when_it_holds_something() {
    let mut report = antivirus::Report::default();
    assert_eq!(antivirus_cache(&report), None);
    report.cache_files = 3;
    report.cache_bytes = 4096;
    let cache = antivirus_cache(&report).expect("cache");
    assert_eq!(
        (
            cache.app.as_str(),
            cache.id.as_str(),
            cache.bytes,
            cache.items
        ),
        ("nas-security-center", "antivirus-scans", 4096, 3)
    );
}

#[tokio::test]
async fn emptying_checks_what_it_is_asked_and_finds_no_cache_without_its_application() {
    let db = database::open_in_memory().await.expect("database");
    assert_eq!(
        clear(&db, "../x", "previews").await.expect_err("app").0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        clear(&db, "nas-file-manager", "a/b")
            .await
            .expect_err("id")
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        clear(&db, "nas-not-installed", "previews")
            .await
            .expect_err("missing")
            .0,
        StatusCode::NOT_FOUND
    );

    assert_eq!(
        clear(&db, "nas-security-center", "antivirus-scans")
            .await
            .expect_err("off")
            .0,
        StatusCode::BAD_REQUEST
    );
}
