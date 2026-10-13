use super::*;
use axum::http::StatusCode;

fn request(action: Action, item: Option<Item>) -> Request {
    Request {
        action,
        item,
        options: Options::default(),
    }
}

#[test]
fn a_request_names_an_item_only_to_preview_or_clean_it_with_bounded_options() {
    request(Action::Overview, None)
        .validate()
        .expect("overview");
    for item in Item::ALL {
        request(Action::Preview, Some(item))
            .validate()
            .expect("preview");
        request(Action::Clean, Some(item))
            .validate()
            .expect("clean");
    }
    assert!(request(Action::Overview, Some(Item::Journal))
        .validate()
        .is_err());
    assert!(request(Action::Clean, None).validate().is_err());
    for options in [
        Options {
            temporary_days: 0,
            journal_mb: 500,
        },
        Options {
            temporary_days: 7,
            journal_mb: 8,
        },
    ] {
        let mut wrong = request(Action::Preview, Some(Item::Temporary));
        wrong.options = options;
        assert!(wrong.validate().is_err());
    }
}

#[test]
fn the_application_sends_only_identifiers_and_numbers() {
    let parsed: Request = serde_json::from_str(
        r#"{"action":"clean","item":"old-kernels","options":{"temporaryDays":3,"journalMb":64}}"#,
    )
    .expect("request");
    assert_eq!(parsed.item, Some(Item::OldKernels));
    for wrong in [
        r#"{"action":"clean","item":"/etc"}"#,
        r#"{"action":"run","item":"journal"}"#,
        r#"{"action":"clean","item":"journal","path":"/"}"#,
    ] {
        assert!(serde_json::from_str::<Request>(wrong).is_err(), "{wrong}");
    }
}

#[tokio::test]
async fn outside_the_nas_nothing_is_cleaned() {
    let refused = run(request(Action::Overview, None))
        .await
        .expect_err("not on the NAS");
    assert_eq!(refused.0, StatusCode::SERVICE_UNAVAILABLE);
    let invalid = run(request(Action::Clean, None))
        .await
        .expect_err("invalid");
    assert_eq!(invalid.0, StatusCode::BAD_REQUEST);
}

#[test]
fn the_answer_of_the_job_comes_first_then_the_reason_of_systemd() {
    let value = serde_json::json!({ "bytes": 1 });
    let answered = |value: Option<serde_json::Value>, error: Option<&str>| {
        Some(Answer {
            value,
            error: error.map(str::to_owned),
        })
    };
    assert_eq!(
        outcome(answered(Some(value.clone()), None), Err("failed".into())).expect("value"),
        value
    );
    let refused = outcome(
        answered(None, Some("an update of the NAS is under way")),
        Ok(()),
    );
    assert_eq!(
        refused.expect_err("refused").1,
        "an update of the NAS is under way"
    );
    let unit = outcome(None, Err("Unit nas-os-cleanup.service not found.".into()));
    assert_eq!(
        unit.expect_err("no unit").1,
        "Unit nas-os-cleanup.service not found."
    );
    let silent = outcome(answered(None, None), Ok(()));
    assert_eq!(silent.expect_err("silent").1, "the cleanup gave no answer");
}
