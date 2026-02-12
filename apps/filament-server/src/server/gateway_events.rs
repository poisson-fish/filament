use std::collections::HashSet;

use filament_core::UserId;
use serde::Serialize;

use super::{
    auth::outbound_event,
    types::{ChannelResponse, MessageResponse},
};

pub(crate) const READY_EVENT: &str = "ready";
pub(crate) const SUBSCRIBED_EVENT: &str = "subscribed";
pub(crate) const MESSAGE_CREATE_EVENT: &str = "message_create";
pub(crate) const MESSAGE_REACTION_EVENT: &str = "message_reaction";
pub(crate) const CHANNEL_CREATE_EVENT: &str = "channel_create";
pub(crate) const PRESENCE_SYNC_EVENT: &str = "presence_sync";
pub(crate) const PRESENCE_UPDATE_EVENT: &str = "presence_update";

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
