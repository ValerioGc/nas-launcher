use super::*;
use crate::services::database;
use axum::http::StatusCode;
use futures_util::StreamExt;

#[test]
fn the_default_look_is_a_valid_one() {
    assert!(TerminalSettings::default().validate().is_ok());
}

#[test]
fn out_of_range_values_and_colours_that_are_not_colours_are_refused() {
    let base = TerminalSettings::default();
    for invalid in [
        TerminalSettings {
            transparency: 101,
            ..base.clone()
        },
        TerminalSettings {
            text_color: "red".to_owned(),
            ..base.clone()
        },
        TerminalSettings {
            font_size: 9,
            ..base.clone()
        },
        TerminalSettings {
            font_size: 33,
            ..base.clone()
        },
        TerminalSettings {
            line_height: 2.6,
            ..base.clone()
        },
        TerminalSettings {
            line_height: f32::NAN,
            ..base.clone()
        },
    ] {
        assert!(invalid.validate().is_err(), "{invalid:?}");
    }
}

#[tokio::test]
async fn saved_settings_are_read_back_and_the_defaults_stand_in_before() {
    let db = database::open_in_memory().await.expect("database");
    assert_eq!(
        load_settings(&db).await.expect("defaults"),
        TerminalSettings::default()
    );

    let chosen = TerminalSettings {
        transparency: 30,
        font_size: 18,
        line_height: 1.2,
        ..TerminalSettings::default()
    };
    save_settings(&db, chosen.clone()).await.expect("saved");

    assert_eq!(load_settings(&db).await.expect("read"), chosen);
}

#[test]
fn a_line_too_long_is_cut_on_a_character_boundary() {
    let cut = clip("è".repeat(MAX_LINE));

    assert!(cut.len() <= MAX_LINE);
    assert!(cut.chars().all(|character| character == 'è'));
}

#[tokio::test]
async fn a_program_is_read_one_line_at_a_time_without_carriage_returns() {
    let read: Vec<String> = lines("printf", &["first\r\nsecond\nthird"])
        .expect("started")
        .collect()
        .await;

    assert_eq!(read, ["first", "second", "third"]);
}

#[test]
fn a_program_that_is_not_there_is_a_typed_failure() {
    let failure = lines("no-such-program-for-the-log", &[])
        .err()
        .expect("refused");

    assert_eq!(failure.0, StatusCode::SERVICE_UNAVAILABLE);
}

#[test]
fn the_log_follows_the_system_journal() {
    let (program, args) = JOURNAL;

    assert_eq!(program, "journalctl");
    assert!(args.contains(&"--follow"));
    assert!(args.contains(&"--no-pager"));
}
