use super::{says_real, Mode};

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
