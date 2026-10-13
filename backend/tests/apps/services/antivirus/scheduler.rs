use super::*;
use crate::{
    apps::services::antivirus::Settings,
    services::{
        database,
        settings::{self, Section},
    },
};
fn job() -> Schedule {
    Schedule {
        id: "one".into(),
        enabled: true,
        kind: super::super::ScanKind::Quick,
        day: "*".into(),
        time: "03:00".into(),
        paths: Vec::new(),
    }
}
#[tokio::test]
async fn ignores_disabled_jobs_rechecks_edited_work_and_reports_a_refused_scan() {
    let db = database::open_in_memory().await.expect("database");
    assert!(upcoming(&db).await.is_empty());
    dispatch(&db, 1, job()).await;
    let mut config = Settings {
        enabled: true,
        scheduler_enabled: true,
        schedules: vec![job()],
        ..Settings::default()
    };
    settings::write(&db, Section::Antivirus, &config)
        .await
        .expect("config");
    assert_eq!(upcoming(&db).await.len(), 1);
    config.schedules[0].time = "04:00".into();
    settings::write(&db, Section::Antivirus, &config)
        .await
        .expect("edited");
    dispatch(&db, 1, job()).await;
    let count: (i64,) = sqlx::query_as("SELECT count(*) FROM notifications")
        .fetch_one(&db)
        .await
        .expect("count");
    assert_eq!(count.0, 0);
    config.schedules[0].enabled = false;
    settings::write(&db, Section::Antivirus, &config)
        .await
        .expect("disabled");
    assert!(upcoming(&db).await.is_empty());
    config.schedules[0] = job();
    config.schedules.push(Schedule {
        id: "two".into(),
        day: "Sun".into(),
        ..job()
    });
    settings::write(&db, Section::Antivirus, &config)
        .await
        .expect("enabled");
    assert_eq!(upcoming(&db).await.len(), 2);
    dispatch(&db, 1, job()).await;
    let notice: (String,) = sqlx::query_as("SELECT key FROM notifications")
        .fetch_one(&db)
        .await
        .expect("notice");
    assert_eq!(notice.0, "antivirusJobSkipped");
    let running = tokio::spawn(run(db));
    tokio::time::sleep(Duration::from_millis(30)).await;
    changed();
    tokio::time::sleep(Duration::from_millis(30)).await;
    running.abort();
}
