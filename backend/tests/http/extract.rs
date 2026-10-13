use super::*;
use axum::http::{Request, StatusCode};

async fn reauthenticated(header: Option<&str>) -> Result<Reauthenticated, ApiFailure> {
    let mut request = Request::builder().uri("/api/system/reset");
    if let Some(value) = header {
        request = request.header(REAUTHENTICATED_HEADER, value);
    }
    let (mut parts, ()) = request.body(()).expect("request").into_parts();
    Reauthenticated::from_request_parts(&mut parts, &()).await
}

#[tokio::test]
async fn only_the_confirmation_of_the_gateway_lets_a_request_through() {
    for header in [None, Some("")] {
        let refused = reauthenticated(header).await.expect_err("refused");
        assert_eq!(refused.0, StatusCode::FORBIDDEN);
    }
    let Reauthenticated(user) = reauthenticated(Some("admin")).await.expect("confirmed");
    assert_eq!(user, "admin");
}
