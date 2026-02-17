#[cfg(test)]
mod tests {
    use super::super::{
        auth::{channel_key, hash_password},
        core::{
            AppConfig, AppState, AuthContext, ChannelRecord, ConnectionControl, ConnectionPresence,
            GuildRecord, GuildVisibility, UserRecord, DEFAULT_MAX_GATEWAY_EVENT_BYTES,
        },
        directory_contract::IpNetwork,
        gateway_events,
        realtime::{
            add_subscription, broadcast_channel_event, broadcast_guild_event, broadcast_user_event,
            create_message_internal,
        },
        router::build_router,
        types::AuthResponse,
    };
    use axum::{body::Body, extract::connect_info::ConnectInfo, http::Request, http::StatusCode};
    use filament_core::{ChannelKind, Role, UserId, Username};
    use serde_json::{json, Value};
    use std::{collections::HashMap, net::SocketAddr, time::Duration};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::{mpsc, watch};
    use tower::ServiceExt;
    use uuid::Uuid;

    async fn register_and_login_as(app: &axum::Router, username: &str, ip: &str) -> AuthResponse {
        let register = Request::builder()
            .method("POST")
            .uri("/auth/register")
            .header("content-type", "application/json")
            .header("x-forwarded-for", ip)
            .body(Body::from(
                json!({"username":username,"password":"super-secure-password"}).to_string(),
            ))
            .unwrap();
        let register_response = app.clone().oneshot(register).await.unwrap();
        assert_eq!(register_response.status(), StatusCode::OK);

        let login = Request::builder()
            .method("POST")
            .uri("/auth/login")
            .header("content-type", "application/json")
            .header("x-forwarded-for", ip)
            .body(Body::from(
                json!({"username":username,"password":"super-secure-password"}).to_string(),
            ))
            .unwrap();
        let login_response = app.clone().oneshot(login).await.unwrap();
        assert_eq!(login_response.status(), StatusCode::OK);
        let login_bytes = axum::body::to_bytes(login_response.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&login_bytes).unwrap()
    }

    async fn register_and_login(app: &axum::Router, ip: &str) -> AuthResponse {
        register_and_login_as(app, "alice_1", ip).await
    }

    async fn spawn_hcaptcha_stub(success: bool) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request_buf = [0_u8; 4096];
            let _ = stream.read(&mut request_buf).await;
            let body = if success {
                r#"{"success":true}"#
            } else {
                r#"{"success":false}"#
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        });
        format!("http://127.0.0.1:{}/siteverify", addr.port())
    }

    async fn authed_json_request(
        app: &axum::Router,
        method: &str,
        uri: String,
        access_token: &str,
        ip: &str,
        body: Option<Value>,
    ) -> (StatusCode, Option<Value>) {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {access_token}"))
            .header("x-forwarded-for", ip);
        if body.is_some() {
            builder = builder.header("content-type", "application/json");
        }
        let request = builder
            .body(match body {
                Some(payload) => Body::from(payload.to_string()),
                None => Body::empty(),
            })
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        if status == StatusCode::NO_CONTENT {
            return (status, None);
        }
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let payload: Value = serde_json::from_slice(&bytes).unwrap();
        (status, Some(payload))
    }

    async fn authed_json_request_with_connect_info(
        app: &axum::Router,
        method: &str,
        uri: String,
        access_token: &str,
        ip: &str,
        body: Option<Value>,
    ) -> (StatusCode, Option<Value>) {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {access_token}"))
            .header("x-forwarded-for", ip);
        if body.is_some() {
            builder = builder.header("content-type", "application/json");
        }
        let request = builder
            .body(match body {
                Some(payload) => Body::from(payload.to_string()),
                None => Body::empty(),
            })
            .unwrap();
        let request = with_connect_info(request, ip);
        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        if status == StatusCode::NO_CONTENT {
            return (status, None);
        }
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let payload: Value = serde_json::from_slice(&bytes).unwrap();
        (status, Some(payload))
    }

    fn with_connect_info(mut request: Request<Body>, peer: &str) -> Request<Body> {
        let socket = format!("{peer}:443")
            .parse::<SocketAddr>()
            .expect("peer socket must parse");
        request.extensions_mut().insert(ConnectInfo(socket));
        request
    }

    async fn user_id_from_me(app: &axum::Router, auth: &AuthResponse, ip: &str) -> String {
        let (status, payload) = authed_json_request(
            app,
            "GET",
            String::from("/auth/me"),
            &auth.access_token,
            ip,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        payload
            .as_ref()
            .and_then(|value| value["user_id"].as_str())
            .unwrap()
            .to_owned()
    }

    async fn create_guild_with_visibility_for_test(
        app: &axum::Router,
        auth: &AuthResponse,
        ip: &str,
        name: &str,
        visibility: &str,
    ) -> String {
        let (status, payload) = authed_json_request(
            app,
            "POST",
            String::from("/guilds"),
            &auth.access_token,
            ip,
            Some(json!({"name":name,"visibility":visibility})),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        payload
            .as_ref()
            .and_then(|value| value["guild_id"].as_str())
            .unwrap()
            .to_owned()
    }

    async fn create_guild_for_test(app: &axum::Router, auth: &AuthResponse, ip: &str) -> String {
        create_guild_with_visibility_for_test(app, auth, ip, "Visibility Test", "private").await
    }

    async fn create_channel_for_test(
        app: &axum::Router,
        auth: &AuthResponse,
        ip: &str,
        guild_id: &str,
    ) -> String {
        let (status, payload) = authed_json_request(
            app,
            "POST",
            format!("/guilds/{guild_id}/channels"),
            &auth.access_token,
            ip,
            Some(json!({"name":"general"})),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        payload
            .as_ref()
            .and_then(|value| value["channel_id"].as_str())
            .unwrap()
            .to_owned()
    }

    async fn add_member_for_test(
        app: &axum::Router,
        auth: &AuthResponse,
        ip: &str,
        guild_id: &str,
        user_id: &str,
    ) {
        let (status, _) = authed_json_request(
            app,
            "POST",
            format!("/guilds/{guild_id}/members/{user_id}"),
            &auth.access_token,
            ip,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    async fn join_public_guild_for_test(
        app: &axum::Router,
        auth: &AuthResponse,
        ip: &str,
        guild_id: &str,
    ) -> (StatusCode, Option<Value>) {
        authed_json_request_with_connect_info(
            app,
            "POST",
            format!("/guilds/{guild_id}/join"),
            &auth.access_token,
            ip,
            None,
        )
        .await
    }

    async fn list_guild_ip_bans_for_test(
        app: &axum::Router,
        auth: &AuthResponse,
        ip: &str,
        guild_id: &str,
        query: Option<&str>,
    ) -> (StatusCode, Option<Value>) {
        let uri = if let Some(query) = query {
            format!("/guilds/{guild_id}/ip-bans?{query}")
        } else {
            format!("/guilds/{guild_id}/ip-bans")
        };
        authed_json_request_with_connect_info(app, "GET", uri, &auth.access_token, ip, None).await
    }

    async fn add_guild_ip_bans_by_user_for_test(
        app: &axum::Router,
        auth: &AuthResponse,
        ip: &str,
        guild_id: &str,
        target_user_id: &str,
        reason: Option<&str>,
        expires_in_secs: Option<u64>,
    ) -> (StatusCode, Option<Value>) {
        let mut body = json!({ "target_user_id": target_user_id });
        if let Some(reason) = reason {
            body["reason"] = json!(reason);
        }
        if let Some(expires_in_secs) = expires_in_secs {
            body["expires_in_secs"] = json!(expires_in_secs);
        }
        authed_json_request_with_connect_info(
            app,
            "POST",
            format!("/guilds/{guild_id}/ip-bans/by-user"),
            &auth.access_token,
            ip,
            Some(body),
        )
        .await
    }

    async fn remove_guild_ip_ban_for_test(
        app: &axum::Router,
        auth: &AuthResponse,
        ip: &str,
        guild_id: &str,
        ban_id: &str,
    ) -> (StatusCode, Option<Value>) {
        authed_json_request_with_connect_info(
            app,
            "DELETE",
            format!("/guilds/{guild_id}/ip-bans/{ban_id}"),
            &auth.access_token,
            ip,
            None,
        )
        .await
    }

    async fn list_guild_audit_for_test(
        app: &axum::Router,
        auth: &AuthResponse,
        ip: &str,
        guild_id: &str,
        query: Option<&str>,
    ) -> (StatusCode, Option<Value>) {
        let uri = if let Some(query) = query {
            format!("/guilds/{guild_id}/audit?{query}")
        } else {
            format!("/guilds/{guild_id}/audit")
        };
        authed_json_request(app, "GET", uri, &auth.access_token, ip, None).await
    }

    async fn create_friend_request_for_test(
        app: &axum::Router,
        auth: &AuthResponse,
        ip: &str,
        recipient_user_id: &str,
    ) -> String {
        let (status, payload) = authed_json_request(
            app,
            "POST",
            String::from("/friends/requests"),
            &auth.access_token,
            ip,
            Some(json!({ "recipient_user_id": recipient_user_id })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        payload
            .as_ref()
            .and_then(|value| value["request_id"].as_str())
            .unwrap()
            .to_owned()
    }

    async fn fetch_self_permissions_for_test(
        app: &axum::Router,
        auth: &AuthResponse,
        ip: &str,
        guild_id: &str,
        channel_id: &str,
    ) -> (StatusCode, Option<Value>) {
        authed_json_request(
            app,
            "GET",
            format!("/guilds/{guild_id}/channels/{channel_id}/permissions/self"),
            &auth.access_token,
            ip,
            None,
        )
        .await
    }

    async fn deny_member_create_message_for_test(
        app: &axum::Router,
        auth: &AuthResponse,
        ip: &str,
        guild_id: &str,
        channel_id: &str,
    ) {
        let (status, _) = authed_json_request(
            app,
            "POST",
            format!("/guilds/{guild_id}/channels/{channel_id}/overrides/member"),
            &auth.access_token,
            ip,
            Some(json!({"allow":[],"deny":["create_message"]})),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    mod auth;
    mod directory;
    mod gateway;
    mod profile;

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
        let channel_id =
            create_channel_for_test(&app, &owner_auth, "203.0.113.74", &guild_id).await;
        let member_user_id = user_id_from_me(&app, &member_auth, "203.0.113.75").await;
        add_member_for_test(
            &app,
            &owner_auth,
            "203.0.113.74",
            &guild_id,
            &member_user_id,
        )
        .await;

        let (owner_status, owner_payload) = fetch_self_permissions_for_test(
            &app,
            &owner_auth,
            "203.0.113.74",
            &guild_id,
            &channel_id,
        )
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

        let (member_status, member_payload) = fetch_self_permissions_for_test(
            &app,
            &member_auth,
            "203.0.113.75",
            &guild_id,
            &channel_id,
        )
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

        deny_member_create_message_for_test(
            &app,
            &owner_auth,
            "203.0.113.74",
            &guild_id,
            &channel_id,
        )
        .await;

        let (member_denied_status, _) = fetch_self_permissions_for_test(
            &app,
            &member_auth,
            "203.0.113.75",
            &guild_id,
            &channel_id,
        )
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

        deny_member_create_message_for_test(
            &app,
            &owner_auth,
            "203.0.113.90",
            &guild_a,
            &channel_a,
        )
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
            list_guild_ip_bans_for_test(&app, &moderator_auth, "203.0.113.241", &guild_id, None)
                .await;
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
            list_guild_ip_bans_for_test(&app, &outsider_auth, "203.0.113.243", &guild_id, None)
                .await;
        assert_eq!(outsider_list_status, StatusCode::FORBIDDEN);
        assert_eq!(
            outsider_list_payload.expect("outsider list payload")["error"],
            "forbidden"
        );

        let (remove_status, remove_payload) = remove_guild_ip_ban_for_test(
            &app,
            &moderator_auth,
            "203.0.113.241",
            &guild_id,
            &ban_id,
        )
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
        let observed_auth =
            register_and_login_as(&app, "observed_join_ip_ban", "203.0.113.246").await;
        let blocked_auth =
            register_and_login_as(&app, "blocked_join_ip_ban", "203.0.113.247").await;

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
        let member_auth =
            register_and_login_as(&app, "member_surface_ip_ban", "203.0.113.249").await;

        let guild_id = create_guild_with_visibility_for_test(
            &app,
            &owner_auth,
            "203.0.113.248",
            "Surface Ban Guild",
            "public",
        )
        .await;
        let channel_id =
            create_channel_for_test(&app, &owner_auth, "203.0.113.248", &guild_id).await;
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

    #[allow(clippy::too_many_lines)]
    #[tokio::test]
    async fn friendship_request_acceptance_and_list_management_work() {
        let app = build_router(&AppConfig::default()).unwrap();
        let alice = register_and_login_as(&app, "alice_friend", "203.0.113.81").await;
        let bob = register_and_login_as(&app, "bob_friend", "203.0.113.82").await;
        let charlie = register_and_login_as(&app, "charlie_friend", "203.0.113.83").await;

        let alice_user_id = user_id_from_me(&app, &alice, "203.0.113.81").await;
        let bob_user_id = user_id_from_me(&app, &bob, "203.0.113.82").await;

        let request_id =
            create_friend_request_for_test(&app, &alice, "203.0.113.81", &bob_user_id).await;

        let (duplicate_status, _) = authed_json_request(
            &app,
            "POST",
            String::from("/friends/requests"),
            &alice.access_token,
            "203.0.113.81",
            Some(json!({ "recipient_user_id": bob_user_id })),
        )
        .await;
        assert_eq!(duplicate_status, StatusCode::BAD_REQUEST);

        let (charlie_accept_status, _) = authed_json_request(
            &app,
            "POST",
            format!("/friends/requests/{request_id}/accept"),
            &charlie.access_token,
            "203.0.113.83",
            None,
        )
        .await;
        assert_eq!(charlie_accept_status, StatusCode::NOT_FOUND);

        let (bob_requests_status, bob_requests_payload) = authed_json_request(
            &app,
            "GET",
            String::from("/friends/requests"),
            &bob.access_token,
            "203.0.113.82",
            None,
        )
        .await;
        assert_eq!(bob_requests_status, StatusCode::OK);
        let bob_requests_payload = bob_requests_payload.unwrap();
        assert_eq!(
            bob_requests_payload["incoming"].as_array().unwrap().len(),
            1
        );
        assert_eq!(
            bob_requests_payload["incoming"][0]["sender_user_id"]
                .as_str()
                .unwrap(),
            alice_user_id
        );

        let (bob_accept_status, _) = authed_json_request(
            &app,
            "POST",
            format!("/friends/requests/{request_id}/accept"),
            &bob.access_token,
            "203.0.113.82",
            None,
        )
        .await;
        assert_eq!(bob_accept_status, StatusCode::OK);

        let (alice_friends_status, alice_friends_payload) = authed_json_request(
            &app,
            "GET",
            String::from("/friends"),
            &alice.access_token,
            "203.0.113.81",
            None,
        )
        .await;
        assert_eq!(alice_friends_status, StatusCode::OK);
        assert_eq!(
            alice_friends_payload.unwrap()["friends"]
                .as_array()
                .unwrap()
                .len(),
            1
        );

        let (bob_friends_status, bob_friends_payload) = authed_json_request(
            &app,
            "GET",
            String::from("/friends"),
            &bob.access_token,
            "203.0.113.82",
            None,
        )
        .await;
        assert_eq!(bob_friends_status, StatusCode::OK);
        assert_eq!(
            bob_friends_payload.unwrap()["friends"][0]["user_id"]
                .as_str()
                .unwrap(),
            alice_user_id
        );

        let (remove_status, _) = authed_json_request(
            &app,
            "DELETE",
            format!("/friends/{bob_user_id}"),
            &alice.access_token,
            "203.0.113.81",
            None,
        )
        .await;
        assert_eq!(remove_status, StatusCode::NO_CONTENT);

        let (alice_empty_status, alice_empty_payload) = authed_json_request(
            &app,
            "GET",
            String::from("/friends"),
            &alice.access_token,
            "203.0.113.81",
            None,
        )
        .await;
        assert_eq!(alice_empty_status, StatusCode::OK);
        assert_eq!(
            alice_empty_payload.unwrap()["friends"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
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
}
