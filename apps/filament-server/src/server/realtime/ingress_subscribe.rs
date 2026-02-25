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
    let enqueue_result = try_enqueue_subscribed_event(outbound_tx, subscribed_event.payload);
    if subscribe_ack_rejected(&enqueue_result) {
        record_gateway_event_dropped("connection", subscribed_event.event_type, "full_queue");
        return Err("outbound_queue_full");
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

pub(crate) fn subscribe_ack_rejected(result: &SubscribeAckEnqueueResult) -> bool {
    matches!(result, SubscribeAckEnqueueResult::Rejected)
}

#[cfg(test)]
mod tests {
    use super::subscribe_ack_rejected;
    use crate::server::realtime::subscribe_ack::SubscribeAckEnqueueResult;

    #[test]
    fn subscribe_ack_rejected_returns_true_for_rejected() {
        assert!(subscribe_ack_rejected(&SubscribeAckEnqueueResult::Rejected));
    }

    #[test]
    fn subscribe_ack_rejected_returns_false_for_enqueued() {
        assert!(!subscribe_ack_rejected(
            &SubscribeAckEnqueueResult::Enqueued
        ));
    }
}
