use super::*;

#[test]
fn the_reset_needs_its_switch_and_the_wizard_installed() {
    assert_eq!(
        availability(true, true, true),
        ResetState {
            available: true,
            reason: None,
            running: false,
            failed: false,
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

#[test]
fn the_state_of_the_unit_is_read_by_name_whatever_the_order() {
    let run = UnitRun::parse("Result=exit-code\nInvocationID=b2\nActiveState=failed\n");

    assert_eq!(
        run,
        UnitRun {
            active: "failed".into(),
            result: "exit-code".into(),
            invocation: "b2".into(),
        }
    );
    assert!(!run.running());
    assert!(UnitRun::parse("ActiveState=activating\n").running());
}

#[test]
fn only_a_failure_after_the_request_of_this_launcher_counts() {
    let failed = UnitRun::parse("ActiveState=failed\nResult=exit-code\nInvocationID=b2\n");

    assert!(failed.failed_since(Some("a1")));
    assert!(failed.failed_since(Some("")));
    assert!(
        !failed.failed_since(Some("b2")),
        "the old failure, the new run has not started"
    );
    assert!(
        !failed.failed_since(None),
        "this launcher asked for no reset"
    );
}

#[test]
fn a_reset_that_runs_or_ended_well_has_not_failed() {
    let running = UnitRun::parse("ActiveState=activating\nResult=success\nInvocationID=b2\n");
    let fine = UnitRun::parse("ActiveState=inactive\nResult=success\nInvocationID=b2\n");
    let timeout = UnitRun::parse("ActiveState=inactive\nResult=timeout\nInvocationID=b2\n");

    assert!(!running.failed_since(Some("a1")));
    assert!(!fine.failed_since(Some("a1")));
    assert!(timeout.failed_since(Some("a1")));
}
