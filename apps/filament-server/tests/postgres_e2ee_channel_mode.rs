use std::{env, time::Duration};

use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use filament_server::{build_router_with_db_bootstrap, AppConfig};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use sqlx::error::DatabaseError;
use tower::ServiceExt;
use ulid::Ulid;

#[derive(serde::Deserialize)]
struct AuthResponse {
    access_token: String,
}

fn postgres_url() -> Option<String> {
    env::var("FILAMENT_TEST_DATABASE_URL").ok()
}

async fn test_app(database_url: String) -> axum::Router {
    build_router_with_db_bootstrap(&AppConfig {
        database_url: Some(database_url),
        request_timeout: Duration::from_secs(3),
        rate_limit_requests_per_minute: 200,
        auth_route_requests_per_minute: 200,
        ..AppConfig::default()
    })
    .await
    .expect("router should build")
}

async fn send_json(
    app: &axum::Router,
    method: &str,
    path: &str,
    access_token: Option<&str>,
    body: Value,
) -> Response {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(token) = access_token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    app.clone()
        .oneshot(
            builder
                .body(Body::from(body.to_string()))
                .expect("request should build"),
        )
        .await
        .expect("request should execute")
}

async fn parse_json<T: DeserializeOwned>(response: Response) -> T {
    let body = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("response body should be bounded");
    serde_json::from_slice(&body).expect("response should be valid JSON")
}

fn constraint_name(error: &sqlx::Error) -> Option<&str> {
    error
        .as_database_error()
        .and_then(DatabaseError::constraint)
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn encrypted_channel_mode_is_immutable_and_blocks_plaintext_storage() {
    let Some(database_url) = postgres_url() else {
        eprintln!("skipping PostgreSQL channel-mode test: FILAMENT_TEST_DATABASE_URL is unset");
        return;
    };
    let app = test_app(database_url.clone()).await;
    let suffix = Ulid::new().to_string();
    let username = format!("e2ee_{}", suffix[suffix.len() - 10..].to_ascii_lowercase());
    let register = send_json(
        &app,
        "POST",
        "/auth/register",
        None,
        json!({"username": username, "password": "CorrectHorseBatteryStaple!42"}),
    )
    .await;
    assert_eq!(register.status(), StatusCode::CREATED);
    let login = send_json(
        &app,
        "POST",
        "/auth/login",
        None,
        json!({"username": username, "password": "CorrectHorseBatteryStaple!42"}),
    )
    .await;
    assert_eq!(login.status(), StatusCode::OK);
    let auth: AuthResponse = parse_json(login).await;
    let me = send_json(&app, "GET", "/auth/me", Some(&auth.access_token), json!({})).await;
    assert_eq!(me.status(), StatusCode::OK);
    let me: Value = parse_json(me).await;
    let user_id = me["user_id"]
        .as_str()
        .expect("authenticated identity should include user_id");

    let create_guild = send_json(
        &app,
        "POST",
        "/guilds",
        Some(&auth.access_token),
        json!({"name": "Phase 6 Boundary"}),
    )
    .await;
    assert_eq!(create_guild.status(), StatusCode::OK);
    let guild: Value = parse_json(create_guild).await;
    let guild_id = guild["guild_id"]
        .as_str()
        .expect("created guild should include guild_id");

    let rejected = send_json(
        &app,
        "POST",
        &format!("/guilds/{guild_id}/channels"),
        Some(&auth.access_token),
        json!({"name": "sealed", "kind": "text", "channel_type": "encrypted"}),
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::CONFLICT);
    let rejected: Value = parse_json(rejected).await;
    assert_eq!(
        rejected["error"],
        Value::from("e2ee_channel_provisioning_required")
    );

    let plaintext = send_json(
        &app,
        "POST",
        &format!("/guilds/{guild_id}/channels"),
        Some(&auth.access_token),
        json!({"name": "general", "kind": "text"}),
    )
    .await;
    assert_eq!(plaintext.status(), StatusCode::OK);
    let plaintext: Value = parse_json(plaintext).await;
    assert_eq!(plaintext["channel_type"], Value::from("plaintext"));
    let plaintext_channel_id = plaintext["channel_id"]
        .as_str()
        .expect("created channel should include channel_id");

    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("test pool should connect");
    let mode_change = sqlx::query("UPDATE channels SET channel_type = 1 WHERE channel_id = $1")
        .bind(plaintext_channel_id)
        .execute(&pool)
        .await
        .expect_err("channel mode changes must fail");
    assert_eq!(
        constraint_name(&mode_change),
        Some("channels_channel_type_immutable")
    );

    let encrypted_channel_id = Ulid::new().to_string();
    sqlx::query(
        "INSERT INTO channels
            (channel_id, guild_id, name, kind, channel_type, created_at_unix)
         VALUES ($1, $2, 'sealed-fixture', 0, 1, 1)",
    )
    .bind(&encrypted_channel_id)
    .bind(guild_id)
    .execute(&pool)
    .await
    .expect("future atomic provisioning may insert the final encrypted mode");

    let plaintext_message = send_json(
        &app,
        "POST",
        &format!("/guilds/{guild_id}/channels/{encrypted_channel_id}/messages"),
        Some(&auth.access_token),
        json!({"content": "must never reach plaintext storage"}),
    )
    .await;
    assert_eq!(plaintext_message.status(), StatusCode::CONFLICT);
    let plaintext_message: Value = parse_json(plaintext_message).await;
    assert_eq!(
        plaintext_message["error"],
        Value::from("encrypted_channel_requires_e2ee")
    );

    let message_insert = sqlx::query(
        "INSERT INTO messages
            (message_id, guild_id, channel_id, author_id, content, created_at_unix)
         VALUES ($1, $2, $3, $4, 'forbidden', 1)",
    )
    .bind(Ulid::new().to_string())
    .bind(guild_id)
    .bind(&encrypted_channel_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .expect_err("database must reject plaintext message bypasses");
    assert_eq!(
        constraint_name(&message_insert),
        Some("plaintext_storage_requires_plaintext_channel")
    );

    let attachment_insert = sqlx::query(
        "INSERT INTO attachments
            (attachment_id, guild_id, channel_id, owner_id, filename, mime_type,
             size_bytes, sha256_hex, object_key, created_at_unix)
         VALUES ($1, $2, $3, $4, 'forbidden.txt', 'text/plain',
                 1, $5, $6, 1)",
    )
    .bind(Ulid::new().to_string())
    .bind(guild_id)
    .bind(&encrypted_channel_id)
    .bind(user_id)
    .bind("00".repeat(32))
    .bind(format!("phase6-forbidden-{suffix}"))
    .execute(&pool)
    .await
    .expect_err("database must reject plaintext attachment bypasses");
    assert_eq!(
        constraint_name(&attachment_insert),
        Some("plaintext_storage_requires_plaintext_channel")
    );
}
