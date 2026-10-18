use crate::support::Machine;
use std::{
    process::{Command, Stdio},
    time::Duration,
};

fn launcher(arguments: &[&str]) -> std::process::ExitStatus {
    Machine::get();
    Command::new(env!("CARGO_BIN_EXE_nas-os-axum"))
        .args(arguments)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("nas-os-axum")
}

#[test]
fn refuses_the_commands_of_the_root_units_without_their_request() {
    for arguments in [
        &["--initialize-storage"][..],
        &["--initialize-storage", "a", "b"],
        &["--rename-storage"],
        &["--rename-storage", "a", "b"],
        &["--eject-storage"],
        &["--eject-storage", "a", "b"],
        &["--read-storage-smart"],
        &["--read-storage-smart", "a", "b"],
        &["--antivirus-job", "extra"],
        &["--antivirus-lifecycle"],
        &["--antivirus-lifecycle", "invalid"],
        &["--antivirus-lifecycle", "boot", "extra"],
        &["--antivirus-lifecycle", "shutdown", "extra"],
    ] {
        assert!(!launcher(arguments).success(), "{arguments:?}");
    }
}

#[test]
fn reads_smart_for_its_root_unit() {
    let machine = Machine::get();
    let request = machine.root.join("binary-smart.json");
    std::fs::write(&request, r#"{"device":"sdz"}"#).expect("request");
    assert!(launcher(&["--read-storage-smart", request.to_str().expect("path")]).success());
    std::fs::write(&request, r#"{"device":"sda"}"#).expect("request");
    assert!(!launcher(&["--read-storage-smart", request.to_str().expect("path")]).success());
}

#[test]
fn serves_until_it_is_asked_to_stop() {
    let machine = Machine::get();
    let mut server = Command::new(env!("CARGO_BIN_EXE_nas-os-axum"))
        .env("PORT", "0")
        .env("NAS_DB_PATH", machine.root.join("binary/launcher.db"))
        .env("NAS_LAUNCHER_SOCKET", machine.root.join("binary/apps.sock"))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("server");
    let output = server.stdout.take().expect("output");
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::BufRead;
        for line in std::io::BufReader::new(output)
            .lines()
            .map_while(Result::ok)
        {
            if line.contains("listening on") {
                let _ = sender.send(());
            }
        }
    });
    let listening = receiver.recv_timeout(Duration::from_secs(20)).is_ok();

    std::thread::sleep(Duration::from_millis(500));
    let stopped = Command::new("kill")
        .args(["-TERM", &server.id().to_string()])
        .status()
        .expect("kill");
    let status = server.wait().expect("server");
    assert!(listening);
    assert!(stopped.success());
    assert!(status.success());
}
