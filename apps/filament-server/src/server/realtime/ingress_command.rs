use filament_protocol::Envelope;
use serde::Deserialize;
use serde_json::Value;
use ulid::Ulid;

use crate::server::{auth::validate_message_content, domain::parse_attachment_ids};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GatewaySubscribeDto {
    guild_id: String,
    channel_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GatewayMessageCreateDto {
    guild_id: String,
    channel_id: String,
    content: String,
    attachment_ids: Option<Vec<String>>,
}

#[derive(Debug)]
pub(crate) enum GatewayIngressCommand {
    Subscribe(GatewaySubscribeCommand),
    MessageCreate(GatewayMessageCreateCommand),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GatewayGuildId(String);

impl GatewayGuildId {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for GatewayGuildId {
    type Error = ();

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if Ulid::from_string(&value).is_err() {
            return Err(());
        }
        Ok(Self(value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GatewayChannelId(String);

impl GatewayChannelId {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for GatewayChannelId {
    type Error = ();

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if Ulid::from_string(&value).is_err() {
            return Err(());
        }
        Ok(Self(value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GatewaySubscribeCommand {
    pub(crate) guild_id: GatewayGuildId,
    pub(crate) channel_id: GatewayChannelId,
}

impl TryFrom<GatewaySubscribeDto> for GatewaySubscribeCommand {
    type Error = ();

    fn try_from(value: GatewaySubscribeDto) -> Result<Self, Self::Error> {
        Ok(Self {
            guild_id: GatewayGuildId::try_from(value.guild_id)?,
            channel_id: GatewayChannelId::try_from(value.channel_id)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GatewayMessageCreateCommand {
    pub(crate) guild_id: GatewayGuildId,
    pub(crate) channel_id: GatewayChannelId,
    pub(crate) content: GatewayMessageContent,
    pub(crate) attachment_ids: GatewayAttachmentIds,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GatewayAttachmentIds(Vec<String>);

impl GatewayAttachmentIds {
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn into_vec(self) -> Vec<String> {
        self.0
    }
}

impl TryFrom<Vec<String>> for GatewayAttachmentIds {
    type Error = ();

    fn try_from(value: Vec<String>) -> Result<Self, Self::Error> {
        let ids = parse_attachment_ids(value).map_err(|_| ())?;
        Ok(Self(ids))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GatewayMessageContent(String);

impl GatewayMessageContent {
    pub(crate) fn into_string(self) -> String {
        self.0
    }
}

impl TryFrom<(String, bool)> for GatewayMessageContent {
    type Error = ();

    fn try_from(value: (String, bool)) -> Result<Self, Self::Error> {
        let (content, has_attachments) = value;
        if content.is_empty() {
            if has_attachments {
                return Ok(Self(content));
            }
            return Err(());
        }
        validate_message_content(&content).map_err(|_| ())?;
        Ok(Self(content))
    }
}

impl TryFrom<GatewayMessageCreateDto> for GatewayMessageCreateCommand {
    type Error = ();

    fn try_from(value: GatewayMessageCreateDto) -> Result<Self, Self::Error> {
        let attachment_ids =
            GatewayAttachmentIds::try_from(value.attachment_ids.unwrap_or_default())?;
        let content = GatewayMessageContent::try_from((value.content, !attachment_ids.is_empty()))?;
        Ok(Self {
            guild_id: GatewayGuildId::try_from(value.guild_id)?,
            channel_id: GatewayChannelId::try_from(value.channel_id)?,
            content,
            attachment_ids,
        })
    }
}

#[derive(Debug)]
pub(crate) enum GatewayIngressCommandParseError {
    InvalidSubscribePayload,
    InvalidMessageCreatePayload,
    UnknownEventType(String),
}

impl GatewayIngressCommandParseError {
    pub(crate) fn disconnect_reason(&self) -> &'static str {
        match self {
            Self::InvalidSubscribePayload => "invalid_subscribe_payload",
            Self::InvalidMessageCreatePayload => "invalid_message_create_payload",
            Self::UnknownEventType(_) => "unknown_event",
        }
    }
}

pub(crate) fn parse_gateway_ingress_command(
    envelope: Envelope<Value>,
) -> Result<GatewayIngressCommand, GatewayIngressCommandParseError> {
    let event_type = envelope.t.as_str().to_owned();
    match event_type.as_str() {
        "subscribe" => serde_json::from_value::<GatewaySubscribeDto>(envelope.d)
            .map_err(|_| GatewayIngressCommandParseError::InvalidSubscribePayload)
            .and_then(|subscribe| {
                GatewaySubscribeCommand::try_from(subscribe)
                    .map_err(|()| GatewayIngressCommandParseError::InvalidSubscribePayload)
            })
            .map(GatewayIngressCommand::Subscribe),
        "message_create" => serde_json::from_value::<GatewayMessageCreateDto>(envelope.d)
            .map_err(|_| GatewayIngressCommandParseError::InvalidMessageCreatePayload)
            .and_then(|message_create| {
                GatewayMessageCreateCommand::try_from(message_create)
                    .map_err(|()| GatewayIngressCommandParseError::InvalidMessageCreatePayload)
            })
            .map(GatewayIngressCommand::MessageCreate),
        _ => Err(GatewayIngressCommandParseError::UnknownEventType(
            event_type,
        )),
    }
}

#[cfg(test)]
mod tests {
    use filament_protocol::{Envelope, EventType, PROTOCOL_VERSION};
    use serde_json::json;

    use super::{
        parse_gateway_ingress_command, GatewayIngressCommand, GatewayIngressCommandParseError,
    };

    fn envelope(event_type: &str, payload: serde_json::Value) -> Envelope<serde_json::Value> {
        Envelope {
            v: PROTOCOL_VERSION,
            t: EventType::try_from(event_type.to_owned()).expect("event type should be valid"),
            d: payload,
        }
    }

    #[test]
    fn parses_subscribe_command() {
        let command = parse_gateway_ingress_command(envelope(
            "subscribe",
            json!({
                "guild_id": "01JYQ4V2YQ8B4FW9P51TE5Z1JK",
                "channel_id": "01JYQ4V3E2BTRWCHKRHV9K8HXT"
            }),
        ))
        .expect("subscribe payload should parse");

        match command {
            GatewayIngressCommand::Subscribe(subscribe) => {
                assert_eq!(subscribe.guild_id.as_str(), "01JYQ4V2YQ8B4FW9P51TE5Z1JK");
                assert_eq!(subscribe.channel_id.as_str(), "01JYQ4V3E2BTRWCHKRHV9K8HXT");
            }
            GatewayIngressCommand::MessageCreate(_) => {
                panic!("expected subscribe command");
            }
        }
    }

    #[test]
    fn parses_message_create_command() {
        let command = parse_gateway_ingress_command(envelope(
            "message_create",
            json!({
                "guild_id": "01JYQ4V2YQ8B4FW9P51TE5Z1JK",
                "channel_id": "01JYQ4V3E2BTRWCHKRHV9K8HXT",
                "content": "hello",
                "attachment_ids": ["01JYQ4V3VW1TC0MCC4GY7Q4RPR"]
            }),
        ))
        .expect("message_create payload should parse");

        match command {
            GatewayIngressCommand::MessageCreate(request) => {
                assert_eq!(request.guild_id.as_str(), "01JYQ4V2YQ8B4FW9P51TE5Z1JK");
                assert_eq!(request.channel_id.as_str(), "01JYQ4V3E2BTRWCHKRHV9K8HXT");
                assert_eq!(request.content.into_string(), "hello");
                assert_eq!(
                    request.attachment_ids.into_vec(),
                    vec![String::from("01JYQ4V3VW1TC0MCC4GY7Q4RPR")]
                );
            }
            GatewayIngressCommand::Subscribe(_) => {
                panic!("expected message_create command");
            }
        }
    }

    #[test]
    fn rejects_invalid_subscribe_payload() {
        let error = parse_gateway_ingress_command(envelope(
            "subscribe",
            json!({
                "guild_id": "01JYQ4V2YQ8B4FW9P51TE5Z1JK"
            }),
        ))
        .expect_err("invalid subscribe payload should fail");

        assert!(matches!(
            error,
            GatewayIngressCommandParseError::InvalidSubscribePayload
        ));
        assert_eq!(error.disconnect_reason(), "invalid_subscribe_payload");
    }

    #[test]
    fn rejects_subscribe_payload_with_unknown_fields() {
        let error = parse_gateway_ingress_command(envelope(
            "subscribe",
            json!({
                "guild_id": "01JYQ4V2YQ8B4FW9P51TE5Z1JK",
                "channel_id": "01JYQ4V3E2BTRWCHKRHV9K8HXT",
                "extra": "unexpected"
            }),
        ))
        .expect_err("subscribe payload with unknown field should fail");

        assert!(matches!(
            error,
            GatewayIngressCommandParseError::InvalidSubscribePayload
        ));
        assert_eq!(error.disconnect_reason(), "invalid_subscribe_payload");
    }

    #[test]
    fn rejects_message_create_payload_with_invalid_ids() {
        let error = parse_gateway_ingress_command(envelope(
            "message_create",
            json!({
                "guild_id": "not-a-ulid",
                "channel_id": "01JYQ4V3E2BTRWCHKRHV9K8HXT",
                "content": "hello"
            }),
        ))
        .expect_err("invalid message_create IDs should fail");

        assert!(matches!(
            error,
            GatewayIngressCommandParseError::InvalidMessageCreatePayload
        ));
        assert_eq!(error.disconnect_reason(), "invalid_message_create_payload");
    }

    #[test]
    fn parses_message_create_without_attachment_ids_as_empty_vec() {
        let command = parse_gateway_ingress_command(envelope(
            "message_create",
            json!({
                "guild_id": "01JYQ4V2YQ8B4FW9P51TE5Z1JK",
                "channel_id": "01JYQ4V3E2BTRWCHKRHV9K8HXT",
                "content": "hello"
            }),
        ))
        .expect("message_create payload should parse");

        match command {
            GatewayIngressCommand::MessageCreate(request) => {
                assert!(request.attachment_ids.into_vec().is_empty());
            }
            GatewayIngressCommand::Subscribe(_) => {
                panic!("expected message_create command");
            }
        }
    }

    #[test]
    fn rejects_message_create_payload_with_unknown_fields() {
        let error = parse_gateway_ingress_command(envelope(
            "message_create",
            json!({
                "guild_id": "01JYQ4V2YQ8B4FW9P51TE5Z1JK",
                "channel_id": "01JYQ4V3E2BTRWCHKRHV9K8HXT",
                "content": "hello",
                "extra": "unexpected"
            }),
        ))
        .expect_err("message_create payload with unknown field should fail");

        assert!(matches!(
            error,
            GatewayIngressCommandParseError::InvalidMessageCreatePayload
        ));
        assert_eq!(error.disconnect_reason(), "invalid_message_create_payload");
    }

    #[test]
    fn rejects_message_create_payload_with_invalid_attachment_ids() {
        let error = parse_gateway_ingress_command(envelope(
            "message_create",
            json!({
                "guild_id": "01JYQ4V2YQ8B4FW9P51TE5Z1JK",
                "channel_id": "01JYQ4V3E2BTRWCHKRHV9K8HXT",
                "content": "hello",
                "attachment_ids": ["not-a-ulid"]
            }),
        ))
        .expect_err("invalid message_create attachment IDs should fail");

        assert!(matches!(
            error,
            GatewayIngressCommandParseError::InvalidMessageCreatePayload
        ));
        assert_eq!(error.disconnect_reason(), "invalid_message_create_payload");
    }

    #[test]
    fn rejects_message_create_empty_content_without_attachments() {
        let error = parse_gateway_ingress_command(envelope(
            "message_create",
            json!({
                "guild_id": "01JYQ4V2YQ8B4FW9P51TE5Z1JK",
                "channel_id": "01JYQ4V3E2BTRWCHKRHV9K8HXT",
                "content": ""
            }),
        ))
        .expect_err("empty message without attachments should fail");

        assert!(matches!(
            error,
            GatewayIngressCommandParseError::InvalidMessageCreatePayload
        ));
        assert_eq!(error.disconnect_reason(), "invalid_message_create_payload");
    }

    #[test]
    fn parses_message_create_with_empty_content_and_attachments() {
        let command = parse_gateway_ingress_command(envelope(
            "message_create",
            json!({
                "guild_id": "01JYQ4V2YQ8B4FW9P51TE5Z1JK",
                "channel_id": "01JYQ4V3E2BTRWCHKRHV9K8HXT",
                "content": "",
                "attachment_ids": ["01JYQ4V3VW1TC0MCC4GY7Q4RPR"]
            }),
        ))
        .expect("message_create payload should parse");

        match command {
            GatewayIngressCommand::MessageCreate(request) => {
                assert_eq!(request.content.into_string(), "");
                assert_eq!(
                    request.attachment_ids.into_vec(),
                    vec![String::from("01JYQ4V3VW1TC0MCC4GY7Q4RPR")]
                );
            }
            GatewayIngressCommand::Subscribe(_) => {
                panic!("expected message_create command");
            }
        }
    }

    #[test]
    fn rejects_subscribe_payload_with_invalid_ids() {
        let error = parse_gateway_ingress_command(envelope(
            "subscribe",
            json!({
                "guild_id": "not-a-ulid",
                "channel_id": "01JYQ4V3E2BTRWCHKRHV9K8HXT"
            }),
        ))
        .expect_err("invalid subscribe IDs should fail");

        assert!(matches!(
            error,
            GatewayIngressCommandParseError::InvalidSubscribePayload
        ));
        assert_eq!(error.disconnect_reason(), "invalid_subscribe_payload");
    }

    #[test]
    fn rejects_unknown_event_type() {
        let error = parse_gateway_ingress_command(envelope("presence_sync", json!({})))
            .expect_err("unknown event should fail");

        match error {
            GatewayIngressCommandParseError::UnknownEventType(event_type) => {
                assert_eq!(event_type, "presence_sync");
            }
            GatewayIngressCommandParseError::InvalidSubscribePayload
            | GatewayIngressCommandParseError::InvalidMessageCreatePayload => {
                panic!("expected unknown event type error")
            }
        }
    }
}
