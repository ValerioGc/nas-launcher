use super::*;

#[test]
fn the_reset_needs_its_switch_and_the_wizard_installed() {
    assert_eq!(
        availability(true, true, true),
        ResetState {
            available: true,
            reason: None
        }
    );
    let disabled = availability(false, true, true);
    assert!(!disabled.available);
    assert_eq!(disabled.reason, Some(ResetBlock::Disabled));
    let no_wizard = availability(true, false, true);
    assert!(!no_wizard.available);
    assert_eq!(no_wizard.reason, Some(ResetBlock::NoWizard));
    let no_original = availability(true, true, false);
    assert!(!no_original.available);
    assert_eq!(no_original.reason, Some(ResetBlock::NoOriginalState));
}

#[tokio::test]
async fn a_confirmed_reset_is_refused_where_it_is_disabled() {
    let refused = start().await.expect_err("refused");

    assert_eq!(refused.0, axum::http::StatusCode::SERVICE_UNAVAILABLE);
}
