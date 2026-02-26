use std::collections::HashMap;

use tokio::sync::mpsc;
use tracing::warn;
use uuid::Uuid;

use crate::server::core::Subscriptions;
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
        warn!(
            event = "gateway.fanout_dispatch.oversized_outbound",
            scope,
            event_type,
            payload_bytes = payload.len(),
            max_payload_bytes,
            "dropped outbound payload because it exceeds configured max size"
        );
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
                warn!(
                    event = "gateway.fanout_dispatch.closed",
                    scope,
                    event_type,
                    connection_id = %connection_id,
                    "dropped outbound payload for closed websocket queue"
                );
                false
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                record_gateway_event_dropped(scope, event_type, "full_queue");
                warn!(
                    event = "gateway.fanout_dispatch.full_queue",
                    scope,
                    event_type,
                    connection_id = %connection_id,
                    "dropped outbound payload for full websocket queue"
                );
                slow_connections.push(*connection_id);
                false
            }
        },
    );
    delivered
}

pub(crate) fn dispatch_channel_payload(
    subscriptions: &mut Subscriptions,
    key: &str,
    payload: &str,
    max_payload_bytes: usize,
    event_type: &'static str,
    slow_connections: &mut Vec<Uuid>,
) -> usize {
    let mut delivered = 0usize;
    if let Some(listeners) = subscriptions.get_mut(key) {
        delivered = dispatch_gateway_payload(
            listeners,
            payload,
            max_payload_bytes,
            event_type,
            "channel",
            slow_connections,
        );
        if listeners.is_empty() {
            subscriptions.remove(key);
        }
    }
    delivered
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use tokio::sync::mpsc;
    use uuid::Uuid;

    use crate::server::metrics::{metrics_state, GATEWAY_DROP_REASON_OVERSIZED_OUTBOUND};

    use super::{dispatch_channel_payload, dispatch_gateway_payload};

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

    #[tokio::test]
    async fn records_closed_and_full_queue_drop_reasons() {
        if let Ok(mut counters) = metrics_state().gateway_events_dropped.lock() {
            counters.clear();
        }

        let full_id = Uuid::new_v4();
        let closed_id = Uuid::new_v4();
        let event_type = "message_create_reason_test";
        let scope = "channel_reason_test";

        let (full_sender, mut full_receiver) = mpsc::channel::<String>(1);
        full_sender
            .try_send(String::from("occupied"))
            .expect("queue should accept first message");
        let (closed_sender, closed_receiver) = mpsc::channel::<String>(1);
        drop(closed_receiver);

        let mut listeners = HashMap::from([(full_id, full_sender), (closed_id, closed_sender)]);
        let mut slow_connections = Vec::new();

        let delivered = dispatch_gateway_payload(
            &mut listeners,
            "payload",
            "payload".len(),
            event_type,
            scope,
            &mut slow_connections,
        );

        assert_eq!(delivered, 0);
        assert_eq!(slow_connections, vec![full_id]);
        assert_eq!(full_receiver.recv().await.as_deref(), Some("occupied"));
        assert!(listeners.is_empty());

        let dropped = metrics_state()
            .gateway_events_dropped
            .lock()
            .expect("gateway dropped metrics mutex should not be poisoned");
        let closed_key = (
            String::from(scope),
            String::from(event_type),
            String::from("closed"),
        );
        let full_key = (
            String::from(scope),
            String::from(event_type),
            String::from("full_queue"),
        );
        assert_eq!(dropped.get(&closed_key).copied(), Some(1));
        assert_eq!(dropped.get(&full_key).copied(), Some(1));
    }

    #[tokio::test]
    async fn dispatch_channel_payload_delivers_and_prunes_empty_key() {
        let keep_id = Uuid::new_v4();
        let (keep_sender, mut keep_receiver) = mpsc::channel::<String>(1);

        let mut subscriptions = HashMap::from([(
            String::from("g1:c1"),
            HashMap::from([(keep_id, keep_sender)]),
        )]);
        let mut slow_connections = Vec::new();

        let delivered = dispatch_channel_payload(
            &mut subscriptions,
            "g1:c1",
            "payload",
            "payload".len(),
            "message_create",
            &mut slow_connections,
        );

        assert_eq!(delivered, 1);
        assert!(slow_connections.is_empty());
        assert_eq!(keep_receiver.recv().await.as_deref(), Some("payload"));

        let listeners = subscriptions
            .get("g1:c1")
            .expect("channel key should remain when listener is active");
        assert!(listeners.contains_key(&keep_id));
    }

    #[tokio::test]
    async fn dispatch_channel_payload_removes_closed_and_full_listeners() {
        let full_id = Uuid::new_v4();
        let closed_id = Uuid::new_v4();

        let (full_sender, mut full_receiver) = mpsc::channel::<String>(1);
        full_sender
            .try_send(String::from("occupied"))
            .expect("queue should fill");

        let (closed_sender, closed_receiver) = mpsc::channel::<String>(1);
        drop(closed_receiver);

        let mut subscriptions = HashMap::from([(
            String::from("g1:c1"),
            HashMap::from([(full_id, full_sender), (closed_id, closed_sender)]),
        )]);
        let mut slow_connections = Vec::new();

        let delivered = dispatch_channel_payload(
            &mut subscriptions,
            "g1:c1",
            "payload",
            "payload".len(),
            "message_create",
            &mut slow_connections,
        );

        assert_eq!(delivered, 0);
        assert_eq!(slow_connections, vec![full_id]);
        assert_eq!(full_receiver.recv().await.as_deref(), Some("occupied"));
        assert!(!subscriptions.contains_key("g1:c1"));
    }
}
