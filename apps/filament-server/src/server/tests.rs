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

    mod audit;
    mod auth;
    mod directory;
    mod friend;
    mod gateway;
    mod guilds;
    mod ip_ban;
    mod profile;
}
