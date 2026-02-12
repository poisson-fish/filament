use std::collections::HashSet;

use filament_core::UserId;
use serde::Serialize;

use super::{
    auth::outbound_event,
    core::GuildVisibility,
    types::{ChannelResponse, MessageResponse},
};

pub(crate) const READY_EVENT: &str = "ready";
pub(crate) const SUBSCRIBED_EVENT: &str = "subscribed";
pub(crate) const MESSAGE_CREATE_EVENT: &str = "message_create";
pub(crate) const MESSAGE_UPDATE_EVENT: &str = "message_update";
pub(crate) const MESSAGE_DELETE_EVENT: &str = "message_delete";
pub(crate) const MESSAGE_REACTION_EVENT: &str = "message_reaction";
pub(crate) const CHANNEL_CREATE_EVENT: &str = "channel_create";
pub(crate) const PRESENCE_SYNC_EVENT: &str = "presence_sync";
pub(crate) const PRESENCE_UPDATE_EVENT: &str = "presence_update";
pub(crate) const WORKSPACE_UPDATE_EVENT: &str = "workspace_update";
pub(crate) const WORKSPACE_MEMBER_ADD_EVENT: &str = "workspace_member_add";
pub(crate) const WORKSPACE_MEMBER_UPDATE_EVENT: &str = "workspace_member_update";
pub(crate) const WORKSPACE_MEMBER_REMOVE_EVENT: &str = "workspace_member_remove";
pub(crate) const WORKSPACE_MEMBER_BAN_EVENT: &str = "workspace_member_ban";
pub(crate) const WORKSPACE_ROLE_CREATE_EVENT: &str = "workspace_role_create";
pub(crate) const WORKSPACE_ROLE_UPDATE_EVENT: &str = "workspace_role_update";
pub(crate) const WORKSPACE_ROLE_DELETE_EVENT: &str = "workspace_role_delete";
pub(crate) const WORKSPACE_ROLE_REORDER_EVENT: &str = "workspace_role_reorder";
pub(crate) const WORKSPACE_ROLE_ASSIGNMENT_ADD_EVENT: &str = "workspace_role_assignment_add";
pub(crate) const WORKSPACE_ROLE_ASSIGNMENT_REMOVE_EVENT: &str = "workspace_role_assignment_remove";
pub(crate) const WORKSPACE_CHANNEL_OVERRIDE_UPDATE_EVENT: &str =
    "workspace_channel_override_update";
pub(crate) const WORKSPACE_IP_BAN_SYNC_EVENT: &str = "workspace_ip_ban_sync";
pub(crate) const PROFILE_UPDATE_EVENT: &str = "profile_update";
pub(crate) const PROFILE_AVATAR_UPDATE_EVENT: &str = "profile_avatar_update";
pub(crate) const FRIEND_REQUEST_CREATE_EVENT: &str = "friend_request_create";
pub(crate) const FRIEND_REQUEST_UPDATE_EVENT: &str = "friend_request_update";
pub(crate) const FRIEND_REQUEST_DELETE_EVENT: &str = "friend_request_delete";
pub(crate) const FRIEND_REMOVE_EVENT: &str = "friend_remove";

pub(crate) struct GatewayEvent {
    pub(crate) event_type: &'static str,
    pub(crate) payload: String,
}

#[derive(Serialize)]
struct ReadyPayload {
    user_id: String,
}

#[derive(Serialize)]
struct SubscribedPayload<'a> {
    guild_id: &'a str,
    channel_id: &'a str,
}

#[derive(Serialize)]
struct MessageReactionPayload<'a> {
    guild_id: &'a str,
    channel_id: &'a str,
    message_id: &'a str,
    emoji: &'a str,
    count: usize,
}

#[derive(Serialize)]
struct MessageUpdatePayload<'a> {
    guild_id: &'a str,
    channel_id: &'a str,
    message_id: &'a str,
    updated_fields: MessageUpdateFieldsPayload<'a>,
    updated_at_unix: i64,
}

#[derive(Serialize)]
struct MessageUpdateFieldsPayload<'a> {
    content: &'a str,
    markdown_tokens: &'a [filament_core::MarkdownToken],
}

#[derive(Serialize)]
struct MessageDeletePayload<'a> {
    guild_id: &'a str,
    channel_id: &'a str,
    message_id: &'a str,
    deleted_at_unix: i64,
}

#[derive(Serialize)]
struct ChannelCreatePayload<'a> {
    guild_id: &'a str,
    channel: ChannelCreateChannelPayload<'a>,
}

#[derive(Serialize)]
struct ChannelCreateChannelPayload<'a> {
    channel_id: &'a str,
    name: &'a str,
    kind: filament_core::ChannelKind,
}

#[derive(Serialize)]
struct PresenceSyncPayload {
    guild_id: String,
    user_ids: HashSet<String>,
}

#[derive(Serialize)]
struct PresenceUpdatePayload {
    guild_id: String,
    user_id: String,
    status: &'static str,
}

#[derive(Serialize)]
struct WorkspaceUpdatePayload<'a> {
    guild_id: &'a str,
    updated_fields: WorkspaceUpdateFieldsPayload<'a>,
    updated_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceUpdateFieldsPayload<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    visibility: Option<GuildVisibility>,
}

#[derive(Serialize)]
struct WorkspaceMemberAddPayload {
    guild_id: String,
    user_id: String,
    role: filament_core::Role,
    joined_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceMemberUpdatePayload {
    guild_id: String,
    user_id: String,
    updated_fields: WorkspaceMemberUpdateFieldsPayload,
    updated_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceMemberUpdateFieldsPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<filament_core::Role>,
}

#[derive(Serialize)]
struct WorkspaceMemberRemovePayload {
    guild_id: String,
    user_id: String,
    reason: &'static str,
    removed_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceMemberBanPayload {
    guild_id: String,
    user_id: String,
    banned_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceRolePayload {
    role_id: String,
    name: String,
    position: i32,
    is_system: bool,
    permissions: Vec<filament_core::Permission>,
}

#[derive(Serialize)]
struct WorkspaceRoleCreatePayload {
    guild_id: String,
    role: WorkspaceRolePayload,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceRoleUpdatePayload {
    guild_id: String,
    role_id: String,
    updated_fields: WorkspaceRoleUpdateFieldsPayload,
    updated_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceRoleUpdateFieldsPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    permissions: Option<Vec<filament_core::Permission>>,
}

#[derive(Serialize)]
struct WorkspaceRoleDeletePayload {
    guild_id: String,
    role_id: String,
    deleted_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceRoleReorderPayload {
    guild_id: String,
    role_ids: Vec<String>,
    updated_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceRoleAssignmentPayload {
    guild_id: String,
    user_id: String,
    role_id: String,
    assigned_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceRoleAssignmentRemovePayload {
    guild_id: String,
    user_id: String,
    role_id: String,
    removed_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceChannelOverrideUpdatePayload {
    guild_id: String,
    channel_id: String,
    role: filament_core::Role,
    updated_fields: WorkspaceChannelOverrideFieldsPayload,
    updated_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceChannelOverrideFieldsPayload {
    allow: Vec<filament_core::Permission>,
    deny: Vec<filament_core::Permission>,
}

#[derive(Serialize)]
struct WorkspaceIpBanSyncPayload {
    guild_id: String,
    summary: WorkspaceIpBanSyncSummaryPayload,
    updated_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceIpBanSyncSummaryPayload {
    action: &'static str,
    changed_count: usize,
}

#[derive(Serialize)]
struct ProfileUpdatePayload<'a> {
    user_id: &'a str,
    updated_fields: ProfileUpdateFieldsPayload<'a>,
    updated_at_unix: i64,
}

#[derive(Serialize)]
struct ProfileUpdateFieldsPayload<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    about_markdown: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    about_markdown_tokens: Option<&'a [filament_core::MarkdownToken]>,
}

#[derive(Serialize)]
struct ProfileAvatarUpdatePayload<'a> {
    user_id: &'a str,
    avatar_version: i64,
    updated_at_unix: i64,
}

#[derive(Serialize)]
struct FriendRequestCreatePayload<'a> {
    request_id: &'a str,
    sender_user_id: &'a str,
    sender_username: &'a str,
    recipient_user_id: &'a str,
    recipient_username: &'a str,
    created_at_unix: i64,
}

#[derive(Serialize)]
struct FriendRequestUpdatePayload<'a> {
    request_id: &'a str,
    state: &'static str,
    user_id: &'a str,
    friend_user_id: &'a str,
    friend_username: &'a str,
    friendship_created_at_unix: i64,
    updated_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct FriendRequestDeletePayload<'a> {
    request_id: &'a str,
    deleted_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct FriendRemovePayload<'a> {
    user_id: &'a str,
    friend_user_id: &'a str,
    removed_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

fn build_event<T: Serialize>(event_type: &'static str, payload: T) -> GatewayEvent {
    GatewayEvent {
        event_type,
        payload: outbound_event(event_type, payload),
    }
}

pub(crate) fn ready(user_id: UserId) -> GatewayEvent {
    build_event(
        READY_EVENT,
        ReadyPayload {
            user_id: user_id.to_string(),
        },
    )
}

pub(crate) fn subscribed(guild_id: &str, channel_id: &str) -> GatewayEvent {
    build_event(
        SUBSCRIBED_EVENT,
        SubscribedPayload {
            guild_id,
            channel_id,
        },
    )
}

pub(crate) fn message_create(message: &MessageResponse) -> GatewayEvent {
    build_event(MESSAGE_CREATE_EVENT, message)
}

pub(crate) fn message_reaction(
    guild_id: &str,
    channel_id: &str,
    message_id: &str,
    emoji: &str,
    count: usize,
) -> GatewayEvent {
    build_event(
        MESSAGE_REACTION_EVENT,
        MessageReactionPayload {
            guild_id,
            channel_id,
            message_id,
            emoji,
            count,
        },
    )
}

pub(crate) fn message_update(
    guild_id: &str,
    channel_id: &str,
    message_id: &str,
    content: &str,
    markdown_tokens: &[filament_core::MarkdownToken],
    updated_at_unix: i64,
) -> GatewayEvent {
    build_event(
        MESSAGE_UPDATE_EVENT,
        MessageUpdatePayload {
            guild_id,
            channel_id,
            message_id,
            updated_fields: MessageUpdateFieldsPayload {
                content,
                markdown_tokens,
            },
            updated_at_unix,
        },
    )
}

pub(crate) fn message_delete(
    guild_id: &str,
    channel_id: &str,
    message_id: &str,
    deleted_at_unix: i64,
) -> GatewayEvent {
    build_event(
        MESSAGE_DELETE_EVENT,
        MessageDeletePayload {
            guild_id,
            channel_id,
            message_id,
            deleted_at_unix,
        },
    )
}

pub(crate) fn channel_create(guild_id: &str, channel: &ChannelResponse) -> GatewayEvent {
    build_event(
        CHANNEL_CREATE_EVENT,
        ChannelCreatePayload {
            guild_id,
            channel: ChannelCreateChannelPayload {
                channel_id: channel.channel_id.as_str(),
                name: channel.name.as_str(),
                kind: channel.kind,
            },
        },
    )
}

pub(crate) fn presence_sync(guild_id: &str, user_ids: HashSet<String>) -> GatewayEvent {
    build_event(
        PRESENCE_SYNC_EVENT,
        PresenceSyncPayload {
            guild_id: guild_id.to_owned(),
            user_ids,
        },
    )
}

pub(crate) fn presence_update(
    guild_id: &str,
    user_id: UserId,
    status: &'static str,
) -> GatewayEvent {
    build_event(
        PRESENCE_UPDATE_EVENT,
        PresenceUpdatePayload {
            guild_id: guild_id.to_owned(),
            user_id: user_id.to_string(),
            status,
        },
    )
}

pub(crate) fn workspace_update(
    guild_id: &str,
    name: Option<&str>,
    visibility: Option<GuildVisibility>,
    updated_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        WORKSPACE_UPDATE_EVENT,
        WorkspaceUpdatePayload {
            guild_id,
            updated_fields: WorkspaceUpdateFieldsPayload { name, visibility },
            updated_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn workspace_member_add(
    guild_id: &str,
    user_id: UserId,
    role: filament_core::Role,
    joined_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        WORKSPACE_MEMBER_ADD_EVENT,
        WorkspaceMemberAddPayload {
            guild_id: guild_id.to_owned(),
            user_id: user_id.to_string(),
            role,
            joined_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn workspace_member_update(
    guild_id: &str,
    user_id: UserId,
    role: Option<filament_core::Role>,
    updated_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        WORKSPACE_MEMBER_UPDATE_EVENT,
        WorkspaceMemberUpdatePayload {
            guild_id: guild_id.to_owned(),
            user_id: user_id.to_string(),
            updated_fields: WorkspaceMemberUpdateFieldsPayload { role },
            updated_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn workspace_member_remove(
    guild_id: &str,
    user_id: UserId,
    reason: &'static str,
    removed_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        WORKSPACE_MEMBER_REMOVE_EVENT,
        WorkspaceMemberRemovePayload {
            guild_id: guild_id.to_owned(),
            user_id: user_id.to_string(),
            reason,
            removed_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn workspace_member_ban(
    guild_id: &str,
    user_id: UserId,
    banned_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        WORKSPACE_MEMBER_BAN_EVENT,
        WorkspaceMemberBanPayload {
            guild_id: guild_id.to_owned(),
            user_id: user_id.to_string(),
            banned_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn workspace_role_create(
    guild_id: &str,
    role_id: &str,
    name: &str,
    position: i32,
    is_system: bool,
    permissions: Vec<filament_core::Permission>,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        WORKSPACE_ROLE_CREATE_EVENT,
        WorkspaceRoleCreatePayload {
            guild_id: guild_id.to_owned(),
            role: WorkspaceRolePayload {
                role_id: role_id.to_owned(),
                name: name.to_owned(),
                position,
                is_system,
                permissions,
            },
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn workspace_role_update(
    guild_id: &str,
    role_id: &str,
    name: Option<&str>,
    permissions: Option<Vec<filament_core::Permission>>,
    updated_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        WORKSPACE_ROLE_UPDATE_EVENT,
        WorkspaceRoleUpdatePayload {
            guild_id: guild_id.to_owned(),
            role_id: role_id.to_owned(),
            updated_fields: WorkspaceRoleUpdateFieldsPayload {
                name: name.map(ToOwned::to_owned),
                permissions,
            },
            updated_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn workspace_role_delete(
    guild_id: &str,
    role_id: &str,
    deleted_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        WORKSPACE_ROLE_DELETE_EVENT,
        WorkspaceRoleDeletePayload {
            guild_id: guild_id.to_owned(),
            role_id: role_id.to_owned(),
            deleted_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn workspace_role_reorder(
    guild_id: &str,
    role_ids: Vec<String>,
    updated_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        WORKSPACE_ROLE_REORDER_EVENT,
        WorkspaceRoleReorderPayload {
            guild_id: guild_id.to_owned(),
            role_ids,
            updated_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn workspace_role_assignment_add(
    guild_id: &str,
    user_id: UserId,
    role_id: &str,
    assigned_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        WORKSPACE_ROLE_ASSIGNMENT_ADD_EVENT,
        WorkspaceRoleAssignmentPayload {
            guild_id: guild_id.to_owned(),
            user_id: user_id.to_string(),
            role_id: role_id.to_owned(),
            assigned_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn workspace_role_assignment_remove(
    guild_id: &str,
    user_id: UserId,
    role_id: &str,
    removed_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        WORKSPACE_ROLE_ASSIGNMENT_REMOVE_EVENT,
        WorkspaceRoleAssignmentRemovePayload {
            guild_id: guild_id.to_owned(),
            user_id: user_id.to_string(),
            role_id: role_id.to_owned(),
            removed_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn workspace_channel_override_update(
    guild_id: &str,
    channel_id: &str,
    role: filament_core::Role,
    allow: Vec<filament_core::Permission>,
    deny: Vec<filament_core::Permission>,
    updated_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        WORKSPACE_CHANNEL_OVERRIDE_UPDATE_EVENT,
        WorkspaceChannelOverrideUpdatePayload {
            guild_id: guild_id.to_owned(),
            channel_id: channel_id.to_owned(),
            role,
            updated_fields: WorkspaceChannelOverrideFieldsPayload { allow, deny },
            updated_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn workspace_ip_ban_sync(
    guild_id: &str,
    action: &'static str,
    changed_count: usize,
    updated_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        WORKSPACE_IP_BAN_SYNC_EVENT,
        WorkspaceIpBanSyncPayload {
            guild_id: guild_id.to_owned(),
            summary: WorkspaceIpBanSyncSummaryPayload {
                action,
                changed_count,
            },
            updated_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn profile_update(
    user_id: &str,
    username: Option<&str>,
    about_markdown: Option<&str>,
    about_markdown_tokens: Option<&[filament_core::MarkdownToken]>,
    updated_at_unix: i64,
) -> GatewayEvent {
    build_event(
        PROFILE_UPDATE_EVENT,
        ProfileUpdatePayload {
            user_id,
            updated_fields: ProfileUpdateFieldsPayload {
                username,
                about_markdown,
                about_markdown_tokens,
            },
            updated_at_unix,
        },
    )
}

pub(crate) fn profile_avatar_update(
    user_id: &str,
    avatar_version: i64,
    updated_at_unix: i64,
) -> GatewayEvent {
    build_event(
        PROFILE_AVATAR_UPDATE_EVENT,
        ProfileAvatarUpdatePayload {
            user_id,
            avatar_version,
            updated_at_unix,
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn friend_request_create(
    request_id: &str,
    sender_user_id: &str,
    sender_username: &str,
    recipient_user_id: &str,
    recipient_username: &str,
    created_at_unix: i64,
) -> GatewayEvent {
    build_event(
        FRIEND_REQUEST_CREATE_EVENT,
        FriendRequestCreatePayload {
            request_id,
            sender_user_id,
            sender_username,
            recipient_user_id,
            recipient_username,
            created_at_unix,
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn friend_request_update(
    request_id: &str,
    user_id: &str,
    friend_user_id: &str,
    friend_username: &str,
    friendship_created_at_unix: i64,
    updated_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        FRIEND_REQUEST_UPDATE_EVENT,
        FriendRequestUpdatePayload {
            request_id,
            state: "accepted",
            user_id,
            friend_user_id,
            friend_username,
            friendship_created_at_unix,
            updated_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn friend_request_delete(
    request_id: &str,
    deleted_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        FRIEND_REQUEST_DELETE_EVENT,
        FriendRequestDeletePayload {
            request_id,
            deleted_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn friend_remove(
    user_id: &str,
    friend_user_id: &str,
    removed_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        FRIEND_REMOVE_EVENT,
        FriendRemovePayload {
            user_id,
            friend_user_id,
            removed_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}
