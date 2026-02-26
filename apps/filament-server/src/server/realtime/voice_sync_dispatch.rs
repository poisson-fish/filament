use tokio::sync::mpsc;

use crate::server::gateway_events::{self, GatewayEvent, VoiceParticipantSnapshot};
use crate::server::metrics::{
    record_gateway_event_dropped, record_gateway_event_emitted, record_voice_sync_repair,
};

use super::voice_presence::{
    try_enqueue_voice_sync_event, voice_sync_dispatch_outcome, VoiceSyncDispatchOutcome,
};

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
    use filament_core::UserId;

    use super::{dispatch_voice_sync_event, voice_sync_reject_reason};
    use crate::server::gateway_events::{self, VoiceParticipantSnapshot};
    use crate::server::metrics::metrics_state;
    use crate::server::realtime::voice_presence::VoiceSyncDispatchOutcome;

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
}
