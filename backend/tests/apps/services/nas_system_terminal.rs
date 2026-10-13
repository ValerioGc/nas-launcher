use super::*;
use crate::services::database;
use futures_util::{Stream, StreamExt};
use std::{pin::Pin, time::Duration};

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
fn a_command_is_one_line_with_no_control_character() {
    assert!(validate_command("ls -la /var/lib").is_ok());
    assert!(validate_command("").is_ok());
    for invalid in ["ls\nrm -rf /tmp/x", "ls\r", "ls\u{0}", "ls\u{1e}0 /"] {
        assert!(validate_command(invalid).is_err(), "{invalid:?}");
    }
    assert!(validate_command(&"x".repeat(MAX_COMMAND + 1)).is_err());
}

#[test]
fn the_end_of_a_command_is_told_apart_from_its_output() {
    assert_eq!(
        event("\u{1e}0 /var/lib".to_owned()),
        TerminalEvent::Done {
            code: 0,
            cwd: "/var/lib".to_owned()
        }
    );
    assert_eq!(
        event("\u{1e}127 /dir with spaces".to_owned()),
        TerminalEvent::Done {
            code: 127,
            cwd: "/dir with spaces".to_owned()
        }
    );
    assert_eq!(
        event("total 0\r".to_owned()),
        TerminalEvent::Line("total 0".to_owned())
    );
}

#[test]
fn a_line_too_long_is_cut_on_a_character_boundary() {
    let cut = clip("è".repeat(MAX_LINE));

    assert!(cut.len() <= MAX_LINE);
    assert!(cut.chars().all(|character| character == 'è'));
}

type Events = Pin<Box<dyn Stream<Item = TerminalEvent> + Send>>;

async fn until_done(events: &mut Events) -> (Vec<String>, (i32, String)) {
    let mut lines = Vec::new();
    loop {
        let next = tokio::time::timeout(Duration::from_secs(10), events.next())
            .await
            .expect("the shell answers")
            .expect("the shell is still there");
        match next {
            TerminalEvent::Line(line) => lines.push(line),
            TerminalEvent::Done { code, cwd } => return (lines, (code, cwd)),
            TerminalEvent::Session(_) => panic!("one session event only"),
        }
    }
}

/// Starts a shell for every connection, as `nas-os-terminal.socket` does on the NAS.
fn serve_shells() -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("nas-terminal-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let listener = tokio::net::UnixListener::bind(&path).expect("a free socket");
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let stream = stream.into_std().expect("a socket");
            stream.set_nonblocking(false).expect("blocking");
            let output = stream.try_clone().expect("a second handle");
            let mut shell = Command::new("bash")
                .args(["--noprofile", "--norc", "-s"])
                .current_dir("/")
                .stdin(Stdio::from(std::os::fd::OwnedFd::from(stream)))
                .stdout(Stdio::from(std::os::fd::OwnedFd::from(output)))
                .spawn()
                .expect("a shell");
            tokio::spawn(async move { shell.wait().await });
        }
    });
    path
}

#[tokio::test]
async fn a_shell_keeps_its_directory_reports_every_exit_and_ends_with_its_window() {
    let socket = serve_shells();
    unsafe {
        std::env::set_var("NAS_TERMINAL_MODE", "real");
        std::env::set_var("NAS_TERMINAL_SOCKET", &socket);
    }
    let mut events: Events = Box::pin(open().await.expect("a shell"));
    let Some(TerminalEvent::Session(id)) = events.next().await else {
        panic!("the session comes first");
    };
    assert_eq!(until_done(&mut events).await.1, (0, "/".to_owned()));

    run(&id, "echo hello").await.expect("sent");
    assert_eq!(
        until_done(&mut events).await,
        (vec!["hello".to_owned()], (0, "/".to_owned()))
    );

    run(&id, "cd /tmp").await.expect("sent");
    assert_eq!(until_done(&mut events).await.1, (0, "/tmp".to_owned()));
    run(&id, "pwd").await.expect("sent");
    assert_eq!(until_done(&mut events).await.0, ["/tmp"]);

    run(&id, "ls /no-such-directory-here").await.expect("sent");
    let (lines, (code, _)) = until_done(&mut events).await;
    assert_ne!(code, 0);
    assert!(lines.concat().contains("no-such-directory-here"));

    drop(events);
    let ended = run(&id, "echo late").await.expect_err("no session");
    assert_eq!(ended.0, StatusCode::NOT_FOUND);
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
