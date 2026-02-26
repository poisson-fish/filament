use std::collections::{HashMap, HashSet};

use filament_core::UserId;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::server::{
    core::ConnectionPresence,
    gateway_events::{self, GatewayEvent},
    metrics::{record_gateway_event_dropped, record_gateway_event_emitted},
};

pub(crate) struct PresenceSubscribeResult {
    pub(crate) snapshot_user_ids: HashSet<String>,
    pub(crate) became_online: bool,
}

pub(crate) struct PresenceSubscribeEvents {
    pub(crate) snapshot: GatewayEvent,
    pub(crate) online_update: Option<GatewayEvent>,
}

#[derive(Debug)]
pub(crate) struct PresenceSubscribeEventBuildError {
    pub(crate) event_type: &'static str,
    pub(crate) source: anyhow::Error,
}

pub(crate) fn build_presence_subscribe_events(
    guild_id: &str,
    user_id: UserId,
    result: PresenceSubscribeResult,
) -> Result<PresenceSubscribeEvents, PresenceSubscribeEventBuildError> {
    let snapshot =
        gateway_events::try_presence_sync(guild_id, result.snapshot_user_ids).map_err(|error| {
            PresenceSubscribeEventBuildError {
                event_type: gateway_events::PRESENCE_SYNC_EVENT,
                source: error,
            }
        })?;
    let online_update = if result.became_online {
        Some(
            gateway_events::try_presence_update(guild_id, user_id, "online").map_err(|error| {
                PresenceSubscribeEventBuildError {
                    event_type: gateway_events::PRESENCE_UPDATE_EVENT,
                    source: error,
                }
            })?,
        )
    } else {
        None
    };
    Ok(PresenceSubscribeEvents {
        snapshot,
        online_update,
    })
}

pub(crate) enum PresenceSyncEnqueueResult {
    Enqueued,
    Closed,
    Full,
    Oversized,
}

pub(crate) enum PresenceSyncDispatchOutcome {
    Emitted,
    DroppedClosed,
    DroppedFull,
    DroppedOversized,
}

pub(crate) fn apply_presence_subscribe(
    presence: &mut HashMap<Uuid, ConnectionPresence>,
    connection_id: Uuid,
    user_id: UserId,
    guild_id: &str,
) -> Option<PresenceSubscribeResult> {
    let guild = guild_id.to_owned();
    let existing = presence.get(&connection_id)?;
    let already_subscribed = existing.guild_ids.contains(&guild);
    let was_online = presence
        .values()
        .any(|entry| entry.user_id == user_id && entry.guild_ids.contains(&guild));

    if let Some(connection) = presence.get_mut(&connection_id) {
        connection.guild_ids.insert(guild.clone());
    }

    let snapshot_user_ids = presence
        .values()
        .filter(|entry| entry.guild_ids.contains(&guild))
        .map(|entry| entry.user_id.to_string())
        .collect::<HashSet<_>>();

    Some(PresenceSubscribeResult {
        snapshot_user_ids,
        became_online: !was_online && !already_subscribed,
    })
}

pub(crate) fn try_enqueue_presence_sync_event(
    outbound_tx: &mpsc::Sender<String>,
    payload: String,
    max_gateway_event_bytes: usize,
) -> PresenceSyncEnqueueResult {
    if payload.len() > max_gateway_event_bytes {
        return PresenceSyncEnqueueResult::Oversized;
    }
    match outbound_tx.try_send(payload) {
        Ok(()) => PresenceSyncEnqueueResult::Enqueued,
        Err(mpsc::error::TrySendError::Closed(_)) => PresenceSyncEnqueueResult::Closed,
        Err(mpsc::error::TrySendError::Full(_)) => PresenceSyncEnqueueResult::Full,
    }
}

pub(crate) fn presence_sync_dispatch_outcome(
    result: &PresenceSyncEnqueueResult,
) -> PresenceSyncDispatchOutcome {
    match result {
        PresenceSyncEnqueueResult::Enqueued => PresenceSyncDispatchOutcome::Emitted,
        PresenceSyncEnqueueResult::Closed => PresenceSyncDispatchOutcome::DroppedClosed,
        PresenceSyncEnqueueResult::Full => PresenceSyncDispatchOutcome::DroppedFull,
        PresenceSyncEnqueueResult::Oversized => PresenceSyncDispatchOutcome::DroppedOversized,
    }
}

pub(crate) fn dispatch_presence_sync_event(
    outbound_tx: &mpsc::Sender<String>,
    event: GatewayEvent,
    max_gateway_event_bytes: usize,
) -> PresenceSyncDispatchOutcome {
    let enqueue_result =
        try_enqueue_presence_sync_event(outbound_tx, event.payload, max_gateway_event_bytes);
    let outcome = presence_sync_dispatch_outcome(&enqueue_result);
    match outcome {
        PresenceSyncDispatchOutcome::Emitted => {
            record_gateway_event_emitted("connection", event.event_type);
        }
        PresenceSyncDispatchOutcome::DroppedClosed => {
            record_gateway_event_dropped("connection", event.event_type, "closed");
        }
        PresenceSyncDispatchOutcome::DroppedFull => {
            record_gateway_event_dropped("connection", event.event_type, "full_queue");
        }
        PresenceSyncDispatchOutcome::DroppedOversized => {
            record_gateway_event_dropped("connection", event.event_type, "oversized_outbound");
        }
    }
    outcome
}

pub(crate) fn presence_sync_reject_reason(
    outcome: &PresenceSyncDispatchOutcome,
) -> Option<&'static str> {
    match outcome {
        PresenceSyncDispatchOutcome::Emitted => None,
        PresenceSyncDispatchOutcome::DroppedClosed => Some("closed"),
        PresenceSyncDispatchOutcome::DroppedFull => Some("full_queue"),
        PresenceSyncDispatchOutcome::DroppedOversized => Some("oversized_outbound"),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use filament_core::UserId;
    use uuid::Uuid;

    use super::{
        apply_presence_subscribe, build_presence_subscribe_events, dispatch_presence_sync_event,
        presence_sync_dispatch_outcome, presence_sync_reject_reason,
        try_enqueue_presence_sync_event, PresenceSyncDispatchOutcome, PresenceSyncEnqueueResult,
    };
    use crate::server::core::ConnectionPresence;
    use crate::server::gateway_events;
    use crate::server::metrics::metrics_state;

    #[test]
    fn inserts_guild_and_marks_first_online_subscription() {
        let user_id = UserId::new();
        let connection_id = Uuid::new_v4();
        let mut presence = HashMap::from([(
            connection_id,
            ConnectionPresence {
                user_id,
                guild_ids: HashSet::new(),
            },
        )]);

        let result = apply_presence_subscribe(&mut presence, connection_id, user_id, "g-1")
            .expect("connection presence should exist");

        assert!(result.became_online);
        assert_eq!(
            result.snapshot_user_ids,
            HashSet::from([user_id.to_string()])
        );
        assert!(presence
            .get(&connection_id)
            .expect("connection should remain")
            .guild_ids
            .contains("g-1"));
    }

    #[test]
    fn does_not_mark_online_when_already_online_in_guild() {
        let user_id = UserId::new();
        let first_connection = Uuid::new_v4();
        let second_connection = Uuid::new_v4();

        let mut presence = HashMap::from([
            (
                first_connection,
                ConnectionPresence {
                    user_id,
                    guild_ids: HashSet::from([String::from("g-1")]),
                },
            ),
            (
                second_connection,
                ConnectionPresence {
                    user_id,
                    guild_ids: HashSet::new(),
                },
            ),
        ]);

        let result = apply_presence_subscribe(&mut presence, second_connection, user_id, "g-1")
            .expect("connection presence should exist");

        assert!(!result.became_online);
        assert_eq!(
            result.snapshot_user_ids,
            HashSet::from([user_id.to_string()])
        );
    }

    #[test]
    fn returns_none_when_connection_is_missing() {
        let mut presence = HashMap::new();

        let result = apply_presence_subscribe(&mut presence, Uuid::new_v4(), UserId::new(), "g-1");

        assert!(result.is_none());
    }

    #[test]
    fn enqueue_presence_sync_event_reports_enqueued() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(1);

        let result = try_enqueue_presence_sync_event(&tx, String::from("payload"), 1024);

        assert!(matches!(result, PresenceSyncEnqueueResult::Enqueued));
        let received = rx.try_recv().expect("payload should be queued");
        assert_eq!(received, "payload");
    }

    #[test]
    fn enqueue_presence_sync_event_reports_full() {
        let (tx, _rx) = tokio::sync::mpsc::channel::<String>(1);
        assert!(matches!(
            try_enqueue_presence_sync_event(&tx, String::from("first"), 1024),
            PresenceSyncEnqueueResult::Enqueued
        ));

        let result = try_enqueue_presence_sync_event(&tx, String::from("second"), 1024);

        assert!(matches!(result, PresenceSyncEnqueueResult::Full));
    }

    #[test]
    fn enqueue_presence_sync_event_reports_closed() {
        let (tx, rx) = tokio::sync::mpsc::channel::<String>(1);
        drop(rx);

        let result = try_enqueue_presence_sync_event(&tx, String::from("payload"), 1024);

        assert!(matches!(result, PresenceSyncEnqueueResult::Closed));
    }

    #[test]
    fn enqueue_presence_sync_event_reports_oversized() {
        let (tx, _rx) = tokio::sync::mpsc::channel::<String>(1);

        let result = try_enqueue_presence_sync_event(&tx, String::from("payload"), 3);

        assert!(matches!(result, PresenceSyncEnqueueResult::Oversized));
    }

    #[test]
    fn presence_sync_dispatch_outcome_maps_all_enqueue_results() {
        assert!(matches!(
            presence_sync_dispatch_outcome(&PresenceSyncEnqueueResult::Enqueued),
            PresenceSyncDispatchOutcome::Emitted
        ));
        assert!(matches!(
            presence_sync_dispatch_outcome(&PresenceSyncEnqueueResult::Closed),
            PresenceSyncDispatchOutcome::DroppedClosed
        ));
        assert!(matches!(
            presence_sync_dispatch_outcome(&PresenceSyncEnqueueResult::Full),
            PresenceSyncDispatchOutcome::DroppedFull
        ));
        assert!(matches!(
            presence_sync_dispatch_outcome(&PresenceSyncEnqueueResult::Oversized),
            PresenceSyncDispatchOutcome::DroppedOversized
        ));
    }

    #[test]
    fn dispatch_presence_sync_event_returns_emitted_for_open_queue() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(1);
        let event = gateway_events::try_presence_sync("g-1", HashSet::new())
            .expect("presence_sync event should serialize");
        let expected_payload = event.payload.clone();

        let outcome = dispatch_presence_sync_event(&tx, event, 1024);

        assert!(matches!(outcome, PresenceSyncDispatchOutcome::Emitted));
        assert_eq!(
            rx.try_recv().ok().as_deref(),
            Some(expected_payload.as_str())
        );
    }

    #[test]
    fn dispatch_presence_sync_event_returns_full_for_full_queue() {
        let (tx, _rx) = tokio::sync::mpsc::channel::<String>(1);
        tx.try_send(String::from("occupied"))
            .expect("queue should be full");
        let event = gateway_events::try_presence_sync("g-1", HashSet::new())
            .expect("presence_sync event should serialize");

        let outcome = dispatch_presence_sync_event(&tx, event, 1024);

        assert!(matches!(outcome, PresenceSyncDispatchOutcome::DroppedFull));
    }

    #[test]
    fn dispatch_presence_sync_event_returns_closed_for_closed_queue() {
        let (tx, rx) = tokio::sync::mpsc::channel::<String>(1);
        drop(rx);
        let event = gateway_events::try_presence_sync("g-1", HashSet::new())
            .expect("presence_sync event should serialize");

        let outcome = dispatch_presence_sync_event(&tx, event, 1024);

        assert!(matches!(
            outcome,
            PresenceSyncDispatchOutcome::DroppedClosed
        ));
    }

    #[test]
    fn dispatch_presence_sync_event_returns_oversized_for_large_payload() {
        let (tx, _rx) = tokio::sync::mpsc::channel::<String>(1);
        let event = gateway_events::try_presence_sync("g-1", HashSet::new())
            .expect("presence_sync event should serialize");

        let outcome = dispatch_presence_sync_event(&tx, event, 3);

        assert!(matches!(
            outcome,
            PresenceSyncDispatchOutcome::DroppedOversized
        ));
    }

    #[test]
    fn oversized_presence_sync_rejection_is_counted_as_drop() {
        let before = metrics_state()
            .gateway_events_dropped
            .lock()
            .expect("gateway dropped metrics mutex should not be poisoned")
            .get(&(
                String::from("connection"),
                String::from(gateway_events::PRESENCE_SYNC_EVENT),
                String::from("oversized_outbound"),
            ))
            .copied()
            .unwrap_or(0);
        let (tx, _rx) = tokio::sync::mpsc::channel::<String>(1);
        let event = gateway_events::try_presence_sync("g-1", HashSet::new())
            .expect("presence_sync event should serialize");

        let outcome = dispatch_presence_sync_event(&tx, event, 3);

        assert!(matches!(
            outcome,
            PresenceSyncDispatchOutcome::DroppedOversized
        ));
        let after = metrics_state()
            .gateway_events_dropped
            .lock()
            .expect("gateway dropped metrics mutex should not be poisoned")
            .get(&(
                String::from("connection"),
                String::from(gateway_events::PRESENCE_SYNC_EVENT),
                String::from("oversized_outbound"),
            ))
            .copied()
            .unwrap_or(0);
        assert!(after > before);
    }

    #[test]
    fn presence_sync_reject_reason_maps_dispatch_outcomes() {
        assert_eq!(
            presence_sync_reject_reason(&PresenceSyncDispatchOutcome::Emitted),
            None
        );
        assert_eq!(
            presence_sync_reject_reason(&PresenceSyncDispatchOutcome::DroppedClosed),
            Some("closed")
        );
        assert_eq!(
            presence_sync_reject_reason(&PresenceSyncDispatchOutcome::DroppedFull),
            Some("full_queue")
        );
        assert_eq!(
            presence_sync_reject_reason(&PresenceSyncDispatchOutcome::DroppedOversized),
            Some("oversized_outbound")
        );
    }

    #[test]
    fn includes_online_update_for_first_online_transition() {
        let user_id = UserId::new();
        let result = super::PresenceSubscribeResult {
            snapshot_user_ids: HashSet::from([user_id.to_string()]),
            became_online: true,
        };

        let events =
            build_presence_subscribe_events("g1", user_id, result).expect("events should build");

        assert_eq!(events.snapshot.event_type, "presence_sync");
        assert!(events.online_update.is_some());
        let online = events.online_update.expect("online update expected");
        assert_eq!(online.event_type, "presence_update");
        assert!(online.payload.contains("\"status\":\"online\""));
    }

    #[test]
    fn omits_online_update_without_transition() {
        let user_id = UserId::new();
        let first_snapshot = UserId::new().to_string();
        let second_snapshot = UserId::new().to_string();
        let result = super::PresenceSubscribeResult {
            snapshot_user_ids: HashSet::from([first_snapshot.clone(), second_snapshot.clone()]),
            became_online: false,
        };

        let events =
            build_presence_subscribe_events("g1", user_id, result).expect("events should build");

        assert_eq!(events.snapshot.event_type, "presence_sync");
        assert!(events.online_update.is_none());
        assert!(events.snapshot.payload.contains(&first_snapshot));
        assert!(events.snapshot.payload.contains(&second_snapshot));
    }

    #[test]
    fn preserves_snapshot_user_membership() {
        let expected: HashSet<String> = HashSet::from([
            UserId::new().to_string(),
            UserId::new().to_string(),
            UserId::new().to_string(),
        ]);
        let result = super::PresenceSubscribeResult {
            snapshot_user_ids: expected.clone(),
            became_online: false,
        };

        let events = build_presence_subscribe_events("g1", UserId::new(), result)
            .expect("events should build");

        for user_id in expected {
            assert!(events.snapshot.payload.contains(&user_id));
        }
    }
}
