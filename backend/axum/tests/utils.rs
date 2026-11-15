use super::{answer, env_path, says_real, Mode};
use std::{
    os::unix::process::ExitStatusExt,
    process::{ExitStatus, Output},
};

#[test]
fn only_real_turns_a_mode_on() {
    assert!(says_real(Some("real")));
    assert!(says_real(Some("Real")));
    assert!(!says_real(Some("simulated")));
    assert!(!says_real(Some("")));
    assert!(!says_real(None));
}

#[test]
fn every_mode_has_its_own_switch() {
    let modes = [
        Mode::Apps,
        Mode::Apt,
        Mode::Firewall,
        Mode::Network,
        Mode::Packages,
        Mode::Power,
        Mode::Processes,
        Mode::Terminal,
        Mode::Token,
    ];
    let mut variables: Vec<_> = modes.iter().map(|mode| mode.variable()).collect();
    variables.sort_unstable();
    variables.dedup();
    assert_eq!(variables.len(), modes.len());
    assert!(variables
        .iter()
        .all(|name| name.starts_with("NAS_") && name.ends_with("_MODE")));
}

fn output(code: i32, stdout: &str, stderr: &str) -> Output {
    Output {
        status: ExitStatus::from_raw(code << 8),
        stdout: stdout.as_bytes().to_vec(),
        stderr: stderr.as_bytes().to_vec(),
    }
}

#[test]
fn systemctl_gives_what_it_printed_or_the_first_line_of_its_error() {
    assert_eq!(
        answer(&output(0, "ActiveState=failed\n", "")),
        Ok("ActiveState=failed\n".to_owned())
    );
    assert_eq!(
        answer(&output(1, "", "  Access denied\nmore detail\n")),
        Err("Access denied".to_owned())
    );
    assert_eq!(
        answer(&output(5, "", "")),
        Err("systemctl refused".to_owned())
    );
}

#[test]
fn a_path_falls_back_to_the_installation_without_its_variable() {
    assert_eq!(
        env_path("NAS_TEST_PATH_NEVER_SET", "/var/lib/nas-os"),
        std::path::PathBuf::from("/var/lib/nas-os")
    );
}
