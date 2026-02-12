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
        assert_eq!(
            refresh_after_logout_response.status(),
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn profile_update_changes_username_and_about() {
        let app = build_router(&AppConfig::default()).unwrap();
        let auth = register_and_login_as(&app, "profile_owner", "203.0.113.141").await;

        let (update_status, update_payload) = authed_json_request(
            &app,
            "PATCH",
            String::from("/users/me/profile"),
            &auth.access_token,
            "203.0.113.141",
            Some(json!({
                "username":"profile_owner_next",
                "about_markdown":"hello **team**"
            })),
        )
        .await;
        assert_eq!(update_status, StatusCode::OK);
        let updated = update_payload.expect("profile update payload");
        assert_eq!(updated["username"], "profile_owner_next");
        assert_eq!(updated["about_markdown"], "hello **team**");
        assert!(updated["about_markdown_tokens"]
            .as_array()
            .is_some_and(|tokens| !tokens.is_empty()));

        let (me_status, me_payload) = authed_json_request(
            &app,
            "GET",
            String::from("/auth/me"),
            &auth.access_token,
            "203.0.113.141",
            None,
        )
        .await;
        assert_eq!(me_status, StatusCode::OK);
        let me = me_payload.expect("me payload");
        assert_eq!(me["username"], "profile_owner_next");
        assert_eq!(me["about_markdown"], "hello **team**");
        assert!(me["about_markdown_tokens"]
            .as_array()
            .is_some_and(|tokens| !tokens.is_empty()));

        let old_login = Request::builder()
            .method("POST")
            .uri("/auth/login")
            .header("content-type", "application/json")
            .header("x-forwarded-for", "203.0.113.141")
            .body(Body::from(
                json!({"username":"profile_owner","password":"super-secure-password"}).to_string(),
            ))
            .unwrap();
        let old_login_response = app.clone().oneshot(old_login).await.unwrap();
        assert_eq!(old_login_response.status(), StatusCode::UNAUTHORIZED);

        let new_login = Request::builder()
            .method("POST")
            .uri("/auth/login")
            .header("content-type", "application/json")
            .header("x-forwarded-for", "203.0.113.141")
            .body(Body::from(
                json!({"username":"profile_owner_next","password":"super-secure-password"})
                    .to_string(),
            ))
            .unwrap();
        let new_login_response = app.oneshot(new_login).await.unwrap();
        assert_eq!(new_login_response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn profile_avatar_upload_and_download_round_trip() {
        const PNG_1X1: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x04, 0x00, 0x00,
            0x00, 0xB5, 0x1C, 0x0C, 0x02, 0x00, 0x00, 0x00, 0x0B, 0x49, 0x44, 0x41, 0x54, 0x78,
            0xDA, 0x63, 0xFC, 0x5F, 0x0F, 0x00, 0x02, 0x7F, 0x01, 0xF5, 0x87, 0xCB, 0xD9, 0x1F,
            0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];

        let app = build_router(&AppConfig::default()).unwrap();
        let auth = register_and_login_as(&app, "avatar_owner", "203.0.113.142").await;
        let user_id = user_id_from_me(&app, &auth, "203.0.113.142").await;

        let upload = Request::builder()
            .method("POST")
            .uri("/users/me/profile/avatar")
            .header("authorization", format!("Bearer {}", auth.access_token))
            .header("content-type", "image/png")
            .header("x-forwarded-for", "203.0.113.142")
            .body(Body::from(PNG_1X1.to_vec()))
            .unwrap();
        let upload_response = app.clone().oneshot(upload).await.unwrap();
        assert_eq!(upload_response.status(), StatusCode::OK);
        let upload_body = axum::body::to_bytes(upload_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let upload_json: Value = serde_json::from_slice(&upload_body).unwrap();
        assert!(upload_json["avatar_version"]
            .as_i64()
            .is_some_and(|value| value > 0));

        let download = Request::builder()
            .method("GET")
            .uri(format!("/users/{user_id}/avatar"))
            .header("x-forwarded-for", "203.0.113.142")
            .body(Body::empty())
            .unwrap();
        let download_response = app.clone().oneshot(download).await.unwrap();
        assert_eq!(download_response.status(), StatusCode::OK);
        assert_eq!(
            download_response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok()),
            Some("image/png")
        );
        let bytes = axum::body::to_bytes(download_response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(bytes.as_ref(), PNG_1X1);

        let bad_upload = Request::builder()
            .method("POST")
            .uri("/users/me/profile/avatar")
            .header("authorization", format!("Bearer {}", auth.access_token))
            .header("content-type", "text/plain")
            .header("x-forwarded-for", "203.0.113.142")
            .body(Body::from("not-an-image"))
            .unwrap();
        let bad_response = app.oneshot(bad_upload).await.unwrap();
        assert_eq!(bad_response.status(), StatusCode::BAD_REQUEST);
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
            trusted_proxy_cidrs: vec![
                IpNetwork::try_from(String::from("10.0.0.0/8")).expect("valid cidr")
            ],
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

    #[tokio::test]
    async fn gateway_broadcasts_message_to_subscribed_connection() {
        let state = AppState::new(&AppConfig::default()).unwrap();
        let user_id = UserId::new();
        let username = Username::try_from(String::from("alice_1")).unwrap();
        state.users.write().await.insert(
            username.as_str().to_owned(),
            UserRecord {
                id: user_id,
                username: username.clone(),
                about_markdown: String::new(),
                avatar: None,
                avatar_version: 0,
                password_hash: hash_password("super-secure-password").unwrap(),
                failed_logins: 0,
                locked_until_unix: None,
            },
        );
        state
            .user_ids
            .write()
            .await
            .insert(user_id.to_string(), username.as_str().to_owned());

        let guild_id = String::from("g");
        let channel_id = String::from("c");
        let mut guild = GuildRecord {
            name: String::from("Gateway Test"),
            visibility: GuildVisibility::Private,
            created_by_user_id: user_id,
            members: HashMap::new(),
            banned_members: std::collections::HashSet::new(),
            channels: HashMap::new(),
        };
        guild.members.insert(user_id, Role::Owner);
        guild.channels.insert(
            channel_id.clone(),
            ChannelRecord {
                name: String::from("gateway-room"),
                kind: ChannelKind::Text,
                messages: Vec::new(),
                role_overrides: HashMap::new(),
            },
        );
        state.guilds.write().await.insert(guild_id.clone(), guild);

        let (tx, mut rx) = mpsc::channel::<String>(4);
        add_subscription(&state, Uuid::new_v4(), channel_key("g", "c"), tx).await;

        let auth = AuthContext {
            user_id,
            username: username.as_str().to_owned(),
        };
        let result = create_message_internal(
            &state,
            &auth,
            &guild_id,
            &channel_id,
            String::from("hello"),
            Vec::new(),
        )
        .await
        .unwrap();
        assert_eq!(result.content, "hello");

        let event = rx.recv().await.unwrap();
        let value: Value = serde_json::from_str(&event).unwrap();
        assert_eq!(value["t"], "message_create");
        assert_eq!(value["d"]["content"], "hello");
    }

    #[tokio::test]
    async fn channel_broadcast_targets_only_matching_subscription_key() {
        let state = AppState::new(&AppConfig::default()).unwrap();
        let (tx_target, mut rx_target) = mpsc::channel::<String>(2);
        let (tx_other, mut rx_other) = mpsc::channel::<String>(2);
        add_subscription(
            &state,
            Uuid::new_v4(),
            channel_key("g", "c-target"),
            tx_target,
        )
        .await;
        add_subscription(
            &state,
            Uuid::new_v4(),
            channel_key("g", "c-other"),
            tx_other,
        )
        .await;

        let event = gateway_events::subscribed("g", "c-target");
        broadcast_channel_event(&state, &channel_key("g", "c-target"), &event).await;

        let target_payload = rx_target.recv().await.expect("target payload");
        let target_value: Value = serde_json::from_str(&target_payload).unwrap();
        assert_eq!(target_value["d"]["channel_id"], "c-target");
        let other_result = tokio::time::timeout(Duration::from_millis(25), rx_other.recv()).await;
        assert!(
            other_result.is_err(),
            "unexpected event on non-target channel"
        );
    }

    #[tokio::test]
    async fn guild_broadcast_delivers_once_per_connection_and_skips_other_guilds() {
        let state = AppState::new(&AppConfig::default()).unwrap();
        let connection_id = Uuid::new_v4();
        let (tx_target, mut rx_target) = mpsc::channel::<String>(4);
        let (tx_other, mut rx_other) = mpsc::channel::<String>(4);
        add_subscription(
            &state,
            connection_id,
            channel_key("g-main", "c-1"),
            tx_target.clone(),
        )
        .await;
        add_subscription(
            &state,
            connection_id,
            channel_key("g-main", "c-2"),
            tx_target,
        )
        .await;
        add_subscription(
            &state,
            Uuid::new_v4(),
            channel_key("g-other", "c-1"),
            tx_other,
        )
        .await;

        let event = gateway_events::presence_update("g-main", UserId::new(), "online");
        broadcast_guild_event(&state, "g-main", &event).await;

        let first = rx_target.recv().await.expect("guild event");
        let value: Value = serde_json::from_str(&first).unwrap();
        assert_eq!(value["d"]["guild_id"], "g-main");
        let duplicate = tokio::time::timeout(Duration::from_millis(25), rx_target.recv()).await;
        assert!(
            duplicate.is_err(),
            "duplicate event delivered for same connection"
        );
        let other = tokio::time::timeout(Duration::from_millis(25), rx_other.recv()).await;
        assert!(other.is_err(), "event delivered to unrelated guild");
    }

    #[tokio::test]
    async fn user_broadcast_targets_only_requested_authenticated_user() {
        let state = AppState::new(&AppConfig::default()).unwrap();
        let user_a = UserId::new();
        let user_b = UserId::new();
        let connection_a1 = Uuid::new_v4();
        let connection_a2 = Uuid::new_v4();
        let connection_b = Uuid::new_v4();
        let (tx_a1, mut rx_a1) = mpsc::channel::<String>(2);
        let (tx_a2, mut rx_a2) = mpsc::channel::<String>(2);
        let (tx_b, mut rx_b) = mpsc::channel::<String>(2);

        state
            .connection_senders
            .write()
            .await
            .insert(connection_a1, tx_a1);
        state
            .connection_senders
            .write()
            .await
            .insert(connection_a2, tx_a2);
        state
            .connection_senders
            .write()
            .await
            .insert(connection_b, tx_b);
        state.connection_presence.write().await.insert(
            connection_a1,
            ConnectionPresence {
                user_id: user_a,
                guild_ids: std::collections::HashSet::new(),
            },
        );
        state.connection_presence.write().await.insert(
            connection_a2,
            ConnectionPresence {
                user_id: user_a,
                guild_ids: std::collections::HashSet::new(),
            },
        );
        state.connection_presence.write().await.insert(
            connection_b,
            ConnectionPresence {
                user_id: user_b,
                guild_ids: std::collections::HashSet::new(),
            },
        );

        let event = gateway_events::ready(user_a);
        broadcast_user_event(&state, user_a, &event).await;

        let payload_a1 = rx_a1.recv().await.expect("first session");
        let payload_a2 = rx_a2.recv().await.expect("second session");
        let value_a1: Value = serde_json::from_str(&payload_a1).unwrap();
        let value_a2: Value = serde_json::from_str(&payload_a2).unwrap();
        assert_eq!(value_a1["d"]["user_id"], user_a.to_string());
        assert_eq!(value_a2["d"]["user_id"], user_a.to_string());
        let other = tokio::time::timeout(Duration::from_millis(25), rx_b.recv()).await;
        assert!(other.is_err(), "user-scoped event leaked to another user");
    }

    #[tokio::test]
    async fn slow_consumer_signal_is_sent_when_outbound_queue_is_full() {
        let state = AppState::new(&AppConfig {
            gateway_outbound_queue: 1,
            ..AppConfig::default()
        })
        .unwrap();

        let connection_id = Uuid::new_v4();
        let (tx, _rx) = mpsc::channel::<String>(1);
        let (control_tx, control_rx) = watch::channel(ConnectionControl::Open);
        state
            .connection_controls
            .write()
            .await
            .insert(connection_id, control_tx);
        state
            .subscriptions
            .write()
            .await
            .entry(channel_key("g", "c"))
            .or_default()
            .insert(connection_id, tx.clone());

        tx.try_send(String::from("first")).unwrap();
        let event = gateway_events::subscribed("g", "c");
        broadcast_channel_event(&state, &channel_key("g", "c"), &event).await;

        assert_eq!(*control_rx.borrow(), ConnectionControl::Close);
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
