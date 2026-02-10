use std::time::Duration;

use axum::{body::Body, http::Request, http::StatusCode};
use filament_server::{build_router, AppConfig, MAX_LIVEKIT_TOKEN_TTL_SECS};
use livekit_api::access_token::TokenVerifier;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use tower::ServiceExt;

const LIVEKIT_KEY: &str = "devkey";
const LIVEKIT_SECRET: &str = "devsecret";
const LIVEKIT_URL: &str = "ws://livekit.test:7880";

#[derive(Debug, serde::Deserialize)]
struct AuthResponse {
    access_token: String,
}

#[derive(Debug, serde::Deserialize)]
struct VoiceTokenResponse {
    token: String,
    livekit_url: String,
    room: String,
    identity: String,
    can_publish: bool,
    can_subscribe: bool,
    publish_sources: Vec<String>,
    expires_in_secs: u64,
}

#[derive(Debug)]
struct ChannelRef {
    guild_id: String,
    channel_id: String,
}

fn test_app(publish_limit: u32, subscribe_cap: usize) -> axum::Router {
    build_router(&AppConfig {
        max_body_bytes: 1024 * 64,
        request_timeout: Duration::from_secs(2),
        rate_limit_requests_per_minute: 200,
        auth_route_requests_per_minute: 200,
        gateway_ingress_events_per_window: 20,
        gateway_ingress_window: Duration::from_secs(10),
        gateway_outbound_queue: 256,
        max_gateway_event_bytes: filament_server::DEFAULT_MAX_GATEWAY_EVENT_BYTES,
        media_token_requests_per_minute: 200,
        media_publish_requests_per_minute: publish_limit,
        media_subscribe_token_cap_per_channel: subscribe_cap,
        livekit_token_ttl: Duration::from_secs(MAX_LIVEKIT_TOKEN_TTL_SECS),
        livekit_url: String::from(LIVEKIT_URL),
        livekit_api_key: Some(String::from(LIVEKIT_KEY)),
        livekit_api_secret: Some(String::from(LIVEKIT_SECRET)),
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

async fn user_id_from_me(app: &axum::Router, auth: &AuthResponse, ip: &str) -> String {
    let me = Request::builder()
        .method("GET")
        .uri("/auth/me")
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", ip)
        .body(Body::empty())
        .expect("me request should build");
    let me_response = app.clone().oneshot(me).await.expect("me should execute");
    assert_eq!(me_response.status(), StatusCode::OK);
    let me_json: Value = parse_json_body(me_response).await;
    me_json["user_id"]
        .as_str()
        .expect("user id should exist")
        .to_owned()
}

async fn create_channel_context(
    app: &axum::Router,
    auth: &AuthResponse,
    ip: &str,
    name: &str,
) -> ChannelRef {
    let create_guild = Request::builder()
        .method("POST")
        .uri("/guilds")
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(json!({"name":name}).to_string()))
        .expect("create guild request should build");
    let guild_response = app
        .clone()
        .oneshot(create_guild)
        .await
        .expect("create guild should execute");
    assert_eq!(guild_response.status(), StatusCode::OK);
    let guild_json: Value = parse_json_body(guild_response).await;
    let guild_id = guild_json["guild_id"].as_str().unwrap().to_owned();

    let create_channel = Request::builder()
        .method("POST")
        .uri(format!("/guilds/{guild_id}/channels"))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(json!({"name":"voice-room"}).to_string()))
        .expect("create channel request should build");
    let channel_response = app
        .clone()
        .oneshot(create_channel)
        .await
        .expect("create channel should execute");
    assert_eq!(channel_response.status(), StatusCode::OK);
    let channel_json: Value = parse_json_body(channel_response).await;

    ChannelRef {
        guild_id,
        channel_id: channel_json["channel_id"].as_str().unwrap().to_owned(),
    }
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn media_token_filters_publish_sources_and_enforces_subscribe_permissions() {
    let app = test_app(200, 3);
    let owner = register_and_login(&app, "phase6_owner", "203.0.113.161").await;
    let member = register_and_login(&app, "phase6_member", "203.0.113.162").await;
    let member_id = user_id_from_me(&app, &member, "203.0.113.162").await;
    let channel = create_channel_context(&app, &owner, "203.0.113.161", "Phase 6 Guild").await;

    let add_member = Request::builder()
        .method("POST")
        .uri(format!("/guilds/{}/members/{member_id}", channel.guild_id))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("x-forwarded-for", "203.0.113.161")
        .body(Body::empty())
        .expect("add member request should build");
    let add_member_response = app.clone().oneshot(add_member).await.unwrap();
    assert_eq!(add_member_response.status(), StatusCode::OK);

    let member_request = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/voice/token",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", member.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.162")
        .body(Body::from(
            json!({
                "can_subscribe": true,
                "publish_sources": ["microphone", "camera", "screen_share"]
            })
            .to_string(),
        ))
        .expect("member token request should build");
    let member_response = app.clone().oneshot(member_request).await.unwrap();
    assert_eq!(member_response.status(), StatusCode::OK);
    let member_token: VoiceTokenResponse = parse_json_body(member_response).await;

    assert_eq!(member_token.livekit_url, LIVEKIT_URL);
    assert!(member_token.room.starts_with("filament.voice."));
    assert!(member_token.identity.starts_with("u."));
    assert!(member_token.can_publish);
    assert!(member_token.can_subscribe);
    assert_eq!(member_token.expires_in_secs, MAX_LIVEKIT_TOKEN_TTL_SECS);
    assert_eq!(
        member_token.publish_sources,
        vec![String::from("microphone")]
    );

    let verifier = TokenVerifier::with_api_key(LIVEKIT_KEY, LIVEKIT_SECRET);
    let member_claims = verifier
        .verify(&member_token.token)
        .expect("member token should verify");
    assert!(member_claims.video.can_publish);
    assert!(member_claims.video.can_subscribe);
    assert_eq!(
        member_claims.video.can_publish_sources,
        vec![String::from("microphone")]
    );

    let override_member = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/overrides/member",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.161")
        .body(Body::from(
            json!({
                "allow": ["publish_video", "publish_screen_share"],
                "deny": ["subscribe_streams"]
            })
            .to_string(),
        ))
        .expect("override request should build");
    let override_response = app.clone().oneshot(override_member).await.unwrap();
    assert_eq!(override_response.status(), StatusCode::OK);

    let member_request_after_override = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/voice/token",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", member.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.162")
        .body(Body::from(
            json!({
                "can_subscribe": true,
                "publish_sources": ["microphone", "camera", "screen_share"]
            })
            .to_string(),
        ))
        .expect("member token request after override should build");
    let overridden_response = app
        .clone()
        .oneshot(member_request_after_override)
        .await
        .unwrap();
    assert_eq!(overridden_response.status(), StatusCode::OK);
    let overridden_token: VoiceTokenResponse = parse_json_body(overridden_response).await;

    assert!(overridden_token.can_publish);
    assert!(!overridden_token.can_subscribe);
    assert_eq!(
        overridden_token.publish_sources,
        vec![
            String::from("microphone"),
            String::from("camera"),
            String::from("screen_share")
        ]
    );

    let overridden_claims = verifier
        .verify(&overridden_token.token)
        .expect("overridden token should verify");
    assert!(overridden_claims.video.can_publish);
    assert!(!overridden_claims.video.can_subscribe);
    assert_eq!(
        overridden_claims.video.can_publish_sources,
        vec![
            String::from("microphone"),
            String::from("camera"),
            String::from("screen_share")
        ]
    );
}

#[tokio::test]
async fn media_token_enforces_subscribe_cap_and_video_publish_churn_limit() {
    let app = test_app(1, 1);
    let owner = register_and_login(&app, "phase6_owner_limits", "203.0.113.163").await;
    let channel =
        create_channel_context(&app, &owner, "203.0.113.163", "Phase 6 Limits Guild").await;

    let first_subscribe_only = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/voice/token",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.163")
        .body(Body::from(
            json!({"can_publish": false, "can_subscribe": true}).to_string(),
        ))
        .expect("subscribe token request should build");
    let first_subscribe_response = app.clone().oneshot(first_subscribe_only).await.unwrap();
    assert_eq!(first_subscribe_response.status(), StatusCode::OK);

    let second_subscribe_only = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/voice/token",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.163")
        .body(Body::from(
            json!({"can_publish": false, "can_subscribe": true}).to_string(),
        ))
        .expect("subscribe token request should build");
    let second_subscribe_response = app.clone().oneshot(second_subscribe_only).await.unwrap();
    assert_eq!(
        second_subscribe_response.status(),
        StatusCode::TOO_MANY_REQUESTS
    );

    let first_video_publish = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/voice/token",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.163")
        .body(Body::from(
            json!({"can_subscribe": false, "publish_sources": ["camera"]}).to_string(),
        ))
        .expect("video publish request should build");
    let first_video_response = app.clone().oneshot(first_video_publish).await.unwrap();
    assert_eq!(first_video_response.status(), StatusCode::OK);

    let second_video_publish = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/voice/token",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.163")
        .body(Body::from(
            json!({"can_subscribe": false, "publish_sources": ["camera"]}).to_string(),
        ))
        .expect("video publish request should build");
    let second_video_response = app.clone().oneshot(second_video_publish).await.unwrap();
    assert_eq!(
        second_video_response.status(),
        StatusCode::TOO_MANY_REQUESTS
    );
}
