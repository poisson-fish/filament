use std::collections::HashMap;

use tokio::sync::mpsc;
use tracing::warn;
use uuid::Uuid;

use crate::server::metrics::{
    record_gateway_event_dropped, record_gateway_event_oversized_outbound,
};

pub(crate) fn dispatch_user_payload(
    senders: &mut HashMap<Uuid, mpsc::Sender<String>>,
    connection_ids: &[Uuid],
    payload: &str,
    max_payload_bytes: usize,
    event_type: &'static str,
    slow_connections: &mut Vec<Uuid>,
) -> usize {
    if payload.len() > max_payload_bytes {
        record_gateway_event_oversized_outbound("user", event_type);
        warn!(
            event = "gateway.user_fanout.oversized_outbound",
            event_type,
            payload_bytes = payload.len(),
            max_payload_bytes,
            "dropped outbound payload for user fanout because it exceeds configured max size"
        );
        return 0;
    }

    let mut delivered = 0usize;

    for connection_id in connection_ids {
        let Some(sender) = senders.get(connection_id) else {
            continue;
        };
        match sender.try_send(payload.to_owned()) {
            Ok(()) => delivered += 1,
            Err(mpsc::error::TrySendError::Closed(_)) => {
                record_gateway_event_dropped("user", event_type, "closed");
                warn!(
                    event = "gateway.user_fanout.closed",
                    event_type,
                    connection_id = %connection_id,
                    "dropped outbound payload for closed websocket queue in user fanout"
                );
                senders.remove(connection_id);
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                record_gateway_event_dropped("user", event_type, "full_queue");
                warn!(
                    event = "gateway.user_fanout.full_queue",
                    event_type,
                    connection_id = %connection_id,
                    "dropped outbound payload for full websocket queue in user fanout"
                );
                slow_connections.push(*connection_id);
                senders.remove(connection_id);
            }
        }
    }

    delivered
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use tokio::sync::mpsc;
    use uuid::Uuid;

    use crate::server::metrics::metrics_state;

    use super::dispatch_user_payload;

    #[tokio::test]
    async fn delivers_to_open_user_connections() {
        let first_id = Uuid::new_v4();
        let second_id = Uuid::new_v4();
        let missing_id = Uuid::new_v4();

        let (first_sender, mut first_receiver) = mpsc::channel::<String>(1);
        let (second_sender, mut second_receiver) = mpsc::channel::<String>(1);

        let mut senders = HashMap::from([(first_id, first_sender), (second_id, second_sender)]);
        let mut slow_connections = Vec::new();
        let delivered = dispatch_user_payload(
            &mut senders,
            &[first_id, missing_id, second_id],
            "payload",
            "payload".len(),
            "presence_update",
            &mut slow_connections,
        );

        assert_eq!(delivered, 2);
        assert!(slow_connections.is_empty());
        assert_eq!(first_receiver.recv().await.as_deref(), Some("payload"));
        assert_eq!(second_receiver.recv().await.as_deref(), Some("payload"));
    }

    #[tokio::test]
    async fn removes_closed_and_full_user_connections() {
        let keep_id = Uuid::new_v4();
        let full_id = Uuid::new_v4();
        let closed_id = Uuid::new_v4();

        let (keep_sender, _keep_receiver) = mpsc::channel::<String>(2);
        let (full_sender, mut full_receiver) = mpsc::channel::<String>(1);
        full_sender
            .try_send(String::from("occupied"))
            .expect("queue should fill");
        let (closed_sender, closed_receiver) = mpsc::channel::<String>(1);
        drop(closed_receiver);

        let mut senders = HashMap::from([
            (keep_id, keep_sender),
            (full_id, full_sender),
            (closed_id, closed_sender),
        ]);
        let mut slow_connections = Vec::new();

        let delivered = dispatch_user_payload(
            &mut senders,
            &[keep_id, full_id, closed_id],
            "payload",
            "payload".len(),
            "presence_update",
            &mut slow_connections,
        );

        assert_eq!(delivered, 1);
        assert_eq!(slow_connections, vec![full_id]);
        assert!(senders.contains_key(&keep_id));
        assert!(!senders.contains_key(&full_id));
        assert!(!senders.contains_key(&closed_id));
        assert_eq!(full_receiver.recv().await.as_deref(), Some("occupied"));
    }

    #[tokio::test]
    async fn rejects_oversized_outbound_payload_before_enqueue() {
        let connection_id = Uuid::new_v4();
        let (sender, mut receiver) = mpsc::channel::<String>(1);
        let mut senders = HashMap::from([(connection_id, sender)]);
        let mut slow_connections = Vec::new();
        let payload = "payload";
        let event_type = "friend_request_update_oversized_reason_test";
        let key = (
            String::from("user"),
            String::from(event_type),
            String::from("oversized_outbound"),
        );
        let before = metrics_state()
            .gateway_events_dropped
            .lock()
            .ok()
            .and_then(|dropped| dropped.get(&key).copied())
            .unwrap_or(0);

        let delivered = dispatch_user_payload(
            &mut senders,
            &[connection_id],
            payload,
            payload.len() - 1,
            event_type,
            &mut slow_connections,
        );

        assert_eq!(delivered, 0);
        assert!(slow_connections.is_empty());
        assert!(senders.contains_key(&connection_id));
        assert!(receiver.try_recv().is_err());

        let dropped = metrics_state()
            .gateway_events_dropped
            .lock()
            .expect("gateway dropped metrics mutex should not be poisoned");
        assert_eq!(dropped.get(&key).copied(), Some(before + 1));
    }

    #[tokio::test]
    async fn records_closed_and_full_queue_drop_reasons() {
        let full_id = Uuid::new_v4();
        let closed_id = Uuid::new_v4();
        let event_type = "friend_request_update_reason_test";
        let closed_key = (
            String::from("user"),
            String::from(event_type),
            String::from("closed"),
        );
        let full_key = (
            String::from("user"),
            String::from(event_type),
            String::from("full_queue"),
        );
        let (closed_before, full_before) = metrics_state()
            .gateway_events_dropped
            .lock()
            .ok()
            .map(|dropped| {
                (
                    dropped.get(&closed_key).copied().unwrap_or(0),
                    dropped.get(&full_key).copied().unwrap_or(0),
                )
            })
            .unwrap_or((0, 0));

        let (full_sender, mut full_receiver) = mpsc::channel::<String>(1);
        full_sender
            .try_send(String::from("occupied"))
            .expect("queue should fill");
        let (closed_sender, closed_receiver) = mpsc::channel::<String>(1);
        drop(closed_receiver);

        let mut senders = HashMap::from([(full_id, full_sender), (closed_id, closed_sender)]);
        let mut slow_connections = Vec::new();

        let delivered = dispatch_user_payload(
            &mut senders,
            &[full_id, closed_id],
            "payload",
            "payload".len(),
            event_type,
            &mut slow_connections,
        );

        assert_eq!(delivered, 0);
        assert_eq!(slow_connections, vec![full_id]);
        assert!(senders.is_empty());
        assert_eq!(full_receiver.recv().await.as_deref(), Some("occupied"));

        let dropped = metrics_state()
            .gateway_events_dropped
            .lock()
            .expect("gateway dropped metrics mutex should not be poisoned");
        assert_eq!(dropped.get(&closed_key).copied(), Some(closed_before + 1));
        assert_eq!(dropped.get(&full_key).copied(), Some(full_before + 1));
    }
}
