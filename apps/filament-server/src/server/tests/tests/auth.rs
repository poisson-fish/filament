use super::*;

#[tokio::test]
async fn auth_flow_register_login_me_refresh_logout_and_replay_detection() {
    let app = build_router(&AppConfig {
        max_body_bytes: 1024 * 10,
        request_timeout: Duration::from_secs(1),
        rate_limit_requests_per_minute: 200,
        auth_route_requests_per_minute: 200,
        gateway_ingress_events_per_window: 20,
        gateway_ingress_window: Duration::from_secs(10),
        gateway_outbound_queue: 256,
        max_gateway_event_bytes: DEFAULT_MAX_GATEWAY_EVENT_BYTES,
        ..AppConfig::default()
    })
    .unwrap();

    let login_body = register_and_login(&app, "203.0.113.10").await;

    let me = Request::builder()
        .method("GET")
        .uri("/auth/me")
        .header(
            "authorization",
            format!("Bearer {}", login_body.access_token),
        )
        .header("x-forwarded-for", "203.0.113.10")
        .body(Body::empty())
        .unwrap();
    let me_response = app.clone().oneshot(me).await.unwrap();
    assert_eq!(me_response.status(), StatusCode::OK);

    let refresh = Request::builder()
        .method("POST")
        .uri("/auth/refresh")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.10")
        .body(Body::from(
            json!({"refresh_token":login_body.refresh_token}).to_string(),
        ))
        .unwrap();
    let refresh_response = app.clone().oneshot(refresh).await.unwrap();
    assert_eq!(refresh_response.status(), StatusCode::OK);
    let refresh_bytes = axum::body::to_bytes(refresh_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let rotated: AuthResponse = serde_json::from_slice(&refresh_bytes).unwrap();

    let replay_refresh = Request::builder()
        .method("POST")
        .uri("/auth/refresh")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.10")
        .body(Body::from(
            json!({"refresh_token":login_body.refresh_token}).to_string(),
        ))
        .unwrap();
    let replay_response = app.clone().oneshot(replay_refresh).await.unwrap();
    assert_eq!(replay_response.status(), StatusCode::UNAUTHORIZED);

    let logout = Request::builder()
        .method("POST")
        .uri("/auth/logout")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.10")
        .body(Body::from(
            json!({"refresh_token":rotated.refresh_token}).to_string(),
        ))
        .unwrap();
    let logout_response = app.clone().oneshot(logout).await.unwrap();
    assert_eq!(logout_response.status(), StatusCode::NO_CONTENT);

    let refresh_after_logout = Request::builder()
        .method("POST")
        .uri("/auth/refresh")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.10")
        .body(Body::from(
            json!({"refresh_token":rotated.refresh_token}).to_string(),
        ))
        .unwrap();
    let refresh_after_logout_response = app.oneshot(refresh_after_logout).await.unwrap();
    assert_eq!(refresh_after_logout_response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn register_requires_valid_hcaptcha_when_enabled() {
    let verify_url = spawn_hcaptcha_stub(false).await;
    let app = build_router(&AppConfig {
        captcha_hcaptcha_site_key: Some(String::from("10000000-ffff-ffff-ffff-000000000001")),
        captcha_hcaptcha_secret: Some(String::from(
            "0x0000000000000000000000000000000000000000",
        )),
        captcha_verify_url: verify_url,
        ..AppConfig::default()
    })
    .unwrap();

    let missing_token = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.12")
        .body(Body::from(
            json!({"username":"captcha_user","password":"super-secure-password"}).to_string(),
        ))
        .unwrap();
    let missing_response = app.clone().oneshot(missing_token).await.unwrap();
    assert_eq!(missing_response.status(), StatusCode::FORBIDDEN);

    let bad_token = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.12")
        .body(Body::from(
            json!({
                "username":"captcha_user",
                "password":"super-secure-password",
                "captcha_token":"tok_000000000000000000000000000000000000"
            })
            .to_string(),
        ))
        .unwrap();
    let bad_response = app.oneshot(bad_token).await.unwrap();
    assert_eq!(bad_response.status(), StatusCode::FORBIDDEN);
    let bad_body = axum::body::to_bytes(bad_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let bad_json: Value = serde_json::from_slice(&bad_body).unwrap();
    assert_eq!(bad_json["error"], "captcha_failed");
}

#[tokio::test]
async fn register_accepts_valid_hcaptcha_when_enabled() {
    let verify_url = spawn_hcaptcha_stub(true).await;
    let app = build_router(&AppConfig {
        captcha_hcaptcha_site_key: Some(String::from("10000000-ffff-ffff-ffff-000000000001")),
        captcha_hcaptcha_secret: Some(String::from(
            "0x0000000000000000000000000000000000000000",
        )),
        captcha_verify_url: verify_url,
        ..AppConfig::default()
    })
    .unwrap();

    let request = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.13")
        .body(Body::from(
            json!({
                "username":"captcha_ok",
                "password":"super-secure-password",
                "captcha_token":"tok_111111111111111111111111111111111111"
            })
            .to_string(),
        ))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn login_errors_do_not_enumerate_accounts() {
    let app = build_router(&AppConfig {
        max_body_bytes: 1024 * 10,
        request_timeout: Duration::from_secs(1),
        rate_limit_requests_per_minute: 200,
        auth_route_requests_per_minute: 200,
        gateway_ingress_events_per_window: 20,
        gateway_ingress_window: Duration::from_secs(10),
        gateway_outbound_queue: 256,
        max_gateway_event_bytes: DEFAULT_MAX_GATEWAY_EVENT_BYTES,
        ..AppConfig::default()
    })
    .unwrap();

    let unknown_user = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.11")
        .body(Body::from(
            json!({"username":"does_not_exist","password":"super-secure-password"}).to_string(),
        ))
        .unwrap();
    let unknown_user_response = app.clone().oneshot(unknown_user).await.unwrap();
    assert_eq!(unknown_user_response.status(), StatusCode::UNAUTHORIZED);
    let unknown_user_body = axum::body::to_bytes(unknown_user_response.into_body(), usize::MAX)
        .await
        .unwrap();

    let bad_password = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.11")
        .body(Body::from(
            json!({"username":"does_not_exist","password":"wrong-password"}).to_string(),
        ))
        .unwrap();
    let bad_password_response = app.clone().oneshot(bad_password).await.unwrap();
    assert_eq!(bad_password_response.status(), StatusCode::UNAUTHORIZED);
    let bad_password_body = axum::body::to_bytes(bad_password_response.into_body(), usize::MAX)
        .await
        .unwrap();

    assert_eq!(unknown_user_body, bad_password_body);
}

#[tokio::test]
async fn auth_route_limit_is_enforced() {
    let app = build_router(&AppConfig {
        auth_route_requests_per_minute: 2,
        ..AppConfig::default()
    })
    .unwrap();

    for expected in [
        StatusCode::UNAUTHORIZED,
        StatusCode::UNAUTHORIZED,
        StatusCode::TOO_MANY_REQUESTS,
    ] {
        let login = Request::builder()
            .method("POST")
            .uri("/auth/login")
            .header("content-type", "application/json")
            .header("x-forwarded-for", "198.51.100.22")
            .body(Body::from(
                json!({"username":"ghost_user","password":"super-secure-password"}).to_string(),
            ))
            .unwrap();
        let response = app.clone().oneshot(login).await.unwrap();
        assert_eq!(response.status(), expected);
    }
}

#[tokio::test]
async fn auth_rate_limit_ignores_forwarded_headers_when_proxy_is_untrusted() {
    let app = build_router(&AppConfig {
        auth_route_requests_per_minute: 1,
        ..AppConfig::default()
    })
    .unwrap();

    let first = with_connect_info(
        Request::builder()
            .method("POST")
            .uri("/auth/login")
            .header("content-type", "application/json")
            .header("x-forwarded-for", "198.51.100.101")
            .body(Body::from(
                json!({"username":"ghost_user","password":"super-secure-password"}).to_string(),
            ))
            .unwrap(),
        "10.0.0.15",
    );
    let first_response = app.clone().oneshot(first).await.unwrap();
    assert_eq!(first_response.status(), StatusCode::UNAUTHORIZED);

    let second = with_connect_info(
        Request::builder()
            .method("POST")
            .uri("/auth/login")
            .header("content-type", "application/json")
            .header("x-forwarded-for", "198.51.100.102")
            .body(Body::from(
                json!({"username":"ghost_user","password":"super-secure-password"}).to_string(),
            ))
            .unwrap(),
        "10.0.0.15",
    );
    let second_response = app.oneshot(second).await.unwrap();
    assert_eq!(second_response.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn auth_rate_limit_uses_forwarded_headers_for_trusted_proxy_peers() {
    let app = build_router(&AppConfig {
        auth_route_requests_per_minute: 1,
        trusted_proxy_cidrs: vec![IpNetwork::try_from(String::from("10.0.0.0/8")).expect("valid cidr")],
        ..AppConfig::default()
    })
    .unwrap();

    let first = with_connect_info(
        Request::builder()
            .method("POST")
            .uri("/auth/login")
            .header("content-type", "application/json")
            .header("x-forwarded-for", "198.51.100.111")
            .body(Body::from(
                json!({"username":"ghost_user","password":"super-secure-password"}).to_string(),
            ))
            .unwrap(),
        "10.0.0.15",
    );
    let first_response = app.clone().oneshot(first).await.unwrap();
    assert_eq!(first_response.status(), StatusCode::UNAUTHORIZED);

    let second = with_connect_info(
        Request::builder()
            .method("POST")
            .uri("/auth/login")
            .header("content-type", "application/json")
            .header("x-forwarded-for", "198.51.100.112")
            .body(Body::from(
                json!({"username":"ghost_user","password":"super-secure-password"}).to_string(),
            ))
            .unwrap(),
        "10.0.0.15",
    );
    let second_response = app.oneshot(second).await.unwrap();
    assert_eq!(second_response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn metrics_endpoint_exposes_auth_and_rate_limit_counters() {
    let app = build_router(&AppConfig {
        auth_route_requests_per_minute: 1,
        ..AppConfig::default()
    })
    .unwrap();

    let me_request = Request::builder()
        .method("GET")
        .uri("/auth/me")
        .header("x-forwarded-for", "198.51.100.44")
        .body(Body::empty())
        .unwrap();
    let me_response = app.clone().oneshot(me_request).await.unwrap();
    assert_eq!(me_response.status(), StatusCode::UNAUTHORIZED);

    for _ in 0..2 {
        let login = Request::builder()
            .method("POST")
            .uri("/auth/login")
            .header("content-type", "application/json")
            .header("x-forwarded-for", "198.51.100.45")
            .body(Body::from(
                json!({"username":"ghost_user","password":"super-secure-password"}).to_string(),
            ))
            .unwrap();
        let _ = app.clone().oneshot(login).await.unwrap();
    }

    let metrics_request = Request::builder()
        .method("GET")
        .uri("/metrics")
        .header("x-forwarded-for", "198.51.100.46")
        .body(Body::empty())
        .unwrap();
    let metrics_response = app.oneshot(metrics_request).await.unwrap();
    assert_eq!(metrics_response.status(), StatusCode::OK);
    let metrics_body = axum::body::to_bytes(metrics_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let metrics_text = String::from_utf8(metrics_body.to_vec()).unwrap();
    assert!(metrics_text.contains("filament_auth_failures_total"));
    assert!(metrics_text.contains("filament_rate_limit_hits_total"));
    assert!(metrics_text.contains("filament_gateway_events_emitted_total"));
    assert!(metrics_text.contains("filament_gateway_events_dropped_total"));
    assert!(metrics_text.contains("filament_gateway_events_unknown_received_total"));
    assert!(metrics_text.contains("filament_gateway_events_parse_rejected_total"));
    assert!(metrics_text.contains("filament_voice_sync_repairs_total"));
}
