use uuid::Uuid;

use crate::server::core::Subscriptions;

use super::fanout_dispatch::dispatch_gateway_payload;

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

    use super::dispatch_channel_payload;

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
