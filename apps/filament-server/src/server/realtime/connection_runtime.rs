use filament_core::UserId;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::server::{
    auth::now_unix,
    core::{
        AppState, VoiceStreamKind, MAX_TRACKED_VOICE_CHANNELS,
        MAX_TRACKED_VOICE_PARTICIPANTS_PER_CHANNEL,
    },
    errors::AuthFailure,
    gateway_events::GatewayEvent,
};

use super::{
    connection_control::signal_slow_connections_close,
    connection_disconnect_followups::plan_disconnect_followups,
    connection_registry::remove_connection_state,
    connection_subscriptions::remove_connection_from_subscriptions,
    emit_metrics::emit_gateway_delivery_metrics,
    fanout_channel::dispatch_channel_payload,
    fanout_guild::dispatch_guild_payload,
    fanout_user::dispatch_user_payload,
    fanout_user_targets::connection_ids_for_user,
    presence_disconnect::compute_disconnect_presence_outcome,
    presence_subscribe::apply_presence_subscribe,
    presence_subscribe_events::build_presence_subscribe_events,
    presence_sync_dispatch::dispatch_presence_sync_event,
    subscription_insert::insert_connection_subscription,
    voice_cleanup_dispatch::{
        broadcast_disconnected_user_voice_removals, broadcast_expired_voice_removals,
    },
    voice_presence::{collect_voice_snapshots, voice_channel_key},
    voice_registration::apply_voice_registration_transition,
    voice_registration_events::plan_voice_registration_events,
    voice_subscribe_sync::build_voice_subscribe_sync_event,
    voice_sync_dispatch::dispatch_voice_sync_event,
};

async fn close_slow_connections(state: &AppState, slow_connections: Vec<Uuid>) {
    if slow_connections.is_empty() {
        return;
    }

    let controls = state.realtime_registry.connection_controls().read().await;
    signal_slow_connections_close(&controls, slow_connections);
}

pub(crate) async fn broadcast_channel_event(state: &AppState, key: &str, event: &GatewayEvent) {
    let mut slow_connections = Vec::new();
    let mut subscriptions = state.realtime_registry.subscriptions().write().await;
    let delivered = dispatch_channel_payload(
        &mut subscriptions,
        key,
        &event.payload,
        event.event_type,
        &mut slow_connections,
    );
    drop(subscriptions);

    close_slow_connections(state, slow_connections).await;
    emit_gateway_delivery_metrics("channel", event.event_type, delivered);
}

pub(crate) async fn broadcast_guild_event(state: &AppState, guild_id: &str, event: &GatewayEvent) {
    let mut slow_connections = Vec::new();
    let mut subscriptions = state.realtime_registry.subscriptions().write().await;
    let delivered = dispatch_guild_payload(
        &mut subscriptions,
        guild_id,
        &event.payload,
        event.event_type,
        &mut slow_connections,
    );
    drop(subscriptions);

    close_slow_connections(state, slow_connections).await;
    emit_gateway_delivery_metrics("guild", event.event_type, delivered);
}

fn should_skip_user_broadcast(connection_ids: &[Uuid]) -> bool {
    connection_ids.is_empty()
}

#[allow(dead_code)]
pub(crate) async fn broadcast_user_event(state: &AppState, user_id: UserId, event: &GatewayEvent) {
    let connection_ids = {
        let presence = state.realtime_registry.connection_presence().read().await;
        connection_ids_for_user(&presence, user_id)
    };
    if should_skip_user_broadcast(&connection_ids) {
        return;
    }

    let mut slow_connections = Vec::new();
    let mut senders = state.realtime_registry.connection_senders().write().await;
    let delivered = dispatch_user_payload(
        &mut senders,
        &connection_ids,
        &event.payload,
        event.event_type,
        &mut slow_connections,
    );
    drop(senders);

    close_slow_connections(state, slow_connections).await;
    emit_gateway_delivery_metrics("user", event.event_type, delivered);
}

async fn prune_expired_voice_participants(state: &AppState, now_unix: i64) {
    broadcast_expired_voice_removals(state, now_unix).await;
}

pub(crate) async fn register_voice_participant_from_token(
    state: &AppState,
    user_id: UserId,
    guild_id: &str,
    channel_id: &str,
    identity: &str,
    publish_streams: &[VoiceStreamKind],
    expires_at_unix: i64,
) -> Result<(), AuthFailure> {
    prune_expired_voice_participants(state, now_unix()).await;
    let now = now_unix();
    let key = voice_channel_key(guild_id, channel_id);
    let transition = {
        let mut channels = state.realtime_registry.voice_participants().write().await;
        apply_voice_registration_transition(
            &mut channels,
            &key,
            user_id,
            identity,
            publish_streams,
            expires_at_unix,
            now,
            MAX_TRACKED_VOICE_CHANNELS,
            MAX_TRACKED_VOICE_PARTICIPANTS_PER_CHANNEL,
        )?
    };
    for (subscription_key, event) in
        plan_voice_registration_events(transition, guild_id, channel_id, user_id, identity, now)
    {
        broadcast_channel_event(state, &subscription_key, &event).await;
    }

    Ok(())
}

pub(crate) async fn handle_voice_subscribe(
    state: &AppState,
    guild_id: &str,
    channel_id: &str,
    outbound_tx: &mpsc::Sender<String>,
) {
    prune_expired_voice_participants(state, now_unix()).await;
    let key = voice_channel_key(guild_id, channel_id);
    let participants = {
        let voice = state.realtime_registry.voice_participants().read().await;
        collect_voice_snapshots(&voice, &key)
    };

    let sync_event =
        build_voice_subscribe_sync_event(guild_id, channel_id, participants, now_unix());
    dispatch_voice_sync_event(outbound_tx, sync_event);
}

async fn remove_disconnected_user_voice_participants(
    state: &AppState,
    user_id: UserId,
    disconnected_at_unix: i64,
) {
    broadcast_disconnected_user_voice_removals(state, user_id, disconnected_at_unix).await;
}

pub(crate) async fn handle_presence_subscribe(
    state: &AppState,
    connection_id: Uuid,
    user_id: UserId,
    guild_id: &str,
    outbound_tx: &mpsc::Sender<String>,
) {
    let result = {
        let mut presence = state.realtime_registry.connection_presence().write().await;
        apply_presence_subscribe(&mut presence, connection_id, user_id, guild_id)
    };
    let Some(result) = result else {
        return;
    };

    let events = build_presence_subscribe_events(guild_id, user_id, result);
    dispatch_presence_sync_event(outbound_tx, events.snapshot);

    if let Some(update) = events.online_update {
        broadcast_guild_event(state, guild_id, &update).await;
    }
}

pub(crate) async fn add_subscription(
    state: &AppState,
    connection_id: Uuid,
    key: String,
    outbound_tx: mpsc::Sender<String>,
) {
    let mut subscriptions = state.realtime_registry.subscriptions().write().await;
    insert_connection_subscription(&mut subscriptions, connection_id, key, outbound_tx);
}

pub(crate) async fn remove_connection(state: &AppState, connection_id: Uuid) {
    let removed_presence = {
        let mut presence = state.realtime_registry.connection_presence().write().await;
        let mut controls = state.realtime_registry.connection_controls().write().await;
        let mut senders = state.realtime_registry.connection_senders().write().await;
        remove_connection_state(&mut presence, &mut controls, &mut senders, connection_id)
    };

    let mut subscriptions = state.realtime_registry.subscriptions().write().await;
    remove_connection_from_subscriptions(&mut subscriptions, connection_id);
    drop(subscriptions);

    let Some(removed_presence) = removed_presence else {
        return;
    };
    let outcome = {
        let remaining = state.realtime_registry.connection_presence().read().await;
        compute_disconnect_presence_outcome(&remaining, &removed_presence)
    };
    let followups = plan_disconnect_followups(outcome, removed_presence.user_id);

    if followups.remove_voice_participants {
        remove_disconnected_user_voice_participants(state, removed_presence.user_id, now_unix())
            .await;
    }

    for (guild_id, update) in followups.offline_updates {
        broadcast_guild_event(state, &guild_id, &update).await;
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::should_skip_user_broadcast;

    #[test]
    fn should_skip_user_broadcast_when_no_targets() {
        assert!(should_skip_user_broadcast(&[]));
    }

    #[test]
    fn should_not_skip_user_broadcast_when_targets_exist() {
        assert!(!should_skip_user_broadcast(&[Uuid::new_v4()]));
    }
}
