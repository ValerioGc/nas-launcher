use super::*;

async fn keys(db: &SqlitePool) -> Vec<(String, String)> {
    sqlx::query_as("SELECT key, source FROM notifications ORDER BY id")
        .fetch_all(db)
        .await
        .expect("notifications")
}

#[tokio::test]
async fn the_option_is_off_until_it_is_turned_on() {
    let db = database::open_in_memory().await.expect("database");
    assert!(!load(&db).await.expect("load").enabled);

    let saved = save(&db, Discovery { enabled: true }).await.expect("save");
    assert!(saved.enabled);
    assert!(load(&db).await.expect("load").enabled);
    apply_saved(&db).await;

    save(&db, Discovery { enabled: false }).await.expect("save");
    assert!(!load(&db).await.expect("load").enabled);
    apply_saved(&db).await;
}

#[tokio::test]
async fn the_end_of_the_wizard_says_the_discovery_is_off_and_where_to_turn_it_on() {
    let db = database::open_in_memory().await.expect("database");
    announce_off(&db).await;
    assert_eq!(
        keys(&db).await,
        [("discoveryOff".to_owned(), APP.to_owned())]
    );
}

#[tokio::test]
async fn the_request_says_on_while_the_option_is_on_and_is_gone_when_it_is_off() {
    let directory = std::env::temp_dir().join(format!("nas-discovery-{}", std::process::id()));
    tokio::fs::create_dir_all(&directory)
        .await
        .expect("directory");
    let file = directory.join("discovery");

    apply_request(Some(file.clone()), true, async { Ok(()) })
        .await
        .expect("on");
    assert_eq!(tokio::fs::read_to_string(&file).await.unwrap(), "on");

    let refused = apply_request(Some(file.clone()), true, async {
        Err(ApiFailure::unavailable("firewall not ready"))
    })
    .await;
    assert!(refused.is_err());
    assert_eq!(tokio::fs::read_to_string(&file).await.unwrap(), "on");

    apply_request(Some(file.clone()), false, async { Ok(()) })
        .await
        .expect("off");
    assert!(!file.exists());

    apply_request(Some(file.clone()), false, async { Ok(()) })
        .await
        .expect("off again");

    let missing = directory.join("missing").join("discovery");
    let applied = std::sync::atomic::AtomicBool::new(false);
    let failed = apply_request(Some(missing.clone()), true, async {
        applied.store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    })
    .await;
    assert!(failed.is_err());
    assert!(!applied.load(std::sync::atomic::Ordering::Relaxed));

    tokio::fs::create_dir_all(missing.parent().unwrap().join("discovery").join("x"))
        .await
        .expect("folder");
    let blocked = directory.join("missing").join("discovery");
    assert!(apply_request(Some(blocked), false, async { Ok(()) })
        .await
        .is_err());

    assert!(apply_request(None, true, async { Ok(()) }).await.is_err());

    tokio::fs::remove_dir_all(&directory).await.expect("clean");
}

#[test]
fn a_socket_bound_on_the_port_is_found_in_the_table_of_the_machine() {
    let table = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode ref pointer drops\n\
   12: 00000000:9BC5 00000000:0000 07 00000000:00000000 00:00000000 00000000   998        0 41001 2 0000000000000000 0\n\
   13: 0100007F:0035 00000000:0000 07 00000000:00000000 00:00000000 00000000   991        0 41002 2 0000000000000000 0\n";
    assert!(holds_port(table, PORT));
    assert!(holds_port(table, 53));
    assert!(!holds_port(table, 39878));
    assert!(!holds_port("  sl  local_address\n", PORT));
    assert!(!holds_port("", PORT));
}

#[tokio::test]
async fn the_status_names_the_port_and_reads_no_firewall_outside_a_nas() {
    let status = status().await;
    assert_eq!(status.port, 39877);
    assert_eq!(status.applied, None);
}
