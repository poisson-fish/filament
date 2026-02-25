use filament_core::UserId;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::{
    add_subscription, handle_presence_subscribe, handle_voice_subscribe,
    subscribe_ack::{try_enqueue_subscribed_event, SubscribeAckEnqueueResult},
};
use crate::server::{
    auth::{channel_key, ClientIp},
    core::AppState,
    domain::{enforce_guild_ip_ban_for_request, user_can_write_channel},
    gateway_events,
    metrics::{record_gateway_event_dropped, record_gateway_event_emitted},
    types::GatewaySubscribe,
};

pub(crate) async fn execute_subscribe_command(
    state: &AppState,
    connection_id: Uuid,
    user_id: UserId,
    client_ip: ClientIp,
    subscribe: GatewaySubscribe,
    outbound_tx: &mpsc::Sender<String>,
) -> Result<(), &'static str> {
    if enforce_guild_ip_ban_for_request(
        state,
        &subscribe.guild_id,
        user_id,
        client_ip,
        "gateway.subscribe",
    )
    .await
    .is_err()
    {
        return Err("ip_banned");
    }
    if !user_can_write_channel(state, user_id, &subscribe.guild_id, &subscribe.channel_id).await {
        return Err("forbidden_channel");
    }

    add_subscription(
        state,
        connection_id,
        channel_key(&subscribe.guild_id, &subscribe.channel_id),
        outbound_tx.clone(),
    )
    .await;
    handle_presence_subscribe(
        state,
        connection_id,
        user_id,
        &subscribe.guild_id,
        outbound_tx,
    )
    .await;

    let subscribed_event =
        match gateway_events::try_subscribed(&subscribe.guild_id, &subscribe.channel_id) {
            Ok(event) => event,
            Err(error) => {
                tracing::error!(
                    event = "gateway.subscribe_ack.serialize_failed",
                    connection_id = %connection_id,
                    user_id = %user_id,
                    guild_id = %subscribe.guild_id,
                    channel_id = %subscribe.channel_id,
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
            guild_id = %subscribe.guild_id,
            channel_id = %subscribe.channel_id,
            reason
        );
    }
    if let Some(reason) = subscribe_ack_error_reason(&enqueue_result) {
        return Err(reason);
    }
    record_gateway_event_emitted("connection", subscribed_event.event_type);

    handle_voice_subscribe(
        state,
        &subscribe.guild_id,
        &subscribe.channel_id,
        outbound_tx,
    )
    .await;
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

#[cfg(test)]
mod tests {
    use super::{
        subscribe_ack_drop_metric_reason, subscribe_ack_error_reason,
        subscribe_ack_reject_log_reason,
    };
    use crate::server::realtime::subscribe_ack::SubscribeAckEnqueueResult;

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
}
