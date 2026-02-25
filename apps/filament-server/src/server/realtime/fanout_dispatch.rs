use std::collections::HashMap;

use tokio::sync::mpsc;
use uuid::Uuid;

use crate::server::metrics::{
    record_gateway_event_dropped, record_gateway_event_oversized_outbound,
};

pub(crate) fn dispatch_gateway_payload(
    listeners: &mut HashMap<Uuid, mpsc::Sender<String>>,
    payload: &str,
    max_payload_bytes: usize,
    event_type: &'static str,
    scope: &'static str,
    slow_connections: &mut Vec<Uuid>,
) -> usize {
    if payload.len() > max_payload_bytes {
        record_gateway_event_oversized_outbound(scope, event_type);
        return 0;
    }

    let mut delivered = 0usize;
    listeners.retain(
        |connection_id, sender| match sender.try_send(payload.to_owned()) {
            Ok(()) => {
                delivered += 1;
                true
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                record_gateway_event_dropped(scope, event_type, "closed");
                false
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                record_gateway_event_dropped(scope, event_type, "full_queue");
                slow_connections.push(*connection_id);
                false
            }
        },
    );
    delivered
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use tokio::sync::mpsc;
    use uuid::Uuid;

    use crate::server::metrics::{metrics_state, GATEWAY_DROP_REASON_OVERSIZED_OUTBOUND};

    use super::dispatch_gateway_payload;

    #[tokio::test]
    async fn delivers_to_open_listeners_and_keeps_them_registered() {
        let connection_id = Uuid::new_v4();
        let (sender, mut receiver) = mpsc::channel::<String>(1);
        let mut listeners = HashMap::new();
        listeners.insert(connection_id, sender);
        let mut slow_connections = Vec::new();

        let delivered = dispatch_gateway_payload(
            &mut listeners,
            "payload",
            "payload".len(),
            "message_create",
            "channel",
            &mut slow_connections,
        );

        assert_eq!(delivered, 1);
        assert!(slow_connections.is_empty());
        assert!(listeners.contains_key(&connection_id));
        assert_eq!(receiver.recv().await.as_deref(), Some("payload"));
    }

    #[tokio::test]
    async fn removes_closed_or_full_listeners_and_marks_slow_connections() {
        let keep_id = Uuid::new_v4();
        let full_id = Uuid::new_v4();
        let closed_id = Uuid::new_v4();

        let (keep_sender, _keep_receiver) = mpsc::channel::<String>(2);
        let (full_sender, mut full_receiver) = mpsc::channel::<String>(1);
        full_sender
            .try_send(String::from("occupied"))
            .expect("queue should accept first message");
        let (closed_sender, closed_receiver) = mpsc::channel::<String>(1);
        drop(closed_receiver);

        let mut listeners = HashMap::new();
        listeners.insert(keep_id, keep_sender);
        listeners.insert(full_id, full_sender);
        listeners.insert(closed_id, closed_sender);
        let mut slow_connections = Vec::new();

        let delivered = dispatch_gateway_payload(
            &mut listeners,
            "payload",
            "payload".len(),
            "message_create",
            "channel",
            &mut slow_connections,
        );

        assert_eq!(delivered, 1);
        assert_eq!(slow_connections, vec![full_id]);
        assert!(listeners.contains_key(&keep_id));
        assert!(!listeners.contains_key(&full_id));
        assert!(!listeners.contains_key(&closed_id));

        let drained = full_receiver
            .recv()
            .await
            .expect("full queue should still hold occupied message");
        assert_eq!(drained, "occupied");
    }

    #[tokio::test]
    async fn rejects_oversized_outbound_payload_before_enqueue() {
        if let Ok(mut counters) = metrics_state().gateway_events_dropped.lock() {
            counters.clear();
        }

        let connection_id = Uuid::new_v4();
        let (sender, mut receiver) = mpsc::channel::<String>(1);
        let mut listeners = HashMap::from([(connection_id, sender)]);
        let mut slow_connections = Vec::new();
        let payload = "payload";

        let delivered = dispatch_gateway_payload(
            &mut listeners,
            payload,
            payload.len() - 1,
            "message_create",
            "channel",
            &mut slow_connections,
        );

        assert_eq!(delivered, 0);
        assert!(slow_connections.is_empty());
        assert!(listeners.contains_key(&connection_id));
        assert!(receiver.try_recv().is_err());

        let dropped = metrics_state()
            .gateway_events_dropped
            .lock()
            .expect("gateway dropped metrics mutex should not be poisoned");
        let key = (
            String::from("channel"),
            String::from("message_create"),
            String::from(GATEWAY_DROP_REASON_OVERSIZED_OUTBOUND),
        );
        assert_eq!(dropped.get(&key).copied(), Some(1));
    }
}
