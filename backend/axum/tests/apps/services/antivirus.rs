use super::*;

#[test]
fn default_configuration_never_enables_antivirus_or_jobs() {
    let config = Settings::default();
    assert!(!config.enabled && !config.scheduler_enabled && !config.auto_quarantine);
    assert!(config.schedules.is_empty());
    config.validate().expect("valid defaults");
}

#[test]
fn rejects_traversal_control_characters_and_invalid_limits() {
    for path in ["relative", "/tmp/../etc", "/tmp/file\nother", "/tmp/\0"] {
        assert!(!valid_path(path));
    }
    assert!(valid_path("/srv/storage/My documents/a: b.pdf"));
    let mut config = Settings::default();
    config.quick.max_file_mb = 0;
    assert!(config.validate().is_err());
    config.quick.max_file_mb = 2049;
    assert!(config.validate().is_err());
    config.quick.max_file_mb = 100;
    config.quick.exclusions.push("../etc".into());
    assert!(config.validate().is_err());
}

#[test]
fn validates_job_times_days_ids_and_file_paths() {
    let mut config = Settings::default();
    let job = Schedule {
        id: "one".into(),
        enabled: true,
        kind: ScanKind::Quick,
        day: "Mon".into(),
        time: "03:00".into(),
        paths: Vec::new(),
    };
    config.schedules.push(job.clone());
    config.validate().expect("valid job");
    for time in ["24:00", "02:60", "2:00", "xx:yy", "💚"] {
        config.schedules[0].time = time.into();
        assert!(config.validate().is_err());
    }
    config.schedules[0] = job.clone();
    config.schedules[0].day = "Monday".into();
    assert!(config.validate().is_err());
    config.schedules[0] = job.clone();
    config.schedules.push(job.clone());
    assert!(config.validate().is_err());
    config.schedules = vec![job];
    config.schedules[0].kind = ScanKind::File;
    assert!(config.validate().is_err());
    config.schedules[0].paths = vec!["/tmp/a".into()];
    config.validate().expect("single scheduled file");
    config.schedules[0].paths.push("/tmp/b".into());
    assert!(config.validate().is_err());
}

#[test]
fn file_scans_require_exactly_one_path_and_custom_at_least_one() {
    for kind in [ScanKind::File, ScanKind::Custom] {
        assert!(ScanRequest {
            kind,
            paths: Vec::new(),
            skip_unchanged: false,
        }
        .validate()
        .is_err());
    }
    assert!(ScanRequest {
        kind: ScanKind::File,
        paths: vec!["/tmp/a".into(), "/tmp/b".into()],
        skip_unchanged: false,
    }
    .validate()
    .is_err());
    ScanRequest {
        kind: ScanKind::Full,
        paths: Vec::new(),
        skip_unchanged: false,
    }
    .validate()
    .expect("full scan");
}

#[test]
fn reads_real_apt_progress_and_rejects_invalid_values() {
    assert_eq!(
        install_progress("dlstatus:1:20:Downloading\npmstatus:clamav:75.5:Unpacking"),
        Some(75.5)
    );
    for text in [
        "pmerror:x:90:failure",
        "pmstatus:x:NaN:test",
        "pmstatus:x:101:test",
        "invalid",
    ] {
        assert_eq!(install_progress(text), None);
    }
}

#[test]
fn a_planned_scan_reports_how_far_it_is() {
    let mut report = Report::default();
    assert_eq!(scan_progress(&report), None);
    report.planned = 200;
    report.scanned = 50;
    assert_eq!(scan_progress(&report), Some(25.0));
    report.scanned = 250;
    assert_eq!(scan_progress(&report), Some(100.0));
}

#[tokio::test]
async fn persists_settings_and_refuses_enabling_via_settings() {
    let db = database::open_in_memory().await.expect("database");
    assert!(!load(&db).await.expect("defaults").enabled);
    let mut config = Settings {
        scheduler_enabled: true,
        ..Settings::default()
    };
    config.custom.paths = vec!["/srv/storage".into()];
    save(&db, config.clone()).await.expect("save options");
    assert_eq!(
        load(&db).await.expect("load options").custom.paths,
        config.custom.paths
    );
    config.enabled = true;
    assert!(save(&db, config).await.is_err());
    assert!(scan(
        &db,
        ScanRequest {
            kind: ScanKind::Quick,
            paths: Vec::new(),
            skip_unchanged: false,
        }
    )
    .await
    .is_err());
    assert!(quarantine_action(&db, "restore", "../../etc".into())
        .await
        .is_err());
    assert!(quarantine_action(&db, "restore", "valid-id".into())
        .await
        .is_err());

    assert!(forget_scanned(&db).await.is_err());
}

#[test]
fn lifecycle_configuration_is_opt_in_backward_compatible_and_validated() {
    let mut config = Settings::default();
    assert!(!config.boot_scan.enabled && !config.shutdown_scan.enabled);
    assert_eq!(config.boot_scan.kind, ScanKind::Quick);
    config.boot_scan.enabled = true;
    config.boot_scan.kind = ScanKind::Custom;
    assert!(config.validate().is_err());
    config.boot_scan.paths = vec!["/srv/storage".into()];
    config.shutdown_scan.enabled = true;
    config.shutdown_scan.kind = ScanKind::File;
    config.shutdown_scan.paths = vec!["/srv/storage/file".into()];
    config.validate().expect("configured lifecycle");
    config.shutdown_scan.paths.push("/srv/storage/other".into());
    assert!(config.validate().is_err());
    config.shutdown_scan.enabled = false;
    config.shutdown_scan.paths = vec!["../etc".into()];
    assert!(config.validate().is_err());
    let mut saved = serde_json::to_value(Settings::default()).expect("settings");
    let object = saved.as_object_mut().expect("object");
    object.remove("bootScan");
    object.remove("shutdownScan");
    let restored: Settings = serde_json::from_value(saved).expect("previous settings");
    assert!(!restored.boot_scan.enabled && !restored.shutdown_scan.enabled);
}
