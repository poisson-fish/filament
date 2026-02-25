use filament_core::UserId;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::server::{
    auth::{now_unix, release_media_subscribe_leases_for_user},
    core::{
        AppState, VoiceStreamKind, MAX_TRACKED_VOICE_CHANNELS,
        MAX_TRACKED_VOICE_PARTICIPANTS_PER_CHANNEL,
    },
    errors::AuthFailure,
    gateway_events::{self, GatewayEvent},
    metrics::record_gateway_event_dropped,
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
    presence_sync_dispatch::{dispatch_presence_sync_event, presence_sync_reject_reason},
    subscription_insert::insert_connection_subscription,
    voice_cleanup_dispatch::{
        broadcast_disconnected_user_voice_removals, broadcast_expired_voice_removals,
    },
    voice_cleanup_registry::channel_user_voice_removal_broadcasts,
    voice_presence::{collect_voice_snapshots, voice_channel_key},
    voice_registration::apply_voice_registration_transition,
    voice_registration_events::plan_voice_registration_events,
    voice_registry::update_channel_user_voice_participant_audio_state,
    voice_subscribe_sync::try_build_voice_subscribe_sync_event,
    voice_sync_dispatch::{dispatch_voice_sync_event, voice_sync_reject_reason},
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
        state.runtime.max_gateway_event_bytes,
        event.event_type,
        &mut slow_connections,
    );
    drop(subscriptions);

    close_slow_connections(state, slow_connections).await;
    emit_gateway_delivery_metrics("channel", event.event_type, delivered);
}

pub(crate) async fn broadcast_guild_event(state: &AppState, guild_id: &str, event: &GatewayEvent) {
    let mut slow_connections = Vec::new();
    let mut guild_connections = state.realtime_registry.guild_connections().write().await;
    let mut senders = state.realtime_registry.connection_senders().write().await;
    let delivered = dispatch_guild_payload(
        &mut guild_connections,
        &mut senders,
        guild_id,
        &event.payload,
        state.runtime.max_gateway_event_bytes,
        event.event_type,
        &mut slow_connections,
    );
    drop(senders);
    drop(guild_connections);

    close_slow_connections(state, slow_connections).await;
    emit_gateway_delivery_metrics("guild", event.event_type, delivered);
}

fn should_skip_user_broadcast(connection_ids: &[Uuid]) -> bool {
    connection_ids.is_empty()
}

fn presence_event_scope(event_type: &'static str) -> &'static str {
    if event_type == gateway_events::PRESENCE_UPDATE_EVENT {
        "guild"
    } else {
        "connection"
    }
}

#[allow(dead_code)]
pub(crate) async fn broadcast_user_event(state: &AppState, user_id: UserId, event: &GatewayEvent) {
    let connection_ids = {
        let user_connections = state.realtime_registry.user_connections().read().await;
        connection_ids_for_user(&user_connections, user_id)
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
        state.runtime.max_gateway_event_bytes,
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
    let planned = match plan_voice_registration_events(
        transition, guild_id, channel_id, user_id, identity, now,
    ) {
        Ok(planned) => planned,
        Err(error) => {
            tracing::warn!(
                event = "gateway.voice_registration.serialize_failed",
                guild_id,
                channel_id,
                event_type = error.event_type,
                error = %error.source,
            );
            record_gateway_event_dropped("channel", error.event_type, "serialize_error");
            return Ok(());
        }
    };
    for (subscription_key, event) in planned {
        broadcast_channel_event(state, &subscription_key, &event).await;
    }

    Ok(())
}

pub(crate) async fn remove_voice_participant_for_channel(
    state: &AppState,
    user_id: UserId,
    guild_id: &str,
    channel_id: &str,
    removed_at_unix: i64,
) {
    let planned = match {
        let mut voice = state.realtime_registry.voice_participants().write().await;
        channel_user_voice_removal_broadcasts(
            &mut voice,
            guild_id,
            channel_id,
            user_id,
            removed_at_unix,
        )
    } {
        Ok(planned) => planned,
        Err(error) => {
            tracing::warn!(
                event = "gateway.voice_cleanup.serialize_failed",
                guild_id,
                channel_id,
                user_id = %user_id,
                event_type = error.event_type,
                error = %error.source,
            );
            record_gateway_event_dropped("channel", error.event_type, "serialize_error");
            return;
        }
    };

    for (channel_subscription_key, event) in planned {
        broadcast_channel_event(state, &channel_subscription_key, &event).await;
    }
}

pub(crate) async fn update_voice_participant_audio_state_for_channel(
    state: &AppState,
    user_id: UserId,
    guild_id: &str,
    channel_id: &str,
    is_muted: Option<bool>,
    is_deafened: Option<bool>,
    updated_at_unix: i64,
) {
    let updated = {
        let mut voice = state.realtime_registry.voice_participants().write().await;
        update_channel_user_voice_participant_audio_state(
            &mut voice,
            guild_id,
            channel_id,
            user_id,
            is_muted,
            is_deafened,
            updated_at_unix,
        )
    };

    let Some((channel_subscription_key, participant, changed_muted, changed_deafened)) = updated
    else {
        return;
    };
    let event = match gateway_events::try_voice_participant_update(
        guild_id,
        channel_id,
        participant.user_id,
        &participant.identity,
        changed_muted,
        changed_deafened,
        None,
        None,
        None,
        participant.updated_at_unix,
    ) {
        Ok(event) => event,
        Err(error) => {
            tracing::warn!(
                event = "gateway.voice_participant_update.serialize_failed",
                guild_id,
                channel_id,
                event_type = gateway_events::VOICE_PARTICIPANT_UPDATE_EVENT,
                error = %error,
            );
            record_gateway_event_dropped(
                "channel",
                gateway_events::VOICE_PARTICIPANT_UPDATE_EVENT,
                "serialize_error",
            );
            return;
        }
    };
    broadcast_channel_event(state, &channel_subscription_key, &event).await;
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

    let sync_event = match try_build_voice_subscribe_sync_event(
        guild_id,
        channel_id,
        participants,
        now_unix(),
    ) {
        Ok(event) => event,
        Err(error) => {
            tracing::warn!(
                event = "gateway.voice_subscribe.serialize_failed",
                guild_id,
                channel_id,
                event_type = gateway_events::VOICE_PARTICIPANT_SYNC_EVENT,
                error = %error,
            );
            record_gateway_event_dropped(
                "connection",
                gateway_events::VOICE_PARTICIPANT_SYNC_EVENT,
                "serialize_error",
            );
            return;
        }
    };
    let outcome = dispatch_voice_sync_event(
        outbound_tx,
        sync_event,
        state.runtime.max_gateway_event_bytes,
    );
    if let Some(reason) = voice_sync_reject_reason(&outcome) {
        tracing::warn!(
            event = "gateway.voice_subscribe.enqueue_rejected",
            guild_id,
            channel_id,
            event_type = gateway_events::VOICE_PARTICIPANT_SYNC_EVENT,
            reject_reason = reason
        );
    }
}

async fn remove_disconnected_user_voice_participants(
    state: &AppState,
    user_id: UserId,
    disconnected_at_unix: i64,
) {
    broadcast_disconnected_user_voice_removals(state, user_id, disconnected_at_unix).await;
}

fn guild_id_from_subscription_key(key: &str) -> Option<&str> {
    let (guild_id, _channel_id) = key.split_once(':')?;
    if guild_id.is_empty() {
        return None;
    }
    Some(guild_id)
}

fn index_connection_for_guild(
    guild_connections: &mut std::collections::HashMap<String, std::collections::HashSet<Uuid>>,
    guild_id: &str,
    connection_id: Uuid,
) {
    guild_connections
        .entry(guild_id.to_owned())
        .or_default()
        .insert(connection_id);
}

fn remove_connection_from_guild_index(
    guild_connections: &mut std::collections::HashMap<String, std::collections::HashSet<Uuid>>,
    connection_id: Uuid,
) {
    guild_connections.retain(|_, connection_ids| {
        connection_ids.remove(&connection_id);
        !connection_ids.is_empty()
    });
}

fn remove_connection_from_user_index(
    user_connections: &mut std::collections::HashMap<UserId, std::collections::HashSet<Uuid>>,
    connection_id: Uuid,
) {
    user_connections.retain(|_, connection_ids| {
        connection_ids.remove(&connection_id);
        !connection_ids.is_empty()
    });
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

    let events = match build_presence_subscribe_events(guild_id, user_id, result) {
        Ok(events) => events,
        Err(error) => {
            tracing::warn!(
                event = "gateway.presence_subscribe.serialize_failed",
                connection_id = %connection_id,
                user_id = %user_id,
                guild_id,
                event_type = error.event_type,
                error = %error.source
            );
            record_gateway_event_dropped(
                presence_event_scope(error.event_type),
                error.event_type,
                "serialize_error",
            );
            return;
        }
    };
    let outcome = dispatch_presence_sync_event(
        outbound_tx,
        events.snapshot,
        state.runtime.max_gateway_event_bytes,
    );
    if let Some(reason) = presence_sync_reject_reason(&outcome) {
        tracing::warn!(
            event = "gateway.presence_subscribe.enqueue_rejected",
            connection_id = %connection_id,
            user_id = %user_id,
            guild_id,
            event_type = gateway_events::PRESENCE_SYNC_EVENT,
            reject_reason = reason
        );
    }

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
    let guild_id = guild_id_from_subscription_key(&key).map(ToOwned::to_owned);
    let mut subscriptions = state.realtime_registry.subscriptions().write().await;
    insert_connection_subscription(&mut subscriptions, connection_id, key, outbound_tx);
    drop(subscriptions);

    if let Some(guild_id) = guild_id {
        let mut guild_connections = state.realtime_registry.guild_connections().write().await;
        index_connection_for_guild(&mut guild_connections, &guild_id, connection_id);
    }
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

    {
        let mut guild_connections = state.realtime_registry.guild_connections().write().await;
        remove_connection_from_guild_index(&mut guild_connections, connection_id);
    }
    {
        let mut user_connections = state.realtime_registry.user_connections().write().await;
        remove_connection_from_user_index(&mut user_connections, connection_id);
    }

    let Some(removed_presence) = removed_presence else {
        return;
    };
    let outcome = {
        let remaining = state.realtime_registry.connection_presence().read().await;
        compute_disconnect_presence_outcome(&remaining, &removed_presence)
    };
    let followups = match plan_disconnect_followups(outcome, removed_presence.user_id) {
        Ok(followups) => followups,
        Err(error) => {
            tracing::warn!(
                event = "gateway.presence_disconnect.serialize_failed",
                connection_id = %connection_id,
                user_id = %removed_presence.user_id,
                event_type = error.event_type,
                error = %error.source
            );
            record_gateway_event_dropped("guild", error.event_type, "serialize_error");
            return;
        }
    };

    if followups.remove_voice_participants {
        release_media_subscribe_leases_for_user(state, removed_presence.user_id).await;
        remove_disconnected_user_voice_participants(state, removed_presence.user_id, now_unix())
            .await;
    }

    for (guild_id, update) in followups.offline_updates {
        broadcast_guild_event(state, &guild_id, &update).await;
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use filament_core::UserId;
    use uuid::Uuid;

    use super::{
        guild_id_from_subscription_key, presence_event_scope, remove_connection_from_guild_index,
        remove_connection_from_user_index, should_skip_user_broadcast,
    };
    use crate::server::gateway_events;

    #[test]
    fn should_skip_user_broadcast_when_no_targets() {
        assert!(should_skip_user_broadcast(&[]));
    }

    #[test]
    fn should_not_skip_user_broadcast_when_targets_exist() {
        assert!(!should_skip_user_broadcast(&[Uuid::new_v4()]));
    }

    #[test]
    fn presence_event_scope_matches_fanout_target() {
        assert_eq!(
            presence_event_scope(gateway_events::PRESENCE_SYNC_EVENT),
            "connection"
        );
        assert_eq!(
            presence_event_scope(gateway_events::PRESENCE_UPDATE_EVENT),
            "guild"
        );
    }

    #[test]
    fn guild_id_from_subscription_key_parses_valid_shape() {
        assert_eq!(guild_id_from_subscription_key("g-1:c-1"), Some("g-1"));
    }

    #[test]
    fn guild_id_from_subscription_key_rejects_invalid_shape() {
        assert_eq!(guild_id_from_subscription_key(""), None);
        assert_eq!(guild_id_from_subscription_key(":c-1"), None);
        assert_eq!(guild_id_from_subscription_key("g-1"), None);
    }

    #[test]
    fn remove_connection_from_guild_index_prunes_empty_sets() {
        let target = Uuid::new_v4();
        let keep = Uuid::new_v4();
        let mut guild_connections: HashMap<String, HashSet<Uuid>> = HashMap::from([
            (String::from("g-1"), HashSet::from([target, keep])),
            (String::from("g-2"), HashSet::from([target])),
        ]);

        remove_connection_from_guild_index(&mut guild_connections, target);

        assert_eq!(
            guild_connections
                .get("g-1")
                .expect("guild should remain with keep connection")
                .len(),
            1
        );
        assert!(guild_connections
            .get("g-1")
            .expect("guild should remain")
            .contains(&keep));
        assert!(!guild_connections.contains_key("g-2"));
    }

    #[test]
    fn remove_connection_from_user_index_prunes_empty_sets() {
        let target = Uuid::new_v4();
        let keep = Uuid::new_v4();
        let user_target = UserId::new();
        let user_mixed = UserId::new();
        let mut user_connections: HashMap<UserId, HashSet<Uuid>> = HashMap::from([
            (user_target, HashSet::from([target])),
            (user_mixed, HashSet::from([target, keep])),
        ]);

        remove_connection_from_user_index(&mut user_connections, target);

        assert!(!user_connections.contains_key(&user_target));
        assert!(user_connections
            .get(&user_mixed)
            .expect("mixed user should remain")
            .contains(&keep));
    }
}
