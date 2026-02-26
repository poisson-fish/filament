use tokio::sync::mpsc;

use crate::server::core::{VoiceParticipant, VoiceParticipantsByChannel};
use crate::server::gateway_events::{self, GatewayEvent, VoiceParticipantSnapshot};
use crate::server::metrics::{
    record_gateway_event_dropped, record_gateway_event_emitted, record_voice_sync_repair,
};

pub(crate) enum OutboundEnqueueResult {
    Enqueued,
    Closed,
    Full,
    Oversized,
}

pub(crate) enum VoiceSyncDispatchOutcome {
    EmittedAndRepaired,
    DroppedClosed,
    DroppedFull,
    DroppedOversized,
}

pub(crate) fn voice_channel_key(guild_id: &str, channel_id: &str) -> String {
    format!("{guild_id}:{channel_id}")
}

pub(crate) fn voice_snapshot_from_record(
    participant: &VoiceParticipant,
) -> VoiceParticipantSnapshot {
    gateway_events::VoiceParticipantSnapshot {
        user_id: participant.user_id,
        identity: participant.identity.clone(),
        joined_at_unix: participant.joined_at_unix,
        updated_at_unix: participant.updated_at_unix,
        is_muted: participant.is_muted,
        is_deafened: participant.is_deafened,
        is_speaking: participant.is_speaking,
        is_video_enabled: participant.is_video_enabled,
        is_screen_share_enabled: participant.is_screen_share_enabled,
    }
}

pub(crate) fn collect_voice_snapshots(
    voice: &VoiceParticipantsByChannel,
    channel_key: &str,
) -> Vec<VoiceParticipantSnapshot> {
    let mut snapshots = Vec::new();
    if let Some(channel_participants) = voice.get(channel_key) {
        snapshots.extend(
            channel_participants
                .values()
                .map(voice_snapshot_from_record),
        );
    }
    snapshots.sort_by(|a, b| {
        a.joined_at_unix
            .cmp(&b.joined_at_unix)
            .then(a.identity.cmp(&b.identity))
    });
    snapshots
}

pub(crate) fn try_enqueue_voice_sync_event(
    outbound_tx: &mpsc::Sender<String>,
    payload: String,
    max_gateway_event_bytes: usize,
) -> OutboundEnqueueResult {
    if payload.len() > max_gateway_event_bytes {
        return OutboundEnqueueResult::Oversized;
    }
    match outbound_tx.try_send(payload) {
        Ok(()) => OutboundEnqueueResult::Enqueued,
        Err(mpsc::error::TrySendError::Closed(_)) => OutboundEnqueueResult::Closed,
        Err(mpsc::error::TrySendError::Full(_)) => OutboundEnqueueResult::Full,
    }
}

pub(crate) fn voice_sync_dispatch_outcome(
    result: &OutboundEnqueueResult,
) -> VoiceSyncDispatchOutcome {
    match result {
        OutboundEnqueueResult::Enqueued => VoiceSyncDispatchOutcome::EmittedAndRepaired,
        OutboundEnqueueResult::Closed => VoiceSyncDispatchOutcome::DroppedClosed,
        OutboundEnqueueResult::Full => VoiceSyncDispatchOutcome::DroppedFull,
        OutboundEnqueueResult::Oversized => VoiceSyncDispatchOutcome::DroppedOversized,
    }
}

pub(crate) fn try_build_voice_subscribe_sync_event(
    guild_id: &str,
    channel_id: &str,
    participants: Vec<VoiceParticipantSnapshot>,
    now_unix: i64,
) -> anyhow::Result<GatewayEvent> {
    gateway_events::try_voice_participant_sync(guild_id, channel_id, participants, now_unix)
}

pub(crate) fn dispatch_voice_sync_event(
    outbound_tx: &mpsc::Sender<String>,
    event: GatewayEvent,
    max_gateway_event_bytes: usize,
) -> VoiceSyncDispatchOutcome {
    let enqueue_result =
        try_enqueue_voice_sync_event(outbound_tx, event.payload, max_gateway_event_bytes);
    let outcome = voice_sync_dispatch_outcome(&enqueue_result);
    match outcome {
        VoiceSyncDispatchOutcome::EmittedAndRepaired => {
            record_gateway_event_emitted("connection", event.event_type);
            record_voice_sync_repair("subscribe");
        }
        VoiceSyncDispatchOutcome::DroppedClosed => {
            record_gateway_event_dropped("connection", event.event_type, "closed");
        }
        VoiceSyncDispatchOutcome::DroppedFull => {
            record_gateway_event_dropped("connection", event.event_type, "full_queue");
        }
        VoiceSyncDispatchOutcome::DroppedOversized => {
            record_gateway_event_dropped("connection", event.event_type, "oversized_outbound");
        }
    }
    outcome
}

pub(crate) fn voice_sync_reject_reason(outcome: &VoiceSyncDispatchOutcome) -> Option<&'static str> {
    match outcome {
        VoiceSyncDispatchOutcome::EmittedAndRepaired => None,
        VoiceSyncDispatchOutcome::DroppedClosed => Some("closed"),
        VoiceSyncDispatchOutcome::DroppedFull => Some("full_queue"),
        VoiceSyncDispatchOutcome::DroppedOversized => Some("oversized_outbound"),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use filament_core::UserId;

    use super::{
        collect_voice_snapshots, dispatch_voice_sync_event, try_enqueue_voice_sync_event,
        voice_channel_key, voice_snapshot_from_record, voice_sync_dispatch_outcome,
        voice_sync_reject_reason, OutboundEnqueueResult, VoiceSyncDispatchOutcome,
    };
    use crate::server::core::{VoiceParticipant, VoiceParticipantsByChannel, VoiceStreamKind};
    use crate::server::gateway_events::{self, VoiceParticipantSnapshot};
    use crate::server::metrics::metrics_state;

    #[test]
    fn voice_channel_key_uses_guild_and_channel_namespace() {
        assert_eq!(voice_channel_key("g-main", "c-lobby"), "g-main:c-lobby");
    }

    #[test]
    fn voice_snapshot_preserves_voice_state_fields() {
        let user_id = UserId::new();
        let participant = VoiceParticipant {
            user_id,
            identity: String::from("alice"),
            joined_at_unix: 10,
            updated_at_unix: 42,
            expires_at_unix: 99,
            is_muted: true,
            is_deafened: false,
            is_speaking: true,
            is_video_enabled: true,
            is_screen_share_enabled: false,
            published_streams: HashSet::from([VoiceStreamKind::Camera]),
        };

        let snapshot = voice_snapshot_from_record(&participant);
        assert_eq!(snapshot.user_id, user_id);
        assert_eq!(snapshot.identity, "alice");
        assert_eq!(snapshot.joined_at_unix, 10);
        assert_eq!(snapshot.updated_at_unix, 42);
        assert!(snapshot.is_muted);
        assert!(!snapshot.is_deafened);
        assert!(snapshot.is_speaking);
        assert!(snapshot.is_video_enabled);
        assert!(!snapshot.is_screen_share_enabled);
    }

    #[test]
    fn collect_voice_snapshots_returns_sorted_channel_snapshot() {
        let first_user = UserId::new();
        let second_user = UserId::new();
        let mut voice: VoiceParticipantsByChannel = HashMap::new();
        voice.insert(
            String::from("g-main:c-lobby"),
            HashMap::from([
                (
                    first_user,
                    VoiceParticipant {
                        user_id: first_user,
                        identity: String::from("zeta"),
                        joined_at_unix: 20,
                        updated_at_unix: 20,
                        expires_at_unix: 50,
                        is_muted: false,
                        is_deafened: false,
                        is_speaking: false,
                        is_video_enabled: false,
                        is_screen_share_enabled: false,
                        published_streams: HashSet::from([VoiceStreamKind::Microphone]),
                    },
                ),
                (
                    second_user,
                    VoiceParticipant {
                        user_id: second_user,
                        identity: String::from("alpha"),
                        joined_at_unix: 10,
                        updated_at_unix: 10,
                        expires_at_unix: 50,
                        is_muted: false,
                        is_deafened: false,
                        is_speaking: false,
                        is_video_enabled: false,
                        is_screen_share_enabled: false,
                        published_streams: HashSet::from([VoiceStreamKind::Camera]),
                    },
                ),
            ]),
        );

        let snapshots = collect_voice_snapshots(&voice, "g-main:c-lobby");

        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].user_id, second_user);
        assert_eq!(snapshots[1].user_id, first_user);
    }

    #[test]
    fn collect_voice_snapshots_returns_empty_for_missing_channel() {
        let voice: VoiceParticipantsByChannel = HashMap::new();
        let snapshots = collect_voice_snapshots(&voice, "g-main:c-missing");
        assert!(snapshots.is_empty());
    }

    #[test]
    fn builds_voice_sync_event_with_expected_payload_fields() {
        let participant = VoiceParticipantSnapshot {
            user_id: UserId::new(),
            identity: String::from("alice"),
            joined_at_unix: 10,
            updated_at_unix: 20,
            is_muted: false,
            is_deafened: false,
            is_speaking: true,
            is_video_enabled: false,
            is_screen_share_enabled: false,
        };

        let event = super::try_build_voice_subscribe_sync_event("g1", "c1", vec![participant], 123)
            .expect("voice sync event should serialize");

        assert_eq!(event.event_type, "voice_participant_sync");
        assert!(event.payload.contains("\"guild_id\":\"g1\""));
        assert!(event.payload.contains("\"channel_id\":\"c1\""));
        assert!(event.payload.contains("\"synced_at_unix\":123"));
        assert!(event.payload.contains("\"identity\":\"alice\""));
    }

    #[test]
    fn supports_empty_participant_snapshot() {
        let event = super::try_build_voice_subscribe_sync_event("g1", "c1", Vec::new(), 456)
            .expect("voice sync event should serialize");

        assert_eq!(event.event_type, "voice_participant_sync");
        assert!(event.payload.contains("\"participants\":[]"));
    }

    #[test]
    fn dispatch_voice_sync_event_returns_emitted_for_open_queue() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(1);
        let event = gateway_events::try_voice_participant_sync("g-1", "c-1", Vec::new(), 10)
            .expect("voice_participant_sync event should serialize");
        let expected_payload = event.payload.clone();

        let outcome = dispatch_voice_sync_event(&tx, event, 1024);

        assert!(matches!(
            outcome,
            VoiceSyncDispatchOutcome::EmittedAndRepaired
        ));
        assert_eq!(
            rx.try_recv().ok().as_deref(),
            Some(expected_payload.as_str())
        );
    }

    #[test]
    fn enqueue_voice_sync_event_reports_enqueued() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(1);

        let result = try_enqueue_voice_sync_event(&tx, String::from("payload"), 1024);

        assert!(matches!(result, OutboundEnqueueResult::Enqueued));
        let received = rx.try_recv().expect("payload should be queued");
        assert_eq!(received, "payload");
    }

    #[test]
    fn enqueue_voice_sync_event_reports_full() {
        let (tx, _rx) = tokio::sync::mpsc::channel::<String>(1);
        assert!(matches!(
            try_enqueue_voice_sync_event(&tx, String::from("first"), 1024),
            OutboundEnqueueResult::Enqueued
        ));

        let result = try_enqueue_voice_sync_event(&tx, String::from("second"), 1024);

        assert!(matches!(result, OutboundEnqueueResult::Full));
    }

    #[test]
    fn enqueue_voice_sync_event_reports_closed() {
        let (tx, rx) = tokio::sync::mpsc::channel::<String>(1);
        drop(rx);

        let result = try_enqueue_voice_sync_event(&tx, String::from("payload"), 1024);

        assert!(matches!(result, OutboundEnqueueResult::Closed));
    }

    #[test]
    fn enqueue_voice_sync_event_reports_oversized() {
        let (tx, _rx) = tokio::sync::mpsc::channel::<String>(1);

        let result = try_enqueue_voice_sync_event(&tx, String::from("payload"), 3);

        assert!(matches!(result, OutboundEnqueueResult::Oversized));
    }

    #[test]
    fn dispatch_voice_sync_event_returns_full_for_full_queue() {
        let (tx, _rx) = tokio::sync::mpsc::channel::<String>(1);
        tx.try_send(String::from("occupied"))
            .expect("queue should be full");
        let event = gateway_events::try_voice_participant_sync("g-1", "c-1", Vec::new(), 10)
            .expect("voice_participant_sync event should serialize");

        let outcome = dispatch_voice_sync_event(&tx, event, 1024);

        assert!(matches!(outcome, VoiceSyncDispatchOutcome::DroppedFull));
    }

    #[test]
    fn dispatch_voice_sync_event_returns_closed_for_closed_queue() {
        let (tx, rx) = tokio::sync::mpsc::channel::<String>(1);
        drop(rx);
        let event = gateway_events::try_voice_participant_sync("g-1", "c-1", Vec::new(), 10)
            .expect("voice_participant_sync event should serialize");

        let outcome = dispatch_voice_sync_event(&tx, event, 1024);

        assert!(matches!(outcome, VoiceSyncDispatchOutcome::DroppedClosed));
    }

    #[test]
    fn dispatch_voice_sync_event_returns_oversized_for_large_payload() {
        let (tx, _rx) = tokio::sync::mpsc::channel::<String>(1);
        let event = gateway_events::try_voice_participant_sync("g-1", "c-1", Vec::new(), 10)
            .expect("voice_participant_sync event should serialize");

        let outcome = dispatch_voice_sync_event(&tx, event, 3);

        assert!(matches!(
            outcome,
            VoiceSyncDispatchOutcome::DroppedOversized
        ));
    }

    #[test]
    fn oversized_voice_sync_rejection_is_counted_as_drop() {
        let before = metrics_state()
            .gateway_events_dropped
            .lock()
            .expect("gateway dropped metrics mutex should not be poisoned")
            .get(&(
                String::from("connection"),
                String::from(gateway_events::VOICE_PARTICIPANT_SYNC_EVENT),
                String::from("oversized_outbound"),
            ))
            .copied()
            .unwrap_or(0);
        let (tx, _rx) = tokio::sync::mpsc::channel::<String>(1);
        let event = gateway_events::try_voice_participant_sync("g-1", "c-1", Vec::new(), 10)
            .expect("voice_participant_sync event should serialize");

        let outcome = dispatch_voice_sync_event(&tx, event, 3);

        assert!(matches!(
            outcome,
            VoiceSyncDispatchOutcome::DroppedOversized
        ));
        let after = metrics_state()
            .gateway_events_dropped
            .lock()
            .expect("gateway dropped metrics mutex should not be poisoned")
            .get(&(
                String::from("connection"),
                String::from(gateway_events::VOICE_PARTICIPANT_SYNC_EVENT),
                String::from("oversized_outbound"),
            ))
            .copied()
            .unwrap_or(0);
        assert!(after > before);
    }

    #[test]
    fn voice_sync_reject_reason_maps_dispatch_outcomes() {
        assert_eq!(
            voice_sync_reject_reason(&VoiceSyncDispatchOutcome::EmittedAndRepaired),
            None
        );
        assert_eq!(
            voice_sync_reject_reason(&VoiceSyncDispatchOutcome::DroppedClosed),
            Some("closed")
        );
        assert_eq!(
            voice_sync_reject_reason(&VoiceSyncDispatchOutcome::DroppedFull),
            Some("full_queue")
        );
        assert_eq!(
            voice_sync_reject_reason(&VoiceSyncDispatchOutcome::DroppedOversized),
            Some("oversized_outbound")
        );
    }

    #[test]
    fn voice_sync_dispatch_outcome_maps_all_enqueue_results() {
        assert!(matches!(
            voice_sync_dispatch_outcome(&OutboundEnqueueResult::Enqueued),
            VoiceSyncDispatchOutcome::EmittedAndRepaired
        ));
        assert!(matches!(
            voice_sync_dispatch_outcome(&OutboundEnqueueResult::Closed),
            VoiceSyncDispatchOutcome::DroppedClosed
        ));
        assert!(matches!(
            voice_sync_dispatch_outcome(&OutboundEnqueueResult::Full),
            VoiceSyncDispatchOutcome::DroppedFull
        ));
        assert!(matches!(
            voice_sync_dispatch_outcome(&OutboundEnqueueResult::Oversized),
            VoiceSyncDispatchOutcome::DroppedOversized
        ));
    }
}
