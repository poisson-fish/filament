#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn metrics() -> Response {
    (
        [(CONTENT_TYPE, METRICS_TEXT_CONTENT_TYPE)],
        render_metrics(),
    )
        .into_response()
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
    captcha_token: Option<String>,
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
struct UserLookupRequest {
    user_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct UserLookupItem {
    user_id: String,
    username: String,
}

#[derive(Debug, Serialize)]
struct UserLookupResponse {
    users: Vec<UserLookupItem>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateFriendRequest {
    recipient_user_id: String,
}

#[derive(Debug, Serialize)]
struct FriendRecordResponse {
    user_id: String,
    username: String,
    created_at_unix: i64,
}

#[derive(Debug, Serialize)]
struct FriendListResponse {
    friends: Vec<FriendRecordResponse>,
}

#[derive(Debug, Serialize)]
struct FriendshipRequestResponse {
    request_id: String,
    sender_user_id: String,
    sender_username: String,
    recipient_user_id: String,
    recipient_username: String,
    created_at_unix: i64,
}

#[derive(Debug, Serialize)]
struct FriendshipRequestListResponse {
    incoming: Vec<FriendshipRequestResponse>,
    outgoing: Vec<FriendshipRequestResponse>,
}

#[derive(Debug, Serialize)]
struct FriendshipRequestCreateResponse {
    request_id: String,
    sender_user_id: String,
    recipient_user_id: String,
    created_at_unix: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateGuildRequest {
    name: String,
    visibility: Option<GuildVisibility>,
}

#[derive(Debug, Serialize)]
struct GuildResponse {
    guild_id: String,
    name: String,
    visibility: GuildVisibility,
}

#[derive(Debug, Serialize)]
struct GuildListResponse {
    guilds: Vec<GuildResponse>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateChannelRequest {
    name: String,
    kind: Option<ChannelKind>,
}

#[derive(Debug, Serialize)]
struct ChannelResponse {
    channel_id: String,
    name: String,
    kind: ChannelKind,
}

#[derive(Debug, Serialize)]
struct ChannelListResponse {
    channels: Vec<ChannelResponse>,
}

#[derive(Debug, Serialize)]
struct ChannelPermissionsResponse {
    role: Role,
    permissions: Vec<Permission>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateMessageRequest {
    content: String,
    attachment_ids: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EditMessageRequest {
    content: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateMemberRoleRequest {
    role: Role,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateChannelRoleOverrideRequest {
    allow: Vec<Permission>,
    deny: Vec<Permission>,
}

#[derive(Debug, Serialize, Clone)]
struct MessageResponse {
    message_id: String,
    guild_id: String,
    channel_id: String,
    author_id: String,
    content: String,
    markdown_tokens: Vec<MarkdownToken>,
    attachments: Vec<AttachmentResponse>,
    reactions: Vec<ReactionResponse>,
    created_at_unix: i64,
}

#[derive(Debug, Serialize, Clone)]
struct ReactionResponse {
    emoji: String,
    count: usize,
}

#[derive(Debug, Serialize, Clone)]
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
struct FriendPath {
    friend_user_id: String,
}

#[derive(Debug, Deserialize)]
struct FriendRequestPath {
    request_id: String,
}

#[derive(Debug, Deserialize)]
struct ChannelRolePath {
    guild_id: String,
    channel_id: String,
    role: Role,
}

#[derive(Debug, Deserialize)]
struct ReactionPath {
    guild_id: String,
    channel_id: String,
    message_id: String,
    emoji: String,
}

#[derive(Debug, Deserialize)]
struct HistoryQuery {
    limit: Option<usize>,
    before: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: String,
    limit: Option<usize>,
    channel_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PublicGuildListQuery {
    q: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
struct PublicGuildListItem {
    guild_id: String,
    name: String,
    visibility: GuildVisibility,
}

#[derive(Debug, Serialize)]
struct PublicGuildListResponse {
    guilds: Vec<PublicGuildListItem>,
}

#[derive(Debug, Serialize)]
struct SearchResponse {
    message_ids: Vec<String>,
    messages: Vec<MessageResponse>,
}

#[derive(Debug, Serialize)]
struct SearchReconcileResponse {
    upserted: usize,
    deleted: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VoiceTokenRequest {
    can_publish: Option<bool>,
    can_subscribe: Option<bool>,
    publish_sources: Option<Vec<MediaPublishSource>>,
}

#[derive(Debug, Serialize)]
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

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
enum MediaPublishSource {
    Microphone,
    Camera,
    ScreenShare,
}

impl MediaPublishSource {
    fn as_livekit_source(self) -> &'static str {
        match self {
            Self::Microphone => "microphone",
            Self::Camera => "camera",
            Self::ScreenShare => "screen_share",
        }
    }
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
    attachment_ids: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct GatewayAuthQuery {
    access_token: Option<String>,
}

#[derive(Debug, Clone)]
struct CaptchaToken(String);

impl CaptchaToken {
    fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CaptchaToken {
    type Error = ();

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if !(MIN_CAPTCHA_TOKEN_CHARS..=MAX_CAPTCHA_TOKEN_CHARS).contains(&value.chars().count()) {
            return Err(());
        }
        if value
            .chars()
            .any(|char| !(('\u{21}'..='\u{7e}').contains(&char)))
        {
            return Err(());
        }
        Ok(Self(value))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HcaptchaVerifyResponse {
    success: bool,
}

