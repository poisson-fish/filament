use super::*;

#[tokio::test]
async fn public_guild_discovery_lists_only_public_guilds() {
    let app = build_router(&AppConfig::default()).unwrap();
    let auth = register_and_login(&app, "203.0.113.71").await;

    let create_private = Request::builder()
        .method("POST")
        .uri("/guilds")
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.71")
        .body(Body::from(json!({"name":"Internal Vault"}).to_string()))
        .unwrap();
    let private_response = app.clone().oneshot(create_private).await.unwrap();
    assert_eq!(private_response.status(), StatusCode::OK);

    let create_public = Request::builder()
        .method("POST")
        .uri("/guilds")
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.71")
        .body(Body::from(
            json!({"name":"Public Lobby","visibility":"public"}).to_string(),
        ))
        .unwrap();
    let public_response = app.clone().oneshot(create_public).await.unwrap();
    assert_eq!(public_response.status(), StatusCode::OK);
    let public_body = axum::body::to_bytes(public_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let public_json: Value = serde_json::from_slice(&public_body).unwrap();
    assert_eq!(public_json["visibility"], "public");

    let list_public = Request::builder()
        .method("GET")
        .uri("/guilds/public")
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", "203.0.113.71")
        .body(Body::empty())
        .unwrap();
    let public_list_response = app.clone().oneshot(list_public).await.unwrap();
    assert_eq!(public_list_response.status(), StatusCode::OK);
    let public_list_body = axum::body::to_bytes(public_list_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let public_list_json: Value = serde_json::from_slice(&public_list_body).unwrap();
    assert_eq!(public_list_json["guilds"].as_array().unwrap().len(), 1);
    assert_eq!(public_list_json["guilds"][0]["name"], "Public Lobby");
    assert_eq!(public_list_json["guilds"][0]["visibility"], "public");

    let filtered = Request::builder()
        .method("GET")
        .uri("/guilds/public?q=lobby")
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", "203.0.113.71")
        .body(Body::empty())
        .unwrap();
    let filtered_response = app.clone().oneshot(filtered).await.unwrap();
    assert_eq!(filtered_response.status(), StatusCode::OK);
    let filtered_body = axum::body::to_bytes(filtered_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let filtered_json: Value = serde_json::from_slice(&filtered_body).unwrap();
    assert_eq!(filtered_json["guilds"].as_array().unwrap().len(), 1);

    let unauthenticated = Request::builder()
        .method("GET")
        .uri("/guilds/public")
        .header("x-forwarded-for", "203.0.113.72")
        .body(Body::empty())
        .unwrap();
    let unauthenticated_response = app.oneshot(unauthenticated).await.unwrap();
    assert_eq!(unauthenticated_response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn directory_join_public_success_and_idempotent_repeat() {
    let app = build_router(&AppConfig::default()).unwrap();
    let owner_auth = register_and_login_as(&app, "owner_join", "203.0.113.210").await;
    let joiner_auth = register_and_login_as(&app, "joiner_join", "203.0.113.211").await;
    let guild_id = create_guild_with_visibility_for_test(
        &app,
        &owner_auth,
        "203.0.113.210",
        "Joinable Guild",
        "public",
    )
    .await;

    let (first_status, first_payload) =
        join_public_guild_for_test(&app, &joiner_auth, "203.0.113.211", &guild_id).await;
    assert_eq!(first_status, StatusCode::OK);
    let first_payload = first_payload.expect("directory join payload");
    assert_eq!(first_payload["guild_id"], guild_id);
    assert_eq!(first_payload["outcome"], "accepted");

    let (second_status, second_payload) =
        join_public_guild_for_test(&app, &joiner_auth, "203.0.113.211", &guild_id).await;
    assert_eq!(second_status, StatusCode::OK);
    let second_payload = second_payload.expect("directory join payload");
    assert_eq!(second_payload["guild_id"], guild_id);
    assert_eq!(second_payload["outcome"], "already_member");
}

#[tokio::test]
async fn directory_join_private_workspace_is_rejected_without_visibility_oracle() {
    let app = build_router(&AppConfig::default()).unwrap();
    let owner_auth = register_and_login_as(&app, "owner_private_join", "203.0.113.212").await;
    let joiner_auth = register_and_login_as(&app, "joiner_private_join", "203.0.113.213").await;
    let guild_id = create_guild_with_visibility_for_test(
        &app,
        &owner_auth,
        "203.0.113.212",
        "Private Guild",
        "private",
    )
    .await;

    let (status, payload) =
        join_public_guild_for_test(&app, &joiner_auth, "203.0.113.213", &guild_id).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let payload = payload.expect("not found payload");
    assert_eq!(payload["error"], "not_found");
}

#[tokio::test]
async fn directory_join_rejects_user_level_guild_ban() {
    let app = build_router(&AppConfig::default()).unwrap();
    let owner_auth = register_and_login_as(&app, "owner_ban_join", "203.0.113.214").await;
    let joiner_auth = register_and_login_as(&app, "joiner_ban_join", "203.0.113.215").await;
    let guild_id = create_guild_with_visibility_for_test(
        &app,
        &owner_auth,
        "203.0.113.214",
        "Ban Join Guild",
        "public",
    )
    .await;
    let joiner_user_id = user_id_from_me(&app, &joiner_auth, "203.0.113.215").await;

    let (ban_status, _) = authed_json_request(
        &app,
        "POST",
        format!("/guilds/{guild_id}/members/{joiner_user_id}/ban"),
        &owner_auth.access_token,
        "203.0.113.214",
        None,
    )
    .await;
    assert_eq!(ban_status, StatusCode::OK);

    let (status, payload) =
        join_public_guild_for_test(&app, &joiner_auth, "203.0.113.215", &guild_id).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let payload = payload.expect("forbidden payload");
    assert_eq!(payload["error"], "directory_join_user_banned");
}

#[tokio::test]
async fn directory_join_route_enforces_per_user_rate_limit() {
    let app = build_router(&AppConfig {
        directory_join_requests_per_minute_per_ip: 100,
        directory_join_requests_per_minute_per_user: 2,
        ..AppConfig::default()
    })
    .unwrap();
    let owner_auth = register_and_login_as(&app, "owner_rate_join", "203.0.113.216").await;
    let joiner_auth = register_and_login_as(&app, "joiner_rate_join", "203.0.113.217").await;
    let guild_id = create_guild_with_visibility_for_test(
        &app,
        &owner_auth,
        "203.0.113.216",
        "Rate Limited Guild",
        "public",
    )
    .await;

    let (first_status, _) =
        join_public_guild_for_test(&app, &joiner_auth, "203.0.113.217", &guild_id).await;
    assert_eq!(first_status, StatusCode::OK);
    let (second_status, _) =
        join_public_guild_for_test(&app, &joiner_auth, "203.0.113.217", &guild_id).await;
    assert_eq!(second_status, StatusCode::OK);
    let (third_status, third_payload) =
        join_public_guild_for_test(&app, &joiner_auth, "203.0.113.217", &guild_id).await;
    assert_eq!(third_status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        third_payload.expect("rate limit payload")["error"],
        "rate_limited"
    );
}
