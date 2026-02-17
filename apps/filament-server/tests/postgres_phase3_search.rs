use std::{
    env,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{body::Body, http::Request, http::StatusCode};
use filament_server::{build_router_with_db_bootstrap, AppConfig};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use ulid::Ulid;

#[derive(Debug, serde::Deserialize)]
struct AuthResponse {
    access_token: String,
}

#[derive(Debug)]
struct ChannelRef {
    guild_id: String,
    channel_id: String,
}

fn postgres_url() -> Option<String> {
    env::var("FILAMENT_TEST_DATABASE_URL").ok()
}

async fn test_app(database_url: String) -> axum::Router {
    build_router_with_db_bootstrap(&AppConfig {
        max_body_bytes: 1024 * 64,
        request_timeout: Duration::from_secs(2),
        rate_limit_requests_per_minute: 200,
        auth_route_requests_per_minute: 200,
        database_url: Some(database_url),
        ..AppConfig::default()
    })
    .await
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
        .body(Body::from(
            json!({"name":"Postgres Search Guild"}).to_string(),
        ))
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
        .body(Body::from(json!({"name":"pg-search"}).to_string()))
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
        .header("x-forwarded-for", "203.0.113.91")
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
        .header("x-forwarded-for", "203.0.113.91")
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

async fn current_user_id(app: &axum::Router, auth: &AuthResponse, ip: &str) -> String {
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
    let me_json: Value = parse_json_body(me_response).await;
    me_json["user_id"].as_str().unwrap().to_owned()
}

fn extract_ids(items: &Value, field: &str) -> Vec<String> {
    items[field]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|value| value.as_str().map(ToOwned::to_owned))
        .collect()
}

fn extract_hydrated_ids(items: &Value) -> Vec<String> {
    items["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|message| message["message_id"].as_str().map(ToOwned::to_owned))
        .collect()
}

#[tokio::test]
async fn postgres_search_reconcile_repairs_missing_and_orphan_docs() {
    let Some(database_url) = postgres_url() else {
        eprintln!("skipping postgres-backed search test: FILAMENT_TEST_DATABASE_URL is unset");
        return;
    };

    let app = test_app(database_url.clone()).await;
    let db_pool = PgPool::connect(&database_url)
        .await
        .expect("postgres pool should connect");

    let suffix = Ulid::new().to_string().to_lowercase();
    let username = format!("pg_search_{}", &suffix[..16]);
    let auth = register_and_login(&app, &username, "203.0.113.91").await;
    let author_id = current_user_id(&app, &auth, "203.0.113.91").await;
    let channel = create_channel_context(&app, &auth, "203.0.113.91").await;

    let baseline_id = create_message(
        &app,
        &auth,
        &channel,
        "203.0.113.91",
        "baseline needle message",
    )
    .await;
    let orphan_id = create_message(
        &app,
        &auth,
        &channel,
        "203.0.113.91",
        "orphan needle message",
    )
    .await;

    let before_mutation = search(&app, &auth, &channel.guild_id, "needle").await;
    let before_ids = extract_ids(&before_mutation, "message_ids");
    assert!(before_ids.contains(&baseline_id));
    assert!(before_ids.contains(&orphan_id));

    let missing_id = Ulid::new().to_string();
    let created_at_unix = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be valid")
            .as_secs(),
    )
    .expect("timestamp should fit i64");
    sqlx::query(
        "INSERT INTO messages (message_id, guild_id, channel_id, author_id, content, created_at_unix)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&missing_id)
    .bind(&channel.guild_id)
    .bind(&channel.channel_id)
    .bind(&author_id)
    .bind("postgres-only needle message")
    .bind(created_at_unix)
    .execute(&db_pool)
    .await
    .expect("missing message insert should succeed");

    sqlx::query("DELETE FROM messages WHERE message_id = $1")
        .bind(&orphan_id)
        .execute(&db_pool)
        .await
        .expect("orphan source delete should succeed");

    let drifted = search(&app, &auth, &channel.guild_id, "needle").await;
    let drifted_message_ids = extract_ids(&drifted, "message_ids");
    let drifted_hydrated_ids = extract_hydrated_ids(&drifted);
    assert!(drifted_message_ids.contains(&orphan_id));
    assert!(!drifted_hydrated_ids.contains(&orphan_id));
    assert!(!drifted_message_ids.contains(&missing_id));

    let reconcile_result = reconcile(&app, &auth, &channel.guild_id).await;
    assert_eq!(reconcile_result["upserted"], 1);
    assert_eq!(reconcile_result["deleted"], 1);

    let repaired = search(&app, &auth, &channel.guild_id, "needle").await;
    let repaired_message_ids = extract_ids(&repaired, "message_ids");
    let repaired_hydrated_ids = extract_hydrated_ids(&repaired);
    assert!(repaired_message_ids.contains(&baseline_id));
    assert!(repaired_message_ids.contains(&missing_id));
    assert!(!repaired_message_ids.contains(&orphan_id));
    assert!(repaired_hydrated_ids.contains(&missing_id));
}
