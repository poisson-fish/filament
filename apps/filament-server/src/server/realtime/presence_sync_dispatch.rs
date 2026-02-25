use tokio::sync::mpsc;

use crate::server::gateway_events::GatewayEvent;
use crate::server::metrics::{record_gateway_event_dropped, record_gateway_event_emitted};

use super::presence_subscribe::{
    presence_sync_dispatch_outcome, try_enqueue_presence_sync_event, PresenceSyncDispatchOutcome,
};

pub(crate) fn dispatch_presence_sync_event(
    outbound_tx: &mpsc::Sender<String>,
    event: GatewayEvent,
) -> PresenceSyncDispatchOutcome {
    let enqueue_result = try_enqueue_presence_sync_event(outbound_tx, event.payload);
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
    }
    outcome
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::dispatch_presence_sync_event;
    use crate::server::gateway_events;
    use crate::server::realtime::presence_subscribe::PresenceSyncDispatchOutcome;

    #[test]
    fn dispatch_presence_sync_event_returns_emitted_for_open_queue() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(1);
        let event = gateway_events::try_presence_sync("g-1", HashSet::new())
            .expect("presence_sync event should serialize");
        let expected_payload = event.payload.clone();

        let outcome = dispatch_presence_sync_event(&tx, event);

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

        let outcome = dispatch_presence_sync_event(&tx, event);

        assert!(matches!(outcome, PresenceSyncDispatchOutcome::DroppedFull));
    }

    #[test]
    fn dispatch_presence_sync_event_returns_closed_for_closed_queue() {
        let (tx, rx) = tokio::sync::mpsc::channel::<String>(1);
        drop(rx);
        let event = gateway_events::try_presence_sync("g-1", HashSet::new())
            .expect("presence_sync event should serialize");

        let outcome = dispatch_presence_sync_event(&tx, event);

        assert!(matches!(
            outcome,
            PresenceSyncDispatchOutcome::DroppedClosed
        ));
    }
}
