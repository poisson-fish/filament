use filament_protocol::Envelope;
use serde_json::Value;

use crate::server::types::{GatewayMessageCreate, GatewaySubscribe};

#[derive(Debug)]
pub(crate) enum GatewayIngressCommand {
    Subscribe(GatewaySubscribe),
    MessageCreate(GatewayMessageCreate),
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
        "subscribe" => serde_json::from_value::<GatewaySubscribe>(envelope.d)
            .map(GatewayIngressCommand::Subscribe)
            .map_err(|_| GatewayIngressCommandParseError::InvalidSubscribePayload),
        "message_create" => serde_json::from_value::<GatewayMessageCreate>(envelope.d)
            .map(GatewayIngressCommand::MessageCreate)
            .map_err(|_| GatewayIngressCommandParseError::InvalidMessageCreatePayload),
        _ => Err(GatewayIngressCommandParseError::UnknownEventType(event_type)),
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
                "guild_id": "g1",
                "channel_id": "c1"
            }),
        ))
        .expect("subscribe payload should parse");

        match command {
            GatewayIngressCommand::Subscribe(subscribe) => {
                assert_eq!(subscribe.guild_id, "g1");
                assert_eq!(subscribe.channel_id, "c1");
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
                "guild_id": "g1",
                "channel_id": "c1",
                "content": "hello",
                "attachment_ids": ["a1"]
            }),
        ))
        .expect("message_create payload should parse");

        match command {
            GatewayIngressCommand::MessageCreate(request) => {
                assert_eq!(request.guild_id, "g1");
                assert_eq!(request.channel_id, "c1");
                assert_eq!(request.content, "hello");
                assert_eq!(request.attachment_ids.as_deref(), Some(&[String::from("a1")][..]));
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
                "guild_id": "g1"
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