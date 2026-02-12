use std::time::Duration;

use axum::{
    http::{header::CONTENT_TYPE, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use filament_core::{ChannelKind, MarkdownToken, Permission, Role};
use serde::{Deserialize, Serialize};

use super::{
    core::{
        GuildVisibility, MAX_CAPTCHA_TOKEN_CHARS, METRICS_TEXT_CONTENT_TYPE,
        MIN_CAPTCHA_TOKEN_CHARS,
    },
    metrics::render_metrics,
};

#[derive(Debug, Serialize)]
pub(crate) struct HealthResponse {
    pub(crate) status: &'static str,
}

pub(crate) async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

pub(crate) async fn metrics() -> Response {
    (
        [(CONTENT_TYPE, METRICS_TEXT_CONTENT_TYPE)],
        render_metrics(),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EchoRequest {
    pub(crate) message: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct EchoResponse {
    pub(crate) message: String,
}

pub(crate) async fn echo(
    Json(payload): Json<EchoRequest>,
) -> Result<Json<EchoResponse>, StatusCode> {
    if payload.message.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    Ok(Json(EchoResponse {
        message: payload.message,
    }))
}

pub(crate) async fn slow() -> Json<HealthResponse> {
    tokio::time::sleep(Duration::from_millis(200)).await;
    Json(HealthResponse { status: "ok" })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RegisterRequest {
    pub(crate) username: String,
    pub(crate) password: String,
    pub(crate) captcha_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LoginRequest {
    pub(crate) username: String,
    pub(crate) password: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RefreshRequest {
    pub(crate) refresh_token: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct AuthResponse {
    pub(crate) access_token: String,
    pub(crate) refresh_token: String,
    pub(crate) expires_in_secs: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct RegisterResponse {
    pub(crate) accepted: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct AuthError {
    pub(crate) error: &'static str,
}

#[derive(Debug, Serialize)]
pub(crate) struct MeResponse {
    pub(crate) user_id: String,
    pub(crate) username: String,
    pub(crate) about_markdown: String,
    pub(crate) about_markdown_tokens: Vec<MarkdownToken>,
    pub(crate) avatar_version: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateProfileRequest {
    pub(crate) username: Option<String>,
    pub(crate) about_markdown: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct UserProfileResponse {
    pub(crate) user_id: String,
    pub(crate) username: String,
    pub(crate) about_markdown: String,
    pub(crate) about_markdown_tokens: Vec<MarkdownToken>,
    pub(crate) avatar_version: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UserLookupRequest {
    pub(crate) user_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct UserLookupItem {
    pub(crate) user_id: String,
    pub(crate) username: String,
    pub(crate) avatar_version: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct UserLookupResponse {
    pub(crate) users: Vec<UserLookupItem>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateFriendRequest {
    pub(crate) recipient_user_id: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct FriendRecordResponse {
    pub(crate) user_id: String,
    pub(crate) username: String,
    pub(crate) created_at_unix: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct FriendListResponse {
    pub(crate) friends: Vec<FriendRecordResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct FriendshipRequestResponse {
    pub(crate) request_id: String,
    pub(crate) sender_user_id: String,
    pub(crate) sender_username: String,
    pub(crate) recipient_user_id: String,
    pub(crate) recipient_username: String,
    pub(crate) created_at_unix: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct FriendshipRequestListResponse {
    pub(crate) incoming: Vec<FriendshipRequestResponse>,
    pub(crate) outgoing: Vec<FriendshipRequestResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct FriendshipRequestCreateResponse {
    pub(crate) request_id: String,
    pub(crate) sender_user_id: String,
    pub(crate) recipient_user_id: String,
    pub(crate) created_at_unix: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateGuildRequest {
    pub(crate) name: String,
    pub(crate) visibility: Option<GuildVisibility>,
}

#[derive(Debug, Serialize)]
pub(crate) struct GuildResponse {
    pub(crate) guild_id: String,
    pub(crate) name: String,
    pub(crate) visibility: GuildVisibility,
}

#[derive(Debug, Serialize)]
pub(crate) struct GuildListResponse {
    pub(crate) guilds: Vec<GuildResponse>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateChannelRequest {
    pub(crate) name: String,
    pub(crate) kind: Option<ChannelKind>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ChannelResponse {
    pub(crate) channel_id: String,
    pub(crate) name: String,
    pub(crate) kind: ChannelKind,
}

#[derive(Debug, Serialize)]
pub(crate) struct ChannelListResponse {
    pub(crate) channels: Vec<ChannelResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ChannelPermissionsResponse {
    pub(crate) role: Role,
    pub(crate) permissions: Vec<Permission>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateMessageRequest {
    pub(crate) content: String,
    pub(crate) attachment_ids: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EditMessageRequest {
    pub(crate) content: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateMemberRoleRequest {
    pub(crate) role: Role,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateChannelRoleOverrideRequest {
    pub(crate) allow: Vec<Permission>,
    pub(crate) deny: Vec<Permission>,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct MessageResponse {
    pub(crate) message_id: String,
    pub(crate) guild_id: String,
    pub(crate) channel_id: String,
    pub(crate) author_id: String,
    pub(crate) content: String,
    pub(crate) markdown_tokens: Vec<MarkdownToken>,
    pub(crate) attachments: Vec<AttachmentResponse>,
    pub(crate) reactions: Vec<ReactionResponse>,
    pub(crate) created_at_unix: i64,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct ReactionResponse {
    pub(crate) emoji: String,
    pub(crate) count: usize,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct AttachmentResponse {
    pub(crate) attachment_id: String,
    pub(crate) guild_id: String,
    pub(crate) channel_id: String,
    pub(crate) owner_id: String,
    pub(crate) filename: String,
    pub(crate) mime_type: String,
    pub(crate) size_bytes: u64,
    pub(crate) sha256_hex: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UploadAttachmentQuery {
    pub(crate) filename: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ModerationResponse {
    pub(crate) accepted: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct MessageHistoryResponse {
    pub(crate) messages: Vec<MessageResponse>,
    pub(crate) next_before: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GuildPath {
    pub(crate) guild_id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GuildIpBanPath {
    pub(crate) guild_id: String,
    pub(crate) ban_id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChannelPath {
    pub(crate) guild_id: String,
    pub(crate) channel_id: String,
}

#[allow(clippy::struct_field_names)]
#[derive(Debug, Deserialize)]
pub(crate) struct MessagePath {
    pub(crate) guild_id: String,
    pub(crate) channel_id: String,
    pub(crate) message_id: String,
}

#[allow(clippy::struct_field_names)]
#[derive(Debug, Deserialize)]
pub(crate) struct AttachmentPath {
    pub(crate) guild_id: String,
    pub(crate) channel_id: String,
    pub(crate) attachment_id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MemberPath {
    pub(crate) guild_id: String,
    pub(crate) user_id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FriendPath {
    pub(crate) friend_user_id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UserPath {
    pub(crate) user_id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FriendRequestPath {
    pub(crate) request_id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChannelRolePath {
    pub(crate) guild_id: String,
    pub(crate) channel_id: String,
    pub(crate) role: Role,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ReactionPath {
    pub(crate) guild_id: String,
    pub(crate) channel_id: String,
    pub(crate) message_id: String,
    pub(crate) emoji: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct HistoryQuery {
    pub(crate) limit: Option<usize>,
    pub(crate) before: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SearchQuery {
    pub(crate) q: String,
    pub(crate) limit: Option<usize>,
    pub(crate) channel_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PublicGuildListQuery {
    pub(crate) q: Option<String>,
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PublicGuildListItem {
    pub(crate) guild_id: String,
    pub(crate) name: String,
    pub(crate) visibility: GuildVisibility,
}

#[derive(Debug, Serialize)]
pub(crate) struct PublicGuildListResponse {
    pub(crate) guilds: Vec<PublicGuildListItem>,
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DirectoryJoinOutcomeResponse {
    Accepted,
    AlreadyMember,
    RejectedVisibility,
    RejectedUserBan,
    RejectedIpBan,
}

#[derive(Debug, Serialize)]
pub(crate) struct DirectoryJoinResponse {
    pub(crate) guild_id: String,
    pub(crate) outcome: DirectoryJoinOutcomeResponse,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct GuildAuditEventResponse {
    pub(crate) audit_id: String,
    pub(crate) actor_user_id: String,
    pub(crate) target_user_id: Option<String>,
    pub(crate) action: String,
    pub(crate) created_at_unix: i64,
    pub(crate) ip_ban_match: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct GuildAuditListResponse {
    pub(crate) events: Vec<GuildAuditEventResponse>,
    pub(crate) next_cursor: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct GuildIpBanRecordResponse {
    pub(crate) ban_id: String,
    pub(crate) source_user_id: Option<String>,
    pub(crate) reason: Option<String>,
    pub(crate) created_at_unix: i64,
    pub(crate) expires_at_unix: Option<i64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct GuildIpBanListResponse {
    pub(crate) bans: Vec<GuildIpBanRecordResponse>,
    pub(crate) next_cursor: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct GuildIpBanApplyResponse {
    pub(crate) created_count: usize,
    pub(crate) ban_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SearchResponse {
    pub(crate) message_ids: Vec<String>,
    pub(crate) messages: Vec<MessageResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SearchReconcileResponse {
    pub(crate) upserted: usize,
    pub(crate) deleted: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct VoiceTokenRequest {
    pub(crate) can_publish: Option<bool>,
    pub(crate) can_subscribe: Option<bool>,
    pub(crate) publish_sources: Option<Vec<MediaPublishSource>>,
}

#[derive(Debug, Serialize)]
pub(crate) struct VoiceTokenResponse {
    pub(crate) token: String,
    pub(crate) livekit_url: String,
    pub(crate) room: String,
    pub(crate) identity: String,
    pub(crate) can_publish: bool,
    pub(crate) can_subscribe: bool,
    pub(crate) publish_sources: Vec<String>,
    pub(crate) expires_in_secs: u64,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MediaPublishSource {
    Microphone,
    Camera,
    ScreenShare,
}

impl MediaPublishSource {
    pub(crate) fn as_livekit_source(self) -> &'static str {
        match self {
            Self::Microphone => "microphone",
            Self::Camera => "camera",
            Self::ScreenShare => "screen_share",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GatewaySubscribe {
    pub(crate) guild_id: String,
    pub(crate) channel_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GatewayMessageCreate {
    pub(crate) guild_id: String,
    pub(crate) channel_id: String,
    pub(crate) content: String,
    pub(crate) attachment_ids: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GatewayAuthQuery {
    pub(crate) access_token: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct CaptchaToken(String);

impl CaptchaToken {
    pub(crate) fn as_str(&self) -> &str {
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
pub(crate) struct HcaptchaVerifyResponse {
    pub(crate) success: bool,
}
