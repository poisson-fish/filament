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
const MAX_CAPTCHA_TOKEN_CHARS: usize = 4096;
const MIN_CAPTCHA_TOKEN_CHARS: usize = 20;
const LOGIN_LOCK_THRESHOLD: u8 = 5;
const LOGIN_LOCK_SECS: i64 = 30;
const MAX_HISTORY_LIMIT: usize = 100;
const MAX_MIME_SNIFF_BYTES: usize = 8192;
const MAX_SEARCH_TERMS: usize = 20;
const MAX_SEARCH_WILDCARDS: usize = 4;
const MAX_SEARCH_FUZZY: usize = 2;
const SEARCH_INDEX_QUEUE_CAPACITY: usize = 1024;
const MAX_SEARCH_RECONCILE_DOCS: usize = 10_000;
const MAX_REACTION_EMOJI_CHARS: usize = 32;
const MAX_USER_LOOKUP_IDS: usize = 64;
const MAX_ATTACHMENTS_PER_MESSAGE: usize = 5;
const METRICS_TEXT_CONTENT_TYPE: &str = "text/plain; version=0.0.4; charset=utf-8";

static METRICS_STATE: OnceLock<MetricsState> = OnceLock::new();

#[derive(Default)]
struct MetricsState {
    auth_failures: Mutex<HashMap<&'static str, u64>>,
    rate_limit_hits: Mutex<HashMap<(&'static str, &'static str), u64>>,
    ws_disconnects: Mutex<HashMap<&'static str, u64>>,
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
    pub user_attachment_quota_bytes: u64,
    pub search_query_max_chars: usize,
    pub search_result_limit_max: usize,
    pub search_query_timeout: Duration,
    pub media_token_requests_per_minute: u32,
    pub media_publish_requests_per_minute: u32,
    pub media_subscribe_token_cap_per_channel: usize,
    pub max_created_guilds_per_user: usize,
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
            user_attachment_quota_bytes: DEFAULT_USER_ATTACHMENT_QUOTA_BYTES,
            search_query_max_chars: DEFAULT_SEARCH_QUERY_MAX_CHARS,
            search_result_limit_max: DEFAULT_SEARCH_RESULT_LIMIT_MAX,
            search_query_timeout: Duration::from_millis(DEFAULT_SEARCH_QUERY_TIMEOUT_MILLIS),
            media_token_requests_per_minute: DEFAULT_MEDIA_TOKEN_REQUESTS_PER_MINUTE,
            media_publish_requests_per_minute: DEFAULT_MEDIA_PUBLISH_REQUESTS_PER_MINUTE,
            media_subscribe_token_cap_per_channel: DEFAULT_MEDIA_SUBSCRIBE_TOKEN_CAP_PER_CHANNEL,
            max_created_guilds_per_user: DEFAULT_MAX_CREATED_GUILDS_PER_USER,
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
struct RuntimeSecurityConfig {
    auth_route_requests_per_minute: u32,
    gateway_ingress_events_per_window: u32,
    gateway_ingress_window: Duration,
    gateway_outbound_queue: usize,
    max_gateway_event_bytes: usize,
    max_attachment_bytes: usize,
    user_attachment_quota_bytes: u64,
    search_query_max_chars: usize,
    search_result_limit_max: usize,
    search_query_timeout: Duration,
    media_token_requests_per_minute: u32,
    media_publish_requests_per_minute: u32,
    media_subscribe_token_cap_per_channel: usize,
    max_created_guilds_per_user: usize,
    livekit_token_ttl: Duration,
    captcha: Option<Arc<CaptchaConfig>>,
}

#[derive(Clone)]
struct LiveKitConfig {
    api_key: String,
    api_secret: String,
    url: String,
}

#[derive(Clone)]
struct CaptchaConfig {
    secret: String,
    verify_url: String,
    verify_timeout: Duration,
}

#[derive(Clone)]
struct SearchService {
    tx: mpsc::Sender<SearchCommand>,
    state: Arc<SearchIndexState>,
}

#[derive(Clone)]
struct SearchIndexState {
    index: tantivy::Index,
    reader: tantivy::IndexReader,
    fields: SearchFields,
}

#[derive(Clone, Copy)]
struct SearchFields {
    message_id: Field,
    guild_id: Field,
    channel_id: Field,
    author_id: Field,
    created_at_unix: Field,
    content: Field,
}

#[derive(Clone)]
struct IndexedMessage {
    message_id: String,
    guild_id: String,
    channel_id: String,
    author_id: String,
    created_at_unix: i64,
    content: String,
}

enum SearchOperation {
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

struct SearchCommand {
    op: SearchOperation,
    ack: Option<oneshot::Sender<Result<(), AuthFailure>>>,
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
    media_token_hits: Arc<RwLock<HashMap<String, Vec<i64>>>>,
    media_publish_hits: Arc<RwLock<HashMap<String, Vec<i64>>>>,
    media_subscribe_leases: Arc<RwLock<HashMap<String, Vec<i64>>>>,
    guilds: Arc<RwLock<HashMap<String, GuildRecord>>>,
    subscriptions: Arc<RwLock<Subscriptions>>,
    connection_controls: Arc<RwLock<HashMap<Uuid, watch::Sender<ConnectionControl>>>>,
    connection_presence: Arc<RwLock<HashMap<Uuid, ConnectionPresence>>>,
    attachment_store: Arc<LocalFileSystem>,
    attachments: Arc<RwLock<HashMap<String, AttachmentRecord>>>,
    friendship_requests: Arc<RwLock<HashMap<String, FriendshipRequestRecord>>>,
    friendships: Arc<RwLock<HashSet<(String, String)>>>,
    audit_logs: Arc<RwLock<Vec<serde_json::Value>>>,
    search: SearchService,
    search_bootstrapped: Arc<OnceCell<()>>,
    runtime: Arc<RuntimeSecurityConfig>,
    livekit: Option<Arc<LiveKitConfig>>,
}

impl AppState {
    fn new(config: &AppConfig) -> anyhow::Result<Self> {
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
                user_attachment_quota_bytes: config.user_attachment_quota_bytes,
                search_query_max_chars: config.search_query_max_chars,
                search_result_limit_max: config.search_result_limit_max,
                search_query_timeout: config.search_query_timeout,
                media_token_requests_per_minute: config.media_token_requests_per_minute,
                media_publish_requests_per_minute: config.media_publish_requests_per_minute,
                media_subscribe_token_cap_per_channel: config.media_subscribe_token_cap_per_channel,
                max_created_guilds_per_user: config.max_created_guilds_per_user,
                livekit_token_ttl: config.livekit_token_ttl,
                captcha: captcha.map(Arc::new),
            }),
            livekit: livekit.map(Arc::new),
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum GuildVisibility {
    Private,
    Public,
}

#[derive(Debug, Clone)]
struct GuildRecord {
    name: String,
    visibility: GuildVisibility,
    created_by_user_id: UserId,
    members: HashMap<UserId, Role>,
    banned_members: HashSet<UserId>,
    channels: HashMap<String, ChannelRecord>,
}

#[derive(Debug, Clone)]
struct ChannelRecord {
    name: String,
    kind: ChannelKind,
    messages: Vec<MessageRecord>,
    role_overrides: HashMap<Role, ChannelPermissionOverwrite>,
}

#[derive(Debug, Clone)]
struct MessageRecord {
    id: String,
    author_id: UserId,
    content: String,
    markdown_tokens: Vec<MarkdownToken>,
    attachment_ids: Vec<String>,
    created_at_unix: i64,
    reactions: HashMap<String, HashSet<UserId>>,
}

#[derive(Debug, Clone)]
struct AttachmentRecord {
    attachment_id: String,
    guild_id: String,
    channel_id: String,
    owner_id: UserId,
    filename: String,
    mime_type: String,
    size_bytes: u64,
    sha256_hex: String,
    object_key: String,
    message_id: Option<String>,
}

#[derive(Debug, Clone)]
struct FriendshipRequestRecord {
    sender_user_id: UserId,
    recipient_user_id: UserId,
    created_at_unix: i64,
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

#[derive(Debug, Clone)]
struct ConnectionPresence {
    user_id: UserId,
    guild_ids: HashSet<String>,
}

