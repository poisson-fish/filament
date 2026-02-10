use std::{path::PathBuf, time::Duration};

use axum::{body::Body, http::Request, http::StatusCode};
use filament_server::{build_router, AppConfig};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use tower::ServiceExt;
use ulid::Ulid;

const GIF_1X1: &[u8] = b"GIF89a\x01\x00\x01\x00\x80\x00\x00\x00\x00\x00\xff\xff\xff!\xf9\x04\x01\x00\x00\x00\x00,\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02D\x01\x00;";

#[derive(Debug, serde::Deserialize)]
struct AuthResponse {
    access_token: String,
}

#[derive(Debug)]
struct ChannelRef {
    guild_id: String,
    channel_id: String,
}

fn attachment_root() -> PathBuf {
    std::env::temp_dir().join(format!("filament-test-attachments-{}", Ulid::new()))
}

fn test_app() -> axum::Router {
    build_router(&AppConfig {
        max_body_bytes: 1024 * 64,
        request_timeout: Duration::from_secs(2),
        rate_limit_requests_per_minute: 200,
        auth_route_requests_per_minute: 200,
        max_attachment_bytes: 1024,
        user_attachment_quota_bytes: 64,
        attachment_root: attachment_root(),
        ..AppConfig::default()
    })
    .expect("router should build")
}

async fn parse_json_body<T: DeserializeOwned>(response: axum::response::Response) -> T {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body should be readable");
    serde_json::from_slice(&body).expect("response body should be valid json")
}

async fn register_and_login(app: &axum::Router, username: &str, ip: &str) -> AuthResponse {
    let register = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(
            json!({"username":username,"password":"super-secure-password"}).to_string(),
        ))
        .expect("register request should build");
    let register_response = app
        .clone()
        .oneshot(register)
        .await
        .expect("register request should execute");
    assert_eq!(register_response.status(), StatusCode::OK);

    let login = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(
            json!({"username":username,"password":"super-secure-password"}).to_string(),
        ))
        .expect("login request should build");
    let login_response = app
        .clone()
        .oneshot(login)
        .await
        .expect("login request should execute");
    assert_eq!(login_response.status(), StatusCode::OK);
    parse_json_body(login_response).await
}

async fn create_channel_context(app: &axum::Router, auth: &AuthResponse, ip: &str) -> ChannelRef {
    let create_guild = Request::builder()
        .method("POST")
        .uri("/guilds")
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(json!({"name":"Phase 2 Guild"}).to_string()))
        .expect("create guild request should build");
    let guild_response = app
        .clone()
        .oneshot(create_guild)
        .await
        .expect("create guild request should execute");
    assert_eq!(guild_response.status(), StatusCode::OK);
    let guild_json: Value = parse_json_body(guild_response).await;
    let guild_id = guild_json["guild_id"]
        .as_str()
        .expect("guild id should exist")
        .to_owned();

    let create_channel = Request::builder()
        .method("POST")
        .uri(format!("/guilds/{guild_id}/channels"))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(json!({"name":"uploads"}).to_string()))
        .expect("create channel request should build");
    let channel_response = app
        .clone()
        .oneshot(create_channel)
        .await
        .expect("create channel request should execute");
    assert_eq!(channel_response.status(), StatusCode::OK);
    let channel_json: Value = parse_json_body(channel_response).await;
    let channel_id = channel_json["channel_id"]
        .as_str()
        .expect("channel id should exist")
        .to_owned();

    ChannelRef {
        guild_id,
        channel_id,
    }
}

#[tokio::test]
async fn attachment_flow_enforces_mime_auth_and_quota_with_reclaim() {
    let app = test_app();
    let auth = register_and_login(&app, "phase2_owner", "203.0.113.70").await;
    let channel = create_channel_context(&app, &auth, "203.0.113.70").await;

    let bad_mime = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/attachments?filename=sample.gif",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "text/plain")
        .header("x-forwarded-for", "203.0.113.70")
        .body(Body::from(GIF_1X1.to_vec()))
        .expect("upload request should build");
    let bad_mime_response = app.clone().oneshot(bad_mime).await.unwrap();
    assert_eq!(bad_mime_response.status(), StatusCode::BAD_REQUEST);

    let upload = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/attachments?filename=one.gif",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "image/gif")
        .header("x-forwarded-for", "203.0.113.70")
        .body(Body::from(GIF_1X1.to_vec()))
        .expect("upload request should build");
    let upload_response = app.clone().oneshot(upload).await.unwrap();
    assert_eq!(upload_response.status(), StatusCode::OK);
    let uploaded_json: Value = parse_json_body(upload_response).await;
    let attachment_id = uploaded_json["attachment_id"].as_str().unwrap().to_owned();

    let unauth_download = Request::builder()
        .method("GET")
        .uri(format!(
            "/guilds/{}/channels/{}/attachments/{}",
            channel.guild_id, channel.channel_id, attachment_id
        ))
        .header("x-forwarded-for", "203.0.113.70")
        .body(Body::empty())
        .expect("download request should build");
    let unauth_download_response = app.clone().oneshot(unauth_download).await.unwrap();
    assert_eq!(unauth_download_response.status(), StatusCode::UNAUTHORIZED);

    let auth_download = Request::builder()
        .method("GET")
        .uri(format!(
            "/guilds/{}/channels/{}/attachments/{}",
            channel.guild_id, channel.channel_id, attachment_id
        ))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", "203.0.113.70")
        .body(Body::empty())
        .expect("download request should build");
    let auth_download_response = app.clone().oneshot(auth_download).await.unwrap();
    assert_eq!(auth_download_response.status(), StatusCode::OK);

    let second_upload = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/attachments?filename=two.gif",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "image/gif")
        .header("x-forwarded-for", "203.0.113.70")
        .body(Body::from(GIF_1X1.to_vec()))
        .expect("second upload request should build");
    let second_upload_response = app.clone().oneshot(second_upload).await.unwrap();
    assert_eq!(second_upload_response.status(), StatusCode::CONFLICT);

    let delete = Request::builder()
        .method("DELETE")
        .uri(format!(
            "/guilds/{}/channels/{}/attachments/{}",
            channel.guild_id, channel.channel_id, attachment_id
        ))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", "203.0.113.70")
        .body(Body::empty())
        .expect("delete request should build");
    let delete_response = app.clone().oneshot(delete).await.unwrap();
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    let upload_after_delete = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/attachments?filename=three.gif",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "image/gif")
        .header("x-forwarded-for", "203.0.113.70")
        .body(Body::from(GIF_1X1.to_vec()))
        .expect("upload-after-delete request should build");
    let upload_after_delete_response = app.oneshot(upload_after_delete).await.unwrap();
    assert_eq!(upload_after_delete_response.status(), StatusCode::OK);
}

#[tokio::test]
async fn message_edit_and_delete_preserve_safe_markdown_tokens() {
    let app = test_app();
    let auth = register_and_login(&app, "phase2_editor", "203.0.113.71").await;
    let channel = create_channel_context(&app, &auth, "203.0.113.71").await;

    let create_message = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/messages",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.71")
        .body(Body::from(
            json!({"content":"**hello** [safe](https://example.com) <b>raw</b>"}).to_string(),
        ))
        .expect("create message request should build");
    let create_response = app.clone().oneshot(create_message).await.unwrap();
    assert_eq!(create_response.status(), StatusCode::OK);
    let create_json: Value = parse_json_body(create_response).await;
    let message_id = create_json["message_id"].as_str().unwrap().to_owned();
    assert!(create_json["markdown_tokens"].is_array());

    let edit_message = Request::builder()
        .method("PATCH")
        .uri(format!(
            "/guilds/{}/channels/{}/messages/{}",
            channel.guild_id, channel.channel_id, message_id
        ))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.71")
        .body(Body::from(
            json!({"content":"_edited_ [bad](javascript:alert(1))"}).to_string(),
        ))
        .expect("edit message request should build");
    let edit_response = app.clone().oneshot(edit_message).await.unwrap();
    assert_eq!(edit_response.status(), StatusCode::OK);
    let edit_json: Value = parse_json_body(edit_response).await;
    let link_tokens = edit_json["markdown_tokens"].to_string();
    assert!(!link_tokens.contains("javascript:"));

    let delete_message = Request::builder()
        .method("DELETE")
        .uri(format!(
            "/guilds/{}/channels/{}/messages/{}",
            channel.guild_id, channel.channel_id, message_id
        ))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", "203.0.113.71")
        .body(Body::empty())
        .expect("delete message request should build");
    let delete_response = app.oneshot(delete_message).await.unwrap();
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn moderation_routes_enforce_membership_state() {
    let app = test_app();
    let owner = register_and_login(&app, "phase2_owner_mod", "203.0.113.72").await;
    let target = register_and_login(&app, "phase2_target_mod", "203.0.113.73").await;
    let channel = create_channel_context(&app, &owner, "203.0.113.72").await;

    let target_me = Request::builder()
        .method("GET")
        .uri("/auth/me")
        .header("authorization", format!("Bearer {}", target.access_token))
        .header("x-forwarded-for", "203.0.113.73")
        .body(Body::empty())
        .expect("me request should build");
    let target_me_response = app.clone().oneshot(target_me).await.unwrap();
    assert_eq!(target_me_response.status(), StatusCode::OK);
    let target_me_json: Value = parse_json_body(target_me_response).await;
    let target_user_id = target_me_json["user_id"].as_str().unwrap().to_owned();

    let kick = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/members/{}/kick",
            channel.guild_id, target_user_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("x-forwarded-for", "203.0.113.72")
        .body(Body::empty())
        .expect("kick request should build");
    let kick_response = app.clone().oneshot(kick).await.unwrap();
    assert_eq!(kick_response.status(), StatusCode::NOT_FOUND);

    let ban = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/members/{}/ban",
            channel.guild_id, target_user_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("x-forwarded-for", "203.0.113.72")
        .body(Body::empty())
        .expect("ban request should build");
    let ban_response = app.clone().oneshot(ban).await.unwrap();
    assert_eq!(ban_response.status(), StatusCode::OK);

    let target_message = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/messages",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", target.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.73")
        .body(Body::from(json!({"content":"hi"}).to_string()))
        .expect("message request should build");
    let target_message_response = app.oneshot(target_message).await.unwrap();
    assert_eq!(target_message_response.status(), StatusCode::FORBIDDEN);
}
