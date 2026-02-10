use std::time::Duration;

use axum::{body::Body, http::Request, http::StatusCode};
use filament_server::{build_router, AppConfig};
use tower::ServiceExt;

#[tokio::test]
async fn rejects_body_over_limit() {
    let config = AppConfig {
        max_body_bytes: 32,
        request_timeout: Duration::from_secs(1),
        rate_limit_requests_per_minute: 60,
        ..AppConfig::default()
    };
    let app = build_router(&config).unwrap();

    let request = Request::builder()
        .method("POST")
        .uri("/echo")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.7")
        .body(Body::from(
            r#"{"message":"this payload is definitely too large"}"#,
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn times_out_slow_requests() {
    let config = AppConfig {
        max_body_bytes: 1024,
        request_timeout: Duration::from_millis(20),
        rate_limit_requests_per_minute: 60,
        ..AppConfig::default()
    };
    let app = build_router(&config).unwrap();

    let request = Request::builder()
        .method("GET")
        .uri("/slow")
        .header("x-forwarded-for", "203.0.113.8")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
}

#[tokio::test]
async fn rate_limits_per_client_ip() {
    let config = AppConfig {
        max_body_bytes: 1024,
        request_timeout: Duration::from_secs(1),
        rate_limit_requests_per_minute: 2,
        ..AppConfig::default()
    };
    let app = build_router(&config).unwrap();

    let request = |ip: &str| {
        Request::builder()
            .method("GET")
            .uri("/health")
            .header("x-forwarded-for", ip)
            .body(Body::empty())
            .unwrap()
    };

    let first = app.clone().oneshot(request("198.51.100.9")).await.unwrap();
    let second = app.clone().oneshot(request("198.51.100.9")).await.unwrap();
    let third = app.oneshot(request("198.51.100.9")).await.unwrap();

    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(second.status(), StatusCode::OK);
    assert_eq!(third.status(), StatusCode::TOO_MANY_REQUESTS);
}
