use std::collections::HashMap;

use tokio::sync::mpsc;
use uuid::Uuid;

use crate::server::metrics::record_gateway_event_dropped;

pub(crate) fn dispatch_gateway_payload(
    listeners: &mut HashMap<Uuid, mpsc::Sender<String>>,
    payload: &str,
    event_type: &'static str,
    scope: &'static str,
    slow_connections: &mut Vec<Uuid>,
) -> usize {
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
}