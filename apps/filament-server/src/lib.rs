#![forbid(unsafe_code)]

use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::anyhow;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    body::Bytes,
    extract::{
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade},
        DefaultBodyLimit, Json, Path, Query, State,
    },
    http::{
        header::AUTHORIZATION, header::CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, StatusCode,
    },
    response::{IntoResponse, Response},
    routing::{get, patch, post},
    Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use filament_core::{
    has_permission, tokenize_markdown, ChannelName, GuildName, MarkdownToken, Permission, Role,
    UserId, Username,
};
use filament_protocol::{parse_envelope, Envelope, EventType, PROTOCOL_VERSION};
use futures_util::{SinkExt, StreamExt};
use object_store::{local::LocalFileSystem, path::Path as ObjectPath, ObjectStore};
use pasetors::{
    claims::{Claims, ClaimsValidationRules},
    keys::SymmetricKey,
    local,
    token::UntrustedToken,
    version4::V4,
    Local,
};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use tokio::sync::{mpsc, watch, OnceCell, RwLock};
use tower::ServiceBuilder;
use tower_governor::{
    governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor, GovernorLayer,
};
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use ulid::Ulid;
use uuid::Uuid;

type ChannelSubscriptions = HashMap<Uuid, mpsc::Sender<String>>;
type Subscriptions = HashMap<String, ChannelSubscriptions>;

pub const DEFAULT_JSON_BODY_LIMIT_BYTES: usize = 1_048_576;
pub const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 10;
pub const DEFAULT_RATE_LIMIT_REQUESTS_PER_MINUTE: u32 = 60;
pub const DEFAULT_AUTH_ROUTE_REQUESTS_PER_MINUTE: u32 = 20;
pub const ACCESS_TOKEN_TTL_SECS: i64 = 15 * 60;
pub const REFRESH_TOKEN_TTL_SECS: i64 = 30 * 24 * 60 * 60;
pub const DEFAULT_GATEWAY_INGRESS_EVENTS_PER_WINDOW: u32 = 20;
pub const DEFAULT_GATEWAY_INGRESS_WINDOW_SECS: u64 = 10;
pub const DEFAULT_GATEWAY_OUTBOUND_QUEUE: usize = 256;
pub const DEFAULT_MAX_GATEWAY_EVENT_BYTES: usize = filament_protocol::MAX_EVENT_BYTES;
pub const DEFAULT_MAX_ATTACHMENT_BYTES: usize = 25 * 1024 * 1024;
pub const DEFAULT_USER_ATTACHMENT_QUOTA_BYTES: u64 = 250 * 1024 * 1024;
const LOGIN_LOCK_THRESHOLD: u8 = 5;
const LOGIN_LOCK_SECS: i64 = 30;
const MAX_HISTORY_LIMIT: usize = 100;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub max_body_bytes: usize,
    pub request_timeout: Duration,
    pub rate_limit_requests_per_minute: u32,
    pub auth_route_requests_per_minute: u32,
    pub gateway_ingress_events_per_window: u32,
    pub gateway_ingress_window: Duration,
    pub gateway_outbound_queue: usize,
    pub max_gateway_event_bytes: usize,
    pub max_attachment_bytes: usize,
    pub user_attachment_quota_bytes: u64,
    pub attachment_root: PathBuf,
    pub database_url: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            max_body_bytes: DEFAULT_JSON_BODY_LIMIT_BYTES,
            request_timeout: Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS),
            rate_limit_requests_per_minute: DEFAULT_RATE_LIMIT_REQUESTS_PER_MINUTE,
            auth_route_requests_per_minute: DEFAULT_AUTH_ROUTE_REQUESTS_PER_MINUTE,
            gateway_ingress_events_per_window: DEFAULT_GATEWAY_INGRESS_EVENTS_PER_WINDOW,
            gateway_ingress_window: Duration::from_secs(DEFAULT_GATEWAY_INGRESS_WINDOW_SECS),
            gateway_outbound_queue: DEFAULT_GATEWAY_OUTBOUND_QUEUE,
            max_gateway_event_bytes: DEFAULT_MAX_GATEWAY_EVENT_BYTES,
            max_attachment_bytes: DEFAULT_MAX_ATTACHMENT_BYTES,
            user_attachment_quota_bytes: DEFAULT_USER_ATTACHMENT_QUOTA_BYTES,
            attachment_root: PathBuf::from("./data/attachments"),
            database_url: None,
        }
    }
}

#[derive(Clone)]
struct RuntimeSecurityConfig {
    auth_route_requests_per_minute: u32,
    gateway_ingress_events_per_window: u32,
    gateway_ingress_window: Duration,
    gateway_outbound_queue: usize,
    max_gateway_event_bytes: usize,
    max_attachment_bytes: usize,
    user_attachment_quota_bytes: u64,
}

#[derive(Clone)]
pub struct AppState {
    db_pool: Option<PgPool>,
    db_init: Arc<OnceCell<()>>,
    users: Arc<RwLock<HashMap<String, UserRecord>>>,
    user_ids: Arc<RwLock<HashMap<String, String>>>,
    sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
    used_refresh_tokens: Arc<RwLock<HashMap<[u8; 32], String>>>,
    token_key: Arc<SymmetricKey<V4>>,
    dummy_password_hash: Arc<String>,
    auth_route_hits: Arc<RwLock<HashMap<String, Vec<i64>>>>,
    guilds: Arc<RwLock<HashMap<String, GuildRecord>>>,
    subscriptions: Arc<RwLock<Subscriptions>>,
    connection_controls: Arc<RwLock<HashMap<Uuid, watch::Sender<ConnectionControl>>>>,
    attachment_store: Arc<LocalFileSystem>,
    attachments: Arc<RwLock<HashMap<String, AttachmentRecord>>>,
    audit_logs: Arc<RwLock<Vec<serde_json::Value>>>,
    runtime: Arc<RuntimeSecurityConfig>,
}

impl AppState {
    fn new(config: &AppConfig) -> anyhow::Result<Self> {
        let mut key_bytes = [0_u8; 32];
        OsRng.fill_bytes(&mut key_bytes);
        let token_key = SymmetricKey::<V4>::from(&key_bytes)
            .map_err(|e| anyhow!("token key init failed: {e}"))?;
        let dummy_password_hash = hash_password("filament-dummy-password")?;
        let db_pool = if let Some(database_url) = &config.database_url {
            Some(
                PgPoolOptions::new()
                    .max_connections(10)
                    .connect_lazy(database_url)
                    .map_err(|e| anyhow!("postgres pool init failed: {e}"))?,
            )
        } else {
            None
        };

        std::fs::create_dir_all(&config.attachment_root)
            .map_err(|e| anyhow!("attachment root init failed: {e}"))?;
        let attachment_store = LocalFileSystem::new_with_prefix(&config.attachment_root)
            .map_err(|e| anyhow!("attachment store init failed: {e}"))?;

        Ok(Self {
            db_pool,
            db_init: Arc::new(OnceCell::new()),
            users: Arc::new(RwLock::new(HashMap::new())),
            user_ids: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            used_refresh_tokens: Arc::new(RwLock::new(HashMap::new())),
            token_key: Arc::new(token_key),
            dummy_password_hash: Arc::new(dummy_password_hash),
            auth_route_hits: Arc::new(RwLock::new(HashMap::new())),
            guilds: Arc::new(RwLock::new(HashMap::new())),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            connection_controls: Arc::new(RwLock::new(HashMap::new())),
            attachment_store: Arc::new(attachment_store),
            attachments: Arc::new(RwLock::new(HashMap::new())),
            audit_logs: Arc::new(RwLock::new(Vec::new())),
            runtime: Arc::new(RuntimeSecurityConfig {
                auth_route_requests_per_minute: config.auth_route_requests_per_minute,
                gateway_ingress_events_per_window: config.gateway_ingress_events_per_window,
                gateway_ingress_window: config.gateway_ingress_window,
                gateway_outbound_queue: config.gateway_outbound_queue,
                max_gateway_event_bytes: config.max_gateway_event_bytes,
                max_attachment_bytes: config.max_attachment_bytes,
                user_attachment_quota_bytes: config.user_attachment_quota_bytes,
            }),
        })
    }
}

#[derive(Debug, Clone)]
struct UserRecord {
    id: UserId,
    username: Username,
    password_hash: String,
    failed_logins: u8,
    locked_until_unix: Option<i64>,
}

#[derive(Debug, Clone)]
struct SessionRecord {
    user_id: UserId,
    refresh_token_hash: [u8; 32],
    expires_at_unix: i64,
    revoked: bool,
}

#[derive(Debug, Clone)]
struct GuildRecord {
    members: HashMap<UserId, Role>,
    banned_members: HashSet<UserId>,
    channels: HashMap<String, ChannelRecord>,
}

#[derive(Debug, Clone)]
struct ChannelRecord {
    messages: Vec<MessageRecord>,
}

#[derive(Debug, Clone)]
struct MessageRecord {
    id: String,
    author_id: UserId,
    content: String,
    markdown_tokens: Vec<MarkdownToken>,
    created_at_unix: i64,
}

#[derive(Debug, Clone)]
struct AttachmentRecord {
    guild_id: String,
    channel_id: String,
    owner_id: UserId,
    mime_type: String,
    size_bytes: u64,
    object_key: String,
}

#[derive(Debug, Clone)]
struct AuthContext {
    user_id: UserId,
    username: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionControl {
    Open,
    Close,
}

#[allow(clippy::too_many_lines)]
async fn ensure_db_schema(state: &AppState) -> Result<(), AuthFailure> {
    let Some(pool) = &state.db_pool else {
        return Ok(());
    };

    state
        .db_init
        .get_or_try_init(|| async move {
            sqlx::query(
                "CREATE TABLE IF NOT EXISTS users (
                    user_id TEXT PRIMARY KEY,
                    username TEXT UNIQUE NOT NULL,
                    password_hash TEXT NOT NULL,
                    failed_logins SMALLINT NOT NULL DEFAULT 0,
                    locked_until_unix BIGINT NULL
                )",
            )
            .execute(pool)
            .await?;

            sqlx::query(
                "CREATE TABLE IF NOT EXISTS sessions (
                    session_id TEXT PRIMARY KEY,
                    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    refresh_token_hash BYTEA NOT NULL,
                    expires_at_unix BIGINT NOT NULL,
                    revoked BOOLEAN NOT NULL DEFAULT FALSE
                )",
            )
            .execute(pool)
            .await?;

            sqlx::query(
                "CREATE TABLE IF NOT EXISTS used_refresh_tokens (
                    token_hash BYTEA PRIMARY KEY,
                    session_id TEXT NOT NULL REFERENCES sessions(session_id) ON DELETE CASCADE
                )",
            )
            .execute(pool)
            .await?;

            sqlx::query(
                "CREATE TABLE IF NOT EXISTS guilds (
                    guild_id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    created_at_unix BIGINT NOT NULL
                )",
            )
            .execute(pool)
            .await?;

            sqlx::query(
                "CREATE TABLE IF NOT EXISTS guild_members (
                    guild_id TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
                    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    role SMALLINT NOT NULL,
                    PRIMARY KEY(guild_id, user_id)
                )",
            )
            .execute(pool)
            .await?;

            sqlx::query(
                "CREATE TABLE IF NOT EXISTS channels (
                    channel_id TEXT PRIMARY KEY,
                    guild_id TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
                    name TEXT NOT NULL,
                    created_at_unix BIGINT NOT NULL
                )",
            )
            .execute(pool)
            .await?;

            sqlx::query(
                "CREATE TABLE IF NOT EXISTS messages (
                    message_id TEXT PRIMARY KEY,
                    guild_id TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
                    channel_id TEXT NOT NULL REFERENCES channels(channel_id) ON DELETE CASCADE,
                    author_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    content TEXT NOT NULL,
                    created_at_unix BIGINT NOT NULL
                )",
            )
            .execute(pool)
            .await?;

            sqlx::query(
                "CREATE TABLE IF NOT EXISTS attachments (
                    attachment_id TEXT PRIMARY KEY,
                    guild_id TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
                    channel_id TEXT NOT NULL REFERENCES channels(channel_id) ON DELETE CASCADE,
                    owner_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    filename TEXT NOT NULL,
                    mime_type TEXT NOT NULL,
                    size_bytes BIGINT NOT NULL,
                    sha256_hex TEXT NOT NULL,
                    object_key TEXT NOT NULL UNIQUE,
                    created_at_unix BIGINT NOT NULL
                )",
            )
            .execute(pool)
            .await?;

            sqlx::query(
                "CREATE INDEX IF NOT EXISTS idx_attachments_owner
                    ON attachments(owner_id)",
            )
            .execute(pool)
            .await?;

            sqlx::query(
                "CREATE TABLE IF NOT EXISTS guild_bans (
                    guild_id TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
                    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    banned_by_user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    created_at_unix BIGINT NOT NULL,
                    PRIMARY KEY(guild_id, user_id)
                )",
            )
            .execute(pool)
            .await?;

            sqlx::query(
                "CREATE TABLE IF NOT EXISTS audit_logs (
                    audit_id TEXT PRIMARY KEY,
                    guild_id TEXT NULL,
                    actor_user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    target_user_id TEXT NULL,
                    action TEXT NOT NULL,
                    details_json TEXT NOT NULL,
                    created_at_unix BIGINT NOT NULL
                )",
            )
            .execute(pool)
            .await?;

            sqlx::query(
                "CREATE INDEX IF NOT EXISTS idx_messages_channel_message_id
                    ON messages(channel_id, message_id DESC)",
            )
            .execute(pool)
            .await?;

            Ok::<(), sqlx::Error>(())
        })
        .await
        .map_err(|e| {
            tracing::error!(event = "db.init", error = %e);
            AuthFailure::Internal
        })?;

    Ok(())
}

fn role_to_i16(role: Role) -> i16 {
    match role {
        Role::Owner => 2,
        Role::Moderator => 1,
        Role::Member => 0,
    }
}

fn role_from_i16(value: i16) -> Option<Role> {
    match value {
        2 => Some(Role::Owner),
        1 => Some(Role::Moderator),
        0 => Some(Role::Member),
        _ => None,
    }
}

/// Build the axum router with global security middleware.
///
/// # Errors
/// Returns an error if configured security limits are invalid.
pub fn build_router(config: &AppConfig) -> anyhow::Result<Router> {
    if config.max_gateway_event_bytes > filament_protocol::MAX_EVENT_BYTES {
        return Err(anyhow!(
            "gateway event limit cannot exceed protocol max of {} bytes",
            filament_protocol::MAX_EVENT_BYTES
        ));
    }

    let governor_config = Arc::new(
        GovernorConfigBuilder::default()
            .period(Duration::from_secs(60))
            .burst_size(config.rate_limit_requests_per_minute)
            .key_extractor(SmartIpKeyExtractor)
            .finish()
            .ok_or_else(|| anyhow!("invalid governor configuration"))?,
    );
    let app_state = AppState::new(config)?;
    let request_id_header = HeaderName::from_static("x-request-id");
    let governor_layer = GovernorLayer::new(governor_config);

    Ok(Router::new()
        .route("/health", get(health))
        .route("/echo", post(echo))
        .route("/slow", get(slow))
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/refresh", post(refresh))
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
        .route("/guilds", post(create_guild))
        .route("/guilds/{guild_id}/channels", post(create_channel))
        .route(
            "/guilds/{guild_id}/channels/{channel_id}/messages",
            post(create_message).get(get_messages),
        )
        .route(
            "/guilds/{guild_id}/channels/{channel_id}/messages/{message_id}",
            patch(edit_message).delete(delete_message),
        )
        .route(
            "/guilds/{guild_id}/channels/{channel_id}/attachments",
            post(upload_attachment),
        )
        .route(
            "/guilds/{guild_id}/channels/{channel_id}/attachments/{attachment_id}",
            get(download_attachment).delete(delete_attachment),
        )
        .route(
            "/guilds/{guild_id}/members/{user_id}/kick",
            post(kick_member),
        )
        .route("/guilds/{guild_id}/members/{user_id}/ban", post(ban_member))
        .route("/gateway/ws", get(gateway_ws))
        .with_state(app_state)
        .layer(DefaultBodyLimit::max(config.max_body_bytes))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(PropagateRequestIdLayer::new(request_id_header.clone()))
                .layer(SetRequestIdLayer::new(request_id_header, MakeRequestUuid))
                .layer(TimeoutLayer::with_status_code(
                    StatusCode::REQUEST_TIMEOUT,
                    config.request_timeout,
                ))
                .layer(governor_layer),
        ))
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EchoRequest {
    message: String,
}

#[derive(Debug, Serialize)]
struct EchoResponse {
    message: String,
}

async fn echo(Json(payload): Json<EchoRequest>) -> Result<Json<EchoResponse>, StatusCode> {
    if payload.message.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    Ok(Json(EchoResponse {
        message: payload.message,
    }))
}

async fn slow() -> Json<HealthResponse> {
    tokio::time::sleep(Duration::from_millis(200)).await;
    Json(HealthResponse { status: "ok" })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegisterRequest {
    username: String,
    password: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RefreshRequest {
    refresh_token: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct AuthResponse {
    access_token: String,
    refresh_token: String,
    expires_in_secs: i64,
}

#[derive(Debug, Serialize)]
struct RegisterResponse {
    accepted: bool,
}

#[derive(Debug, Serialize)]
struct AuthError {
    error: &'static str,
}

#[derive(Debug, Serialize)]
struct MeResponse {
    user_id: String,
    username: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateGuildRequest {
    name: String,
}

#[derive(Debug, Serialize)]
struct GuildResponse {
    guild_id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateChannelRequest {
    name: String,
}

#[derive(Debug, Serialize)]
struct ChannelResponse {
    channel_id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateMessageRequest {
    content: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EditMessageRequest {
    content: String,
}

#[derive(Debug, Serialize, Clone)]
struct MessageResponse {
    message_id: String,
    guild_id: String,
    channel_id: String,
    author_id: String,
    content: String,
    markdown_tokens: Vec<MarkdownToken>,
    created_at_unix: i64,
}

#[derive(Debug, Serialize)]
struct AttachmentResponse {
    attachment_id: String,
    guild_id: String,
    channel_id: String,
    owner_id: String,
    filename: String,
    mime_type: String,
    size_bytes: u64,
    sha256_hex: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UploadAttachmentQuery {
    filename: Option<String>,
}

#[derive(Debug, Serialize)]
struct ModerationResponse {
    accepted: bool,
}

#[derive(Debug, Serialize)]
struct MessageHistoryResponse {
    messages: Vec<MessageResponse>,
    next_before: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GuildPath {
    guild_id: String,
}

#[derive(Debug, Deserialize)]
struct ChannelPath {
    guild_id: String,
    channel_id: String,
}

#[allow(clippy::struct_field_names)]
#[derive(Debug, Deserialize)]
struct MessagePath {
    guild_id: String,
    channel_id: String,
    message_id: String,
}

#[allow(clippy::struct_field_names)]
#[derive(Debug, Deserialize)]
struct AttachmentPath {
    guild_id: String,
    channel_id: String,
    attachment_id: String,
}

#[derive(Debug, Deserialize)]
struct MemberPath {
    guild_id: String,
    user_id: String,
}

#[derive(Debug, Deserialize)]
struct HistoryQuery {
    limit: Option<usize>,
    before: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GatewaySubscribe {
    guild_id: String,
    channel_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GatewayMessageCreate {
    guild_id: String,
    channel_id: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct GatewayAuthQuery {
    access_token: Option<String>,
}

async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, AuthFailure> {
    enforce_auth_route_rate_limit(&state, &headers, "register").await?;
    ensure_db_schema(&state).await?;

    let username = Username::try_from(payload.username).map_err(|_| AuthFailure::InvalidRequest)?;
    validate_password(&payload.password).map_err(|_| AuthFailure::InvalidRequest)?;

    if let Some(pool) = &state.db_pool {
        let password_hash = hash_password(&payload.password).map_err(|_| AuthFailure::Internal)?;
        let user_id = UserId::new();
        let insert_result = sqlx::query(
            "INSERT INTO users (user_id, username, password_hash, failed_logins, locked_until_unix)
             VALUES ($1, $2, $3, 0, NULL)
             ON CONFLICT (username) DO NOTHING",
        )
        .bind(user_id.to_string())
        .bind(username.as_str())
        .bind(password_hash)
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;

        if insert_result.rows_affected() == 0 {
            tracing::info!(event = "auth.register", outcome = "existing_user");
        } else {
            tracing::info!(event = "auth.register", outcome = "created", user_id = %user_id);
        }
        return Ok(Json(RegisterResponse { accepted: true }));
    }

    let mut users = state.users.write().await;
    if users.contains_key(username.as_str()) {
        tracing::info!(event = "auth.register", outcome = "existing_user");
        return Ok(Json(RegisterResponse { accepted: true }));
    }

    let password_hash = hash_password(&payload.password).map_err(|_| AuthFailure::Internal)?;
    let user_id = UserId::new();
    users.insert(
        username.as_str().to_owned(),
        UserRecord {
            id: user_id,
            username: username.clone(),
            password_hash,
            failed_logins: 0,
            locked_until_unix: None,
        },
    );
    drop(users);

    state
        .user_ids
        .write()
        .await
        .insert(user_id.to_string(), username.as_str().to_owned());
    tracing::info!(event = "auth.register", outcome = "created", user_id = %user_id);

    Ok(Json(RegisterResponse { accepted: true }))
}

#[allow(clippy::too_many_lines)]
async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, AuthFailure> {
    enforce_auth_route_rate_limit(&state, &headers, "login").await?;
    ensure_db_schema(&state).await?;

    let username = Username::try_from(payload.username).map_err(|_| AuthFailure::Unauthorized)?;
    validate_password(&payload.password).map_err(|_| AuthFailure::Unauthorized)?;

    if let Some(pool) = &state.db_pool {
        let row = sqlx::query(
            "SELECT user_id, password_hash, failed_logins, locked_until_unix
             FROM users WHERE username = $1",
        )
        .bind(username.as_str())
        .fetch_optional(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;

        let now = now_unix();
        let (user_id, verified) = if let Some(row) = row {
            let user_id_text: String = row.try_get("user_id").map_err(|_| AuthFailure::Internal)?;
            let user_id = UserId::try_from(user_id_text).map_err(|_| AuthFailure::Internal)?;
            let password_hash: String = row
                .try_get("password_hash")
                .map_err(|_| AuthFailure::Internal)?;
            let failed_logins: i16 = row
                .try_get("failed_logins")
                .map_err(|_| AuthFailure::Internal)?;
            let locked_until_unix: Option<i64> = row
                .try_get("locked_until_unix")
                .map_err(|_| AuthFailure::Internal)?;

            if locked_until_unix.is_some_and(|lock_until| lock_until > now) {
                tracing::warn!(event = "auth.login", outcome = "locked", username = %username.as_str());
                return Err(AuthFailure::Unauthorized);
            }

            let verified = verify_password(&password_hash, &payload.password);
            if verified {
                sqlx::query(
                    "UPDATE users SET failed_logins = 0, locked_until_unix = NULL WHERE user_id = $1",
                )
                .bind(user_id.to_string())
                .execute(pool)
                .await
                .map_err(|_| AuthFailure::Internal)?;
                (Some(user_id), true)
            } else {
                let mut updated_failed = i32::from(failed_logins) + 1;
                let mut lock_until = None;
                if updated_failed >= i32::from(LOGIN_LOCK_THRESHOLD) {
                    updated_failed = 0;
                    lock_until = Some(now + LOGIN_LOCK_SECS);
                    tracing::warn!(event = "auth.login", outcome = "lockout", username = %username.as_str());
                } else {
                    tracing::warn!(event = "auth.login", outcome = "bad_password", username = %username.as_str());
                }
                sqlx::query(
                    "UPDATE users SET failed_logins = $2, locked_until_unix = $3 WHERE user_id = $1",
                )
                .bind(user_id.to_string())
                .bind(i16::try_from(updated_failed).unwrap_or(i16::MAX))
                .bind(lock_until)
                .execute(pool)
                .await
                .map_err(|_| AuthFailure::Internal)?;
                (None, false)
            }
        } else {
            let _ = verify_password(&state.dummy_password_hash, &payload.password);
            tracing::warn!(event = "auth.login", outcome = "invalid_credentials");
            (None, false)
        };

        if !verified {
            return Err(AuthFailure::Unauthorized);
        }
        let user_id = user_id.ok_or(AuthFailure::Internal)?;

        let session_id = Ulid::new().to_string();
        let (access_token, refresh_token, refresh_hash) =
            issue_tokens(&state, user_id, username.as_str(), &session_id)
                .map_err(|_| AuthFailure::Internal)?;
        sqlx::query(
            "INSERT INTO sessions (session_id, user_id, refresh_token_hash, expires_at_unix, revoked)
             VALUES ($1, $2, $3, $4, FALSE)",
        )
        .bind(&session_id)
        .bind(user_id.to_string())
        .bind(refresh_hash.as_slice())
        .bind(now + REFRESH_TOKEN_TTL_SECS)
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        tracing::info!(event = "auth.login", outcome = "success", user_id = %user_id);

        return Ok(Json(AuthResponse {
            access_token,
            refresh_token,
            expires_in_secs: ACCESS_TOKEN_TTL_SECS,
        }));
    }

    let mut users = state.users.write().await;
    let now = now_unix();
    let mut user_id = None;
    let mut username_text = None;
    let mut verified = false;

    if let Some(user) = users.get_mut(username.as_str()) {
        if user
            .locked_until_unix
            .is_some_and(|lock_until| lock_until > now)
        {
            tracing::warn!(event = "auth.login", outcome = "locked", username = %username.as_str());
            return Err(AuthFailure::Unauthorized);
        }

        verified = verify_password(&user.password_hash, &payload.password);
        if verified {
            user.failed_logins = 0;
            user.locked_until_unix = None;
            user_id = Some(user.id);
            username_text = Some(user.username.as_str().to_owned());
        } else {
            user.failed_logins = user.failed_logins.saturating_add(1);
            if user.failed_logins >= LOGIN_LOCK_THRESHOLD {
                user.locked_until_unix = Some(now + LOGIN_LOCK_SECS);
                user.failed_logins = 0;
                tracing::warn!(event = "auth.login", outcome = "lockout", username = %username.as_str());
            } else {
                tracing::warn!(event = "auth.login", outcome = "bad_password", username = %username.as_str());
            }
        }
    } else {
        let _ = verify_password(&state.dummy_password_hash, &payload.password);
        tracing::warn!(event = "auth.login", outcome = "invalid_credentials");
    }
    drop(users);

    if !verified {
        return Err(AuthFailure::Unauthorized);
    }

    let user_id = user_id.ok_or(AuthFailure::Internal)?;
    let username_text = username_text.ok_or(AuthFailure::Internal)?;
    let session_id = Ulid::new().to_string();
    let (access_token, refresh_token, refresh_hash) =
        issue_tokens(&state, user_id, &username_text, &session_id)
            .map_err(|_| AuthFailure::Internal)?;
    state.sessions.write().await.insert(
        session_id,
        SessionRecord {
            user_id,
            refresh_token_hash: refresh_hash,
            expires_at_unix: now + REFRESH_TOKEN_TTL_SECS,
            revoked: false,
        },
    );
    tracing::info!(event = "auth.login", outcome = "success", user_id = %user_id);

    Ok(Json(AuthResponse {
        access_token,
        refresh_token,
        expires_in_secs: ACCESS_TOKEN_TTL_SECS,
    }))
}

#[allow(clippy::too_many_lines)]
async fn refresh(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RefreshRequest>,
) -> Result<Json<AuthResponse>, AuthFailure> {
    enforce_auth_route_rate_limit(&state, &headers, "refresh").await?;
    ensure_db_schema(&state).await?;

    if payload.refresh_token.is_empty() || payload.refresh_token.len() > 512 {
        tracing::warn!(event = "auth.refresh", outcome = "invalid_token_format");
        return Err(AuthFailure::Unauthorized);
    }

    if let Some(pool) = &state.db_pool {
        let token_hash = hash_refresh_token(&payload.refresh_token);
        if let Some(row) =
            sqlx::query("SELECT session_id FROM used_refresh_tokens WHERE token_hash = $1")
                .bind(token_hash.as_slice())
                .fetch_optional(pool)
                .await
                .map_err(|_| AuthFailure::Internal)?
        {
            let replay_session_id: String = row
                .try_get("session_id")
                .map_err(|_| AuthFailure::Internal)?;
            sqlx::query("UPDATE sessions SET revoked = TRUE WHERE session_id = $1")
                .bind(&replay_session_id)
                .execute(pool)
                .await
                .map_err(|_| AuthFailure::Internal)?;
            tracing::warn!(event = "auth.refresh", outcome = "replay_detected", session_id = %replay_session_id);
            return Err(AuthFailure::Unauthorized);
        }

        let session_id = payload
            .refresh_token
            .split('.')
            .next()
            .ok_or(AuthFailure::Unauthorized)?
            .to_owned();

        let row = sqlx::query(
            "SELECT user_id, refresh_token_hash, expires_at_unix, revoked
             FROM sessions WHERE session_id = $1",
        )
        .bind(&session_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        let row = row.ok_or(AuthFailure::Unauthorized)?;

        let session_user_id: String = row.try_get("user_id").map_err(|_| AuthFailure::Internal)?;
        let refresh_hash: Vec<u8> = row
            .try_get("refresh_token_hash")
            .map_err(|_| AuthFailure::Internal)?;
        let expires_at_unix: i64 = row
            .try_get("expires_at_unix")
            .map_err(|_| AuthFailure::Internal)?;
        let revoked: bool = row.try_get("revoked").map_err(|_| AuthFailure::Internal)?;

        if revoked
            || expires_at_unix < now_unix()
            || refresh_hash.as_slice() != token_hash.as_slice()
        {
            tracing::warn!(event = "auth.refresh", outcome = "rejected", session_id = %session_id);
            return Err(AuthFailure::Unauthorized);
        }

        let user_id = UserId::try_from(session_user_id).map_err(|_| AuthFailure::Internal)?;
        let username = find_username_by_user_id(&state, user_id)
            .await
            .ok_or(AuthFailure::Unauthorized)?;

        let (access_token, refresh_token, rotated_hash) =
            issue_tokens(&state, user_id, &username, &session_id)
                .map_err(|_| AuthFailure::Internal)?;
        sqlx::query(
            "UPDATE sessions SET refresh_token_hash = $2, expires_at_unix = $3 WHERE session_id = $1",
        )
        .bind(&session_id)
        .bind(rotated_hash.as_slice())
        .bind(now_unix() + REFRESH_TOKEN_TTL_SECS)
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;

        sqlx::query(
            "INSERT INTO used_refresh_tokens (token_hash, session_id) VALUES ($1, $2)
             ON CONFLICT (token_hash) DO NOTHING",
        )
        .bind(token_hash.as_slice())
        .bind(&session_id)
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;

        tracing::info!(event = "auth.refresh", outcome = "success", session_id = %session_id, user_id = %user_id);
        return Ok(Json(AuthResponse {
            access_token,
            refresh_token,
            expires_in_secs: ACCESS_TOKEN_TTL_SECS,
        }));
    }

    let token_hash = hash_refresh_token(&payload.refresh_token);
    if let Some(session_id) = state
        .used_refresh_tokens
        .read()
        .await
        .get(&token_hash)
        .cloned()
    {
        if let Some(session) = state.sessions.write().await.get_mut(&session_id) {
            session.revoked = true;
        }
        tracing::warn!(event = "auth.refresh", outcome = "replay_detected", session_id = %session_id);
        return Err(AuthFailure::Unauthorized);
    }

    let session_id = payload
        .refresh_token
        .split('.')
        .next()
        .ok_or(AuthFailure::Unauthorized)?
        .to_owned();

    let mut sessions = state.sessions.write().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or(AuthFailure::Unauthorized)?;
    if session.revoked
        || session.expires_at_unix < now_unix()
        || session.refresh_token_hash != token_hash
    {
        tracing::warn!(event = "auth.refresh", outcome = "rejected", session_id = %session_id);
        return Err(AuthFailure::Unauthorized);
    }

    let user_id = session.user_id;
    let username = find_username_by_user_id(&state, user_id)
        .await
        .ok_or(AuthFailure::Unauthorized)?;

    let old_hash = session.refresh_token_hash;
    let (access_token, refresh_token, refresh_hash) =
        issue_tokens(&state, user_id, &username, &session_id).map_err(|_| AuthFailure::Internal)?;
    session.refresh_token_hash = refresh_hash;
    session.expires_at_unix = now_unix() + REFRESH_TOKEN_TTL_SECS;
    drop(sessions);

    state
        .used_refresh_tokens
        .write()
        .await
        .insert(old_hash, session_id.clone());

    tracing::info!(event = "auth.refresh", outcome = "success", session_id = %session_id, user_id = %user_id);

    Ok(Json(AuthResponse {
        access_token,
        refresh_token,
        expires_in_secs: ACCESS_TOKEN_TTL_SECS,
    }))
}

async fn logout(
    State(state): State<AppState>,
    Json(payload): Json<RefreshRequest>,
) -> Result<StatusCode, AuthFailure> {
    ensure_db_schema(&state).await?;

    if payload.refresh_token.is_empty() || payload.refresh_token.len() > 512 {
        tracing::warn!(event = "auth.logout", outcome = "invalid_token_format");
        return Err(AuthFailure::Unauthorized);
    }

    if let Some(pool) = &state.db_pool {
        let session_id = payload
            .refresh_token
            .split('.')
            .next()
            .ok_or(AuthFailure::Unauthorized)?
            .to_owned();
        let token_hash = hash_refresh_token(&payload.refresh_token);
        let row =
            sqlx::query("SELECT user_id, refresh_token_hash FROM sessions WHERE session_id = $1")
                .bind(&session_id)
                .fetch_optional(pool)
                .await
                .map_err(|_| AuthFailure::Internal)?;
        let row = row.ok_or(AuthFailure::Unauthorized)?;
        let user_id: String = row.try_get("user_id").map_err(|_| AuthFailure::Internal)?;
        let session_hash: Vec<u8> = row
            .try_get("refresh_token_hash")
            .map_err(|_| AuthFailure::Internal)?;

        if session_hash.as_slice() != token_hash.as_slice() {
            tracing::warn!(event = "auth.logout", outcome = "hash_mismatch", session_id = %session_id);
            return Err(AuthFailure::Unauthorized);
        }

        sqlx::query("UPDATE sessions SET revoked = TRUE WHERE session_id = $1")
            .bind(&session_id)
            .execute(pool)
            .await
            .map_err(|_| AuthFailure::Internal)?;
        tracing::info!(event = "auth.logout", outcome = "success", session_id = %session_id, user_id = %user_id);
        return Ok(StatusCode::NO_CONTENT);
    }

    let session_id = payload
        .refresh_token
        .split('.')
        .next()
        .ok_or(AuthFailure::Unauthorized)?
        .to_owned();
    let token_hash = hash_refresh_token(&payload.refresh_token);
    let mut sessions = state.sessions.write().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or(AuthFailure::Unauthorized)?;
    if session.refresh_token_hash != token_hash {
        tracing::warn!(event = "auth.logout", outcome = "hash_mismatch", session_id = %session_id);
        return Err(AuthFailure::Unauthorized);
    }
    session.revoked = true;
    tracing::info!(event = "auth.logout", outcome = "success", session_id = %session_id, user_id = %session.user_id);
    Ok(StatusCode::NO_CONTENT)
}

async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<MeResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;

    Ok(Json(MeResponse {
        user_id: auth.user_id.to_string(),
        username: auth.username,
    }))
}

async fn create_guild(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateGuildRequest>,
) -> Result<Json<GuildResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    let name = GuildName::try_from(payload.name).map_err(|_| AuthFailure::InvalidRequest)?;

    let guild_id = Ulid::new().to_string();
    if let Some(pool) = &state.db_pool {
        let mut tx = pool.begin().await.map_err(|_| AuthFailure::Internal)?;
        sqlx::query("INSERT INTO guilds (guild_id, name, created_at_unix) VALUES ($1, $2, $3)")
            .bind(&guild_id)
            .bind(name.as_str())
            .bind(now_unix())
            .execute(&mut *tx)
            .await
            .map_err(|_| AuthFailure::Internal)?;
        sqlx::query("INSERT INTO guild_members (guild_id, user_id, role) VALUES ($1, $2, $3)")
            .bind(&guild_id)
            .bind(auth.user_id.to_string())
            .bind(role_to_i16(Role::Owner))
            .execute(&mut *tx)
            .await
            .map_err(|_| AuthFailure::Internal)?;
        tx.commit().await.map_err(|_| AuthFailure::Internal)?;

        return Ok(Json(GuildResponse {
            guild_id,
            name: name.as_str().to_owned(),
        }));
    }

    let mut members = HashMap::new();
    members.insert(auth.user_id, Role::Owner);

    state.guilds.write().await.insert(
        guild_id.clone(),
        GuildRecord {
            members,
            banned_members: HashSet::new(),
            channels: HashMap::new(),
        },
    );

    Ok(Json(GuildResponse {
        guild_id,
        name: name.as_str().to_owned(),
    }))
}

async fn create_channel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<GuildPath>,
    Json(payload): Json<CreateChannelRequest>,
) -> Result<Json<ChannelResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    let name = ChannelName::try_from(payload.name).map_err(|_| AuthFailure::InvalidRequest)?;

    if let Some(pool) = &state.db_pool {
        let role_row =
            sqlx::query("SELECT role FROM guild_members WHERE guild_id = $1 AND user_id = $2")
                .bind(&path.guild_id)
                .bind(auth.user_id.to_string())
                .fetch_optional(pool)
                .await
                .map_err(|_| AuthFailure::Internal)?;
        let role_row = role_row.ok_or(AuthFailure::Forbidden)?;
        let role_value: i16 = role_row
            .try_get("role")
            .map_err(|_| AuthFailure::Internal)?;
        let role = role_from_i16(role_value).ok_or(AuthFailure::Forbidden)?;
        if !matches!(role, Role::Owner | Role::Moderator) {
            return Err(AuthFailure::Forbidden);
        }

        let channel_id = Ulid::new().to_string();
        sqlx::query(
            "INSERT INTO channels (channel_id, guild_id, name, created_at_unix) VALUES ($1, $2, $3, $4)",
        )
        .bind(&channel_id)
        .bind(&path.guild_id)
        .bind(name.as_str())
        .bind(now_unix())
        .execute(pool)
        .await
        .map_err(|e| {
            if matches!(e, sqlx::Error::Database(_)) {
                AuthFailure::NotFound
            } else {
                AuthFailure::Internal
            }
        })?;

        return Ok(Json(ChannelResponse {
            channel_id,
            name: name.as_str().to_owned(),
        }));
    }

    let mut guilds = state.guilds.write().await;
    let guild = guilds
        .get_mut(&path.guild_id)
        .ok_or(AuthFailure::NotFound)?;
    let role = guild
        .members
        .get(&auth.user_id)
        .copied()
        .ok_or(AuthFailure::Forbidden)?;
    if !matches!(role, Role::Owner | Role::Moderator) {
        return Err(AuthFailure::Forbidden);
    }

    let channel_id = Ulid::new().to_string();
    guild.channels.insert(
        channel_id.clone(),
        ChannelRecord {
            messages: Vec::new(),
        },
    );

    Ok(Json(ChannelResponse {
        channel_id,
        name: name.as_str().to_owned(),
    }))
}

async fn create_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<ChannelPath>,
    Json(payload): Json<CreateMessageRequest>,
) -> Result<Json<MessageResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;
    let response = create_message_internal(
        &state,
        &auth,
        &path.guild_id,
        &path.channel_id,
        payload.content,
    )
    .await?;
    Ok(Json(response))
}

#[allow(clippy::too_many_lines)]
async fn get_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<ChannelPath>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<MessageHistoryResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    let limit = query.limit.unwrap_or(20);
    if limit == 0 || limit > MAX_HISTORY_LIMIT {
        return Err(AuthFailure::InvalidRequest);
    }

    if let Some(pool) = &state.db_pool {
        let role_row =
            sqlx::query("SELECT role FROM guild_members WHERE guild_id = $1 AND user_id = $2")
                .bind(&path.guild_id)
                .bind(auth.user_id.to_string())
                .fetch_optional(pool)
                .await
                .map_err(|_| AuthFailure::Internal)?;
        let role_row = role_row.ok_or(AuthFailure::Forbidden)?;
        let role_value: i16 = role_row
            .try_get("role")
            .map_err(|_| AuthFailure::Internal)?;
        let role = role_from_i16(role_value).ok_or(AuthFailure::Forbidden)?;
        if !has_permission(role, Permission::CreateMessage) {
            return Err(AuthFailure::Forbidden);
        }

        let limit_i64 = i64::try_from(limit).map_err(|_| AuthFailure::InvalidRequest)?;
        let rows = sqlx::query(
            "SELECT message_id, author_id, content, created_at_unix
             FROM messages
             WHERE guild_id = $1 AND channel_id = $2 AND ($3::text IS NULL OR message_id < $3)
             ORDER BY message_id DESC
             LIMIT $4",
        )
        .bind(&path.guild_id)
        .bind(&path.channel_id)
        .bind(query.before.clone())
        .bind(limit_i64)
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;

        let mut messages = Vec::with_capacity(rows.len());
        for row in rows {
            let message_id: String = row
                .try_get("message_id")
                .map_err(|_| AuthFailure::Internal)?;
            let author_id: String = row
                .try_get("author_id")
                .map_err(|_| AuthFailure::Internal)?;
            let content: String = row.try_get("content").map_err(|_| AuthFailure::Internal)?;
            let created_at_unix: i64 = row
                .try_get("created_at_unix")
                .map_err(|_| AuthFailure::Internal)?;
            messages.push(MessageResponse {
                message_id,
                guild_id: path.guild_id.clone(),
                channel_id: path.channel_id.clone(),
                author_id,
                content: content.clone(),
                markdown_tokens: tokenize_markdown(&content),
                created_at_unix,
            });
        }
        let next_before = messages.last().map(|message| message.message_id.clone());
        return Ok(Json(MessageHistoryResponse {
            messages,
            next_before,
        }));
    }

    let guilds = state.guilds.read().await;
    let guild = guilds.get(&path.guild_id).ok_or(AuthFailure::NotFound)?;
    let role = guild
        .members
        .get(&auth.user_id)
        .copied()
        .ok_or(AuthFailure::Forbidden)?;
    if !has_permission(role, Permission::CreateMessage) {
        return Err(AuthFailure::Forbidden);
    }
    let channel = guild
        .channels
        .get(&path.channel_id)
        .ok_or(AuthFailure::NotFound)?;

    let mut messages = Vec::with_capacity(limit);
    let mut collecting = query.before.is_none();

    for message in channel.messages.iter().rev() {
        if !collecting {
            if query.before.as_deref() == Some(message.id.as_str()) {
                collecting = true;
            }
            continue;
        }

        if messages.len() >= limit {
            break;
        }

        messages.push(MessageResponse {
            message_id: message.id.clone(),
            guild_id: path.guild_id.clone(),
            channel_id: path.channel_id.clone(),
            author_id: message.author_id.to_string(),
            content: message.content.clone(),
            markdown_tokens: message.markdown_tokens.clone(),
            created_at_unix: message.created_at_unix,
        });
    }

    let next_before = messages.last().map(|message| message.message_id.clone());

    Ok(Json(MessageHistoryResponse {
        messages,
        next_before,
    }))
}

async fn edit_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<MessagePath>,
    Json(payload): Json<EditMessageRequest>,
) -> Result<Json<MessageResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    validate_message_content(&payload.content)?;
    let markdown_tokens = tokenize_markdown(&payload.content);

    if let Some(pool) = &state.db_pool {
        let row = sqlx::query(
            "SELECT m.author_id, gm.role
             FROM messages m
             JOIN guild_members gm ON gm.guild_id = m.guild_id AND gm.user_id = $1
             WHERE m.guild_id = $2 AND m.channel_id = $3 AND m.message_id = $4",
        )
        .bind(auth.user_id.to_string())
        .bind(&path.guild_id)
        .bind(&path.channel_id)
        .bind(&path.message_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        let row = row.ok_or(AuthFailure::NotFound)?;
        let author_id: String = row
            .try_get("author_id")
            .map_err(|_| AuthFailure::Internal)?;
        let role_value: i16 = row.try_get("role").map_err(|_| AuthFailure::Internal)?;
        let role = role_from_i16(role_value).ok_or(AuthFailure::Forbidden)?;
        if author_id != auth.user_id.to_string() && !has_permission(role, Permission::DeleteMessage)
        {
            return Err(AuthFailure::Forbidden);
        }

        sqlx::query(
            "UPDATE messages SET content = $4
             WHERE guild_id = $1 AND channel_id = $2 AND message_id = $3",
        )
        .bind(&path.guild_id)
        .bind(&path.channel_id)
        .bind(&path.message_id)
        .bind(&payload.content)
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;

        let response = MessageResponse {
            message_id: path.message_id.clone(),
            guild_id: path.guild_id.clone(),
            channel_id: path.channel_id.clone(),
            author_id: author_id.clone(),
            content: payload.content,
            markdown_tokens,
            created_at_unix: now_unix(),
        };
        if author_id != auth.user_id.to_string() {
            write_audit_log(
                &state,
                Some(path.guild_id.clone()),
                auth.user_id,
                Some(UserId::try_from(author_id).map_err(|_| AuthFailure::Internal)?),
                "message.edit.moderation",
                serde_json::json!({"message_id": path.message_id, "channel_id": path.channel_id}),
            )
            .await?;
        }
        return Ok(Json(response));
    }

    let mut guilds = state.guilds.write().await;
    let guild = guilds
        .get_mut(&path.guild_id)
        .ok_or(AuthFailure::NotFound)?;
    let role = guild
        .members
        .get(&auth.user_id)
        .copied()
        .ok_or(AuthFailure::Forbidden)?;
    let channel = guild
        .channels
        .get_mut(&path.channel_id)
        .ok_or(AuthFailure::NotFound)?;
    let message = channel
        .messages
        .iter_mut()
        .find(|message| message.id == path.message_id)
        .ok_or(AuthFailure::NotFound)?;
    if message.author_id != auth.user_id && !has_permission(role, Permission::DeleteMessage) {
        return Err(AuthFailure::Forbidden);
    }
    message.content.clone_from(&payload.content);
    message.markdown_tokens.clone_from(&markdown_tokens);

    Ok(Json(MessageResponse {
        message_id: message.id.clone(),
        guild_id: path.guild_id,
        channel_id: path.channel_id,
        author_id: message.author_id.to_string(),
        content: message.content.clone(),
        markdown_tokens,
        created_at_unix: message.created_at_unix,
    }))
}

async fn delete_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<MessagePath>,
) -> Result<StatusCode, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;

    if let Some(pool) = &state.db_pool {
        let row = sqlx::query(
            "SELECT m.author_id, gm.role
             FROM messages m
             JOIN guild_members gm ON gm.guild_id = m.guild_id AND gm.user_id = $1
             WHERE m.guild_id = $2 AND m.channel_id = $3 AND m.message_id = $4",
        )
        .bind(auth.user_id.to_string())
        .bind(&path.guild_id)
        .bind(&path.channel_id)
        .bind(&path.message_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        let row = row.ok_or(AuthFailure::NotFound)?;
        let author_id: String = row
            .try_get("author_id")
            .map_err(|_| AuthFailure::Internal)?;
        let role_value: i16 = row.try_get("role").map_err(|_| AuthFailure::Internal)?;
        let role = role_from_i16(role_value).ok_or(AuthFailure::Forbidden)?;
        if author_id != auth.user_id.to_string() && !has_permission(role, Permission::DeleteMessage)
        {
            return Err(AuthFailure::Forbidden);
        }

        sqlx::query(
            "DELETE FROM messages
             WHERE guild_id = $1 AND channel_id = $2 AND message_id = $3",
        )
        .bind(&path.guild_id)
        .bind(&path.channel_id)
        .bind(&path.message_id)
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;

        if author_id != auth.user_id.to_string() {
            write_audit_log(
                &state,
                Some(path.guild_id.clone()),
                auth.user_id,
                Some(UserId::try_from(author_id).map_err(|_| AuthFailure::Internal)?),
                "message.delete.moderation",
                serde_json::json!({"message_id": path.message_id, "channel_id": path.channel_id}),
            )
            .await?;
        }
        return Ok(StatusCode::NO_CONTENT);
    }

    let mut guilds = state.guilds.write().await;
    let guild = guilds
        .get_mut(&path.guild_id)
        .ok_or(AuthFailure::NotFound)?;
    let role = guild
        .members
        .get(&auth.user_id)
        .copied()
        .ok_or(AuthFailure::Forbidden)?;
    let channel = guild
        .channels
        .get_mut(&path.channel_id)
        .ok_or(AuthFailure::NotFound)?;
    let Some(index) = channel
        .messages
        .iter()
        .position(|message| message.id == path.message_id)
    else {
        return Err(AuthFailure::NotFound);
    };
    let author_id = channel.messages[index].author_id;
    if author_id != auth.user_id && !has_permission(role, Permission::DeleteMessage) {
        return Err(AuthFailure::Forbidden);
    }
    channel.messages.remove(index);
    Ok(StatusCode::NO_CONTENT)
}

#[allow(clippy::too_many_lines)]
async fn upload_attachment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<ChannelPath>,
    Query(query): Query<UploadAttachmentQuery>,
    body: Bytes,
) -> Result<Json<AttachmentResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    if body.is_empty() {
        return Err(AuthFailure::InvalidRequest);
    }
    if body.len() > state.runtime.max_attachment_bytes {
        return Err(AuthFailure::PayloadTooLarge);
    }
    if !user_can_write_channel(&state, auth.user_id, &path.guild_id, &path.channel_id).await {
        return Err(AuthFailure::Forbidden);
    }

    let sniffed = infer::get(&body).ok_or(AuthFailure::InvalidRequest)?;
    let sniffed_mime = sniffed.mime_type();
    if let Some(content_type) = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
    {
        let declared = content_type
            .parse::<mime::Mime>()
            .map_err(|_| AuthFailure::InvalidRequest)?;
        if declared.essence_str() != sniffed_mime {
            return Err(AuthFailure::InvalidRequest);
        }
    }

    let filename =
        validate_attachment_filename(query.filename.unwrap_or_else(|| String::from("upload.bin")))?;
    let body_len = u64::try_from(body.len()).map_err(|_| AuthFailure::InvalidRequest)?;
    let usage = attachment_usage_for_user(&state, auth.user_id).await?;
    if usage.saturating_add(body_len) > state.runtime.user_attachment_quota_bytes {
        return Err(AuthFailure::QuotaExceeded);
    }

    let attachment_id = Ulid::new().to_string();
    let object_key = format!("attachments/{attachment_id}");
    let object_path = ObjectPath::from(object_key.clone());
    state
        .attachment_store
        .put(&object_path, body.clone().into())
        .await
        .map_err(|_| AuthFailure::Internal)?;

    let sha256_hex = {
        let digest = Sha256::digest(&body);
        let mut out = String::with_capacity(digest.len() * 2);
        for byte in digest {
            let _ = std::fmt::Write::write_fmt(&mut out, format_args!("{byte:02x}"));
        }
        out
    };

    if let Some(pool) = &state.db_pool {
        sqlx::query(
            "INSERT INTO attachments (attachment_id, guild_id, channel_id, owner_id, filename, mime_type, size_bytes, sha256_hex, object_key, created_at_unix)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(&attachment_id)
        .bind(&path.guild_id)
        .bind(&path.channel_id)
        .bind(auth.user_id.to_string())
        .bind(&filename)
        .bind(sniffed_mime)
        .bind(i64::try_from(body_len).map_err(|_| AuthFailure::InvalidRequest)?)
        .bind(&sha256_hex)
        .bind(&object_key)
        .bind(now_unix())
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
    } else {
        state.attachments.write().await.insert(
            attachment_id.clone(),
            AttachmentRecord {
                guild_id: path.guild_id.clone(),
                channel_id: path.channel_id.clone(),
                owner_id: auth.user_id,
                mime_type: String::from(sniffed_mime),
                size_bytes: body_len,
                object_key: object_key.clone(),
            },
        );
    }

    Ok(Json(AttachmentResponse {
        attachment_id,
        guild_id: path.guild_id,
        channel_id: path.channel_id,
        owner_id: auth.user_id.to_string(),
        filename,
        mime_type: String::from(sniffed_mime),
        size_bytes: body_len,
        sha256_hex,
    }))
}

async fn download_attachment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<AttachmentPath>,
) -> Result<Response, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    if !user_can_write_channel(&state, auth.user_id, &path.guild_id, &path.channel_id).await {
        return Err(AuthFailure::Forbidden);
    }

    let record = find_attachment(&state, &path).await?;
    let object_path = ObjectPath::from(record.object_key.clone());
    let get_result = state
        .attachment_store
        .get(&object_path)
        .await
        .map_err(|_| AuthFailure::NotFound)?;
    let payload = get_result
        .bytes()
        .await
        .map_err(|_| AuthFailure::Internal)?;

    let mut response = Response::new(payload.into());
    let content_type =
        HeaderValue::from_str(&record.mime_type).map_err(|_| AuthFailure::Internal)?;
    response.headers_mut().insert(CONTENT_TYPE, content_type);
    Ok(response)
}

async fn delete_attachment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<AttachmentPath>,
) -> Result<StatusCode, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    let role = user_role_in_guild(&state, auth.user_id, &path.guild_id).await?;
    let record = find_attachment(&state, &path).await?;
    if record.owner_id != auth.user_id && !has_permission(role, Permission::DeleteMessage) {
        return Err(AuthFailure::Forbidden);
    }

    if let Some(pool) = &state.db_pool {
        sqlx::query(
            "DELETE FROM attachments
             WHERE attachment_id = $1 AND guild_id = $2 AND channel_id = $3",
        )
        .bind(&path.attachment_id)
        .bind(&path.guild_id)
        .bind(&path.channel_id)
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
    } else {
        state.attachments.write().await.remove(&path.attachment_id);
    }

    let object_path = ObjectPath::from(record.object_key);
    let _ = state.attachment_store.delete(&object_path).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn kick_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<MemberPath>,
) -> Result<Json<ModerationResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    let actor_role = user_role_in_guild(&state, auth.user_id, &path.guild_id).await?;
    if !has_permission(actor_role, Permission::BanMember) {
        return Err(AuthFailure::Forbidden);
    }
    let target_user_id = UserId::try_from(path.user_id).map_err(|_| AuthFailure::InvalidRequest)?;

    if let Some(pool) = &state.db_pool {
        let deleted = sqlx::query("DELETE FROM guild_members WHERE guild_id = $1 AND user_id = $2")
            .bind(&path.guild_id)
            .bind(target_user_id.to_string())
            .execute(pool)
            .await
            .map_err(|_| AuthFailure::Internal)?;
        if deleted.rows_affected() == 0 {
            return Err(AuthFailure::NotFound);
        }
    } else {
        let mut guilds = state.guilds.write().await;
        let guild = guilds
            .get_mut(&path.guild_id)
            .ok_or(AuthFailure::NotFound)?;
        if guild.members.remove(&target_user_id).is_none() {
            return Err(AuthFailure::NotFound);
        }
    }

    write_audit_log(
        &state,
        Some(path.guild_id),
        auth.user_id,
        Some(target_user_id),
        "member.kick",
        serde_json::json!({}),
    )
    .await?;
    Ok(Json(ModerationResponse { accepted: true }))
}

async fn ban_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<MemberPath>,
) -> Result<Json<ModerationResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    let actor_role = user_role_in_guild(&state, auth.user_id, &path.guild_id).await?;
    if !has_permission(actor_role, Permission::BanMember) {
        return Err(AuthFailure::Forbidden);
    }
    let target_user_id = UserId::try_from(path.user_id).map_err(|_| AuthFailure::InvalidRequest)?;

    if let Some(pool) = &state.db_pool {
        let mut tx = pool.begin().await.map_err(|_| AuthFailure::Internal)?;
        sqlx::query(
            "INSERT INTO guild_bans (guild_id, user_id, banned_by_user_id, created_at_unix)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (guild_id, user_id) DO UPDATE SET banned_by_user_id = EXCLUDED.banned_by_user_id, created_at_unix = EXCLUDED.created_at_unix",
        )
        .bind(&path.guild_id)
        .bind(target_user_id.to_string())
        .bind(auth.user_id.to_string())
        .bind(now_unix())
        .execute(&mut *tx)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        sqlx::query("DELETE FROM guild_members WHERE guild_id = $1 AND user_id = $2")
            .bind(&path.guild_id)
            .bind(target_user_id.to_string())
            .execute(&mut *tx)
            .await
            .map_err(|_| AuthFailure::Internal)?;
        tx.commit().await.map_err(|_| AuthFailure::Internal)?;
    } else {
        let mut guilds = state.guilds.write().await;
        let guild = guilds
            .get_mut(&path.guild_id)
            .ok_or(AuthFailure::NotFound)?;
        guild.members.remove(&target_user_id);
        guild.banned_members.insert(target_user_id);
    }

    write_audit_log(
        &state,
        Some(path.guild_id),
        auth.user_id,
        Some(target_user_id),
        "member.ban",
        serde_json::json!({}),
    )
    .await?;
    Ok(Json(ModerationResponse { accepted: true }))
}

async fn gateway_ws(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
    Query(query): Query<GatewayAuthQuery>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AuthFailure> {
    let token = query
        .access_token
        .or_else(|| bearer_token(&headers).map(ToOwned::to_owned))
        .ok_or(AuthFailure::Unauthorized)?;
    let auth = authenticate_with_token(&state, &token).await?;

    Ok(ws.on_upgrade(move |socket| async move {
        handle_gateway_connection(state, socket, auth).await;
    }))
}

#[allow(clippy::too_many_lines)]
async fn handle_gateway_connection(state: AppState, socket: WebSocket, auth: AuthContext) {
    let connection_id = Uuid::new_v4();
    let (mut sink, mut stream) = socket.split();

    let (outbound_tx, mut outbound_rx) =
        mpsc::channel::<String>(state.runtime.gateway_outbound_queue);
    let (control_tx, mut control_rx) = watch::channel(ConnectionControl::Open);
    state
        .connection_controls
        .write()
        .await
        .insert(connection_id, control_tx);

    let ready_payload = outbound_event(
        "ready",
        serde_json::json!({"user_id": auth.user_id.to_string()}),
    );
    let _ = outbound_tx.send(ready_payload).await;

    let send_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                control_change = control_rx.changed() => {
                    if control_change.is_ok() && *control_rx.borrow() == ConnectionControl::Close {
                        let _ = sink
                            .send(Message::Close(Some(CloseFrame {
                                code: 1008,
                                reason: "slow_consumer".into(),
                            })))
                            .await;
                        break;
                    }
                }
                maybe_payload = outbound_rx.recv() => {
                    match maybe_payload {
                        Some(payload) => {
                            if sink.send(Message::Text(payload.into())).await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
            }
        }
    });

    let mut ingress = VecDeque::new();
    while let Some(incoming) = stream.next().await {
        let Ok(message) = incoming else {
            break;
        };

        let payload: Vec<u8> = match message {
            Message::Text(text) => {
                if text.len() > state.runtime.max_gateway_event_bytes {
                    break;
                }
                text.as_bytes().to_vec()
            }
            Message::Binary(bytes) => {
                if bytes.len() > state.runtime.max_gateway_event_bytes {
                    break;
                }
                bytes.to_vec()
            }
            Message::Close(_) => break,
            Message::Ping(_) | Message::Pong(_) => continue,
        };

        if !allow_gateway_ingress(
            &mut ingress,
            state.runtime.gateway_ingress_events_per_window,
            state.runtime.gateway_ingress_window,
        ) {
            break;
        }

        let Ok(envelope) = parse_envelope(&payload) else {
            break;
        };

        match envelope.t.as_str() {
            "subscribe" => {
                let Ok(subscribe) = serde_json::from_value::<GatewaySubscribe>(envelope.d) else {
                    break;
                };
                if !user_can_write_channel(
                    &state,
                    auth.user_id,
                    &subscribe.guild_id,
                    &subscribe.channel_id,
                )
                .await
                {
                    break;
                }

                add_subscription(
                    &state,
                    connection_id,
                    channel_key(&subscribe.guild_id, &subscribe.channel_id),
                    outbound_tx.clone(),
                )
                .await;

                let subscribed = outbound_event(
                    "subscribed",
                    serde_json::json!({
                        "guild_id": subscribe.guild_id,
                        "channel_id": subscribe.channel_id,
                    }),
                );
                if outbound_tx.try_send(subscribed).is_err() {
                    break;
                }
            }
            "message_create" => {
                let Ok(request) = serde_json::from_value::<GatewayMessageCreate>(envelope.d) else {
                    break;
                };
                if create_message_internal(
                    &state,
                    &auth,
                    &request.guild_id,
                    &request.channel_id,
                    request.content,
                )
                .await
                .is_err()
                {
                    break;
                }
            }
            _ => break,
        }
    }

    remove_connection(&state, connection_id).await;
    send_task.abort();
}

#[allow(clippy::too_many_lines)]
async fn create_message_internal(
    state: &AppState,
    auth: &AuthContext,
    guild_id: &str,
    channel_id: &str,
    content: String,
) -> Result<MessageResponse, AuthFailure> {
    validate_message_content(&content)?;
    let markdown_tokens = tokenize_markdown(&content);

    if let Some(pool) = &state.db_pool {
        ensure_db_schema(state).await?;
        let role_row = sqlx::query(
            "SELECT gm.role
             FROM guild_members gm
             JOIN channels c ON c.guild_id = gm.guild_id
             WHERE gm.guild_id = $1 AND gm.user_id = $2 AND c.channel_id = $3",
        )
        .bind(guild_id)
        .bind(auth.user_id.to_string())
        .bind(channel_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        let role_row = role_row.ok_or(AuthFailure::Forbidden)?;
        let role_value: i16 = role_row
            .try_get("role")
            .map_err(|_| AuthFailure::Internal)?;
        let role = role_from_i16(role_value).ok_or(AuthFailure::Forbidden)?;
        if !has_permission(role, Permission::CreateMessage) {
            return Err(AuthFailure::Forbidden);
        }

        let message_id = Ulid::new().to_string();
        let created_at_unix = now_unix();
        sqlx::query(
            "INSERT INTO messages (message_id, guild_id, channel_id, author_id, content, created_at_unix)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(&message_id)
        .bind(guild_id)
        .bind(channel_id)
        .bind(auth.user_id.to_string())
        .bind(&content)
        .bind(created_at_unix)
        .execute(pool)
        .await
        .map_err(|e| {
            if matches!(e, sqlx::Error::Database(_)) {
                AuthFailure::NotFound
            } else {
                AuthFailure::Internal
            }
        })?;

        let response = MessageResponse {
            message_id,
            guild_id: guild_id.to_owned(),
            channel_id: channel_id.to_owned(),
            author_id: auth.user_id.to_string(),
            content,
            markdown_tokens,
            created_at_unix,
        };

        let event = outbound_event(
            "message_create",
            serde_json::json!({
                "message_id": response.message_id,
                "guild_id": response.guild_id,
                "channel_id": response.channel_id,
                "author_id": response.author_id,
                "content": response.content,
                "markdown_tokens": response.markdown_tokens,
                "created_at_unix": response.created_at_unix,
            }),
        );

        broadcast_channel_event(state, &channel_key(guild_id, channel_id), event).await;
        return Ok(response);
    }

    let mut guilds = state.guilds.write().await;
    let guild = guilds.get_mut(guild_id).ok_or(AuthFailure::NotFound)?;
    let role = guild
        .members
        .get(&auth.user_id)
        .copied()
        .ok_or(AuthFailure::Forbidden)?;
    if !has_permission(role, Permission::CreateMessage) {
        return Err(AuthFailure::Forbidden);
    }
    let channel = guild
        .channels
        .get_mut(channel_id)
        .ok_or(AuthFailure::NotFound)?;

    let message_id = Ulid::new().to_string();
    let record = MessageRecord {
        id: message_id.clone(),
        author_id: auth.user_id,
        content,
        markdown_tokens: markdown_tokens.clone(),
        created_at_unix: now_unix(),
    };
    channel.messages.push(record.clone());
    drop(guilds);

    let response = MessageResponse {
        message_id,
        guild_id: guild_id.to_owned(),
        channel_id: channel_id.to_owned(),
        author_id: auth.user_id.to_string(),
        content: record.content,
        markdown_tokens: record.markdown_tokens,
        created_at_unix: record.created_at_unix,
    };

    let event = outbound_event(
        "message_create",
        serde_json::json!({
            "message_id": response.message_id,
            "guild_id": response.guild_id,
            "channel_id": response.channel_id,
            "author_id": response.author_id,
            "content": response.content,
            "markdown_tokens": response.markdown_tokens,
            "created_at_unix": response.created_at_unix,
        }),
    );

    broadcast_channel_event(state, &channel_key(guild_id, channel_id), event).await;

    Ok(response)
}

async fn broadcast_channel_event(state: &AppState, key: &str, payload: String) {
    let mut slow_connections = Vec::new();

    let mut subscriptions = state.subscriptions.write().await;
    if let Some(listeners) = subscriptions.get_mut(key) {
        listeners.retain(
            |connection_id, sender| match sender.try_send(payload.clone()) {
                Ok(()) => true,
                Err(mpsc::error::TrySendError::Closed(_)) => false,
                Err(mpsc::error::TrySendError::Full(_)) => {
                    slow_connections.push(*connection_id);
                    false
                }
            },
        );

        if listeners.is_empty() {
            subscriptions.remove(key);
        }
    }
    drop(subscriptions);

    if !slow_connections.is_empty() {
        let controls = state.connection_controls.read().await;
        for connection_id in slow_connections {
            if let Some(control) = controls.get(&connection_id) {
                let _ = control.send(ConnectionControl::Close);
            }
        }
    }
}

async fn add_subscription(
    state: &AppState,
    connection_id: Uuid,
    key: String,
    outbound_tx: mpsc::Sender<String>,
) {
    let mut subscriptions = state.subscriptions.write().await;
    subscriptions
        .entry(key)
        .or_default()
        .insert(connection_id, outbound_tx);
}

async fn remove_connection(state: &AppState, connection_id: Uuid) {
    state
        .connection_controls
        .write()
        .await
        .remove(&connection_id);

    let mut subscriptions = state.subscriptions.write().await;
    subscriptions.retain(|_, listeners| {
        listeners.remove(&connection_id);
        !listeners.is_empty()
    });
}

fn allow_gateway_ingress(ingress: &mut VecDeque<Instant>, limit: u32, window: Duration) -> bool {
    let now = Instant::now();
    while ingress
        .front()
        .is_some_and(|oldest| now.duration_since(*oldest) > window)
    {
        let _ = ingress.pop_front();
    }

    if ingress.len() >= limit as usize {
        return false;
    }

    ingress.push_back(now);
    true
}

async fn user_can_write_channel(
    state: &AppState,
    user_id: UserId,
    guild_id: &str,
    channel_id: &str,
) -> bool {
    if let Some(pool) = &state.db_pool {
        if ensure_db_schema(state).await.is_err() {
            return false;
        }
        let banned = sqlx::query("SELECT 1 FROM guild_bans WHERE guild_id = $1 AND user_id = $2")
            .bind(guild_id)
            .bind(user_id.to_string())
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .is_some();
        if banned {
            return false;
        }
        let row = sqlx::query(
            "SELECT gm.role
             FROM guild_members gm
             JOIN channels c ON c.guild_id = gm.guild_id
             WHERE gm.guild_id = $1 AND gm.user_id = $2 AND c.channel_id = $3",
        )
        .bind(guild_id)
        .bind(user_id.to_string())
        .bind(channel_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
        let Some(row) = row else {
            return false;
        };
        let role = row.try_get::<i16, _>("role").ok().and_then(role_from_i16);
        return role.is_some_and(|value| has_permission(value, Permission::CreateMessage));
    }

    let guilds = state.guilds.read().await;
    let Some(guild) = guilds.get(guild_id) else {
        return false;
    };
    let Some(role) = guild.members.get(&user_id).copied() else {
        return false;
    };
    if guild.banned_members.contains(&user_id) {
        return false;
    }
    if !guild.channels.contains_key(channel_id) {
        return false;
    }
    has_permission(role, Permission::CreateMessage)
}

async fn user_role_in_guild(
    state: &AppState,
    user_id: UserId,
    guild_id: &str,
) -> Result<Role, AuthFailure> {
    if let Some(pool) = &state.db_pool {
        let banned = sqlx::query("SELECT 1 FROM guild_bans WHERE guild_id = $1 AND user_id = $2")
            .bind(guild_id)
            .bind(user_id.to_string())
            .fetch_optional(pool)
            .await
            .map_err(|_| AuthFailure::Internal)?;
        if banned.is_some() {
            return Err(AuthFailure::Forbidden);
        }

        let row =
            sqlx::query("SELECT role FROM guild_members WHERE guild_id = $1 AND user_id = $2")
                .bind(guild_id)
                .bind(user_id.to_string())
                .fetch_optional(pool)
                .await
                .map_err(|_| AuthFailure::Internal)?;
        let row = row.ok_or(AuthFailure::Forbidden)?;
        let role_value: i16 = row.try_get("role").map_err(|_| AuthFailure::Internal)?;
        return role_from_i16(role_value).ok_or(AuthFailure::Forbidden);
    }

    let guilds = state.guilds.read().await;
    let guild = guilds.get(guild_id).ok_or(AuthFailure::NotFound)?;
    if guild.banned_members.contains(&user_id) {
        return Err(AuthFailure::Forbidden);
    }
    guild
        .members
        .get(&user_id)
        .copied()
        .ok_or(AuthFailure::Forbidden)
}

async fn attachment_usage_for_user(state: &AppState, user_id: UserId) -> Result<u64, AuthFailure> {
    if let Some(pool) = &state.db_pool {
        let row = sqlx::query(
            "SELECT COALESCE(SUM(size_bytes), 0) AS total FROM attachments WHERE owner_id = $1",
        )
        .bind(user_id.to_string())
        .fetch_one(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        let total: i64 = row.try_get("total").map_err(|_| AuthFailure::Internal)?;
        return u64::try_from(total).map_err(|_| AuthFailure::Internal);
    }

    let usage = state
        .attachments
        .read()
        .await
        .values()
        .filter(|record| record.owner_id == user_id)
        .map(|record| record.size_bytes)
        .sum();
    Ok(usage)
}

async fn find_attachment(
    state: &AppState,
    path: &AttachmentPath,
) -> Result<AttachmentRecord, AuthFailure> {
    if let Some(pool) = &state.db_pool {
        let row = sqlx::query(
            "SELECT guild_id, channel_id, owner_id, mime_type, size_bytes, object_key
             FROM attachments
             WHERE attachment_id = $1 AND guild_id = $2 AND channel_id = $3",
        )
        .bind(&path.attachment_id)
        .bind(&path.guild_id)
        .bind(&path.channel_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        let row = row.ok_or(AuthFailure::NotFound)?;
        let owner_id: String = row.try_get("owner_id").map_err(|_| AuthFailure::Internal)?;
        let size_bytes: i64 = row
            .try_get("size_bytes")
            .map_err(|_| AuthFailure::Internal)?;
        return Ok(AttachmentRecord {
            guild_id: row.try_get("guild_id").map_err(|_| AuthFailure::Internal)?,
            channel_id: row
                .try_get("channel_id")
                .map_err(|_| AuthFailure::Internal)?,
            owner_id: UserId::try_from(owner_id).map_err(|_| AuthFailure::Internal)?,
            mime_type: row
                .try_get("mime_type")
                .map_err(|_| AuthFailure::Internal)?,
            size_bytes: u64::try_from(size_bytes).map_err(|_| AuthFailure::Internal)?,
            object_key: row
                .try_get("object_key")
                .map_err(|_| AuthFailure::Internal)?,
        });
    }
    state
        .attachments
        .read()
        .await
        .get(&path.attachment_id)
        .filter(|record| record.guild_id == path.guild_id && record.channel_id == path.channel_id)
        .cloned()
        .ok_or(AuthFailure::NotFound)
}

fn validate_attachment_filename(value: String) -> Result<String, AuthFailure> {
    if value.is_empty() || value.len() > 128 {
        return Err(AuthFailure::InvalidRequest);
    }
    if value.contains('/') || value.contains('\\') || value.contains('\0') {
        return Err(AuthFailure::InvalidRequest);
    }
    Ok(value)
}

async fn write_audit_log(
    state: &AppState,
    guild_id: Option<String>,
    actor_user_id: UserId,
    target_user_id: Option<UserId>,
    action: &str,
    details_json: serde_json::Value,
) -> Result<(), AuthFailure> {
    if let Some(pool) = &state.db_pool {
        sqlx::query(
            "INSERT INTO audit_logs (audit_id, guild_id, actor_user_id, target_user_id, action, details_json, created_at_unix)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(Ulid::new().to_string())
        .bind(guild_id)
        .bind(actor_user_id.to_string())
        .bind(target_user_id.map(|value| value.to_string()))
        .bind(action)
        .bind(details_json.to_string())
        .bind(now_unix())
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        return Ok(());
    }

    state.audit_logs.write().await.push(serde_json::json!({
        "guild_id": guild_id,
        "actor_user_id": actor_user_id.to_string(),
        "target_user_id": target_user_id.map(|value| value.to_string()),
        "action": action,
        "details": details_json,
        "created_at_unix": now_unix(),
    }));
    Ok(())
}

fn validate_password(value: &str) -> Result<(), AuthFailure> {
    let len = value.len();
    if (12..=128).contains(&len) {
        Ok(())
    } else {
        Err(AuthFailure::InvalidRequest)
    }
}

fn validate_message_content(content: &str) -> Result<(), AuthFailure> {
    let len = content.len();
    if (1..=2000).contains(&len) {
        Ok(())
    } else {
        Err(AuthFailure::InvalidRequest)
    }
}

fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow!("password hash failed: {e}"))?
        .to_string();
    Ok(hash)
}

fn verify_password(stored_hash: &str, supplied_password: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored_hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(supplied_password.as_bytes(), &parsed)
        .is_ok()
}

fn issue_tokens(
    state: &AppState,
    user_id: UserId,
    username: &str,
    session_id: &str,
) -> anyhow::Result<(String, String, [u8; 32])> {
    let mut claims = Claims::new_expires_in(&Duration::from_secs(ACCESS_TOKEN_TTL_SECS as u64))
        .map_err(|e| anyhow!("claims init failed: {e}"))?;
    claims
        .subject(&user_id.to_string())
        .map_err(|e| anyhow!("claim sub failed: {e}"))?;
    claims
        .add_additional("username", username)
        .map_err(|e| anyhow!("claim username failed: {e}"))?;

    let access_token = local::encrypt(&state.token_key, &claims, None, None)
        .map_err(|e| anyhow!("access token mint failed: {e}"))?;

    let mut refresh_secret = [0_u8; 32];
    OsRng.fill_bytes(&mut refresh_secret);
    let refresh_secret = URL_SAFE_NO_PAD.encode(refresh_secret);
    let refresh_token = format!("{session_id}.{refresh_secret}");
    let refresh_hash = hash_refresh_token(&refresh_token);

    Ok((access_token, refresh_token, refresh_hash))
}

fn verify_access_token(state: &AppState, token: &str) -> anyhow::Result<Claims> {
    let untrusted = UntrustedToken::<Local, V4>::try_from(token).map_err(|e| anyhow!("{e}"))?;
    let validation_rules = ClaimsValidationRules::new();
    let trusted = local::decrypt(&state.token_key, &untrusted, &validation_rules, None, None)
        .map_err(|e| anyhow!("token decrypt failed: {e}"))?;
    trusted
        .payload_claims()
        .cloned()
        .ok_or_else(|| anyhow!("token claims missing"))
}

async fn authenticate(state: &AppState, headers: &HeaderMap) -> Result<AuthContext, AuthFailure> {
    let access_token = bearer_token(headers).ok_or(AuthFailure::Unauthorized)?;
    authenticate_with_token(state, access_token).await
}

async fn authenticate_with_token(
    state: &AppState,
    access_token: &str,
) -> Result<AuthContext, AuthFailure> {
    ensure_db_schema(state).await?;
    let claims = verify_access_token(state, access_token).map_err(|_| AuthFailure::Unauthorized)?;
    let user_id = claims
        .get_claim("sub")
        .and_then(serde_json::Value::as_str)
        .ok_or(AuthFailure::Unauthorized)?;
    let username = find_username_by_subject(state, user_id)
        .await
        .ok_or(AuthFailure::Unauthorized)?;
    let user_id = UserId::try_from(user_id.to_owned()).map_err(|_| AuthFailure::Unauthorized)?;
    Ok(AuthContext { user_id, username })
}

async fn find_username_by_subject(state: &AppState, user_id: &str) -> Option<String> {
    if let Some(pool) = &state.db_pool {
        let row = sqlx::query("SELECT username FROM users WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .ok()?;
        return row.and_then(|value| value.try_get("username").ok());
    }
    state.user_ids.read().await.get(user_id).cloned()
}

async fn find_username_by_user_id(state: &AppState, user_id: UserId) -> Option<String> {
    if let Some(pool) = &state.db_pool {
        let row = sqlx::query("SELECT username FROM users WHERE user_id = $1")
            .bind(user_id.to_string())
            .fetch_optional(pool)
            .await
            .ok()?;
        return row.and_then(|value| value.try_get("username").ok());
    }
    state
        .user_ids
        .read()
        .await
        .get(&user_id.to_string())
        .cloned()
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let header = headers.get(AUTHORIZATION)?;
    let header = header.to_str().ok()?;
    header.strip_prefix("Bearer ")
}

fn hash_refresh_token(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}

fn now_unix() -> i64 {
    let now = SystemTime::now();
    let seconds = now
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs();
    i64::try_from(seconds).unwrap_or(i64::MAX)
}

fn outbound_event<T: Serialize>(event_type: &str, data: T) -> String {
    let envelope = Envelope {
        v: PROTOCOL_VERSION,
        t: EventType::try_from(event_type.to_owned()).unwrap_or_else(|_| {
            EventType::try_from(String::from("ready")).expect("valid event type")
        }),
        d: serde_json::to_value(data).unwrap_or(serde_json::Value::Null),
    };

    serde_json::to_string(&envelope)
        .unwrap_or_else(|_| String::from(r#"{"v":1,"t":"ready","d":{}}"#))
}

fn channel_key(guild_id: &str, channel_id: &str) -> String {
    format!("{guild_id}:{channel_id}")
}

async fn enforce_auth_route_rate_limit(
    state: &AppState,
    headers: &HeaderMap,
    route: &str,
) -> Result<(), AuthFailure> {
    let ip = extract_client_ip(headers);
    let key = format!("{route}:{ip}");
    let now = now_unix();

    let mut hits = state.auth_route_hits.write().await;
    let route_hits = hits.entry(key).or_default();
    route_hits.retain(|timestamp| now.saturating_sub(*timestamp) < 60);
    let max_hits =
        usize::try_from(state.runtime.auth_route_requests_per_minute).unwrap_or(usize::MAX);
    if route_hits.len() >= max_hits {
        tracing::warn!(event = "auth.rate_limit", route = %route, ip = %ip);
        return Err(AuthFailure::RateLimited);
    }
    route_hits.push(now);
    Ok(())
}

fn extract_client_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map_or_else(|| String::from("unknown"), ToOwned::to_owned)
}

#[derive(Debug)]
enum AuthFailure {
    InvalidRequest,
    Unauthorized,
    Forbidden,
    NotFound,
    RateLimited,
    PayloadTooLarge,
    QuotaExceeded,
    Internal,
}

impl IntoResponse for AuthFailure {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::InvalidRequest => (
                StatusCode::BAD_REQUEST,
                Json(AuthError {
                    error: "invalid_request",
                }),
            )
                .into_response(),
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                Json(AuthError {
                    error: "invalid_credentials",
                }),
            )
                .into_response(),
            Self::Forbidden => (
                StatusCode::FORBIDDEN,
                Json(AuthError { error: "forbidden" }),
            )
                .into_response(),
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                Json(AuthError { error: "not_found" }),
            )
                .into_response(),
            Self::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                Json(AuthError {
                    error: "rate_limited",
                }),
            )
                .into_response(),
            Self::PayloadTooLarge => (
                StatusCode::PAYLOAD_TOO_LARGE,
                Json(AuthError {
                    error: "payload_too_large",
                }),
            )
                .into_response(),
            Self::QuotaExceeded => (
                StatusCode::CONFLICT,
                Json(AuthError {
                    error: "quota_exceeded",
                }),
            )
                .into_response(),
            Self::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthError {
                    error: "internal_error",
                }),
            )
                .into_response(),
        }
    }
}

pub fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .json()
        .with_env_filter(filter)
        .with_current_span(true)
        .with_span_list(true)
        .init();
}

#[cfg(test)]
mod tests {
    use super::{
        build_router, channel_key, AppConfig, AppState, AuthResponse, ConnectionControl,
        DEFAULT_MAX_GATEWAY_EVENT_BYTES,
    };
    use axum::{body::Body, http::Request, http::StatusCode};
    use serde_json::{json, Value};
    use std::{collections::HashMap, time::Duration};
    use tokio::sync::{mpsc, watch};
    use tower::ServiceExt;
    use uuid::Uuid;

    async fn register_and_login(app: &axum::Router, ip: &str) -> AuthResponse {
        let register = Request::builder()
            .method("POST")
            .uri("/auth/register")
            .header("content-type", "application/json")
            .header("x-forwarded-for", ip)
            .body(Body::from(
                json!({"username":"alice_1","password":"super-secure-password"}).to_string(),
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
                json!({"username":"alice_1","password":"super-secure-password"}).to_string(),
            ))
            .unwrap();
        let login_response = app.clone().oneshot(login).await.unwrap();
        assert_eq!(login_response.status(), StatusCode::OK);
        let login_bytes = axum::body::to_bytes(login_response.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&login_bytes).unwrap()
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
    async fn gateway_broadcasts_message_to_subscribed_connection() {
        let state = AppState::new(&AppConfig::default()).unwrap();
        let user_id = super::UserId::new();
        let username = super::Username::try_from(String::from("alice_1")).unwrap();
        state.users.write().await.insert(
            username.as_str().to_owned(),
            super::UserRecord {
                id: user_id,
                username: username.clone(),
                password_hash: super::hash_password("super-secure-password").unwrap(),
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
        let mut guild = super::GuildRecord {
            members: HashMap::new(),
            banned_members: std::collections::HashSet::new(),
            channels: HashMap::new(),
        };
        guild.members.insert(user_id, super::Role::Owner);
        guild.channels.insert(
            channel_id.clone(),
            super::ChannelRecord {
                messages: Vec::new(),
            },
        );
        state.guilds.write().await.insert(guild_id.clone(), guild);

        let (tx, mut rx) = mpsc::channel::<String>(4);
        super::add_subscription(&state, Uuid::new_v4(), channel_key("g", "c"), tx).await;

        let auth = super::AuthContext {
            user_id,
            username: username.as_str().to_owned(),
        };
        let result = super::create_message_internal(
            &state,
            &auth,
            &guild_id,
            &channel_id,
            String::from("hello"),
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
        super::broadcast_channel_event(&state, &channel_key("g", "c"), String::from("second"))
            .await;

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
}
