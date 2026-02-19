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
    expires_in_secs: u64,
}

#[derive(Debug)]
struct ChannelRef {
    guild_id: String,
    channel_id: String,
}

fn test_app(media_limit: u32) -> axum::Router {
    build_router(&AppConfig {
        max_body_bytes: 1024 * 64,
        request_timeout: Duration::from_secs(2),
        rate_limit_requests_per_minute: 200,
        auth_route_requests_per_minute: 200,
        gateway_ingress_events_per_window: 20,
        gateway_ingress_window: Duration::from_secs(10),
        gateway_outbound_queue: 256,
        max_gateway_event_bytes: AppConfig::default().max_gateway_event_bytes,
        media_token_requests_per_minute: media_limit,
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
async fn voice_token_is_scoped_signed_and_short_lived() {
    let app = test_app(20);
    let owner = register_and_login(&app, "phase5_owner", "203.0.113.151").await;
    let channel = create_channel_context(&app, &owner, "203.0.113.151", "Phase 5 Guild").await;

    let token_request = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/voice/token",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.151")
        .body(Body::from(
            json!({"can_publish":true,"can_subscribe":true}).to_string(),
        ))
        .expect("voice token request should build");
    let token_response = app
        .clone()
        .oneshot(token_request)
        .await
        .expect("voice token request should execute");
    assert_eq!(token_response.status(), StatusCode::OK);
    let body: VoiceTokenResponse = parse_json_body(token_response).await;

    assert_eq!(body.livekit_url, LIVEKIT_URL);
    assert!(body.room.starts_with("filament.voice."));
    assert!(body.identity.starts_with("u."));
    assert!(body.can_publish);
    assert!(body.can_subscribe);
    assert_eq!(body.expires_in_secs, MAX_LIVEKIT_TOKEN_TTL_SECS);

    let verifier = TokenVerifier::with_api_key(LIVEKIT_KEY, LIVEKIT_SECRET);
    let claims = verifier.verify(&body.token).expect("token should verify");
    assert_eq!(claims.sub, body.identity);
    assert_eq!(claims.video.room, body.room);
    assert!(claims.video.room_join);
    assert!(claims.video.can_publish);
    assert!(claims.video.can_subscribe);
    assert_eq!(
        claims.video.can_publish_sources,
        vec![String::from("microphone")]
    );
    let token_ttl = u64::try_from(claims.exp.saturating_sub(claims.nbf)).unwrap_or(u64::MAX);
    assert!(token_ttl <= MAX_LIVEKIT_TOKEN_TTL_SECS);
}

#[tokio::test]
async fn voice_token_identity_is_stable_for_same_user_and_channel() {
    let app = test_app(20);
    let owner = register_and_login(&app, "phase5_owner_stable", "203.0.113.155").await;
    let channel = create_channel_context(
        &app,
        &owner,
        "203.0.113.155",
        "Phase 5 Stable Identity Guild",
    )
    .await;

    let build_request = || {
        Request::builder()
            .method("POST")
            .uri(format!(
                "/guilds/{}/channels/{}/voice/token",
                channel.guild_id, channel.channel_id
            ))
            .header("authorization", format!("Bearer {}", owner.access_token))
            .header("content-type", "application/json")
            .header("x-forwarded-for", "203.0.113.155")
            .body(Body::from(json!({}).to_string()))
            .expect("voice token request should build")
    };

    let first_response = app
        .clone()
        .oneshot(build_request())
        .await
        .expect("first voice token request should execute");
    assert_eq!(first_response.status(), StatusCode::OK);
    let first_body: VoiceTokenResponse = parse_json_body(first_response).await;

    let second_response = app
        .clone()
        .oneshot(build_request())
        .await
        .expect("second voice token request should execute");
    assert_eq!(second_response.status(), StatusCode::OK);
    let second_body: VoiceTokenResponse = parse_json_body(second_response).await;

    assert_eq!(first_body.identity, second_body.identity);
}

#[tokio::test]
async fn voice_token_enforces_channel_permissions_and_rate_limits() {
    let app = test_app(1);
    let owner = register_and_login(&app, "phase5_owner2", "203.0.113.152").await;
    let member = register_and_login(&app, "phase5_member2", "203.0.113.153").await;

    let member_id = user_id_from_me(&app, &member, "203.0.113.153").await;
    let channel = create_channel_context(&app, &owner, "203.0.113.152", "Phase 5 Guild B").await;

    let add_member = Request::builder()
        .method("POST")
        .uri(format!("/guilds/{}/members/{member_id}", channel.guild_id))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("x-forwarded-for", "203.0.113.152")
        .body(Body::empty())
        .expect("add member request should build");
    let add_member_response = app.clone().oneshot(add_member).await.unwrap();
    assert_eq!(add_member_response.status(), StatusCode::OK);

    let deny_member_write = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/overrides/member",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.152")
        .body(Body::from(
            json!({"allow":[],"deny":["create_message"]}).to_string(),
        ))
        .expect("override request should build");
    let deny_member_response = app.clone().oneshot(deny_member_write).await.unwrap();
    assert_eq!(deny_member_response.status(), StatusCode::OK);

    let denied_token_request = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/voice/token",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", member.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.153")
        .body(Body::from(json!({}).to_string()))
        .expect("denied voice token request should build");
    let denied_response = app.clone().oneshot(denied_token_request).await.unwrap();
    assert_eq!(denied_response.status(), StatusCode::FORBIDDEN);

    let owner_token_request = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/voice/token",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.152")
        .body(Body::from(json!({}).to_string()))
        .expect("owner voice token request should build");
    let first_owner_response = app.clone().oneshot(owner_token_request).await.unwrap();
    assert_eq!(first_owner_response.status(), StatusCode::OK);

    let owner_rate_limited = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/voice/token",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.152")
        .body(Body::from(json!({}).to_string()))
        .expect("owner rate limited request should build");
    let rate_limited_response = app.clone().oneshot(owner_rate_limited).await.unwrap();
    assert_eq!(
        rate_limited_response.status(),
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[tokio::test]
async fn voice_leave_endpoint_is_available_and_idempotent() {
    let app = test_app(20);
    let owner = register_and_login(&app, "phase5_owner3", "203.0.113.154").await;
    let channel = create_channel_context(&app, &owner, "203.0.113.154", "Phase 5 Guild C").await;

    let issue_token = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/voice/token",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.154")
        .body(Body::from(json!({}).to_string()))
        .expect("voice token request should build");
    let issue_token_response = app.clone().oneshot(issue_token).await.unwrap();
    assert_eq!(issue_token_response.status(), StatusCode::OK);

    let first_leave = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/voice/leave",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("x-forwarded-for", "203.0.113.154")
        .body(Body::empty())
        .expect("voice leave request should build");
    let first_leave_response = app.clone().oneshot(first_leave).await.unwrap();
    assert_eq!(first_leave_response.status(), StatusCode::NO_CONTENT);

    let second_leave = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/voice/leave",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("x-forwarded-for", "203.0.113.154")
        .body(Body::empty())
        .expect("second voice leave request should build");
    let second_leave_response = app.clone().oneshot(second_leave).await.unwrap();
    assert_eq!(second_leave_response.status(), StatusCode::NO_CONTENT);
}
