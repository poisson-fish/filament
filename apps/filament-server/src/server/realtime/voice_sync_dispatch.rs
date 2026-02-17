use tokio::sync::mpsc;

use crate::server::gateway_events::GatewayEvent;
use crate::server::metrics::{
    record_gateway_event_dropped, record_gateway_event_emitted, record_voice_sync_repair,
};

use super::voice_presence::{
    try_enqueue_voice_sync_event, voice_sync_dispatch_outcome, VoiceSyncDispatchOutcome,
};

pub(crate) fn dispatch_voice_sync_event(
    outbound_tx: &mpsc::Sender<String>,
    event: GatewayEvent,
) -> VoiceSyncDispatchOutcome {
    let enqueue_result = try_enqueue_voice_sync_event(outbound_tx, event.payload);
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
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::dispatch_voice_sync_event;
    use crate::server::gateway_events;
    use crate::server::realtime::voice_presence::VoiceSyncDispatchOutcome;

    #[test]
    fn dispatch_voice_sync_event_returns_emitted_for_open_queue() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(1);
        let event = gateway_events::voice_participant_sync("g-1", "c-1", Vec::new(), 10);
        let expected_payload = event.payload.clone();

        let outcome = dispatch_voice_sync_event(&tx, event);

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
        let event = gateway_events::voice_participant_sync("g-1", "c-1", Vec::new(), 10);

        let outcome = dispatch_voice_sync_event(&tx, event);

        assert!(matches!(outcome, VoiceSyncDispatchOutcome::DroppedFull));
    }

    #[test]
    fn dispatch_voice_sync_event_returns_closed_for_closed_queue() {
        let (tx, rx) = tokio::sync::mpsc::channel::<String>(1);
        drop(rx);
        let event = gateway_events::voice_participant_sync("g-1", "c-1", Vec::new(), 10);

        let outcome = dispatch_voice_sync_event(&tx, event);

        assert!(matches!(outcome, VoiceSyncDispatchOutcome::DroppedClosed));
    }
}
