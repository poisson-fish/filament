use serde::Serialize;

use super::{
    envelope::{build_event, try_build_event},
    GatewayEvent,
};
use crate::server::types::{ChannelResponse, MessageResponse};

pub(crate) const MESSAGE_CREATE_EVENT: &str = "message_create";
pub(crate) const MESSAGE_UPDATE_EVENT: &str = "message_update";
pub(crate) const MESSAGE_DELETE_EVENT: &str = "message_delete";
pub(crate) const MESSAGE_REACTION_EVENT: &str = "message_reaction";
pub(crate) const CHANNEL_CREATE_EVENT: &str = "channel_create";

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

pub(crate) fn try_message_create(message: &MessageResponse) -> anyhow::Result<GatewayEvent> {
    try_build_event(MESSAGE_CREATE_EVENT, message)
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

#[cfg(test)]
mod tests {
    use filament_core::{ChannelKind, MarkdownToken, UserId};
    use serde_json::Value;

    use super::*;

    fn parse_payload(event: &GatewayEvent) -> Value {
        let value: Value =
            serde_json::from_str(&event.payload).expect("gateway event payload should be valid");
        assert_eq!(value["v"], Value::from(1));
        assert_eq!(value["t"], Value::from(event.event_type));
        value["d"].clone()
    }

    #[test]
    fn message_create_event_emits_message_identifier() {
        let user_id = UserId::new();
        let message = MessageResponse {
            message_id: String::from("msg-1"),
            guild_id: String::from("guild-1"),
            channel_id: String::from("channel-1"),
            author_id: user_id.to_string(),
            content: String::from("hello"),
            markdown_tokens: vec![MarkdownToken::Text {
                text: String::from("hello"),
            }],
            attachments: Vec::new(),
            reactions: Vec::new(),
            created_at_unix: 1,
        };

        let payload =
            parse_payload(&try_message_create(&message).expect("message_create should serialize"));
        assert_eq!(payload["message_id"], Value::from("msg-1"));
    }

    #[test]
    fn message_update_event_emits_updated_fields() {
        let payload = parse_payload(&message_update(
            "guild-1",
            "channel-1",
            "msg-1",
            "updated",
            &[MarkdownToken::Text {
                text: String::from("updated"),
            }],
            99,
        ));
        assert_eq!(payload["updated_fields"]["content"], Value::from("updated"));
        assert_eq!(payload["updated_at_unix"], Value::from(99));
    }

    #[test]
    fn channel_create_event_emits_channel_metadata() {
        let channel = ChannelResponse {
            channel_id: String::from("channel-1"),
            name: String::from("general"),
            kind: ChannelKind::Text,
        };

        let payload = parse_payload(&channel_create("guild-1", &channel));
        assert_eq!(payload["guild_id"], Value::from("guild-1"));
        assert_eq!(payload["channel"]["name"], Value::from("general"));
    }
}
