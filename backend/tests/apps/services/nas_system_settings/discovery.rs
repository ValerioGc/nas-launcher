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

    // Outside the NAS the firewall is not touched: the choice is only kept.
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

    // A firewall that refuses keeps the request, so the next start applies it again.
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
    // Closing what is closed already is no failure.
    apply_request(Some(file.clone()), false, async { Ok(()) })
        .await
        .expect("off again");

    // A request that cannot be written leaves the firewall alone.
    let missing = directory.join("missing").join("discovery");
    let applied = std::sync::atomic::AtomicBool::new(false);
    let failed = apply_request(Some(missing.clone()), true, async {
        applied.store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    })
    .await;
    assert!(failed.is_err());
    assert!(!applied.load(std::sync::atomic::Ordering::Relaxed));
    // Nor one that cannot be removed: a folder stands where the file should be.
    tokio::fs::create_dir_all(missing.parent().unwrap().join("discovery").join("x"))
        .await
        .expect("folder");
    let blocked = directory.join("missing").join("discovery");
    assert!(apply_request(Some(blocked), false, async { Ok(()) })
        .await
        .is_err());

    // No state directory, no request: nothing is applied.
    assert!(apply_request(None, true, async { Ok(()) }).await.is_err());

    tokio::fs::remove_dir_all(&directory).await.expect("clean");
}
