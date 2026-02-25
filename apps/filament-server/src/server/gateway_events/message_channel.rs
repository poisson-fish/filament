use serde::Serialize;

use super::{
    envelope::try_build_event,
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

pub(crate) fn try_message_update(
    guild_id: &str,
    channel_id: &str,
    message_id: &str,
    content: &str,
    markdown_tokens: &[filament_core::MarkdownToken],
    updated_at_unix: i64,
) -> anyhow::Result<GatewayEvent> {
    try_build_event(
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

pub(crate) fn try_message_delete(
    guild_id: &str,
    channel_id: &str,
    message_id: &str,
    deleted_at_unix: i64,
) -> anyhow::Result<GatewayEvent> {
    try_build_event(
        MESSAGE_DELETE_EVENT,
        MessageDeletePayload {
            guild_id,
            channel_id,
            message_id,
            deleted_at_unix,
        },
    )
}

pub(crate) fn try_message_reaction(
    guild_id: &str,
    channel_id: &str,
    message_id: &str,
    emoji: &str,
    count: usize,
) -> anyhow::Result<GatewayEvent> {
    try_build_message_reaction_event(
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

#[cfg(test)]
pub(crate) fn message_reaction(
    guild_id: &str,
    channel_id: &str,
    message_id: &str,
    emoji: &str,
    count: usize,
) -> GatewayEvent {
    try_message_reaction(guild_id, channel_id, message_id, emoji, count).unwrap_or_else(|error| {
        panic!("failed to build outbound gateway event {MESSAGE_REACTION_EVENT}: {error}")
    })
}

pub(crate) fn try_channel_create(
    guild_id: &str,
    channel: &ChannelResponse,
) -> anyhow::Result<GatewayEvent> {
    try_build_channel_create_event(
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
pub(crate) fn channel_create(guild_id: &str, channel: &ChannelResponse) -> GatewayEvent {
    try_channel_create(guild_id, channel).unwrap_or_else(|error| {
        panic!("failed to build outbound gateway event {CHANNEL_CREATE_EVENT}: {error}")
    })
}

fn try_build_channel_create_event(
    event_type: &'static str,
    payload: ChannelCreatePayload<'_>,
) -> anyhow::Result<GatewayEvent> {
    try_build_event(event_type, payload)
}

fn try_build_message_reaction_event(
    event_type: &'static str,
    payload: MessageReactionPayload<'_>,
) -> anyhow::Result<GatewayEvent> {
    try_build_event(event_type, payload)
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
        let payload = parse_payload(
            &try_message_update(
                "guild-1",
                "channel-1",
                "msg-1",
                "updated",
                &[MarkdownToken::Text {
                    text: String::from("updated"),
                }],
                99,
            )
            .expect("message_update should serialize"),
        );
        assert_eq!(payload["updated_fields"]["content"], Value::from("updated"));
        assert_eq!(payload["updated_at_unix"], Value::from(99));
    }

    #[test]
    fn message_delete_event_emits_deleted_timestamp() {
        let payload = parse_payload(
            &try_message_delete("guild-1", "channel-1", "msg-1", 77)
                .expect("message_delete should serialize"),
        );
        assert_eq!(payload["message_id"], Value::from("msg-1"));
        assert_eq!(payload["deleted_at_unix"], Value::from(77));
    }

    #[test]
    fn message_reaction_event_emits_reaction_fields() {
        let payload = parse_payload(
            &try_message_reaction("guild-1", "channel-1", "msg-1", ":+1:", 4)
                .expect("message_reaction should serialize"),
        );
        assert_eq!(payload["guild_id"], Value::from("guild-1"));
        assert_eq!(payload["channel_id"], Value::from("channel-1"));
        assert_eq!(payload["message_id"], Value::from("msg-1"));
        assert_eq!(payload["emoji"], Value::from(":+1:"));
        assert_eq!(payload["count"], Value::from(4));
    }

    #[test]
    fn try_message_reaction_rejects_invalid_event_type() {
        let Err(error) = try_build_message_reaction_event(
            "message reaction",
            MessageReactionPayload {
                guild_id: "guild-1",
                channel_id: "channel-1",
                message_id: "msg-1",
                emoji: ":+1:",
                count: 1,
            },
        ) else {
            panic!("invalid event type should fail");
        };
        assert!(
            error.to_string().contains("invalid outbound event type"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn channel_create_event_emits_channel_metadata() {
        let channel = ChannelResponse {
            channel_id: String::from("channel-1"),
            name: String::from("general"),
            kind: ChannelKind::Text,
        };

        let payload = parse_payload(
            &try_channel_create("guild-1", &channel).expect("channel_create should serialize"),
        );
        assert_eq!(payload["guild_id"], Value::from("guild-1"));
        assert_eq!(payload["channel"]["name"], Value::from("general"));
    }

    #[test]
    fn try_channel_create_rejects_invalid_event_type() {
        let channel = ChannelResponse {
            channel_id: String::from("channel-1"),
            name: String::from("general"),
            kind: ChannelKind::Text,
        };
        let Err(error) = try_build_channel_create_event(
            "channel create",
            ChannelCreatePayload {
                guild_id: "guild-1",
                channel: ChannelCreateChannelPayload {
                    channel_id: channel.channel_id.as_str(),
                    name: channel.name.as_str(),
                    kind: channel.kind,
                },
            },
        ) else {
            panic!("invalid event type should fail");
        };
        assert!(
            error.to_string().contains("invalid outbound event type"),
            "unexpected error: {error}"
        );
    }
}
