use super::*;

#[tokio::test]
async fn guild_audit_endpoint_enforces_authz_and_returns_redacted_events() {
    let app = build_router(&AppConfig::default()).unwrap();
    let owner_auth = register_and_login_as(&app, "owner_audit", "203.0.113.221").await;
    let moderator_auth = register_and_login_as(&app, "moderator_audit", "203.0.113.222").await;
    let member_auth = register_and_login_as(&app, "member_audit", "203.0.113.223").await;
    let outsider_auth = register_and_login_as(&app, "outsider_audit", "203.0.113.224").await;
    let joiner_auth = register_and_login_as(&app, "joiner_audit", "203.0.113.225").await;

    let guild_id = create_guild_with_visibility_for_test(
        &app,
        &owner_auth,
        "203.0.113.221",
        "Audit Guild",
        "public",
    )
    .await;
    let moderator_user_id = user_id_from_me(&app, &moderator_auth, "203.0.113.222").await;
    let member_user_id = user_id_from_me(&app, &member_auth, "203.0.113.223").await;

    add_member_for_test(
        &app,
        &owner_auth,
        "203.0.113.221",
        &guild_id,
        &moderator_user_id,
    )
    .await;
    let (moderator_role_status, _) = authed_json_request(
        &app,
        "PATCH",
        format!("/guilds/{guild_id}/members/{moderator_user_id}"),
        &owner_auth.access_token,
        "203.0.113.221",
        Some(json!({ "role": "moderator" })),
    )
    .await;
    assert_eq!(moderator_role_status, StatusCode::OK);
    add_member_for_test(
        &app,
        &owner_auth,
        "203.0.113.221",
        &guild_id,
        &member_user_id,
    )
    .await;

    let (join_status, _) =
        join_public_guild_for_test(&app, &joiner_auth, "203.0.113.225", &guild_id).await;
    assert_eq!(join_status, StatusCode::OK);

    let (owner_status, owner_payload) =
        list_guild_audit_for_test(&app, &owner_auth, "203.0.113.221", &guild_id, None).await;
    assert_eq!(owner_status, StatusCode::OK);
    let owner_payload = owner_payload.expect("owner audit payload");
    let owner_events = owner_payload["events"]
        .as_array()
        .expect("events array expected");
    assert!(!owner_events.is_empty());
    assert!(owner_events
        .iter()
        .all(|event| event.get("details").is_none()
            && event.get("ip").is_none()
            && event.get("ip_cidr").is_none()
            && event.get("cidr").is_none()));

    let (moderator_status, moderator_payload) =
        list_guild_audit_for_test(&app, &moderator_auth, "203.0.113.222", &guild_id, None)
            .await;
    assert_eq!(moderator_status, StatusCode::OK);
    assert!(moderator_payload.is_some());

    let (member_status, member_payload) =
        list_guild_audit_for_test(&app, &member_auth, "203.0.113.223", &guild_id, None).await;
    assert_eq!(member_status, StatusCode::FORBIDDEN);
    assert_eq!(
        member_payload.expect("member denial payload")["error"],
        "audit_access_denied"
    );

    let (outsider_status, outsider_payload) =
        list_guild_audit_for_test(&app, &outsider_auth, "203.0.113.224", &guild_id, None).await;
    assert_eq!(outsider_status, StatusCode::FORBIDDEN);
    assert_eq!(
        outsider_payload.expect("outsider denial payload")["error"],
        "audit_access_denied"
    );

    let (unknown_status, unknown_payload) = list_guild_audit_for_test(
        &app,
        &owner_auth,
        "203.0.113.221",
        "01ARZ3NDEKTSV4RRFFQ69G5FB9",
        None,
    )
    .await;
    assert_eq!(unknown_status, StatusCode::NOT_FOUND);
    assert_eq!(
        unknown_payload.expect("not found payload")["error"],
        "not_found"
    );
}

#[tokio::test]
async fn guild_audit_endpoint_supports_action_filter_and_cursor_pagination() {
    let app = build_router(&AppConfig::default()).unwrap();
    let owner_auth = register_and_login_as(&app, "owner_audit_filter", "203.0.113.226").await;
    let accepted_joiner =
        register_and_login_as(&app, "accepted_joiner_audit", "203.0.113.227").await;
    let banned_joiner =
        register_and_login_as(&app, "banned_joiner_audit", "203.0.113.228").await;

    let guild_id = create_guild_with_visibility_for_test(
        &app,
        &owner_auth,
        "203.0.113.226",
        "Audit Filter Guild",
        "public",
    )
    .await;
    let banned_joiner_user_id = user_id_from_me(&app, &banned_joiner, "203.0.113.228").await;

    let (first_join_status, _) =
        join_public_guild_for_test(&app, &accepted_joiner, "203.0.113.227", &guild_id).await;
    assert_eq!(first_join_status, StatusCode::OK);
    let (repeat_join_status, _) =
        join_public_guild_for_test(&app, &accepted_joiner, "203.0.113.227", &guild_id).await;
    assert_eq!(repeat_join_status, StatusCode::OK);

    let (ban_status, _) = authed_json_request(
        &app,
        "POST",
        format!("/guilds/{guild_id}/members/{banned_joiner_user_id}/ban"),
        &owner_auth.access_token,
        "203.0.113.226",
        None,
    )
    .await;
    assert_eq!(ban_status, StatusCode::OK);
    let (banned_join_status, banned_join_payload) =
        join_public_guild_for_test(&app, &banned_joiner, "203.0.113.228", &guild_id).await;
    assert_eq!(banned_join_status, StatusCode::FORBIDDEN);
    assert_eq!(
        banned_join_payload.expect("banned join payload")["error"],
        "directory_join_user_banned"
    );

    let (filtered_status, filtered_payload) = list_guild_audit_for_test(
        &app,
        &owner_auth,
        "203.0.113.226",
        &guild_id,
        Some("action_prefix=directory.join.rejected"),
    )
    .await;
    assert_eq!(filtered_status, StatusCode::OK);
    let filtered_payload = filtered_payload.expect("filtered audit payload");
    let filtered_events = filtered_payload["events"]
        .as_array()
        .expect("filtered events array");
    assert_eq!(filtered_events.len(), 1);
    assert_eq!(
        filtered_events[0]["action"].as_str().unwrap(),
        "directory.join.rejected.user_ban"
    );

    let (page_one_status, page_one_payload) = list_guild_audit_for_test(
        &app,
        &owner_auth,
        "203.0.113.226",
        &guild_id,
        Some("limit=1"),
    )
    .await;
    assert_eq!(page_one_status, StatusCode::OK);
    let page_one_payload = page_one_payload.expect("page one payload");
    let page_one_events = page_one_payload["events"]
        .as_array()
        .expect("page one events");
    assert_eq!(page_one_events.len(), 1);
    let first_audit_id = page_one_events[0]["audit_id"]
        .as_str()
        .expect("audit id")
        .to_owned();
    let next_cursor = page_one_payload["next_cursor"]
        .as_str()
        .expect("next cursor")
        .to_owned();

    let (page_two_status, page_two_payload) = list_guild_audit_for_test(
        &app,
        &owner_auth,
        "203.0.113.226",
        &guild_id,
        Some(&format!("limit=1&cursor={next_cursor}")),
    )
    .await;
    assert_eq!(page_two_status, StatusCode::OK);
    let page_two_payload = page_two_payload.expect("page two payload");
    let page_two_events = page_two_payload["events"]
        .as_array()
        .expect("page two events");
    assert_eq!(page_two_events.len(), 1);
    let second_audit_id = page_two_events[0]["audit_id"]
        .as_str()
        .expect("audit id")
        .to_owned();
    assert_ne!(first_audit_id, second_audit_id);
}

#[tokio::test]
async fn guild_audit_endpoint_rejects_invalid_filters_and_limit_overrides() {
    let app = build_router(&AppConfig {
        audit_list_limit_max: 1,
        ..AppConfig::default()
    })
    .unwrap();
    let owner_auth = register_and_login_as(&app, "owner_audit_limits", "203.0.113.229").await;
    let joiner_auth = register_and_login_as(&app, "joiner_audit_limits", "203.0.113.230").await;

    let guild_id = create_guild_with_visibility_for_test(
        &app,
        &owner_auth,
        "203.0.113.229",
        "Audit Limit Guild",
        "public",
    )
    .await;
    let (join_status, _) =
        join_public_guild_for_test(&app, &joiner_auth, "203.0.113.230", &guild_id).await;
    assert_eq!(join_status, StatusCode::OK);

    let (limit_status, limit_payload) = list_guild_audit_for_test(
        &app,
        &owner_auth,
        "203.0.113.229",
        &guild_id,
        Some("limit=2"),
    )
    .await;
    assert_eq!(limit_status, StatusCode::BAD_REQUEST);
    assert_eq!(
        limit_payload.expect("invalid limit payload")["error"],
        "invalid_request"
    );

    let (prefix_status, prefix_payload) = list_guild_audit_for_test(
        &app,
        &owner_auth,
        "203.0.113.229",
        &guild_id,
        Some("action_prefix=Directory.Join"),
    )
    .await;
    assert_eq!(prefix_status, StatusCode::BAD_REQUEST);
    assert_eq!(
        prefix_payload.expect("invalid prefix payload")["error"],
        "invalid_request"
    );

    let (cursor_status, cursor_payload) = list_guild_audit_for_test(
        &app,
        &owner_auth,
        "203.0.113.229",
        &guild_id,
        Some("cursor=not-a-valid-cursor"),
    )
    .await;
    assert_eq!(cursor_status, StatusCode::BAD_REQUEST);
    assert_eq!(
        cursor_payload.expect("invalid cursor payload")["error"],
        "invalid_request"
    );
}
