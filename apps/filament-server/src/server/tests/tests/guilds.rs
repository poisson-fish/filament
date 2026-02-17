use super::*;

#[tokio::test]
async fn history_pagination_returns_persisted_messages() {
    let app = build_router(&AppConfig::default()).unwrap();
    let auth = register_and_login(&app, "203.0.113.30").await;

    let create_guild = Request::builder()
        .method("POST")
        .uri("/guilds")
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.30")
        .body(Body::from(json!({"name":"General"}).to_string()))
        .unwrap();
    let guild_response = app.clone().oneshot(create_guild).await.unwrap();
    assert_eq!(guild_response.status(), StatusCode::OK);
    let guild_body = axum::body::to_bytes(guild_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let guild: Value = serde_json::from_slice(&guild_body).unwrap();
    let guild_id = guild["guild_id"].as_str().unwrap().to_owned();

    let create_channel = Request::builder()
        .method("POST")
        .uri(format!("/guilds/{guild_id}/channels"))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.30")
        .body(Body::from(json!({"name":"general-chat"}).to_string()))
        .unwrap();
    let channel_response = app.clone().oneshot(create_channel).await.unwrap();
    assert_eq!(channel_response.status(), StatusCode::OK);
    let channel_body = axum::body::to_bytes(channel_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let channel: Value = serde_json::from_slice(&channel_body).unwrap();
    let channel_id = channel["channel_id"].as_str().unwrap().to_owned();

    for content in ["one", "two", "three"] {
        let create_message = Request::builder()
            .method("POST")
            .uri(format!("/guilds/{guild_id}/channels/{channel_id}/messages"))
            .header("authorization", format!("Bearer {}", auth.access_token))
            .header("content-type", "application/json")
            .header("x-forwarded-for", "203.0.113.30")
            .body(Body::from(json!({"content":content}).to_string()))
            .unwrap();
        let response = app.clone().oneshot(create_message).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    let page_one = Request::builder()
        .method("GET")
        .uri(format!(
            "/guilds/{guild_id}/channels/{channel_id}/messages?limit=2"
        ))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", "203.0.113.30")
        .body(Body::empty())
        .unwrap();
    let page_one_response = app.clone().oneshot(page_one).await.unwrap();
    assert_eq!(page_one_response.status(), StatusCode::OK);
    let page_one_body = axum::body::to_bytes(page_one_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let page_one_json: Value = serde_json::from_slice(&page_one_body).unwrap();
    assert_eq!(page_one_json["messages"][0]["content"], "three");
    assert_eq!(page_one_json["messages"][1]["content"], "two");

    let before = page_one_json["next_before"].as_str().unwrap();
    let page_two = Request::builder()
        .method("GET")
        .uri(format!(
            "/guilds/{guild_id}/channels/{channel_id}/messages?limit=2&before={before}"
        ))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", "203.0.113.30")
        .body(Body::empty())
        .unwrap();
    let page_two_response = app.oneshot(page_two).await.unwrap();
    assert_eq!(page_two_response.status(), StatusCode::OK);
    let page_two_body = axum::body::to_bytes(page_two_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let page_two_json: Value = serde_json::from_slice(&page_two_body).unwrap();
    assert_eq!(page_two_json["messages"][0]["content"], "one");
}

#[tokio::test]
async fn channel_permissions_endpoint_enforces_least_visibility() {
    let app = build_router(&AppConfig::default()).unwrap();
    let owner_auth = register_and_login_as(&app, "owner_ux", "203.0.113.74").await;
    let member_auth = register_and_login_as(&app, "member_ux", "203.0.113.75").await;
    let stranger_auth = register_and_login_as(&app, "stranger_ux", "203.0.113.76").await;
    let guild_id = create_guild_for_test(&app, &owner_auth, "203.0.113.74").await;
    let channel_id = create_channel_for_test(&app, &owner_auth, "203.0.113.74", &guild_id).await;
    let member_user_id = user_id_from_me(&app, &member_auth, "203.0.113.75").await;
    add_member_for_test(
        &app,
        &owner_auth,
        "203.0.113.74",
        &guild_id,
        &member_user_id,
    )
    .await;

    let (owner_status, owner_payload) =
        fetch_self_permissions_for_test(&app, &owner_auth, "203.0.113.74", &guild_id, &channel_id)
            .await;
    assert_eq!(owner_status, StatusCode::OK);
    let owner_permissions_json = owner_payload.unwrap();
    assert_eq!(owner_permissions_json["role"], "owner");
    assert!(owner_permissions_json["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|permission| permission == "manage_roles"));
    assert!(owner_permissions_json["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|permission| permission == "create_message"));

    let (member_status, member_payload) =
        fetch_self_permissions_for_test(&app, &member_auth, "203.0.113.75", &guild_id, &channel_id)
            .await;
    assert_eq!(member_status, StatusCode::OK);
    let member_permissions_json = member_payload.unwrap();
    assert_eq!(member_permissions_json["role"], "member");
    assert!(member_permissions_json["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|permission| permission == "create_message"));
    assert!(!member_permissions_json["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|permission| permission == "manage_roles"));

    deny_member_create_message_for_test(&app, &owner_auth, "203.0.113.74", &guild_id, &channel_id)
        .await;

    let (member_denied_status, _) =
        fetch_self_permissions_for_test(&app, &member_auth, "203.0.113.75", &guild_id, &channel_id)
            .await;
    assert_eq!(member_denied_status, StatusCode::FORBIDDEN);

    let (stranger_status, _) = fetch_self_permissions_for_test(
        &app,
        &stranger_auth,
        "203.0.113.76",
        &guild_id,
        &channel_id,
    )
    .await;
    assert_eq!(stranger_status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn guild_and_channel_list_endpoints_are_member_scoped() {
    let app = build_router(&AppConfig::default()).unwrap();
    let owner_auth = register_and_login_as(&app, "owner_list", "203.0.113.90").await;
    let member_auth = register_and_login_as(&app, "member_list", "203.0.113.91").await;
    let stranger_auth = register_and_login_as(&app, "stranger_list", "203.0.113.92").await;

    let member_user_id = user_id_from_me(&app, &member_auth, "203.0.113.91").await;

    let guild_a = create_guild_for_test(&app, &owner_auth, "203.0.113.90").await;
    let guild_b = create_guild_for_test(&app, &owner_auth, "203.0.113.90").await;
    let channel_a = create_channel_for_test(&app, &owner_auth, "203.0.113.90", &guild_a).await;
    let _channel_b = create_channel_for_test(&app, &owner_auth, "203.0.113.90", &guild_b).await;

    add_member_for_test(&app, &owner_auth, "203.0.113.90", &guild_a, &member_user_id).await;

    let (guild_list_status, guild_list_payload) = authed_json_request(
        &app,
        "GET",
        String::from("/guilds"),
        &member_auth.access_token,
        "203.0.113.91",
        None,
    )
    .await;
    assert_eq!(guild_list_status, StatusCode::OK);
    let guilds = guild_list_payload.unwrap()["guilds"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(guilds.len(), 1);
    assert_eq!(guilds[0]["guild_id"].as_str().unwrap(), guild_a);

    let (channel_list_status, channel_list_payload) = authed_json_request(
        &app,
        "GET",
        format!("/guilds/{guild_a}/channels"),
        &member_auth.access_token,
        "203.0.113.91",
        None,
    )
    .await;
    assert_eq!(channel_list_status, StatusCode::OK);
    let channels = channel_list_payload.unwrap()["channels"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(channels.len(), 1);
    assert_eq!(channels[0]["channel_id"].as_str().unwrap(), channel_a);

    deny_member_create_message_for_test(&app, &owner_auth, "203.0.113.90", &guild_a, &channel_a)
        .await;

    let (restricted_status, restricted_payload) = authed_json_request(
        &app,
        "GET",
        format!("/guilds/{guild_a}/channels"),
        &member_auth.access_token,
        "203.0.113.91",
        None,
    )
    .await;
    assert_eq!(restricted_status, StatusCode::OK);
    assert_eq!(
        restricted_payload.unwrap()["channels"]
            .as_array()
            .unwrap()
            .len(),
        0
    );

    let (stranger_status, _) = authed_json_request(
        &app,
        "GET",
        format!("/guilds/{guild_a}/channels"),
        &stranger_auth.access_token,
        "203.0.113.92",
        None,
    )
    .await;
    assert_eq!(stranger_status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn create_guild_enforces_per_user_creation_limit() {
    let app = build_router(&AppConfig {
        max_created_guilds_per_user: 1,
        ..AppConfig::default()
    })
    .unwrap();
    let auth = register_and_login(&app, "203.0.113.73").await;

    let first_create = Request::builder()
        .method("POST")
        .uri("/guilds")
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.73")
        .body(Body::from(json!({"name":"Alpha"}).to_string()))
        .unwrap();
    let first_response = app.clone().oneshot(first_create).await.unwrap();
    assert_eq!(first_response.status(), StatusCode::OK);

    let second_create = Request::builder()
        .method("POST")
        .uri("/guilds")
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.73")
        .body(Body::from(json!({"name":"Beta"}).to_string()))
        .unwrap();
    let second_response = app.oneshot(second_create).await.unwrap();
    assert_eq!(second_response.status(), StatusCode::FORBIDDEN);
    let body = axum::body::to_bytes(second_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["error"], "guild_creation_limit_reached");
}

#[test]
fn invalid_postgres_url_is_rejected() {
    let result = build_router(&AppConfig {
        database_url: Some(String::from("postgres://bad url")),
        ..AppConfig::default()
    });
    assert!(result.is_err());
}

#[test]
fn zero_created_guild_limit_is_rejected() {
    let result = build_router(&AppConfig {
        max_created_guilds_per_user: 0,
        ..AppConfig::default()
    });
    assert!(result.is_err());
}

#[test]
fn zero_directory_join_per_ip_limit_is_rejected() {
    let result = build_router(&AppConfig {
        directory_join_requests_per_minute_per_ip: 0,
        ..AppConfig::default()
    });
    assert!(result.is_err());
}

#[test]
fn zero_directory_join_per_user_limit_is_rejected() {
    let result = build_router(&AppConfig {
        directory_join_requests_per_minute_per_user: 0,
        ..AppConfig::default()
    });
    assert!(result.is_err());
}

#[test]
fn zero_audit_list_limit_max_is_rejected() {
    let result = build_router(&AppConfig {
        audit_list_limit_max: 0,
        ..AppConfig::default()
    });
    assert!(result.is_err());
}

#[test]
fn zero_guild_ip_ban_max_entries_is_rejected() {
    let result = build_router(&AppConfig {
        guild_ip_ban_max_entries: 0,
        ..AppConfig::default()
    });
    assert!(result.is_err());
}

#[test]
fn partial_hcaptcha_config_is_rejected() {
    let result = build_router(&AppConfig {
        captcha_hcaptcha_site_key: Some(String::from("site")),
        ..AppConfig::default()
    });
    assert!(result.is_err());
}
