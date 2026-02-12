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
