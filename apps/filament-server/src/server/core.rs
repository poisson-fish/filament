use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

use anyhow::anyhow;
use filament_core::{
    ChannelKind, ChannelPermissionOverwrite, MarkdownToken, Role, UserId, Username,
};
use object_store::local::LocalFileSystem;
use pasetors::{keys::SymmetricKey, version4::V4};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, PgPool};
use tantivy::schema::Field;
use tokio::sync::{mpsc, oneshot, watch, OnceCell, RwLock};
use uuid::Uuid;

use super::{
    auth::{build_captcha_config, build_livekit_config, hash_password},
    directory_contract::{
        IpNetwork, DEFAULT_AUDIT_LIST_LIMIT_MAX, DEFAULT_DIRECTORY_JOIN_REQUESTS_PER_MINUTE_PER_IP,
        DEFAULT_DIRECTORY_JOIN_REQUESTS_PER_MINUTE_PER_USER, DEFAULT_GUILD_IP_BAN_MAX_ENTRIES,
    },
    errors::AuthFailure,
    realtime::init_search_service,
};

pub(crate) type ChannelSubscriptions = HashMap<Uuid, mpsc::Sender<String>>;
pub(crate) type Subscriptions = HashMap<String, ChannelSubscriptions>;

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
pub const DEFAULT_MAX_PROFILE_AVATAR_BYTES: usize = 2 * 1024 * 1024;
pub const DEFAULT_USER_ATTACHMENT_QUOTA_BYTES: u64 = 250 * 1024 * 1024;
pub const DEFAULT_SEARCH_QUERY_MAX_CHARS: usize = 256;
pub const DEFAULT_SEARCH_RESULT_LIMIT: usize = 20;
pub const DEFAULT_SEARCH_RESULT_LIMIT_MAX: usize = 50;
pub const DEFAULT_SEARCH_QUERY_TIMEOUT_MILLIS: u64 = 200;
pub const DEFAULT_MEDIA_TOKEN_REQUESTS_PER_MINUTE: u32 = 20;
pub const DEFAULT_MEDIA_PUBLISH_REQUESTS_PER_MINUTE: u32 = 6;
pub const DEFAULT_LIVEKIT_TOKEN_TTL_SECS: u64 = 5 * 60;
pub const DEFAULT_MEDIA_SUBSCRIBE_TOKEN_CAP_PER_CHANNEL: usize = 3;
pub const DEFAULT_MAX_CREATED_GUILDS_PER_USER: usize = 5;
pub const DEFAULT_CAPTCHA_VERIFY_TIMEOUT_SECS: u64 = 3;
pub const MAX_LIVEKIT_TOKEN_TTL_SECS: u64 = 5 * 60;
pub(crate) const MAX_CAPTCHA_TOKEN_CHARS: usize = 4096;
pub(crate) const MIN_CAPTCHA_TOKEN_CHARS: usize = 20;
pub(crate) const LOGIN_LOCK_THRESHOLD: u8 = 5;
pub(crate) const LOGIN_LOCK_SECS: i64 = 30;
pub(crate) const MAX_HISTORY_LIMIT: usize = 100;
pub(crate) const MAX_MIME_SNIFF_BYTES: usize = 8192;
pub(crate) const MAX_SEARCH_TERMS: usize = 20;
pub(crate) const MAX_SEARCH_WILDCARDS: usize = 4;
pub(crate) const MAX_SEARCH_FUZZY: usize = 2;
pub(crate) const SEARCH_INDEX_QUEUE_CAPACITY: usize = 1024;
pub(crate) const MAX_SEARCH_RECONCILE_DOCS: usize = 10_000;
pub(crate) const MAX_REACTION_EMOJI_CHARS: usize = 32;
pub(crate) const MAX_USER_LOOKUP_IDS: usize = 64;
pub(crate) const MAX_ATTACHMENTS_PER_MESSAGE: usize = 5;
pub(crate) const MAX_PROFILE_AVATAR_MIME_CHARS: usize = 64;
pub(crate) const MAX_PROFILE_AVATAR_OBJECT_KEY_CHARS: usize = 128;
pub(crate) const METRICS_TEXT_CONTENT_TYPE: &str = "text/plain; version=0.0.4; charset=utf-8";

pub(crate) static METRICS_STATE: OnceLock<MetricsState> = OnceLock::new();

#[derive(Default)]
pub(crate) struct MetricsState {
    pub(crate) auth_failures: Mutex<HashMap<&'static str, u64>>,
    pub(crate) rate_limit_hits: Mutex<HashMap<(&'static str, &'static str), u64>>,
    pub(crate) ws_disconnects: Mutex<HashMap<&'static str, u64>>,
}

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
    pub max_profile_avatar_bytes: usize,
    pub user_attachment_quota_bytes: u64,
    pub search_query_max_chars: usize,
    pub search_result_limit_max: usize,
    pub search_query_timeout: Duration,
    pub media_token_requests_per_minute: u32,
    pub media_publish_requests_per_minute: u32,
    pub directory_join_requests_per_minute_per_ip: u32,
    pub directory_join_requests_per_minute_per_user: u32,
    pub audit_list_limit_max: usize,
    pub guild_ip_ban_max_entries: usize,
    pub media_subscribe_token_cap_per_channel: usize,
    pub max_created_guilds_per_user: usize,
    pub trusted_proxy_cidrs: Vec<IpNetwork>,
    pub livekit_token_ttl: Duration,
    pub captcha_hcaptcha_site_key: Option<String>,
    pub captcha_hcaptcha_secret: Option<String>,
    pub captcha_verify_url: String,
    pub captcha_verify_timeout: Duration,
    pub livekit_url: String,
    pub livekit_api_key: Option<String>,
    pub livekit_api_secret: Option<String>,
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
            max_profile_avatar_bytes: DEFAULT_MAX_PROFILE_AVATAR_BYTES,
            user_attachment_quota_bytes: DEFAULT_USER_ATTACHMENT_QUOTA_BYTES,
            search_query_max_chars: DEFAULT_SEARCH_QUERY_MAX_CHARS,
            search_result_limit_max: DEFAULT_SEARCH_RESULT_LIMIT_MAX,
            search_query_timeout: Duration::from_millis(DEFAULT_SEARCH_QUERY_TIMEOUT_MILLIS),
            media_token_requests_per_minute: DEFAULT_MEDIA_TOKEN_REQUESTS_PER_MINUTE,
            media_publish_requests_per_minute: DEFAULT_MEDIA_PUBLISH_REQUESTS_PER_MINUTE,
            directory_join_requests_per_minute_per_ip:
                DEFAULT_DIRECTORY_JOIN_REQUESTS_PER_MINUTE_PER_IP,
            directory_join_requests_per_minute_per_user:
                DEFAULT_DIRECTORY_JOIN_REQUESTS_PER_MINUTE_PER_USER,
            audit_list_limit_max: DEFAULT_AUDIT_LIST_LIMIT_MAX,
            guild_ip_ban_max_entries: DEFAULT_GUILD_IP_BAN_MAX_ENTRIES,
            media_subscribe_token_cap_per_channel: DEFAULT_MEDIA_SUBSCRIBE_TOKEN_CAP_PER_CHANNEL,
            max_created_guilds_per_user: DEFAULT_MAX_CREATED_GUILDS_PER_USER,
            trusted_proxy_cidrs: Vec::new(),
            livekit_token_ttl: Duration::from_secs(DEFAULT_LIVEKIT_TOKEN_TTL_SECS),
            captcha_hcaptcha_site_key: None,
            captcha_hcaptcha_secret: None,
            captcha_verify_url: String::from("https://hcaptcha.com/siteverify"),
            captcha_verify_timeout: Duration::from_secs(DEFAULT_CAPTCHA_VERIFY_TIMEOUT_SECS),
            livekit_url: String::from("ws://127.0.0.1:7880"),
            livekit_api_key: None,
            livekit_api_secret: None,
            attachment_root: PathBuf::from("./data/attachments"),
            database_url: None,
        }
    }
}

#[derive(Clone)]
pub(crate) struct RuntimeSecurityConfig {
    pub(crate) auth_route_requests_per_minute: u32,
    pub(crate) gateway_ingress_events_per_window: u32,
    pub(crate) gateway_ingress_window: Duration,
    pub(crate) gateway_outbound_queue: usize,
    pub(crate) max_gateway_event_bytes: usize,
    pub(crate) max_attachment_bytes: usize,
    pub(crate) max_profile_avatar_bytes: usize,
    pub(crate) user_attachment_quota_bytes: u64,
    pub(crate) search_query_max_chars: usize,
    pub(crate) search_result_limit_max: usize,
    pub(crate) search_query_timeout: Duration,
    pub(crate) media_token_requests_per_minute: u32,
    pub(crate) media_publish_requests_per_minute: u32,
    pub(crate) media_subscribe_token_cap_per_channel: usize,
    pub(crate) max_created_guilds_per_user: usize,
    pub(crate) trusted_proxy_cidrs: Arc<Vec<IpNetwork>>,
    pub(crate) livekit_token_ttl: Duration,
    pub(crate) captcha: Option<Arc<CaptchaConfig>>,
}

#[derive(Clone)]
pub(crate) struct LiveKitConfig {
    pub(crate) api_key: String,
    pub(crate) api_secret: String,
    pub(crate) url: String,
}

#[derive(Clone)]
pub(crate) struct CaptchaConfig {
    pub(crate) secret: String,
    pub(crate) verify_url: String,
    pub(crate) verify_timeout: Duration,
}

#[derive(Clone)]
pub(crate) struct SearchService {
    pub(crate) tx: mpsc::Sender<SearchCommand>,
    pub(crate) state: Arc<SearchIndexState>,
}

#[derive(Clone)]
pub(crate) struct SearchIndexState {
    pub(crate) index: tantivy::Index,
    pub(crate) reader: tantivy::IndexReader,
    pub(crate) fields: SearchFields,
}

#[derive(Clone, Copy)]
pub(crate) struct SearchFields {
    pub(crate) message_id: Field,
    pub(crate) guild_id: Field,
    pub(crate) channel_id: Field,
    pub(crate) author_id: Field,
    pub(crate) created_at_unix: Field,
    pub(crate) content: Field,
}

#[derive(Clone)]
pub(crate) struct IndexedMessage {
    pub(crate) message_id: String,
    pub(crate) guild_id: String,
    pub(crate) channel_id: String,
    pub(crate) author_id: String,
    pub(crate) created_at_unix: i64,
    pub(crate) content: String,
}

pub(crate) enum SearchOperation {
    Upsert(IndexedMessage),
    Delete {
        message_id: String,
    },
    Rebuild {
        docs: Vec<IndexedMessage>,
    },
    Reconcile {
        upserts: Vec<IndexedMessage>,
        delete_message_ids: Vec<String>,
    },
}

pub(crate) struct SearchCommand {
    pub(crate) op: SearchOperation,
    pub(crate) ack: Option<oneshot::Sender<Result<(), AuthFailure>>>,
}

#[derive(Clone)]
pub struct AppState {
    pub(crate) db_pool: Option<PgPool>,
    pub(crate) db_init: Arc<OnceCell<()>>,
    pub(crate) users: Arc<RwLock<HashMap<String, UserRecord>>>,
    pub(crate) user_ids: Arc<RwLock<HashMap<String, String>>>,
    pub(crate) sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
    pub(crate) used_refresh_tokens: Arc<RwLock<HashMap<[u8; 32], String>>>,
    pub(crate) token_key: Arc<SymmetricKey<V4>>,
    pub(crate) dummy_password_hash: Arc<String>,
    pub(crate) auth_route_hits: Arc<RwLock<HashMap<String, Vec<i64>>>>,
    pub(crate) media_token_hits: Arc<RwLock<HashMap<String, Vec<i64>>>>,
    pub(crate) media_publish_hits: Arc<RwLock<HashMap<String, Vec<i64>>>>,
    pub(crate) media_subscribe_leases: Arc<RwLock<HashMap<String, Vec<i64>>>>,
    pub(crate) guilds: Arc<RwLock<HashMap<String, GuildRecord>>>,
    pub(crate) subscriptions: Arc<RwLock<Subscriptions>>,
    pub(crate) connection_controls: Arc<RwLock<HashMap<Uuid, watch::Sender<ConnectionControl>>>>,
    pub(crate) connection_presence: Arc<RwLock<HashMap<Uuid, ConnectionPresence>>>,
    pub(crate) attachment_store: Arc<LocalFileSystem>,
    pub(crate) attachments: Arc<RwLock<HashMap<String, AttachmentRecord>>>,
    pub(crate) friendship_requests: Arc<RwLock<HashMap<String, FriendshipRequestRecord>>>,
    pub(crate) friendships: Arc<RwLock<HashSet<(String, String)>>>,
    pub(crate) audit_logs: Arc<RwLock<Vec<serde_json::Value>>>,
    pub(crate) search: SearchService,
    pub(crate) search_bootstrapped: Arc<OnceCell<()>>,
    pub(crate) runtime: Arc<RuntimeSecurityConfig>,
    pub(crate) livekit: Option<Arc<LiveKitConfig>>,
}

impl AppState {
    pub(crate) fn new(config: &AppConfig) -> anyhow::Result<Self> {
        let mut key_bytes = [0_u8; 32];
        OsRng.fill_bytes(&mut key_bytes);
        let token_key = SymmetricKey::<V4>::from(&key_bytes)
            .map_err(|e| anyhow!("token key init failed: {e}"))?;
        let dummy_password_hash = hash_password("filament-dummy-password")?;
        let livekit = build_livekit_config(config)?;
        let captcha = build_captcha_config(config)?;
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
        let search = init_search_service().map_err(|e| anyhow!("search init failed: {e}"))?;

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
            media_token_hits: Arc::new(RwLock::new(HashMap::new())),
            media_publish_hits: Arc::new(RwLock::new(HashMap::new())),
            media_subscribe_leases: Arc::new(RwLock::new(HashMap::new())),
            guilds: Arc::new(RwLock::new(HashMap::new())),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            connection_controls: Arc::new(RwLock::new(HashMap::new())),
            connection_presence: Arc::new(RwLock::new(HashMap::new())),
            attachment_store: Arc::new(attachment_store),
            attachments: Arc::new(RwLock::new(HashMap::new())),
            friendship_requests: Arc::new(RwLock::new(HashMap::new())),
            friendships: Arc::new(RwLock::new(HashSet::new())),
            audit_logs: Arc::new(RwLock::new(Vec::new())),
            search,
            search_bootstrapped: Arc::new(OnceCell::new()),
            runtime: Arc::new(RuntimeSecurityConfig {
                auth_route_requests_per_minute: config.auth_route_requests_per_minute,
                gateway_ingress_events_per_window: config.gateway_ingress_events_per_window,
                gateway_ingress_window: config.gateway_ingress_window,
                gateway_outbound_queue: config.gateway_outbound_queue,
                max_gateway_event_bytes: config.max_gateway_event_bytes,
                max_attachment_bytes: config.max_attachment_bytes,
                max_profile_avatar_bytes: config.max_profile_avatar_bytes,
                user_attachment_quota_bytes: config.user_attachment_quota_bytes,
                search_query_max_chars: config.search_query_max_chars,
                search_result_limit_max: config.search_result_limit_max,
                search_query_timeout: config.search_query_timeout,
                media_token_requests_per_minute: config.media_token_requests_per_minute,
                media_publish_requests_per_minute: config.media_publish_requests_per_minute,
                media_subscribe_token_cap_per_channel: config.media_subscribe_token_cap_per_channel,
                max_created_guilds_per_user: config.max_created_guilds_per_user,
                trusted_proxy_cidrs: Arc::new(config.trusted_proxy_cidrs.clone()),
                livekit_token_ttl: config.livekit_token_ttl,
                captcha: captcha.map(Arc::new),
            }),
            livekit: livekit.map(Arc::new),
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct UserRecord {
    pub(crate) id: UserId,
    pub(crate) username: Username,
    pub(crate) about_markdown: String,
    pub(crate) avatar: Option<ProfileAvatarRecord>,
    pub(crate) avatar_version: i64,
    pub(crate) password_hash: String,
    pub(crate) failed_logins: u8,
    pub(crate) locked_until_unix: Option<i64>,
}

#[derive(Debug, Clone)]
pub(crate) struct ProfileAvatarRecord {
    pub(crate) object_key: String,
    pub(crate) mime_type: String,
    pub(crate) size_bytes: u64,
    pub(crate) sha256_hex: String,
}

#[derive(Debug, Clone)]
pub(crate) struct SessionRecord {
    pub(crate) user_id: UserId,
    pub(crate) refresh_token_hash: [u8; 32],
    pub(crate) expires_at_unix: i64,
    pub(crate) revoked: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GuildVisibility {
    Private,
    Public,
}

#[derive(Debug, Clone)]
pub(crate) struct GuildRecord {
    pub(crate) name: String,
    pub(crate) visibility: GuildVisibility,
    pub(crate) created_by_user_id: UserId,
    pub(crate) members: HashMap<UserId, Role>,
    pub(crate) banned_members: HashSet<UserId>,
    pub(crate) channels: HashMap<String, ChannelRecord>,
}

#[derive(Debug, Clone)]
pub(crate) struct ChannelRecord {
    pub(crate) name: String,
    pub(crate) kind: ChannelKind,
    pub(crate) messages: Vec<MessageRecord>,
    pub(crate) role_overrides: HashMap<Role, ChannelPermissionOverwrite>,
}

#[derive(Debug, Clone)]
pub(crate) struct MessageRecord {
    pub(crate) id: String,
    pub(crate) author_id: UserId,
    pub(crate) content: String,
    pub(crate) markdown_tokens: Vec<MarkdownToken>,
    pub(crate) attachment_ids: Vec<String>,
    pub(crate) created_at_unix: i64,
    pub(crate) reactions: HashMap<String, HashSet<UserId>>,
}

#[derive(Debug, Clone)]
pub(crate) struct AttachmentRecord {
    pub(crate) attachment_id: String,
    pub(crate) guild_id: String,
    pub(crate) channel_id: String,
    pub(crate) owner_id: UserId,
    pub(crate) filename: String,
    pub(crate) mime_type: String,
    pub(crate) size_bytes: u64,
    pub(crate) sha256_hex: String,
    pub(crate) object_key: String,
    pub(crate) message_id: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct FriendshipRequestRecord {
    pub(crate) sender_user_id: UserId,
    pub(crate) recipient_user_id: UserId,
    pub(crate) created_at_unix: i64,
}

#[derive(Debug, Clone)]
pub(crate) struct AuthContext {
    pub(crate) user_id: UserId,
    pub(crate) username: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConnectionControl {
    Open,
    Close,
}

#[derive(Debug, Clone)]
pub(crate) struct ConnectionPresence {
    pub(crate) user_id: UserId,
    pub(crate) guild_ids: HashSet<String>,
}
