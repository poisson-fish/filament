use std::time::Duration;

use axum::{body::Body, http::Request, http::StatusCode};
use filament_server::{build_router, AppConfig};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use tower::ServiceExt;

#[derive(Debug, serde::Deserialize)]
struct AuthResponse {
    access_token: String,
}

#[derive(Debug)]
struct ChannelRef {
    guild_id: String,
    channel_id: String,
}

fn test_app() -> axum::Router {
    build_router(&AppConfig {
        max_body_bytes: 1024 * 64,
        request_timeout: Duration::from_secs(2),
        rate_limit_requests_per_minute: 200,
        auth_route_requests_per_minute: 200,
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
        .body(Body::from(json!({"name":"Phase 3 Guild"}).to_string()))
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
        .body(Body::from(json!({"name":"search"}).to_string()))
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

async fn create_message(
    app: &axum::Router,
    auth: &AuthResponse,
    channel: &ChannelRef,
    ip: &str,
    content: &str,
) -> String {
    let create_message = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/messages",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(json!({"content":content}).to_string()))
        .expect("message create request should build");
    let response = app
        .clone()
        .oneshot(create_message)
        .await
        .expect("message create request should execute");
    assert_eq!(response.status(), StatusCode::OK);
    let json: Value = parse_json_body(response).await;
    json["message_id"].as_str().unwrap().to_owned()
}

async fn search(app: &axum::Router, auth: &AuthResponse, guild_id: &str, q: &str) -> Value {
    let request = Request::builder()
        .method("GET")
        .uri(format!("/guilds/{guild_id}/search?q={q}"))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", "203.0.113.80")
        .body(Body::empty())
        .expect("search request should build");
    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("search request should execute");
    assert_eq!(response.status(), StatusCode::OK);
    parse_json_body(response).await
}

async fn reconcile(app: &axum::Router, auth: &AuthResponse, guild_id: &str) -> Value {
    let request = Request::builder()
        .method("POST")
        .uri(format!("/guilds/{guild_id}/search/reconcile"))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", "203.0.113.80")
        .body(Body::empty())
        .expect("reconcile request should build");
    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("reconcile request should execute");
    assert_eq!(response.status(), StatusCode::OK);
    parse_json_body(response).await
}

#[tokio::test]
async fn search_indexes_create_edit_delete_and_rebuild_paths() {
    let app = test_app();
    let auth = register_and_login(&app, "phase3_owner", "203.0.113.80").await;
    let channel = create_channel_context(&app, &auth, "203.0.113.80").await;

    let first = create_message(
        &app,
        &auth,
        &channel,
        "203.0.113.80",
        "alpha needle message",
    )
    .await;
    let second = create_message(&app, &auth, &channel, "203.0.113.80", "beta needle message").await;

    let initial = search(&app, &auth, &channel.guild_id, "needle").await;
    let mut initial_ids: Vec<String> = initial["message_ids"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|id| id.as_str().map(ToOwned::to_owned))
        .collect();
    initial_ids.sort_unstable();
    assert_eq!(initial_ids, {
        let mut expected = vec![first.clone(), second.clone()];
        expected.sort_unstable();
        expected
    });

    let edit = Request::builder()
        .method("PATCH")
        .uri(format!(
            "/guilds/{}/channels/{}/messages/{}",
            channel.guild_id, channel.channel_id, first
        ))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.80")
        .body(Body::from(
            json!({"content":"alpha replaced content"}).to_string(),
        ))
        .expect("edit request should build");
    let edit_response = app.clone().oneshot(edit).await.unwrap();
    assert_eq!(edit_response.status(), StatusCode::OK);

    let after_edit = search(&app, &auth, &channel.guild_id, "needle").await;
    let after_edit_ids: Vec<String> = after_edit["message_ids"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|id| id.as_str().map(ToOwned::to_owned))
        .collect();
    assert_eq!(after_edit_ids, vec![second.clone()]);

    let delete = Request::builder()
        .method("DELETE")
        .uri(format!(
            "/guilds/{}/channels/{}/messages/{}",
            channel.guild_id, channel.channel_id, second
        ))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", "203.0.113.80")
        .body(Body::empty())
        .expect("delete request should build");
    let delete_response = app.clone().oneshot(delete).await.unwrap();
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    let after_delete = search(&app, &auth, &channel.guild_id, "needle").await;
    assert_eq!(after_delete["message_ids"].as_array().unwrap().len(), 0);

    let third = create_message(
        &app,
        &auth,
        &channel,
        "203.0.113.80",
        "gamma rebuild needle",
    )
    .await;
    let rebuild = Request::builder()
        .method("POST")
        .uri(format!("/guilds/{}/search/rebuild", channel.guild_id))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", "203.0.113.80")
        .body(Body::empty())
        .expect("rebuild request should build");
    let rebuild_response = app.clone().oneshot(rebuild).await.unwrap();
    assert_eq!(rebuild_response.status(), StatusCode::NO_CONTENT);

    let after_rebuild = search(&app, &auth, &channel.guild_id, "needle").await;
    let ids_after_rebuild: Vec<String> = after_rebuild["message_ids"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|id| id.as_str().map(ToOwned::to_owned))
        .collect();
    assert_eq!(ids_after_rebuild, vec![third.clone()]);

    let hydrated_ids: Vec<String> = after_rebuild["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|message| message["message_id"].as_str().map(ToOwned::to_owned))
        .collect();
    assert_eq!(hydrated_ids, vec![third]);
}

#[tokio::test]
async fn search_query_abuse_limits_are_enforced() {
    let app = test_app();
    let auth = register_and_login(&app, "phase3_limits", "203.0.113.81").await;
    let channel = create_channel_context(&app, &auth, "203.0.113.81").await;

    let too_long = "a".repeat(257);
    let too_long_request = Request::builder()
        .method("GET")
        .uri(format!("/guilds/{}/search?q={too_long}", channel.guild_id))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", "203.0.113.81")
        .body(Body::empty())
        .expect("too-long search request should build");
    let too_long_response = app.clone().oneshot(too_long_request).await.unwrap();
    assert_eq!(too_long_response.status(), StatusCode::BAD_REQUEST);

    let limit_request = Request::builder()
        .method("GET")
        .uri(format!(
            "/guilds/{}/search?q=hello&limit=51",
            channel.guild_id
        ))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", "203.0.113.81")
        .body(Body::empty())
        .expect("limit search request should build");
    let limit_response = app.clone().oneshot(limit_request).await.unwrap();
    assert_eq!(limit_response.status(), StatusCode::BAD_REQUEST);

    let wildcard_request = Request::builder()
        .method("GET")
        .uri(format!("/guilds/{}/search?q=a*b*c*d*e*", channel.guild_id))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", "203.0.113.81")
        .body(Body::empty())
        .expect("wildcard search request should build");
    let wildcard_response = app.clone().oneshot(wildcard_request).await.unwrap();
    assert_eq!(wildcard_response.status(), StatusCode::BAD_REQUEST);

    let field_query_request = Request::builder()
        .method("GET")
        .uri(format!(
            "/guilds/{}/search?q=content:hello",
            channel.guild_id
        ))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", "203.0.113.81")
        .body(Body::empty())
        .expect("field-query search request should build");
    let field_query_response = app.oneshot(field_query_request).await.unwrap();
    assert_eq!(field_query_response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn search_reconcile_reports_noop_when_index_is_consistent() {
    let app = test_app();
    let auth = register_and_login(&app, "phase3_reconcile_noop", "203.0.113.82").await;
    let channel = create_channel_context(&app, &auth, "203.0.113.82").await;
    let _ = create_message(
        &app,
        &auth,
        &channel,
        "203.0.113.82",
        "phase3 reconcile noop message",
    )
    .await;

    let reconcile_json = reconcile(&app, &auth, &channel.guild_id).await;
    assert_eq!(reconcile_json["upserted"], 0);
    assert_eq!(reconcile_json["deleted"], 0);
}
