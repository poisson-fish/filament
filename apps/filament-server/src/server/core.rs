use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

use anyhow::anyhow;
use filament_core::{
    ChannelKind, ChannelPermissionOverwrite, MarkdownToken, PermissionSet, Role, UserId, Username,
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
pub(crate) type GuildIpBanMap = HashMap<String, Vec<GuildIpBanRecord>>;
pub(crate) type GuildRoleMap = HashMap<String, HashMap<String, WorkspaceRoleRecord>>;
pub(crate) type GuildRoleAssignmentMap = HashMap<String, HashMap<UserId, HashSet<String>>>;
pub(crate) type GuildChannelPermissionOverrideMap =
    HashMap<String, HashMap<String, ChannelPermissionOverrideRecord>>;
pub(crate) type VoiceParticipantsByChannel = HashMap<String, HashMap<UserId, VoiceParticipant>>;

pub const DEFAULT_JSON_BODY_LIMIT_BYTES: usize = 1_048_576;
pub const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 10;
pub const DEFAULT_RATE_LIMIT_REQUESTS_PER_MINUTE: u32 = 600;
pub const DEFAULT_AUTH_ROUTE_REQUESTS_PER_MINUTE: u32 = 60;
pub const ACCESS_TOKEN_TTL_SECS: i64 = 15 * 60;
pub const REFRESH_TOKEN_TTL_SECS: i64 = 30 * 24 * 60 * 60;
pub const DEFAULT_GATEWAY_INGRESS_EVENTS_PER_WINDOW: u32 = 60;
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
pub const DEFAULT_MEDIA_TOKEN_REQUESTS_PER_MINUTE: u32 = 60;
pub const DEFAULT_MEDIA_PUBLISH_REQUESTS_PER_MINUTE: u32 = 24;
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
pub(crate) const MAX_TRACKED_VOICE_CHANNELS: usize = 1024;
pub(crate) const MAX_TRACKED_VOICE_PARTICIPANTS_PER_CHANNEL: usize = 512;
pub(crate) const METRICS_TEXT_CONTENT_TYPE: &str = "text/plain; version=0.0.4; charset=utf-8";

pub(crate) static METRICS_STATE: OnceLock<MetricsState> = OnceLock::new();

#[derive(Default)]
pub(crate) struct MetricsState {
    pub(crate) auth_failures: Mutex<HashMap<&'static str, u64>>,
    pub(crate) rate_limit_hits: Mutex<HashMap<(&'static str, &'static str), u64>>,
    pub(crate) ws_disconnects: Mutex<HashMap<&'static str, u64>>,
    pub(crate) gateway_events_emitted: Mutex<HashMap<(String, String), u64>>,
    pub(crate) gateway_events_dropped: Mutex<HashMap<(String, String, String), u64>>,
    pub(crate) gateway_events_unknown_received: Mutex<HashMap<(String, String), u64>>,
    pub(crate) gateway_events_parse_rejected: Mutex<HashMap<(String, String), u64>>,
    pub(crate) voice_sync_repairs: Mutex<HashMap<String, u64>>,
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
    pub server_owner_user_id: Option<UserId>,
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
            server_owner_user_id: None,
            attachment_root: PathBuf::from("./data/attachments"),
            database_url: None,
        }
    }
}

#[derive(Clone)]
pub(crate) struct RuntimeSecurityConfig {
    pub(crate) auth_route_requests_per_minute: u32,
    pub(crate) directory_join_requests_per_minute_per_ip: u32,
    pub(crate) directory_join_requests_per_minute_per_user: u32,
    pub(crate) audit_list_limit_max: usize,
    pub(crate) guild_ip_ban_max_entries: usize,
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
    pub(crate) server_owner_user_id: Option<UserId>,
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
    pub(crate) session_store: SessionStore,
    pub(crate) token_key: Arc<SymmetricKey<V4>>,
    pub(crate) dummy_password_hash: Arc<String>,
    pub(crate) auth_route_hits: Arc<RwLock<HashMap<String, Vec<i64>>>>,
    pub(crate) directory_join_ip_hits: Arc<RwLock<HashMap<String, Vec<i64>>>>,
    pub(crate) directory_join_user_hits: Arc<RwLock<HashMap<String, Vec<i64>>>>,
    pub(crate) user_ip_observation_writes: Arc<RwLock<HashMap<String, i64>>>,
    pub(crate) media_token_hits: Arc<RwLock<HashMap<String, Vec<i64>>>>,
    pub(crate) media_publish_hits: Arc<RwLock<HashMap<String, Vec<i64>>>>,
    pub(crate) media_subscribe_leases: Arc<RwLock<HashMap<String, Vec<i64>>>>,
    pub(crate) membership_store: MembershipStore,
    #[allow(dead_code)]
    pub(crate) guilds: Arc<RwLock<HashMap<String, GuildRecord>>>,
    #[allow(dead_code)]
    pub(crate) guild_roles: Arc<RwLock<GuildRoleMap>>,
    #[allow(dead_code)]
    pub(crate) guild_role_assignments: Arc<RwLock<GuildRoleAssignmentMap>>,
    #[allow(dead_code)]
    pub(crate) guild_channel_permission_overrides: Arc<RwLock<GuildChannelPermissionOverrideMap>>,
    pub(crate) user_ip_observations: Arc<RwLock<HashMap<(UserId, IpNetwork), i64>>>,
    pub(crate) guild_ip_bans: Arc<RwLock<GuildIpBanMap>>,
    pub(crate) realtime_registry: RealtimeRegistry,
    #[allow(dead_code)]
    pub(crate) subscriptions: Arc<RwLock<Subscriptions>>,
    #[allow(dead_code)]
    pub(crate) connection_senders: Arc<RwLock<HashMap<Uuid, mpsc::Sender<String>>>>,
    #[allow(dead_code)]
    pub(crate) connection_controls: Arc<RwLock<HashMap<Uuid, watch::Sender<ConnectionControl>>>>,
    #[allow(dead_code)]
    pub(crate) connection_presence: Arc<RwLock<HashMap<Uuid, ConnectionPresence>>>,
    #[allow(dead_code)]
    pub(crate) voice_participants: Arc<RwLock<VoiceParticipantsByChannel>>,
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
        let guilds = Arc::new(RwLock::new(HashMap::new()));
        let guild_roles = Arc::new(RwLock::new(HashMap::new()));
        let guild_role_assignments = Arc::new(RwLock::new(HashMap::new()));
        let guild_channel_permission_overrides = Arc::new(RwLock::new(HashMap::new()));
        let subscriptions = Arc::new(RwLock::new(HashMap::new()));
        let connection_senders = Arc::new(RwLock::new(HashMap::new()));
        let connection_controls = Arc::new(RwLock::new(HashMap::new()));
        let connection_presence = Arc::new(RwLock::new(HashMap::new()));
        let voice_participants = Arc::new(RwLock::new(HashMap::new()));
        let membership_store = MembershipStore::new(
            guilds.clone(),
            guild_roles.clone(),
            guild_role_assignments.clone(),
            guild_channel_permission_overrides.clone(),
        );
        let realtime_registry = RealtimeRegistry::new(
            subscriptions.clone(),
            connection_senders.clone(),
            connection_controls.clone(),
            connection_presence.clone(),
            voice_participants.clone(),
        );

        Ok(Self {
            db_pool,
            db_init: Arc::new(OnceCell::new()),
            users: Arc::new(RwLock::new(HashMap::new())),
            user_ids: Arc::new(RwLock::new(HashMap::new())),
            session_store: SessionStore::new(),
            token_key: Arc::new(token_key),
            dummy_password_hash: Arc::new(dummy_password_hash),
            auth_route_hits: Arc::new(RwLock::new(HashMap::new())),
            directory_join_ip_hits: Arc::new(RwLock::new(HashMap::new())),
            directory_join_user_hits: Arc::new(RwLock::new(HashMap::new())),
            user_ip_observation_writes: Arc::new(RwLock::new(HashMap::new())),
            media_token_hits: Arc::new(RwLock::new(HashMap::new())),
            media_publish_hits: Arc::new(RwLock::new(HashMap::new())),
            media_subscribe_leases: Arc::new(RwLock::new(HashMap::new())),
            membership_store,
            guilds,
            guild_roles,
            guild_role_assignments,
            guild_channel_permission_overrides,
            user_ip_observations: Arc::new(RwLock::new(HashMap::new())),
            guild_ip_bans: Arc::new(RwLock::new(HashMap::new())),
            realtime_registry,
            subscriptions,
            connection_senders,
            connection_controls,
            connection_presence,
            voice_participants,
            attachment_store: Arc::new(attachment_store),
            attachments: Arc::new(RwLock::new(HashMap::new())),
            friendship_requests: Arc::new(RwLock::new(HashMap::new())),
            friendships: Arc::new(RwLock::new(HashSet::new())),
            audit_logs: Arc::new(RwLock::new(Vec::new())),
            search,
            search_bootstrapped: Arc::new(OnceCell::new()),
            runtime: Arc::new(RuntimeSecurityConfig {
                auth_route_requests_per_minute: config.auth_route_requests_per_minute,
                directory_join_requests_per_minute_per_ip: config
                    .directory_join_requests_per_minute_per_ip,
                directory_join_requests_per_minute_per_user: config
                    .directory_join_requests_per_minute_per_user,
                audit_list_limit_max: config.audit_list_limit_max,
                guild_ip_ban_max_entries: config.guild_ip_ban_max_entries,
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
                server_owner_user_id: config.server_owner_user_id,
                livekit_token_ttl: config.livekit_token_ttl,
                captcha: captcha.map(Arc::new),
            }),
            livekit: livekit.map(Arc::new),
        })
    }
}

#[derive(Clone)]
pub(crate) struct MembershipStore {
    guilds: Arc<RwLock<HashMap<String, GuildRecord>>>,
    guild_roles: Arc<RwLock<GuildRoleMap>>,
    guild_role_assignments: Arc<RwLock<GuildRoleAssignmentMap>>,
    guild_channel_permission_overrides: Arc<RwLock<GuildChannelPermissionOverrideMap>>,
}

impl MembershipStore {
    pub(crate) fn new(
        guilds: Arc<RwLock<HashMap<String, GuildRecord>>>,
        guild_roles: Arc<RwLock<GuildRoleMap>>,
        guild_role_assignments: Arc<RwLock<GuildRoleAssignmentMap>>,
        guild_channel_permission_overrides: Arc<RwLock<GuildChannelPermissionOverrideMap>>,
    ) -> Self {
        Self {
            guilds,
            guild_roles,
            guild_role_assignments,
            guild_channel_permission_overrides,
        }
    }

    pub(crate) fn guilds(&self) -> &Arc<RwLock<HashMap<String, GuildRecord>>> {
        &self.guilds
    }

    pub(crate) fn guild_roles(&self) -> &Arc<RwLock<GuildRoleMap>> {
        &self.guild_roles
    }

    pub(crate) fn guild_role_assignments(&self) -> &Arc<RwLock<GuildRoleAssignmentMap>> {
        &self.guild_role_assignments
    }

    pub(crate) fn guild_channel_permission_overrides(
        &self,
    ) -> &Arc<RwLock<GuildChannelPermissionOverrideMap>> {
        &self.guild_channel_permission_overrides
    }
}

#[derive(Clone)]
pub(crate) struct RealtimeRegistry {
    subscriptions: Arc<RwLock<Subscriptions>>,
    connection_senders: Arc<RwLock<HashMap<Uuid, mpsc::Sender<String>>>>,
    connection_controls: Arc<RwLock<HashMap<Uuid, watch::Sender<ConnectionControl>>>>,
    connection_presence: Arc<RwLock<HashMap<Uuid, ConnectionPresence>>>,
    voice_participants: Arc<RwLock<VoiceParticipantsByChannel>>,
}

impl RealtimeRegistry {
    pub(crate) fn new(
        subscriptions: Arc<RwLock<Subscriptions>>,
        connection_senders: Arc<RwLock<HashMap<Uuid, mpsc::Sender<String>>>>,
        connection_controls: Arc<RwLock<HashMap<Uuid, watch::Sender<ConnectionControl>>>>,
        connection_presence: Arc<RwLock<HashMap<Uuid, ConnectionPresence>>>,
        voice_participants: Arc<RwLock<VoiceParticipantsByChannel>>,
    ) -> Self {
        Self {
            subscriptions,
            connection_senders,
            connection_controls,
            connection_presence,
            voice_participants,
        }
    }

    pub(crate) fn subscriptions(&self) -> &Arc<RwLock<Subscriptions>> {
        &self.subscriptions
    }

    pub(crate) fn connection_senders(&self) -> &Arc<RwLock<HashMap<Uuid, mpsc::Sender<String>>>> {
        &self.connection_senders
    }

    pub(crate) fn connection_controls(
        &self,
    ) -> &Arc<RwLock<HashMap<Uuid, watch::Sender<ConnectionControl>>>> {
        &self.connection_controls
    }

    pub(crate) fn connection_presence(&self) -> &Arc<RwLock<HashMap<Uuid, ConnectionPresence>>> {
        &self.connection_presence
    }

    pub(crate) fn voice_participants(&self) -> &Arc<RwLock<VoiceParticipantsByChannel>> {
        &self.voice_participants
    }
}

#[derive(Clone, Default)]
pub(crate) struct SessionStore {
    sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
    used_refresh_tokens: Arc<RwLock<HashMap<[u8; 32], String>>>,
}

impl SessionStore {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) async fn insert(&self, session_id: String, record: SessionRecord) {
        self.sessions.write().await.insert(session_id, record);
    }

    pub(crate) async fn revoke_if_replayed_token(&self, token_hash: [u8; 32]) -> Option<String> {
        let replayed_session_id = self
            .used_refresh_tokens
            .read()
            .await
            .get(&token_hash)
            .cloned();
        if let Some(session_id) = replayed_session_id.clone() {
            if let Some(session) = self.sessions.write().await.get_mut(&session_id) {
                session.revoked = true;
            }
        }
        replayed_session_id
    }

    pub(crate) async fn rotate_refresh_hash(
        &self,
        session_id: &str,
        token_hash: [u8; 32],
        next_hash: [u8; 32],
        now_unix: i64,
        next_expires_at_unix: i64,
    ) -> Result<UserId, ()> {
        let previous_hash = {
            let mut sessions = self.sessions.write().await;
            let session = sessions.get_mut(session_id).ok_or(())?;
            if session.revoked
                || session.expires_at_unix < now_unix
                || session.refresh_token_hash != token_hash
            {
                return Err(());
            }
            let previous_hash = session.refresh_token_hash;
            session.refresh_token_hash = next_hash;
            session.expires_at_unix = next_expires_at_unix;
            (session.user_id, previous_hash)
        };

        self.used_refresh_tokens
            .write()
            .await
            .insert(previous_hash.1, session_id.to_owned());
        Ok(previous_hash.0)
    }

    pub(crate) async fn validate_refresh_token(
        &self,
        session_id: &str,
        token_hash: [u8; 32],
        now_unix: i64,
    ) -> Result<UserId, ()> {
        let sessions = self.sessions.read().await;
        let session = sessions.get(session_id).ok_or(())?;
        if session.revoked
            || session.expires_at_unix < now_unix
            || session.refresh_token_hash != token_hash
        {
            return Err(());
        }
        Ok(session.user_id)
    }

    pub(crate) async fn revoke_with_token(
        &self,
        session_id: &str,
        token_hash: [u8; 32],
    ) -> Result<UserId, ()> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(session_id).ok_or(())?;
        if session.refresh_token_hash != token_hash {
            return Err(());
        }
        session.revoked = true;
        Ok(session.user_id)
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
pub(crate) struct WorkspaceRoleRecord {
    pub(crate) role_id: String,
    pub(crate) name: String,
    pub(crate) position: i32,
    pub(crate) is_system: bool,
    pub(crate) system_key: Option<String>,
    pub(crate) permissions_allow: PermissionSet,
    pub(crate) created_at_unix: i64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ChannelPermissionOverrideRecord {
    pub(crate) role_overrides: HashMap<String, ChannelPermissionOverwrite>,
    pub(crate) member_overrides: HashMap<UserId, ChannelPermissionOverwrite>,
}

#[derive(Debug, Clone)]
pub(crate) struct GuildIpBanRecord {
    pub(crate) ban_id: String,
    pub(crate) ip_network: IpNetwork,
    pub(crate) source_user_id: Option<UserId>,
    pub(crate) reason: String,
    pub(crate) created_at_unix: i64,
    pub(crate) expires_at_unix: Option<i64>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum VoiceStreamKind {
    Microphone,
    Camera,
    ScreenShare,
}

#[derive(Debug, Clone)]
pub(crate) struct VoiceParticipant {
    pub(crate) user_id: UserId,
    pub(crate) identity: String,
    pub(crate) joined_at_unix: i64,
    pub(crate) updated_at_unix: i64,
    pub(crate) expires_at_unix: i64,
    pub(crate) is_muted: bool,
    pub(crate) is_deafened: bool,
    pub(crate) is_speaking: bool,
    pub(crate) is_video_enabled: bool,
    pub(crate) is_screen_share_enabled: bool,
    pub(crate) published_streams: HashSet<VoiceStreamKind>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn session_store_replay_detection_revokes_session() {
        let store = SessionStore::new();
        let user_id = UserId::new();
        let initial_hash = [1_u8; 32];
        let replay_hash = [9_u8; 32];
        let session_id = String::from("session-1");
        store
            .insert(
                session_id.clone(),
                SessionRecord {
                    user_id,
                    refresh_token_hash: initial_hash,
                    expires_at_unix: i64::MAX,
                    revoked: false,
                },
            )
            .await;
        let _ = store
            .rotate_refresh_hash(&session_id, initial_hash, replay_hash, 0, i64::MAX)
            .await
            .expect("rotation should succeed");

        let replay = store.revoke_if_replayed_token(initial_hash).await;
        assert_eq!(replay.as_deref(), Some(session_id.as_str()));
        let second_rotate = store
            .rotate_refresh_hash(&session_id, replay_hash, [2_u8; 32], 0, i64::MAX)
            .await;
        assert!(second_rotate.is_err());
    }

    #[tokio::test]
    async fn session_store_revoke_with_token_rejects_hash_mismatch() {
        let store = SessionStore::new();
        let user_id = UserId::new();
        let session_id = String::from("session-2");
        store
            .insert(
                session_id.clone(),
                SessionRecord {
                    user_id,
                    refresh_token_hash: [3_u8; 32],
                    expires_at_unix: i64::MAX,
                    revoked: false,
                },
            )
            .await;

        let rejected = store.revoke_with_token(&session_id, [4_u8; 32]).await;
        assert!(rejected.is_err());
    }

    #[tokio::test]
    async fn session_store_validate_and_rotate_refresh_hash() {
        let store = SessionStore::new();
        let user_id = UserId::new();
        let initial_hash = [5_u8; 32];
        let rotated_hash = [6_u8; 32];
        let session_id = String::from("session-3");
        store
            .insert(
                session_id.clone(),
                SessionRecord {
                    user_id,
                    refresh_token_hash: initial_hash,
                    expires_at_unix: 100,
                    revoked: false,
                },
            )
            .await;

        let validated = store
            .validate_refresh_token(&session_id, initial_hash, 50)
            .await
            .expect("token should validate");
        assert_eq!(validated, user_id);

        let rotated = store
            .rotate_refresh_hash(&session_id, initial_hash, rotated_hash, 50, 200)
            .await
            .expect("rotation should succeed");
        assert_eq!(rotated, user_id);

        let replay = store.revoke_if_replayed_token(initial_hash).await;
        assert_eq!(replay.as_deref(), Some(session_id.as_str()));
    }

    #[tokio::test]
    async fn membership_store_shares_backing_maps_with_app_state_fields() {
        let state = AppState::new(&AppConfig::default()).expect("state should initialize");
        let guild_id = String::from("guild-membership-store");

        state
            .membership_store
            .guilds()
            .write()
            .await
            .insert(
            guild_id.clone(),
            GuildRecord {
                name: String::from("guild"),
                visibility: GuildVisibility::Private,
                created_by_user_id: UserId::new(),
                members: HashMap::new(),
                banned_members: HashSet::new(),
                channels: HashMap::new(),
            },
        );

        let read = state.membership_store.guilds().read().await;
        assert!(read.contains_key(&guild_id));
    }

    #[tokio::test]
    async fn membership_store_role_writes_update_legacy_role_map_field() {
        let state = AppState::new(&AppConfig::default()).expect("state should initialize");
        let guild_id = String::from("guild-membership-store-roles");
        let role_id = String::from("role-1");

        state
            .membership_store
            .guild_roles()
            .write()
            .await
            .insert(
                guild_id.clone(),
                HashMap::from([(
                    role_id.clone(),
                    WorkspaceRoleRecord {
                        role_id: role_id.clone(),
                        name: String::from("Member"),
                        position: 1,
                        is_system: false,
                        system_key: None,
                        permissions_allow: PermissionSet::empty(),
                        created_at_unix: 1,
                    },
                )]),
            );

        let read = state.membership_store.guild_roles().read().await;
        let role_map = read.get(&guild_id).expect("guild role map should exist");
        assert!(role_map.contains_key(&role_id));
    }

    #[tokio::test]
    async fn realtime_registry_shares_backing_maps_with_app_state_fields() {
        let state = AppState::new(&AppConfig::default()).expect("state should initialize");
        let connection_id = Uuid::new_v4();

        state
            .realtime_registry
            .connection_presence()
            .write()
            .await
            .insert(
            connection_id,
            ConnectionPresence {
                user_id: UserId::new(),
                guild_ids: HashSet::new(),
            },
        );

        let presence = state.realtime_registry.connection_presence().read().await;
        assert!(presence.contains_key(&connection_id));
    }
}
