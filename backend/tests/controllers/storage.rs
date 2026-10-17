use super::*;
use axum::http::{HeaderValue, StatusCode};

#[test]
fn a_password_or_a_key_needs_the_word_of_the_gateway() {
    let mut headers = HeaderMap::new();
    assert_eq!(secure(&headers).unwrap_err().0, StatusCode::FORBIDDEN);
    headers.insert(SECURE_HEADER, HeaderValue::from_static("0"));
    assert!(secure(&headers).is_err());
    headers.insert(SECURE_HEADER, HeaderValue::from_static("1"));
    assert!(secure(&headers).is_ok());
}
