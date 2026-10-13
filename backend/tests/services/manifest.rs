use super::*;

#[test]
fn a_socket_is_named_after_the_application_and_nothing_else() {
    unsafe { std::env::set_var("NAS_APP_SOCKETS", "/run/probe") };

    assert_eq!(
        socket_of("fileExplorer"),
        PathBuf::from("/run/probe/fileExplorer.sock")
    );
}

#[tokio::test]
async fn an_identifier_that_is_not_one_reads_nothing() {
    for id in [
        "",
        "../etc",
        "file/explorer",
        "file explorer",
        &"x".repeat(65),
    ] {
        assert!(read(id).await.is_none(), "{id} is not an identifier");
    }
}

fn offering(id: &str, provides: &[(&str, &str)]) -> Manifest {
    Manifest {
        id: id.to_owned(),
        name: id.to_owned(),
        version: "1.0.0".to_owned(),
        icon: String::new(),
        category: None,
        system: false,
        protocol: 1,
        provides: provides
            .iter()
            .map(|(capability, endpoint)| Capability {
                capability: (*capability).to_owned(),
                version: 1,
                endpoint: (*endpoint).to_owned(),
            })
            .collect(),
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

#[test]
fn only_the_applications_that_answer_a_capability_on_a_public_path_provide_it() {
    let manifests = [
        offering(
            "explorer",
            &[("search", "/api/search"), ("files", "/api/files")],
        ),
        offering("photos", &[("search", "/internal/search")]),
        offering("music", &[("search", "/../api/apps")]),
        offering("notes", &[("search", "//elsewhere.test/search")]),
        offering("backup", &[]),
    ];

    assert_eq!(
        providers(&manifests, "search"),
        [Provider {
            app: "explorer".to_owned(),
            endpoint: "/api/search".to_owned(),
            version: 1,
        }]
    );
    assert!(providers(&manifests, "unknown").is_empty());
}

#[tokio::test]
async fn an_unpacked_package_says_which_paths_it_asks_for() {
    let directory = std::env::temp_dir().join(format!("nas-manifest-paths-{}", std::process::id()));
    tokio::fs::create_dir_all(&directory).await.unwrap();
    tokio::fs::write(
        directory.join("nas-app.json"),
        r#"{"manifest_version":1,"app":{"id":"probe","name":"Probe","category":"storage","system":true},
            "permissions":{"paths":["/srv/storage","/proc"]}}"#,
    )
    .await
    .unwrap();

    let found = read_unpacked(&directory, "probe").await.expect("manifest");
    assert_eq!(found.paths, ["/srv/storage", "/proc"]);
    assert_eq!(found.category, Some(Category::Storage));
    assert!(found.system);
    assert!(read_unpacked(&directory, "other").await.is_none());

    tokio::fs::write(
        directory.join("nas-app.json"),
        r#"{"manifest_version":1,"app":{"id":"probe","name":"Probe"}}"#,
    )
    .await
    .unwrap();
    assert!(
        !read_unpacked(&directory, "probe")
            .await
            .expect("legacy manifest")
            .system
    );
    assert!(read_unpacked(&directory, "probe")
        .await
        .expect("manifest")
        .paths
        .is_empty());
    let _ = tokio::fs::remove_dir_all(&directory).await;
}

#[tokio::test]
async fn an_application_that_works_without_its_window_asks_for_the_lower_priority() {
    let folder =
        std::env::temp_dir().join(format!("nas-manifest-background-{}", std::process::id()));
    std::fs::create_dir_all(&folder).expect("folder");
    std::fs::write(
        folder.join("nas-app.json"),
        r#"{"manifest_version":1,"app":{"id":"worker","name":"Worker"},
            "run":{"exec":"bin/worker","background":true},"permissions":{"paths":["/srv/storage"]}}"#,
    )
    .expect("manifest");

    let worker = read_unpacked(&folder, "worker").await.expect("read");
    assert!(worker.background);
    assert_eq!(worker.grants_asked(), ["/srv/storage", "run:background"]);
    let mut quiet = worker.clone();
    quiet.background = false;
    assert_eq!(quiet.grants_asked(), ["/srv/storage"]);
    std::fs::remove_dir_all(folder).expect("clean up");
}

#[tokio::test]
async fn a_manifest_is_read_again_only_when_its_file_changes() {
    let directory = std::env::temp_dir().join(format!("nas-manifest-cache-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("package folder");
    let file = directory.join("nas-app.json");
    let manifest =
        |name: &str| format!(r#"{{"manifest_version":1,"app":{{"id":"cached","name":"{name}"}}}}"#);
    std::fs::write(&file, manifest("First")).expect("manifest");

    assert_eq!(
        read_from(&directory, "cached").await.expect("read").name,
        "First"
    );
    assert_eq!(
        read_from(&directory, "cached").await.expect("cached").name,
        "First"
    );

    // Rewritten, with another size: read again.
    std::fs::write(&file, manifest("Second one")).expect("updated manifest");
    assert_eq!(
        read_from(&directory, "cached")
            .await
            .expect("read again")
            .name,
        "Second one"
    );

    std::fs::remove_file(&file).expect("removed");
    assert!(read_from(&directory, "cached").await.is_none());
    std::fs::remove_dir_all(directory).expect("cleanup");
}
