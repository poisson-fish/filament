use super::*;

#[tokio::test]
async fn guild_ip_ban_endpoints_add_list_remove_and_redact_payloads() {
    let app = build_router(&AppConfig::default()).unwrap();
    let owner_auth = register_and_login_as(&app, "owner_ip_ban", "203.0.113.240").await;
    let moderator_auth = register_and_login_as(&app, "moderator_ip_ban", "203.0.113.241").await;
    let member_auth = register_and_login_as(&app, "member_ip_ban", "203.0.113.242").await;
    let outsider_auth = register_and_login_as(&app, "outsider_ip_ban", "203.0.113.243").await;
    let target_auth = register_and_login_as(&app, "target_ip_ban", "203.0.113.244").await;

    let guild_id = create_guild_with_visibility_for_test(
        &app,
        &owner_auth,
        "203.0.113.240",
        "IP Ban Guild",
        "public",
    )
    .await;
    let moderator_user_id = user_id_from_me(&app, &moderator_auth, "203.0.113.241").await;
    let member_user_id = user_id_from_me(&app, &member_auth, "203.0.113.242").await;
    add_member_for_test(
        &app,
        &owner_auth,
        "203.0.113.240",
        &guild_id,
        &moderator_user_id,
    )
    .await;
    let (promote_status, _) = authed_json_request(
        &app,
        "PATCH",
        format!("/guilds/{guild_id}/members/{moderator_user_id}"),
        &owner_auth.access_token,
        "203.0.113.240",
        Some(json!({ "role": "moderator" })),
    )
    .await;
    assert_eq!(promote_status, StatusCode::OK);
    add_member_for_test(
        &app,
        &owner_auth,
        "203.0.113.240",
        &guild_id,
        &member_user_id,
    )
    .await;
    let target_user_id = user_id_from_me(&app, &target_auth, "203.0.113.244").await;

    let (target_join_status, _) =
        join_public_guild_for_test(&app, &target_auth, "198.51.100.44", &guild_id).await;
    assert_eq!(target_join_status, StatusCode::OK);

    let (add_status, add_payload) = add_guild_ip_bans_by_user_for_test(
        &app,
        &owner_auth,
        "203.0.113.240",
        &guild_id,
        &target_user_id,
        Some("repeat raid joins"),
        None,
    )
    .await;
    assert_eq!(add_status, StatusCode::OK);
    let add_payload = add_payload.expect("add payload");
    assert_eq!(add_payload["created_count"], 1);
    let ban_id = add_payload["ban_ids"][0]
        .as_str()
        .expect("ban_id")
        .to_owned();

    let (owner_list_status, owner_list_payload) =
        list_guild_ip_bans_for_test(&app, &owner_auth, "203.0.113.240", &guild_id, None).await;
    assert_eq!(owner_list_status, StatusCode::OK);
    let owner_list_payload = owner_list_payload.expect("owner list payload");
    let owner_bans = owner_list_payload["bans"].as_array().expect("bans array");
    assert_eq!(owner_bans.len(), 1);
    assert!(owner_bans[0].get("ip").is_none());
    assert!(owner_bans[0].get("cidr").is_none());
    assert!(owner_bans[0].get("ip_cidr").is_none());
    assert_eq!(owner_bans[0]["ban_id"], ban_id);
    assert_eq!(owner_bans[0]["source_user_id"], target_user_id);

    let (moderator_list_status, moderator_list_payload) =
        list_guild_ip_bans_for_test(&app, &moderator_auth, "203.0.113.241", &guild_id, None).await;
    assert_eq!(moderator_list_status, StatusCode::OK);
    assert!(moderator_list_payload.is_some());

    let (member_list_status, member_list_payload) =
        list_guild_ip_bans_for_test(&app, &member_auth, "203.0.113.242", &guild_id, None).await;
    assert_eq!(member_list_status, StatusCode::FORBIDDEN);
    assert_eq!(
        member_list_payload.expect("member list payload")["error"],
        "forbidden"
    );

    let (outsider_list_status, outsider_list_payload) =
        list_guild_ip_bans_for_test(&app, &outsider_auth, "203.0.113.243", &guild_id, None).await;
    assert_eq!(outsider_list_status, StatusCode::FORBIDDEN);
    assert_eq!(
        outsider_list_payload.expect("outsider list payload")["error"],
        "forbidden"
    );

    let (remove_status, remove_payload) =
        remove_guild_ip_ban_for_test(&app, &moderator_auth, "203.0.113.241", &guild_id, &ban_id)
            .await;
    assert_eq!(remove_status, StatusCode::OK);
    assert_eq!(remove_payload.expect("remove payload")["accepted"], true);

    let (empty_list_status, empty_list_payload) =
        list_guild_ip_bans_for_test(&app, &owner_auth, "203.0.113.240", &guild_id, None).await;
    assert_eq!(empty_list_status, StatusCode::OK);
    assert_eq!(
        empty_list_payload.expect("empty list payload")["bans"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
}

#[tokio::test]
async fn directory_join_rejects_on_matching_guild_ip_ban() {
    let app = build_router(&AppConfig::default()).unwrap();
    let owner_auth = register_and_login_as(&app, "owner_join_ip_ban", "203.0.113.245").await;
    let observed_auth = register_and_login_as(&app, "observed_join_ip_ban", "203.0.113.246").await;
    let blocked_auth = register_and_login_as(&app, "blocked_join_ip_ban", "203.0.113.247").await;

    let guild_id = create_guild_with_visibility_for_test(
        &app,
        &owner_auth,
        "203.0.113.245",
        "Join IP Ban Guild",
        "public",
    )
    .await;
    let observed_user_id = user_id_from_me(&app, &observed_auth, "203.0.113.246").await;

    let (observed_join_status, _) =
        join_public_guild_for_test(&app, &observed_auth, "198.51.100.46", &guild_id).await;
    assert_eq!(observed_join_status, StatusCode::OK);

    let (add_status, add_payload) = add_guild_ip_bans_by_user_for_test(
        &app,
        &owner_auth,
        "203.0.113.245",
        &guild_id,
        &observed_user_id,
        Some("cross-account join abuse"),
        None,
    )
    .await;
    assert_eq!(add_status, StatusCode::OK);
    assert_eq!(add_payload.expect("add payload")["created_count"], 1);

    let (blocked_join_status, blocked_join_payload) =
        join_public_guild_for_test(&app, &blocked_auth, "198.51.100.46", &guild_id).await;
    assert_eq!(blocked_join_status, StatusCode::FORBIDDEN);
    assert_eq!(
        blocked_join_payload.expect("blocked payload")["error"],
        "directory_join_ip_banned"
    );
}

#[tokio::test]
async fn guild_scoped_endpoints_reject_active_ip_bans_and_allow_after_expiry() {
    let app = build_router(&AppConfig::default()).unwrap();
    let owner_auth = register_and_login_as(&app, "owner_surface_ip_ban", "203.0.113.248").await;
    let member_auth = register_and_login_as(&app, "member_surface_ip_ban", "203.0.113.249").await;

    let guild_id = create_guild_with_visibility_for_test(
        &app,
        &owner_auth,
        "203.0.113.248",
        "Surface Ban Guild",
        "public",
    )
    .await;
    let channel_id = create_channel_for_test(&app, &owner_auth, "203.0.113.248", &guild_id).await;
    let member_user_id = user_id_from_me(&app, &member_auth, "203.0.113.249").await;

    let (join_status, _) =
        join_public_guild_for_test(&app, &member_auth, "198.51.100.49", &guild_id).await;
    assert_eq!(join_status, StatusCode::OK);
    let (add_status, add_payload) = add_guild_ip_bans_by_user_for_test(
        &app,
        &owner_auth,
        "203.0.113.248",
        &guild_id,
        &member_user_id,
        Some("temporary lock"),
        Some(1),
    )
    .await;
    assert_eq!(add_status, StatusCode::OK);
    assert_eq!(add_payload.expect("add payload")["created_count"], 1);

    let (channels_status, channels_payload) = authed_json_request_with_connect_info(
        &app,
        "GET",
        format!("/guilds/{guild_id}/channels"),
        &member_auth.access_token,
        "198.51.100.49",
        None,
    )
    .await;
    assert_eq!(channels_status, StatusCode::FORBIDDEN);
    assert_eq!(
        channels_payload.expect("channels payload")["error"],
        "forbidden"
    );

    let (messages_status, messages_payload) = authed_json_request_with_connect_info(
        &app,
        "GET",
        format!("/guilds/{guild_id}/channels/{channel_id}/messages"),
        &member_auth.access_token,
        "198.51.100.49",
        None,
    )
    .await;
    assert_eq!(messages_status, StatusCode::FORBIDDEN);
    assert_eq!(
        messages_payload.expect("messages payload")["error"],
        "forbidden"
    );

    let (search_status, search_payload) = authed_json_request_with_connect_info(
        &app,
        "GET",
        format!("/guilds/{guild_id}/search?q=hello"),
        &member_auth.access_token,
        "198.51.100.49",
        None,
    )
    .await;
    assert_eq!(search_status, StatusCode::FORBIDDEN);
    assert_eq!(
        search_payload.expect("search payload")["error"],
        "forbidden"
    );

    let (voice_status, voice_payload) = authed_json_request_with_connect_info(
        &app,
        "POST",
        format!("/guilds/{guild_id}/channels/{channel_id}/voice/token"),
        &member_auth.access_token,
        "198.51.100.49",
        Some(json!({ "can_publish": true, "can_subscribe": true })),
    )
    .await;
    assert_eq!(voice_status, StatusCode::FORBIDDEN);
    assert_eq!(voice_payload.expect("voice payload")["error"], "forbidden");

    let (audit_status, audit_payload) =
        list_guild_audit_for_test(&app, &owner_auth, "203.0.113.248", &guild_id, None).await;
    assert_eq!(audit_status, StatusCode::OK);
    let audit_events = audit_payload.expect("audit payload")["events"]
        .as_array()
        .expect("events array")
        .clone();
    assert!(audit_events
        .iter()
        .any(|entry| entry["action"] == "moderation.ip_ban.hit"));

    tokio::time::sleep(Duration::from_secs(2)).await;
    let (post_expiry_status, post_expiry_payload) = authed_json_request_with_connect_info(
        &app,
        "GET",
        format!("/guilds/{guild_id}/channels"),
        &member_auth.access_token,
        "198.51.100.49",
        None,
    )
    .await;
    assert_eq!(post_expiry_status, StatusCode::OK);
    assert!(post_expiry_payload.is_some());
}
