use std::{env, time::Duration};

use axum::{body::Body, http::Request, http::StatusCode};
use filament_server::{build_router, AppConfig};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use tower::ServiceExt;
use ulid::Ulid;

#[derive(Debug, serde::Deserialize)]
struct AuthResponse {
    access_token: String,
    refresh_token: String,
}

#[derive(Debug)]
struct ChannelRef {
    guild_id: String,
    channel_id: String,
}

fn postgres_url() -> Option<String> {
    env::var("FILAMENT_TEST_DATABASE_URL").ok()
}

fn test_app(database_url: String) -> axum::Router {
    build_router(&AppConfig {
        max_body_bytes: 1024 * 32,
        request_timeout: Duration::from_secs(2),
        rate_limit_requests_per_minute: 200,
        auth_route_requests_per_minute: 200,
        gateway_ingress_events_per_window: 20,
        gateway_ingress_window: Duration::from_secs(10),
        gateway_outbound_queue: 256,
        max_gateway_event_bytes: filament_server::DEFAULT_MAX_GATEWAY_EVENT_BYTES,
        database_url: Some(database_url),
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

async fn register_user(app: &axum::Router, ip: &str, username: &str, password: &str) -> StatusCode {
    let request = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(
            json!({"username":username,"password":password}).to_string(),
        ))
        .expect("register request should build");
    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("register request should execute");
    response.status()
}

async fn login_user(
    app: &axum::Router,
    ip: &str,
    username: &str,
    password: &str,
) -> axum::response::Response {
    let request = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(
            json!({"username":username,"password":password}).to_string(),
        ))
        .expect("login request should build");
    app.clone()
        .oneshot(request)
        .await
        .expect("login request should execute")
}

async fn assert_enumeration_resistance(
    app: &axum::Router,
    ip: &str,
    suffix: &str,
    username: &str,
    valid_password: &str,
    wrong_password: &str,
) {
    let unknown = login_user(app, ip, &format!("missing_{suffix}"), valid_password).await;
    assert_eq!(unknown.status(), StatusCode::UNAUTHORIZED);
    let unknown_body = axum::body::to_bytes(unknown.into_body(), usize::MAX)
        .await
        .expect("unknown-user body should be readable");

    let bad_password = login_user(app, ip, username, wrong_password).await;
    assert_eq!(bad_password.status(), StatusCode::UNAUTHORIZED);
    let bad_password_body = axum::body::to_bytes(bad_password.into_body(), usize::MAX)
        .await
        .expect("bad-password body should be readable");

    assert_eq!(unknown_body, bad_password_body);
}

async fn assert_auth_rotation_and_replay(
    app: &axum::Router,
    ip: &str,
    username: &str,
    password: &str,
) {
    let login = login_user(app, ip, username, password).await;
    assert_eq!(login.status(), StatusCode::OK);
    let login_body: AuthResponse = parse_json_body(login).await;

    let me = Request::builder()
        .method("GET")
        .uri("/auth/me")
        .header(
            "authorization",
            format!("Bearer {}", login_body.access_token),
        )
        .header("x-forwarded-for", ip)
        .body(Body::empty())
        .expect("me request should build");
    let me_response = app
        .clone()
        .oneshot(me)
        .await
        .expect("me request should execute");
    assert_eq!(me_response.status(), StatusCode::OK);
    let me_body: Value = parse_json_body(me_response).await;
    assert_eq!(me_body["username"], username);

    let refresh = Request::builder()
        .method("POST")
        .uri("/auth/refresh")
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(
            json!({"refresh_token":login_body.refresh_token}).to_string(),
        ))
        .expect("refresh request should build");
    let refresh_response = app
        .clone()
        .oneshot(refresh)
        .await
        .expect("refresh request should execute");
    assert_eq!(refresh_response.status(), StatusCode::OK);
    let rotated: AuthResponse = parse_json_body(refresh_response).await;

    let replay_refresh = Request::builder()
        .method("POST")
        .uri("/auth/refresh")
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(
            json!({"refresh_token":login_body.refresh_token}).to_string(),
        ))
        .expect("replay refresh request should build");
    let replay_response = app
        .clone()
        .oneshot(replay_refresh)
        .await
        .expect("replay refresh request should execute");
    assert_eq!(replay_response.status(), StatusCode::UNAUTHORIZED);

    let logout = Request::builder()
        .method("POST")
        .uri("/auth/logout")
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(
            json!({"refresh_token":rotated.refresh_token}).to_string(),
        ))
        .expect("logout request should build");
    let logout_response = app
        .clone()
        .oneshot(logout)
        .await
        .expect("logout request should execute");
    assert_eq!(logout_response.status(), StatusCode::NO_CONTENT);

    let refresh_after_logout = Request::builder()
        .method("POST")
        .uri("/auth/refresh")
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(
            json!({"refresh_token":rotated.refresh_token}).to_string(),
        ))
        .expect("refresh-after-logout request should build");
    let refresh_after_logout_response = app
        .clone()
        .oneshot(refresh_after_logout)
        .await
        .expect("refresh-after-logout request should execute");
    assert_eq!(
        refresh_after_logout_response.status(),
        StatusCode::UNAUTHORIZED
    );
}

async fn create_channel_context(
    app: &axum::Router,
    auth: &AuthResponse,
    ip: &str,
    guild_name: &str,
) -> ChannelRef {
    let create_guild = Request::builder()
        .method("POST")
        .uri("/guilds")
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(json!({"name":guild_name}).to_string()))
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
        .body(Body::from(json!({"name":"pg-chat"}).to_string()))
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

async fn fetch_me_profile(app: &axum::Router, auth: &AuthResponse, ip: &str) -> Value {
    let me = Request::builder()
        .method("GET")
        .uri("/auth/me")
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", ip)
        .body(Body::empty())
        .expect("me request should build");
    let me_response = app
        .clone()
        .oneshot(me)
        .await
        .expect("me request should execute");
    assert_eq!(me_response.status(), StatusCode::OK);
    parse_json_body(me_response).await
}

async fn assert_message_pagination(
    app: &axum::Router,
    auth: &AuthResponse,
    ip: &str,
    channel_ref: &ChannelRef,
) {
    for content in ["pg-one", "pg-two", "pg-three"] {
        let create_message = Request::builder()
            .method("POST")
            .uri(format!(
                "/guilds/{}/channels/{}/messages",
                channel_ref.guild_id, channel_ref.channel_id
            ))
            .header("authorization", format!("Bearer {}", auth.access_token))
            .header("content-type", "application/json")
            .header("x-forwarded-for", ip)
            .body(Body::from(json!({"content":content}).to_string()))
            .expect("create message request should build");
        let response = app
            .clone()
            .oneshot(create_message)
            .await
            .expect("create message request should execute");
        assert_eq!(response.status(), StatusCode::OK);
    }

    let page_one = Request::builder()
        .method("GET")
        .uri(format!(
            "/guilds/{}/channels/{}/messages?limit=2",
            channel_ref.guild_id, channel_ref.channel_id
        ))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", ip)
        .body(Body::empty())
        .expect("history page one request should build");
    let page_one_response = app
        .clone()
        .oneshot(page_one)
        .await
        .expect("history page one request should execute");
    assert_eq!(page_one_response.status(), StatusCode::OK);
    let page_one_json: Value = parse_json_body(page_one_response).await;
    assert_eq!(page_one_json["messages"][0]["content"], "pg-three");
    assert_eq!(page_one_json["messages"][1]["content"], "pg-two");

    let before = page_one_json["next_before"]
        .as_str()
        .expect("next_before should exist");
    let page_two = Request::builder()
        .method("GET")
        .uri(format!(
            "/guilds/{}/channels/{}/messages?limit=2&before={before}",
            channel_ref.guild_id, channel_ref.channel_id
        ))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", ip)
        .body(Body::empty())
        .expect("history page two request should build");
    let page_two_response = app
        .clone()
        .oneshot(page_two)
        .await
        .expect("history page two request should execute");
    assert_eq!(page_two_response.status(), StatusCode::OK);
    let page_two_json: Value = parse_json_body(page_two_response).await;
    assert_eq!(page_two_json["messages"][0]["content"], "pg-one");
}

#[tokio::test]
async fn postgres_backed_phase1_auth_and_realtime_text_flow() {
    let Some(database_url) = postgres_url() else {
        eprintln!("skipping postgres-backed test: FILAMENT_TEST_DATABASE_URL is unset");
        return;
    };

    let app = test_app(database_url);
    let suffix = Ulid::new().to_string().to_lowercase();
    let username = format!("pg_{}", &suffix[..20]);
    let wrong_password = "definitely-wrong-password";
    let password = "super-secure-password";
    let client_ip = "203.0.113.61";

    assert_eq!(
        register_user(&app, client_ip, &username, password).await,
        StatusCode::OK
    );
    assert_auth_rotation_and_replay(&app, client_ip, &username, password).await;
    assert_enumeration_resistance(
        &app,
        client_ip,
        &suffix,
        &username,
        password,
        wrong_password,
    )
    .await;

    let login_response = login_user(&app, client_ip, &username, password).await;
    assert_eq!(login_response.status(), StatusCode::OK);
    let auth: AuthResponse = parse_json_body(login_response).await;

    let guild_name = format!("PG Guild {suffix}");
    let channel_ref = create_channel_context(&app, &auth, client_ip, &guild_name).await;
    assert_message_pagination(&app, &auth, client_ip, &channel_ref).await;
}

#[tokio::test]
async fn postgres_backed_user_lookup_batch_endpoint_returns_valid_users() {
    let Some(database_url) = postgres_url() else {
        eprintln!("skipping postgres-backed test: FILAMENT_TEST_DATABASE_URL is unset");
        return;
    };

    let app = test_app(database_url);
    let suffix = Ulid::new().to_string().to_lowercase();
    let alice_username = format!("alice_{}", &suffix[..20]);
    let bob_username = format!("bob_{}", &suffix[..20]);
    let password = "super-secure-password";
    let alice_ip = "203.0.113.71";
    let bob_ip = "203.0.113.72";

    assert_eq!(
        register_user(&app, alice_ip, &alice_username, password).await,
        StatusCode::OK
    );
    assert_eq!(
        register_user(&app, bob_ip, &bob_username, password).await,
        StatusCode::OK
    );

    let alice_login = login_user(&app, alice_ip, &alice_username, password).await;
    assert_eq!(alice_login.status(), StatusCode::OK);
    let alice_auth: AuthResponse = parse_json_body(alice_login).await;

    let bob_login = login_user(&app, bob_ip, &bob_username, password).await;
    assert_eq!(bob_login.status(), StatusCode::OK);
    let bob_auth: AuthResponse = parse_json_body(bob_login).await;

    let alice_profile = fetch_me_profile(&app, &alice_auth, alice_ip).await;
    let bob_profile = fetch_me_profile(&app, &bob_auth, bob_ip).await;
    let missing_user_id = Ulid::new().to_string();

    let lookup_request = Request::builder()
        .method("POST")
        .uri("/users/lookup")
        .header(
            "authorization",
            format!("Bearer {}", alice_auth.access_token),
        )
        .header("content-type", "application/json")
        .header("x-forwarded-for", alice_ip)
        .body(Body::from(
            json!({
                "user_ids": [
                    alice_profile["user_id"].as_str().expect("alice user id should exist"),
                    bob_profile["user_id"].as_str().expect("bob user id should exist"),
                    bob_profile["user_id"].as_str().expect("bob user id should exist"),
                    missing_user_id
                ]
            })
            .to_string(),
        ))
        .expect("lookup request should build");
    let lookup_response = app
        .clone()
        .oneshot(lookup_request)
        .await
        .expect("lookup request should execute");
    assert_eq!(lookup_response.status(), StatusCode::OK);
    let lookup_json: Value = parse_json_body(lookup_response).await;
    let users = lookup_json["users"]
        .as_array()
        .expect("users should be an array");
    assert_eq!(users.len(), 2);
    assert_eq!(users[0]["username"], alice_username);
    assert_eq!(users[1]["username"], bob_username);

    let unauthorized_request = Request::builder()
        .method("POST")
        .uri("/users/lookup")
        .header("content-type", "application/json")
        .header("x-forwarded-for", alice_ip)
        .body(Body::from(
            json!({"user_ids":[alice_profile["user_id"].clone()]}).to_string(),
        ))
        .expect("unauthorized lookup request should build");
    let unauthorized_response = app
        .clone()
        .oneshot(unauthorized_request)
        .await
        .expect("unauthorized lookup should execute");
    assert_eq!(unauthorized_response.status(), StatusCode::UNAUTHORIZED);
}
