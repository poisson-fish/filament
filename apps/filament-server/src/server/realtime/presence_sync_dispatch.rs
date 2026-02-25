use tokio::sync::mpsc;

use crate::server::gateway_events::GatewayEvent;
use crate::server::metrics::{record_gateway_event_dropped, record_gateway_event_emitted};

use super::presence_subscribe::{
    presence_sync_dispatch_outcome, try_enqueue_presence_sync_event, PresenceSyncDispatchOutcome,
};

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
    use std::collections::HashSet;

    use super::{dispatch_presence_sync_event, presence_sync_reject_reason};
    use crate::server::gateway_events;
    use crate::server::metrics::metrics_state;
    use crate::server::realtime::presence_subscribe::PresenceSyncDispatchOutcome;

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
}
