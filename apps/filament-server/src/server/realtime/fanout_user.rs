use std::collections::HashMap;

use tokio::sync::mpsc;
use uuid::Uuid;

use crate::server::metrics::record_gateway_event_dropped;

pub(crate) fn dispatch_user_payload(
    senders: &mut HashMap<Uuid, mpsc::Sender<String>>,
    connection_ids: &[Uuid],
    payload: &str,
    max_payload_bytes: usize,
    event_type: &'static str,
    slow_connections: &mut Vec<Uuid>,
) -> usize {
    if payload.len() > max_payload_bytes {
        record_gateway_event_dropped("user", event_type, "oversized_outbound");
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
                senders.remove(connection_id);
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                record_gateway_event_dropped("user", event_type, "full_queue");
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
        if let Ok(mut counters) = metrics_state().gateway_events_dropped.lock() {
            counters.clear();
        }

        let connection_id = Uuid::new_v4();
        let (sender, mut receiver) = mpsc::channel::<String>(1);
        let mut senders = HashMap::from([(connection_id, sender)]);
        let mut slow_connections = Vec::new();
        let payload = "payload";

        let delivered = dispatch_user_payload(
            &mut senders,
            &[connection_id],
            payload,
            payload.len() - 1,
            "friend_request_update",
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
        let key = (
            String::from("user"),
            String::from("friend_request_update"),
            String::from("oversized_outbound"),
        );
        assert_eq!(dropped.get(&key).copied(), Some(1));
    }
}
