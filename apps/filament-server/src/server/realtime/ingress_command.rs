use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use axum::extract::ws::Message;
use filament_core::UserId;
use filament_protocol::Envelope;
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::mpsc;
use ulid::Ulid;
use uuid::Uuid;

use crate::server::{
    auth::{validate_message_content, ClientIp},
    core::{AppState, AuthContext},
    domain::{enforce_guild_ip_ban_for_request, parse_attachment_ids, user_can_write_channel},
    gateway_events,
    metrics::{record_gateway_event_dropped, record_gateway_event_emitted},
};

use super::{
    add_subscription, create_message_internal_from_ingress_validated, handle_presence_subscribe,
    handle_voice_subscribe,
};

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

impl TryFrom<Envelope<Value>> for GatewayIngressCommand {
    type Error = GatewayIngressCommandParseError;

    fn try_from(envelope: Envelope<Value>) -> Result<Self, Self::Error> {
        let event_type = envelope.t.as_str().to_owned();
        match event_type.as_str() {
            "subscribe" => serde_json::from_value::<GatewaySubscribeDto>(envelope.d)
                .map_err(|_| GatewayIngressCommandParseError::InvalidSubscribePayload)
                .and_then(|subscribe| {
                    GatewaySubscribeCommand::try_from(subscribe)
                        .map_err(|()| GatewayIngressCommandParseError::InvalidSubscribePayload)
                })
                .map(Self::Subscribe),
            "message_create" => serde_json::from_value::<GatewayMessageCreateDto>(envelope.d)
                .map_err(|_| GatewayIngressCommandParseError::InvalidMessageCreatePayload)
                .and_then(|message_create| {
                    GatewayMessageCreateCommand::try_from(message_create)
                        .map_err(|()| GatewayIngressCommandParseError::InvalidMessageCreatePayload)
                })
                .map(Self::MessageCreate),
            _ => Err(GatewayIngressCommandParseError::UnknownEventType(
                event_type,
            )),
        }
    }
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
    pub(crate) subscription_key: GatewaySubscriptionKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GatewaySubscriptionKey(String);

impl GatewaySubscriptionKey {
    pub(crate) fn into_string(self) -> String {
        self.0
    }
}

impl TryFrom<GatewaySubscribeDto> for GatewaySubscribeCommand {
    type Error = ();

    fn try_from(value: GatewaySubscribeDto) -> Result<Self, Self::Error> {
        let guild_id = GatewayGuildId::try_from(value.guild_id)?;
        let channel_id = GatewayChannelId::try_from(value.channel_id)?;
        Ok(Self {
            subscription_key: GatewaySubscriptionKey(format!(
                "{}:{}",
                guild_id.as_str(),
                channel_id.as_str()
            )),
            guild_id,
            channel_id,
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

pub(crate) enum IngressCommandParseClassification<'a> {
    ParseRejected(&'static str),
    UnknownEventType(&'a str),
}

pub(crate) fn classify_ingress_command_parse_error(
    error: &GatewayIngressCommandParseError,
) -> IngressCommandParseClassification<'_> {
    match error {
        GatewayIngressCommandParseError::InvalidSubscribePayload => {
            IngressCommandParseClassification::ParseRejected("invalid_subscribe_payload")
        }
        GatewayIngressCommandParseError::InvalidMessageCreatePayload => {
            IngressCommandParseClassification::ParseRejected("invalid_message_create_payload")
        }
        GatewayIngressCommandParseError::UnknownEventType(event_type) => {
            IngressCommandParseClassification::UnknownEventType(event_type)
        }
    }
}

pub(crate) fn parse_gateway_ingress_command(
    envelope: Envelope<Value>,
) -> Result<GatewayIngressCommand, GatewayIngressCommandParseError> {
    GatewayIngressCommand::try_from(envelope)
}

pub(crate) enum SubscribeAckEnqueueResult {
    Enqueued,
    Closed,
    Full,
    Oversized,
}

pub(crate) fn try_enqueue_subscribed_event(
    outbound_tx: &mpsc::Sender<String>,
    payload: String,
    max_gateway_event_bytes: usize,
) -> SubscribeAckEnqueueResult {
    if payload.len() > max_gateway_event_bytes {
        return SubscribeAckEnqueueResult::Oversized;
    }

    match outbound_tx.try_send(payload) {
        Ok(()) => SubscribeAckEnqueueResult::Enqueued,
        Err(mpsc::error::TrySendError::Closed(_)) => SubscribeAckEnqueueResult::Closed,
        Err(mpsc::error::TrySendError::Full(_)) => SubscribeAckEnqueueResult::Full,
    }
}

pub(crate) async fn execute_message_create_command(
    state: &AppState,
    auth: &AuthContext,
    client_ip: ClientIp,
    request: GatewayMessageCreateCommand,
) -> Result<(), &'static str> {
    if enforce_guild_ip_ban_for_request(
        state,
        request.guild_id.as_str(),
        auth.user_id,
        client_ip,
        "gateway.message_create",
    )
    .await
    .is_err()
    {
        return Err("ip_banned");
    }

    if create_message_internal_from_ingress_validated(
        state,
        auth,
        request.guild_id.as_str(),
        request.channel_id.as_str(),
        request.content,
        request.attachment_ids,
    )
    .await
    .is_err()
    {
        return Err("message_rejected");
    }

    Ok(())
}

pub(crate) async fn execute_subscribe_command(
    state: &AppState,
    connection_id: Uuid,
    user_id: UserId,
    client_ip: ClientIp,
    subscribe: GatewaySubscribeCommand,
    outbound_tx: &mpsc::Sender<String>,
) -> Result<(), &'static str> {
    let GatewaySubscribeCommand {
        guild_id,
        channel_id,
        subscription_key,
    } = subscribe;
    let guild_id = guild_id.as_str();
    let channel_id = channel_id.as_str();

    if enforce_guild_ip_ban_for_request(state, guild_id, user_id, client_ip, "gateway.subscribe")
        .await
        .is_err()
    {
        return Err("ip_banned");
    }
    if !user_can_write_channel(state, user_id, guild_id, channel_id).await {
        return Err("forbidden_channel");
    }

    add_subscription(
        state,
        connection_id,
        subscription_key.into_string(),
        outbound_tx.clone(),
    )
    .await;
    handle_presence_subscribe(state, connection_id, user_id, guild_id, outbound_tx).await;

    let subscribed_event = match gateway_events::try_subscribed(guild_id, channel_id) {
        Ok(event) => event,
        Err(error) => {
            tracing::error!(
                event = "gateway.subscribe_ack.serialize_failed",
                connection_id = %connection_id,
                user_id = %user_id,
                guild_id,
                channel_id,
                error = %error
            );
            record_gateway_event_dropped(
                "connection",
                gateway_events::SUBSCRIBED_EVENT,
                "serialize_error",
            );
            return Err("outbound_serialize_error");
        }
    };
    let enqueue_result = try_enqueue_subscribed_event(
        outbound_tx,
        subscribed_event.payload,
        state.runtime.max_gateway_event_bytes,
    );
    if let Some(reason) = subscribe_ack_drop_metric_reason(&enqueue_result) {
        record_gateway_event_dropped("connection", subscribed_event.event_type, reason);
    }
    if let Some(reason) = subscribe_ack_reject_log_reason(&enqueue_result) {
        tracing::warn!(
            event = "gateway.subscribe_ack.enqueue_rejected",
            connection_id = %connection_id,
            user_id = %user_id,
            guild_id,
            channel_id,
            reason
        );
    }
    if let Some(reason) = subscribe_ack_error_reason(&enqueue_result) {
        return Err(reason);
    }
    record_gateway_event_emitted("connection", subscribed_event.event_type);

    handle_voice_subscribe(state, guild_id, channel_id, outbound_tx).await;
    Ok(())
}

pub(crate) fn subscribe_ack_error_reason(
    result: &SubscribeAckEnqueueResult,
) -> Option<&'static str> {
    match result {
        SubscribeAckEnqueueResult::Enqueued => None,
        SubscribeAckEnqueueResult::Full => Some("outbound_queue_full"),
        SubscribeAckEnqueueResult::Closed => Some("outbound_queue_closed"),
        SubscribeAckEnqueueResult::Oversized => Some("outbound_payload_too_large"),
    }
}

pub(crate) fn subscribe_ack_drop_metric_reason(
    result: &SubscribeAckEnqueueResult,
) -> Option<&'static str> {
    match result {
        SubscribeAckEnqueueResult::Enqueued => None,
        SubscribeAckEnqueueResult::Full => Some("full_queue"),
        SubscribeAckEnqueueResult::Closed => Some("closed"),
        SubscribeAckEnqueueResult::Oversized => Some("oversized_outbound"),
    }
}

pub(crate) fn subscribe_ack_reject_log_reason(
    result: &SubscribeAckEnqueueResult,
) -> Option<&'static str> {
    match result {
        SubscribeAckEnqueueResult::Enqueued => None,
        SubscribeAckEnqueueResult::Full => Some("full_queue"),
        SubscribeAckEnqueueResult::Closed => Some("closed"),
        SubscribeAckEnqueueResult::Oversized => Some("oversized_outbound"),
    }
}

pub(crate) enum GatewayIngressMessageDecode {
    Payload(Vec<u8>),
    Continue,
    Disconnect(&'static str),
}

pub(crate) fn decode_gateway_ingress_message(
    message: Message,
    max_gateway_event_bytes: usize,
) -> GatewayIngressMessageDecode {
    match message {
        Message::Text(text) => {
            if text.len() > max_gateway_event_bytes {
                return GatewayIngressMessageDecode::Disconnect("event_too_large");
            }
            GatewayIngressMessageDecode::Payload(text.as_bytes().to_vec())
        }
        Message::Binary(bytes) => {
            if bytes.len() > max_gateway_event_bytes {
                return GatewayIngressMessageDecode::Disconnect("event_too_large");
            }
            GatewayIngressMessageDecode::Payload(bytes.to_vec())
        }
        Message::Close(_) => GatewayIngressMessageDecode::Disconnect("client_close"),
        Message::Ping(_) | Message::Pong(_) => GatewayIngressMessageDecode::Continue,
    }
}

pub(crate) fn allow_gateway_ingress(
    ingress: &mut VecDeque<Instant>,
    limit: u32,
    window: Duration,
) -> bool {
    let now = Instant::now();
    while ingress
        .front()
        .is_some_and(|oldest| now.duration_since(*oldest) > window)
    {
        let _ = ingress.pop_front();
    }

    if ingress.len() >= limit as usize {
        return false;
    }

    ingress.push_back(now);
    true
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        time::{Duration, Instant},
    };

    use filament_protocol::{Envelope, EventType, PROTOCOL_VERSION};
    use serde_json::json;

    use super::{
        allow_gateway_ingress, classify_ingress_command_parse_error,
        decode_gateway_ingress_message, parse_gateway_ingress_command,
        subscribe_ack_drop_metric_reason, subscribe_ack_error_reason,
        subscribe_ack_reject_log_reason, try_enqueue_subscribed_event, GatewayIngressCommand,
        GatewayIngressCommandParseError, GatewayIngressMessageDecode,
        IngressCommandParseClassification, SubscribeAckEnqueueResult,
    };
    use axum::extract::ws::Message;
    use tokio::sync::mpsc;

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
                assert_eq!(
                    subscribe.subscription_key.into_string(),
                    "01JYQ4V2YQ8B4FW9P51TE5Z1JK:01JYQ4V3E2BTRWCHKRHV9K8HXT"
                );
            }
            GatewayIngressCommand::MessageCreate(_) => {
                panic!("expected subscribe command");
            }
        }
    }

    #[test]
    fn rejects_subscribe_command_with_invalid_ulid_in_try_from() {
        let envelope = envelope(
            "subscribe",
            json!({
                "guild_id": "not-a-ulid",
                "channel_id": "01JYQ4V3E2BTRWCHKRHV9K8HXT"
            }),
        );

        let error = GatewayIngressCommand::try_from(envelope)
            .expect_err("invalid subscribe ids should fail in try_from");

        assert!(matches!(
            error,
            GatewayIngressCommandParseError::InvalidSubscribePayload
        ));
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
    fn parses_message_create_command_deduping_attachment_ids() {
        let command = parse_gateway_ingress_command(envelope(
            "message_create",
            json!({
                "guild_id": "01JYQ4V2YQ8B4FW9P51TE5Z1JK",
                "channel_id": "01JYQ4V3E2BTRWCHKRHV9K8HXT",
                "content": "hello",
                "attachment_ids": [
                    "01JYQ4V3VW1TC0MCC4GY7Q4RPR",
                    "01JYQ4V4EA6J2QY3K8Y6DX93Q2",
                    "01JYQ4V3VW1TC0MCC4GY7Q4RPR"
                ]
            }),
        ))
        .expect("message_create payload with duplicate attachment ids should parse");

        match command {
            GatewayIngressCommand::MessageCreate(request) => {
                assert_eq!(
                    request.attachment_ids.into_vec(),
                    vec![
                        String::from("01JYQ4V3VW1TC0MCC4GY7Q4RPR"),
                        String::from("01JYQ4V4EA6J2QY3K8Y6DX93Q2"),
                    ]
                );
            }
            GatewayIngressCommand::Subscribe(_) => {
                panic!("expected message_create command");
            }
        }
    }

    #[test]
    fn rejects_message_create_command_with_invalid_attachment_ids_in_try_from() {
        let envelope = envelope(
            "message_create",
            json!({
                "guild_id": "01JYQ4V2YQ8B4FW9P51TE5Z1JK",
                "channel_id": "01JYQ4V3E2BTRWCHKRHV9K8HXT",
                "content": "hello",
                "attachment_ids": ["not-a-ulid"]
            }),
        );

        let error = GatewayIngressCommand::try_from(envelope)
            .expect_err("invalid attachment ids should fail in try_from");

        assert!(matches!(
            error,
            GatewayIngressCommandParseError::InvalidMessageCreatePayload
        ));
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

    #[test]
    fn classifies_invalid_subscribe_payload_as_parse_rejected() {
        let classification = classify_ingress_command_parse_error(
            &GatewayIngressCommandParseError::InvalidSubscribePayload,
        );

        assert!(matches!(
            classification,
            IngressCommandParseClassification::ParseRejected("invalid_subscribe_payload")
        ));
    }

    #[test]
    fn classifies_invalid_message_create_payload_as_parse_rejected() {
        let classification = classify_ingress_command_parse_error(
            &GatewayIngressCommandParseError::InvalidMessageCreatePayload,
        );

        assert!(matches!(
            classification,
            IngressCommandParseClassification::ParseRejected("invalid_message_create_payload")
        ));
    }

    #[test]
    fn classifies_unknown_event_type_as_unknown_event() {
        let error =
            GatewayIngressCommandParseError::UnknownEventType(String::from("presence_sync"));
        let classification = classify_ingress_command_parse_error(&error);

        assert!(matches!(
            classification,
            IngressCommandParseClassification::UnknownEventType("presence_sync")
        ));
    }

    #[test]
    fn decodes_text_payload_when_within_cap() {
        let message = Message::Text("{\"v\":1,\"t\":\"subscribe\",\"d\":{}}".into());

        match decode_gateway_ingress_message(message, 256) {
            GatewayIngressMessageDecode::Payload(payload) => {
                assert_eq!(payload, b"{\"v\":1,\"t\":\"subscribe\",\"d\":{}}".to_vec());
            }
            GatewayIngressMessageDecode::Continue => panic!("expected payload"),
            GatewayIngressMessageDecode::Disconnect(reason) => {
                panic!("unexpected disconnect: {reason}")
            }
        }
    }

    #[test]
    fn rejects_oversized_binary_payload() {
        let message = Message::Binary(vec![1_u8, 2_u8, 3_u8].into());

        match decode_gateway_ingress_message(message, 2) {
            GatewayIngressMessageDecode::Disconnect(reason) => {
                assert_eq!(reason, "event_too_large");
            }
            GatewayIngressMessageDecode::Payload(_) | GatewayIngressMessageDecode::Continue => {
                panic!("expected disconnect")
            }
        }
    }

    #[test]
    fn maps_close_to_client_close_disconnect() {
        let message = Message::Close(None);

        match decode_gateway_ingress_message(message, 256) {
            GatewayIngressMessageDecode::Disconnect(reason) => {
                assert_eq!(reason, "client_close");
            }
            GatewayIngressMessageDecode::Payload(_) | GatewayIngressMessageDecode::Continue => {
                panic!("expected disconnect")
            }
        }
    }

    #[test]
    fn ignores_ping_messages() {
        let message = Message::Ping(vec![1_u8].into());

        match decode_gateway_ingress_message(message, 256) {
            GatewayIngressMessageDecode::Continue => {}
            GatewayIngressMessageDecode::Payload(_) => panic!("expected continue"),
            GatewayIngressMessageDecode::Disconnect(reason) => {
                panic!("unexpected disconnect: {reason}")
            }
        }
    }

    #[test]
    fn ingress_rate_limit_allows_when_under_limit() {
        let mut ingress = VecDeque::new();
        assert!(allow_gateway_ingress(
            &mut ingress,
            2,
            Duration::from_millis(250),
        ));
        assert_eq!(ingress.len(), 1);
    }

    #[test]
    fn ingress_rate_limit_rejects_when_at_limit_inside_window() {
        let mut ingress = VecDeque::new();
        let now = Instant::now();
        ingress.push_back(
            now.checked_sub(Duration::from_millis(50))
                .expect("instant subtraction should succeed"),
        );
        ingress.push_back(
            now.checked_sub(Duration::from_millis(10))
                .expect("instant subtraction should succeed"),
        );

        assert!(!allow_gateway_ingress(
            &mut ingress,
            2,
            Duration::from_millis(250),
        ));
    }

    #[test]
    fn ingress_rate_limit_evicts_expired_entries_before_checking_limit() {
        let mut ingress = VecDeque::new();
        let now = Instant::now();
        ingress.push_back(
            now.checked_sub(Duration::from_secs(2))
                .expect("instant subtraction should succeed"),
        );

        assert!(allow_gateway_ingress(
            &mut ingress,
            1,
            Duration::from_millis(100),
        ));
        assert_eq!(ingress.len(), 1);
    }

    #[test]
    fn subscribe_ack_error_reason_returns_none_for_enqueued() {
        assert_eq!(
            subscribe_ack_error_reason(&SubscribeAckEnqueueResult::Enqueued),
            None
        );
    }

    #[test]
    fn subscribe_ack_error_reason_maps_all_rejections() {
        assert_eq!(
            subscribe_ack_error_reason(&SubscribeAckEnqueueResult::Full),
            Some("outbound_queue_full")
        );
        assert_eq!(
            subscribe_ack_error_reason(&SubscribeAckEnqueueResult::Closed),
            Some("outbound_queue_closed")
        );
        assert_eq!(
            subscribe_ack_error_reason(&SubscribeAckEnqueueResult::Oversized),
            Some("outbound_payload_too_large")
        );
    }

    #[test]
    fn subscribe_ack_drop_metric_reason_maps_all_rejections() {
        assert_eq!(
            subscribe_ack_drop_metric_reason(&SubscribeAckEnqueueResult::Enqueued),
            None
        );
        assert_eq!(
            subscribe_ack_drop_metric_reason(&SubscribeAckEnqueueResult::Full),
            Some("full_queue")
        );
        assert_eq!(
            subscribe_ack_drop_metric_reason(&SubscribeAckEnqueueResult::Closed),
            Some("closed")
        );
        assert_eq!(
            subscribe_ack_drop_metric_reason(&SubscribeAckEnqueueResult::Oversized),
            Some("oversized_outbound")
        );
    }

    #[test]
    fn subscribe_ack_reject_log_reason_maps_all_rejections() {
        assert_eq!(
            subscribe_ack_reject_log_reason(&SubscribeAckEnqueueResult::Enqueued),
            None
        );
        assert_eq!(
            subscribe_ack_reject_log_reason(&SubscribeAckEnqueueResult::Full),
            Some("full_queue")
        );
        assert_eq!(
            subscribe_ack_reject_log_reason(&SubscribeAckEnqueueResult::Closed),
            Some("closed")
        );
        assert_eq!(
            subscribe_ack_reject_log_reason(&SubscribeAckEnqueueResult::Oversized),
            Some("oversized_outbound")
        );
    }

    #[test]
    fn try_enqueue_subscribed_event_returns_enqueued_when_sender_has_capacity() {
        let (tx, _rx) = mpsc::channel::<String>(1);

        let result = try_enqueue_subscribed_event(&tx, String::from("payload"), 1024);

        assert!(matches!(result, SubscribeAckEnqueueResult::Enqueued));
    }

    #[test]
    fn try_enqueue_subscribed_event_returns_full_when_sender_is_full() {
        let (tx, rx) = mpsc::channel::<String>(1);
        tx.try_send(String::from("first"))
            .expect("first send should fill queue");

        let full_result = try_enqueue_subscribed_event(&tx, String::from("second"), 1024);
        assert!(matches!(full_result, SubscribeAckEnqueueResult::Full));

        drop(rx);
    }

    #[test]
    fn try_enqueue_subscribed_event_returns_closed_when_sender_is_closed() {
        let (tx, rx) = mpsc::channel::<String>(1);
        drop(rx);
        let closed_result = try_enqueue_subscribed_event(&tx, String::from("third"), 1024);
        assert!(matches!(closed_result, SubscribeAckEnqueueResult::Closed));
    }

    #[test]
    fn try_enqueue_subscribed_event_returns_oversized_when_payload_exceeds_limit() {
        let (tx, _rx) = mpsc::channel::<String>(1);

        let result = try_enqueue_subscribed_event(&tx, String::from("payload"), 3);

        assert!(matches!(result, SubscribeAckEnqueueResult::Oversized));
    }
}
