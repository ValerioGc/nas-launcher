use super::*;

fn code(to: &str, purpose: &str, value: &str) -> Code {
    Code {
        to: to.to_owned(),
        purpose: purpose.to_owned(),
        code: value.to_owned(),
    }
}

#[test]
fn only_a_six_digit_code_for_an_address_and_a_known_purpose_is_sent() {
    assert!(code("me@example.com", "verify", "012345").valid());
    assert!(code("me@example.com", "reset", "999999").valid());
    for wrong in [
        code("me", "verify", "012345"),
        code("me@example.com\r\nBcc: x@y.z", "verify", "012345"),
        code("me@example.com", "login", "012345"),
        code("me@example.com", "reset", "01234"),
        code("me@example.com", "reset", "01234a"),
    ] {
        assert!(
            !wrong.valid(),
            "{} {} {}",
            wrong.to,
            wrong.purpose,
            wrong.code
        );
    }
}

#[tokio::test]
async fn without_the_email_app_nothing_is_sent_and_the_status_says_so() {
    assert_eq!(mail_events::status().await, MailStatus::default());
    let refused = send_code(ValidJson(code("me@example.com", "verify", "012345")))
        .await
        .expect_err("no application");
    assert_eq!(refused.0, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        send_code(ValidJson(code("me", "verify", "012345")))
            .await
            .expect_err("invalid")
            .0,
        StatusCode::BAD_REQUEST
    );
}
