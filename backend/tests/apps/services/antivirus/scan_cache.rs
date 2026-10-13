use super::*;

fn stamp(size: u64) -> Stamp {
    Stamp {
        device: 1,
        inode: 2,
        size,
        modified_ns: 3,
        changed_ns: 4,
    }
}

#[tokio::test]
async fn a_file_is_left_out_only_while_it_is_the_same_clean_one() {
    let path = std::env::temp_dir().join(format!("nas-scan-cache-{}.sqlite3", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let cache = ScanCache::open(&path).await.expect("archive");
    let options = fingerprint(&ScanOptions::default());
    let now = 10 * RESCAN_AFTER_MS;
    let file = (b"/srv/a.txt".to_vec(), stamp(5));

    assert!(cache
        .unchanged(&[], &options, now)
        .await
        .expect("nothing")
        .is_empty());
    assert!(cache
        .unchanged(std::slice::from_ref(&file), &options, now)
        .await
        .expect("unknown")
        .is_empty());

    cache
        .record(std::slice::from_ref(&file), &options, now)
        .await
        .expect("recorded");
    cache
        .record(&[], &options, now)
        .await
        .expect("nothing to record");
    assert!(cache
        .unchanged(std::slice::from_ref(&file), &options, now)
        .await
        .expect("known")
        .contains(&file.0));
    // Another size, other options, or 30 days later: scanned again.
    let changed = (file.0.clone(), stamp(6));
    assert!(cache
        .unchanged(&[changed], &options, now)
        .await
        .expect("changed")
        .is_empty());
    let other = fingerprint(&ScanOptions {
        archives: false,
        ..ScanOptions::default()
    });
    assert!(cache
        .unchanged(std::slice::from_ref(&file), &other, now)
        .await
        .expect("other options")
        .is_empty());
    assert!(cache
        .unchanged(std::slice::from_ref(&file), &options, now + RESCAN_AFTER_MS)
        .await
        .expect("too old")
        .is_empty());

    cache
        .forget(std::ffi::OsStr::new("/srv/a.txt"))
        .await
        .expect("forgotten");
    assert!(cache
        .unchanged(std::slice::from_ref(&file), &options, now)
        .await
        .expect("forgotten")
        .is_empty());
    cache
        .record(std::slice::from_ref(&file), &options, now)
        .await
        .expect("again");
    cache.prune(now + RESCAN_AFTER_MS).await.expect("pruned");
    assert!(cache
        .unchanged(&[file], &options, now)
        .await
        .expect("pruned")
        .is_empty());
    cache.close().await;
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn an_archive_that_fails_says_so_and_never_panics() {
    let directory = std::env::temp_dir().join(format!("nas-scan-cache-dir-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("folder");
    // A folder is no database.
    assert!(ScanCache::open(&directory).await.is_err());

    let path = directory.join("broken.sqlite3");
    let _ = std::fs::remove_file(&path);
    let cache = ScanCache::open(&path).await.expect("archive");
    sqlx::query("DROP TABLE clean_files")
        .execute(&cache.pool)
        .await
        .expect("dropped");
    let options = fingerprint(&ScanOptions::default());
    let file = (b"/srv/a.txt".to_vec(), stamp(5));
    assert!(cache
        .unchanged(std::slice::from_ref(&file), &options, 1)
        .await
        .is_err());
    assert!(cache
        .record(std::slice::from_ref(&file), &options, 1)
        .await
        .is_err());
    assert!(cache
        .forget(std::ffi::OsStr::new("/srv/a.txt"))
        .await
        .is_err());
    assert!(cache.prune(1).await.is_err());
    assert!(cache.count().await.is_err());
    cache.close().await;
    let _ = std::fs::remove_dir_all(directory);
}
