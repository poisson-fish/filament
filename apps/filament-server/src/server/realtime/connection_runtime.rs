use std::collections::HashMap;

use filament_core::UserId;
use tokio::sync::{mpsc, watch};
use uuid::Uuid;

use crate::server::{
    auth::{now_unix, release_media_subscribe_leases_for_user},
    core::{
        AppState, VoiceStreamKind, MAX_TRACKED_VOICE_CHANNELS,
        MAX_TRACKED_VOICE_PARTICIPANTS_PER_CHANNEL,
    },
    core::{
        ConnectionControl, ConnectionPresence, GuildConnectionIndex, Subscriptions,
        UserConnectionIndex,
    },
    errors::AuthFailure,
    gateway_events::{self, GatewayEvent},
    metrics::{record_gateway_event_dropped, record_gateway_event_emitted},
};

use super::{
    connection_disconnect_followups::{
        compute_disconnect_presence_outcome, plan_disconnect_followups,
    },
    fanout_dispatch::{
        connection_ids_for_user, dispatch_channel_payload, dispatch_guild_payload,
        dispatch_user_payload,
    },
    presence_subscribe::{
        apply_presence_subscribe, build_presence_subscribe_events, dispatch_presence_sync_event,
        presence_sync_reject_reason,
    },
    voice_cleanup_dispatch::{
        broadcast_disconnected_user_voice_removals, broadcast_expired_voice_removals,
        channel_user_voice_removal_broadcasts,
    },
    voice_registration::{apply_voice_registration_transition, plan_voice_registration_events},
    voice_registry::update_channel_user_voice_participant_audio_state,
    voice_sync_dispatch::{
        collect_voice_snapshots, dispatch_voice_sync_event, try_build_voice_subscribe_sync_event,
        voice_channel_key, voice_sync_reject_reason,
    },
};

fn signal_slow_connections_close(
    controls: &HashMap<Uuid, watch::Sender<ConnectionControl>>,
    slow_connections: Vec<Uuid>,
) {
    for connection_id in slow_connections {
        if let Some(control) = controls.get(&connection_id) {
            let _ = control.send(ConnectionControl::Close);
        }
    }
}

fn remove_connection_state(
    presence: &mut HashMap<Uuid, ConnectionPresence>,
    controls: &mut HashMap<Uuid, watch::Sender<ConnectionControl>>,
    senders: &mut HashMap<Uuid, mpsc::Sender<String>>,
    connection_id: Uuid,
) -> Option<ConnectionPresence> {
    let removed_presence = presence.remove(&connection_id);
    controls.remove(&connection_id);
    senders.remove(&connection_id);
    removed_presence
}

fn remove_connection_from_subscription_indexes(
    subscriptions: &mut Subscriptions,
    guild_connections: &mut GuildConnectionIndex,
    user_connections: &mut UserConnectionIndex,
    connection_id: Uuid,
) {
    subscriptions.retain(|_, listeners| {
        listeners.remove(&connection_id);
        !listeners.is_empty()
    });
    guild_connections.retain(|_, connection_ids| {
        connection_ids.remove(&connection_id);
        !connection_ids.is_empty()
    });
    user_connections.retain(|_, connection_ids| {
        connection_ids.remove(&connection_id);
        !connection_ids.is_empty()
    });
}

fn guild_id_from_subscription_key(key: &str) -> Option<&str> {
    let (guild_id, _channel_id) = key.split_once(':')?;
    if guild_id.is_empty() {
        return None;
    }
    Some(guild_id)
}

fn insert_connection_subscription(
    subscriptions: &mut Subscriptions,
    guild_connections: &mut GuildConnectionIndex,
    connection_id: Uuid,
    key: String,
    outbound_tx: mpsc::Sender<String>,
) {
    let guild_id = guild_id_from_subscription_key(&key).map(ToOwned::to_owned);
    subscriptions
        .entry(key)
        .or_default()
        .insert(connection_id, outbound_tx);
    if let Some(guild_id) = guild_id {
        guild_connections
            .entry(guild_id)
            .or_default()
            .insert(connection_id);
    }
}

fn emit_gateway_delivery_metrics(
    scope: &'static str,
    event_type: &'static str,
    delivered: usize,
) -> usize {
    if delivered == 0 {
        return 0;
    }

    tracing::debug!(event = "gateway.event.emit", scope, event_type, delivered);
    for _ in 0..delivered {
        record_gateway_event_emitted(scope, event_type);
    }

    delivered
}

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
    let planned_result = {
        let mut voice = state.realtime_registry.voice_participants().write().await;
        channel_user_voice_removal_broadcasts(
            &mut voice,
            guild_id,
            channel_id,
            user_id,
            removed_at_unix,
        )
    };
    let planned = match planned_result {
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
    let mut subscriptions = state.realtime_registry.subscriptions().write().await;
    let mut guild_connections = state.realtime_registry.guild_connections().write().await;
    insert_connection_subscription(
        &mut subscriptions,
        &mut guild_connections,
        connection_id,
        key,
        outbound_tx,
    );
}

pub(crate) async fn remove_connection(state: &AppState, connection_id: Uuid) {
    let removed_presence = {
        let mut presence = state.realtime_registry.connection_presence().write().await;
        let mut controls = state.realtime_registry.connection_controls().write().await;
        let mut senders = state.realtime_registry.connection_senders().write().await;
        remove_connection_state(&mut presence, &mut controls, &mut senders, connection_id)
    };

    let mut subscriptions = state.realtime_registry.subscriptions().write().await;
    let mut guild_connections = state.realtime_registry.guild_connections().write().await;
    let mut user_connections = state.realtime_registry.user_connections().write().await;
    remove_connection_from_subscription_indexes(
        &mut subscriptions,
        &mut guild_connections,
        &mut user_connections,
        connection_id,
    );

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
    use tokio::sync::mpsc;
    use tokio::sync::watch;
    use uuid::Uuid;

    use super::{
        emit_gateway_delivery_metrics, guild_id_from_subscription_key,
        insert_connection_subscription, presence_event_scope,
        remove_connection_from_subscription_indexes, remove_connection_state,
        should_skip_user_broadcast, signal_slow_connections_close,
    };
    use crate::server::{
        core::{
            ConnectionControl, ConnectionPresence, GuildConnectionIndex, Subscriptions,
            UserConnectionIndex,
        },
        gateway_events,
    };

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
    fn closes_only_requested_connections_with_registered_controls() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let missing = Uuid::new_v4();

        let (first_tx, first_rx) = watch::channel(ConnectionControl::Open);
        let (second_tx, second_rx) = watch::channel(ConnectionControl::Open);
        let mut controls = HashMap::new();
        controls.insert(first, first_tx);
        controls.insert(second, second_tx);

        signal_slow_connections_close(&controls, vec![first, missing]);

        assert_eq!(*first_rx.borrow(), ConnectionControl::Close);
        assert_eq!(*second_rx.borrow(), ConnectionControl::Open);
    }

    #[test]
    fn removes_presence_controls_and_sender_for_connection() {
        let connection_id = Uuid::new_v4();
        let user_id = UserId::new();
        let mut presence = HashMap::new();
        presence.insert(
            connection_id,
            ConnectionPresence {
                user_id,
                guild_ids: HashSet::new(),
            },
        );
        let (control_tx, _control_rx) = watch::channel(ConnectionControl::Open);
        let mut controls = HashMap::new();
        controls.insert(connection_id, control_tx);
        let (sender_tx, _sender_rx) = mpsc::channel::<String>(1);
        let mut senders = HashMap::new();
        senders.insert(connection_id, sender_tx);

        let removed =
            remove_connection_state(&mut presence, &mut controls, &mut senders, connection_id);

        assert_eq!(
            removed.expect("presence should be removed").user_id,
            user_id
        );
        assert!(!presence.contains_key(&connection_id));
        assert!(!controls.contains_key(&connection_id));
        assert!(!senders.contains_key(&connection_id));
    }

    #[test]
    fn remove_connection_state_returns_none_when_presence_is_missing() {
        let connection_id = Uuid::new_v4();
        let (control_tx, _control_rx) = watch::channel(ConnectionControl::Open);
        let mut controls = HashMap::new();
        controls.insert(connection_id, control_tx);
        let (sender_tx, _sender_rx) = mpsc::channel::<String>(1);
        let mut senders = HashMap::new();
        senders.insert(connection_id, sender_tx);

        let removed = remove_connection_state(
            &mut HashMap::new(),
            &mut controls,
            &mut senders,
            connection_id,
        );

        assert!(removed.is_none());
        assert!(!controls.contains_key(&connection_id));
        assert!(!senders.contains_key(&connection_id));
    }

    #[test]
    fn remove_connection_indexes_prunes_empty_entries() {
        let target = Uuid::new_v4();
        let keep = Uuid::new_v4();
        let target_user = UserId::new();
        let mixed_user = UserId::new();
        let (target_tx, _) = mpsc::channel::<String>(1);
        let (keep_tx, _) = mpsc::channel::<String>(1);

        let mut subscriptions: Subscriptions = HashMap::from([
            (String::from("g1:c1"), HashMap::from([(target, target_tx)])),
            (String::from("g1:c2"), HashMap::from([(keep, keep_tx)])),
        ]);
        let mut guild_connections: GuildConnectionIndex = HashMap::from([
            (String::from("g1"), HashSet::from([target, keep])),
            (String::from("g2"), HashSet::from([target])),
        ]);
        let mut user_connections: UserConnectionIndex = HashMap::from([
            (target_user, HashSet::from([target])),
            (mixed_user, HashSet::from([target, keep])),
        ]);

        remove_connection_from_subscription_indexes(
            &mut subscriptions,
            &mut guild_connections,
            &mut user_connections,
            target,
        );

        assert!(!subscriptions.contains_key("g1:c1"));
        assert!(subscriptions.contains_key("g1:c2"));
        assert!(!guild_connections.contains_key("g2"));
        assert!(guild_connections
            .get("g1")
            .expect("mixed guild should remain")
            .contains(&keep));
        assert!(!user_connections.contains_key(&target_user));
        assert!(user_connections
            .get(&mixed_user)
            .expect("mixed user should remain")
            .contains(&keep));
    }

    #[test]
    fn remove_connection_indexes_retains_entries_with_remaining_connections() {
        let target = Uuid::new_v4();
        let keep = Uuid::new_v4();
        let mixed_user = UserId::new();
        let (target_tx, _) = mpsc::channel::<String>(1);
        let (keep_tx, _) = mpsc::channel::<String>(1);

        let mut subscriptions: Subscriptions = HashMap::from([(
            String::from("g1:c1"),
            HashMap::from([(target, target_tx), (keep, keep_tx)]),
        )]);
        let mut guild_connections: GuildConnectionIndex =
            HashMap::from([(String::from("g1"), HashSet::from([target, keep]))]);
        let mut user_connections: UserConnectionIndex =
            HashMap::from([(mixed_user, HashSet::from([target, keep]))]);

        remove_connection_from_subscription_indexes(
            &mut subscriptions,
            &mut guild_connections,
            &mut user_connections,
            target,
        );

        let listeners = subscriptions
            .get("g1:c1")
            .expect("entry should be retained for remaining listeners");
        assert_eq!(listeners.len(), 1);
        assert!(listeners.contains_key(&keep));
        assert!(guild_connections
            .get("g1")
            .expect("guild should remain")
            .contains(&keep));
        assert!(user_connections
            .get(&mixed_user)
            .expect("user should remain")
            .contains(&keep));
    }

    #[test]
    fn insert_connection_subscription_indexes_guild_from_valid_key() {
        let connection_id = Uuid::new_v4();
        let (tx, _rx) = mpsc::channel::<String>(1);
        let mut subscriptions = HashMap::new();
        let mut guild_connections = GuildConnectionIndex::new();

        insert_connection_subscription(
            &mut subscriptions,
            &mut guild_connections,
            connection_id,
            String::from("guild:channel"),
            tx,
        );

        let listeners = subscriptions
            .get("guild:channel")
            .expect("listener map should exist");
        assert_eq!(listeners.len(), 1);
        assert!(listeners.contains_key(&connection_id));
        assert!(guild_connections
            .get("guild")
            .expect("guild index should exist")
            .contains(&connection_id));
    }

    #[test]
    fn insert_connection_subscription_rejects_invalid_guild_index_key_shape() {
        let connection_id = Uuid::new_v4();
        let (tx, _rx) = mpsc::channel::<String>(1);
        let mut subscriptions = HashMap::new();
        let mut guild_connections = GuildConnectionIndex::new();

        insert_connection_subscription(
            &mut subscriptions,
            &mut guild_connections,
            connection_id,
            String::from("invalid-key"),
            tx,
        );

        assert!(subscriptions.contains_key("invalid-key"));
        assert!(guild_connections.is_empty());
    }

    #[test]
    fn returns_zero_when_nothing_delivered() {
        let emitted = emit_gateway_delivery_metrics("channel", "message_create", 0);

        assert_eq!(emitted, 0);
    }

    #[test]
    fn returns_delivered_count_when_events_emitted() {
        let emitted = emit_gateway_delivery_metrics("guild", "presence_update", 3);

        assert_eq!(emitted, 3);
    }
}
