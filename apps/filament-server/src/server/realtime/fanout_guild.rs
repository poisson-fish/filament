use std::collections::{HashMap, HashSet};

use tokio::sync::mpsc;
use tracing::warn;
use uuid::Uuid;

use crate::server::metrics::{
    record_gateway_event_dropped, record_gateway_event_oversized_outbound,
};

pub(crate) fn dispatch_guild_payload(
    subscriptions: &mut HashMap<String, HashMap<Uuid, mpsc::Sender<String>>>,
    guild_id: &str,
    payload: &str,
    max_payload_bytes: usize,
    event_type: &'static str,
    slow_connections: &mut Vec<Uuid>,
) -> usize {
    if payload.len() > max_payload_bytes {
        record_gateway_event_oversized_outbound("guild", event_type);
        warn!(
            event = "gateway.guild_fanout.oversized_outbound",
            event_type,
            guild_id,
            payload_bytes = payload.len(),
            max_payload_bytes,
            "dropped outbound payload for guild fanout because it exceeds configured max size"
        );
        return 0;
    }

    let mut seen_connections = HashSet::new();
    let mut delivered = 0usize;

    for (key, listeners) in subscriptions.iter_mut() {
        if !key.starts_with(guild_id) || !key[guild_id.len()..].starts_with(':') {
            continue;
        }

        let mut stale_connections = Vec::new();
        for (connection_id, sender) in listeners.iter() {
            if !seen_connections.insert(*connection_id) {
                continue;
            }

            match sender.try_send(payload.to_owned()) {
                Ok(()) => delivered += 1,
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    record_gateway_event_dropped("guild", event_type, "closed");
                    warn!(
                        event = "gateway.guild_fanout.closed",
                        event_type,
                        guild_id,
                        connection_id = %connection_id,
                        "dropped outbound payload for closed websocket queue in guild fanout"
                    );
                    stale_connections.push(*connection_id);
                }
                Err(mpsc::error::TrySendError::Full(_)) => {
                    record_gateway_event_dropped("guild", event_type, "full_queue");
                    warn!(
                        event = "gateway.guild_fanout.full_queue",
                        event_type,
                        guild_id,
                        connection_id = %connection_id,
                        "dropped outbound payload for full websocket queue in guild fanout"
                    );
                    slow_connections.push(*connection_id);
                    stale_connections.push(*connection_id);
                }
            }
        }

        for connection_id in stale_connections {
            listeners.remove(&connection_id);
        }
    }

    subscriptions.retain(|_, listeners| !listeners.is_empty());
    delivered
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use tokio::sync::mpsc;
    use uuid::Uuid;

    use crate::server::metrics::metrics_state;

    use super::dispatch_guild_payload;

    #[tokio::test]
    async fn delivers_once_per_connection_across_guild_channels() {
        let guild_id = "g-1";
        let keep_id = Uuid::new_v4();
        let second_id = Uuid::new_v4();

        let (keep_sender, mut keep_receiver) = mpsc::channel::<String>(2);
        let (second_sender, mut second_receiver) = mpsc::channel::<String>(2);

        let mut subscriptions: HashMap<String, HashMap<Uuid, mpsc::Sender<String>>> =
            HashMap::new();
        subscriptions.insert(
            String::from("g-1:c-1"),
            HashMap::from([(keep_id, keep_sender.clone()), (second_id, second_sender)]),
        );
        subscriptions.insert(
            String::from("g-1:c-2"),
            HashMap::from([(keep_id, keep_sender)]),
        );
        subscriptions.insert(String::from("g-2:c-1"), HashMap::new());

        let mut slow_connections = Vec::new();
        let delivered = dispatch_guild_payload(
            &mut subscriptions,
            guild_id,
            "payload",
            "payload".len(),
            "presence_update",
            &mut slow_connections,
        );

        assert_eq!(delivered, 2);
        assert!(slow_connections.is_empty());
        assert_eq!(keep_receiver.recv().await.as_deref(), Some("payload"));
        assert_eq!(second_receiver.recv().await.as_deref(), Some("payload"));
        assert!(!subscriptions.contains_key("g-2:c-1"));
    }

    #[tokio::test]
    async fn prunes_closed_and_marks_full_connections() {
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

        let mut subscriptions: HashMap<String, HashMap<Uuid, mpsc::Sender<String>>> =
            HashMap::new();
        subscriptions.insert(
            String::from("g-1:c-1"),
            HashMap::from([
                (keep_id, keep_sender),
                (full_id, full_sender),
                (closed_id, closed_sender),
            ]),
        );

        let mut slow_connections = Vec::new();
        let delivered = dispatch_guild_payload(
            &mut subscriptions,
            "g-1",
            "payload",
            "payload".len(),
            "message_create",
            &mut slow_connections,
        );

        assert_eq!(delivered, 1);
        assert_eq!(slow_connections, vec![full_id]);

        let listeners = subscriptions
            .get("g-1:c-1")
            .expect("guild key should remain with keep listener");
        assert!(listeners.contains_key(&keep_id));
        assert!(!listeners.contains_key(&full_id));
        assert!(!listeners.contains_key(&closed_id));

        assert_eq!(full_receiver.recv().await.as_deref(), Some("occupied"));
    }

    #[tokio::test]
    async fn rejects_oversized_outbound_payload_before_scan() {
        let connection_id = Uuid::new_v4();
        let (sender, mut receiver) = mpsc::channel::<String>(1);
        let mut subscriptions: HashMap<String, HashMap<Uuid, mpsc::Sender<String>>> =
            HashMap::from([(
                String::from("g-1:c-1"),
                HashMap::from([(connection_id, sender)]),
            )]);
        let mut slow_connections = Vec::new();
        let payload = "payload";
        let event_type = "message_create_oversized_reason_test";
        let key = (
            String::from("guild"),
            String::from(event_type),
            String::from("oversized_outbound"),
        );
        let before = metrics_state()
            .gateway_events_dropped
            .lock()
            .ok()
            .and_then(|dropped| dropped.get(&key).copied())
            .unwrap_or(0);

        let delivered = dispatch_guild_payload(
            &mut subscriptions,
            "g-1",
            payload,
            payload.len() - 1,
            event_type,
            &mut slow_connections,
        );

        assert_eq!(delivered, 0);
        assert!(slow_connections.is_empty());
        assert!(subscriptions
            .get("g-1:c-1")
            .expect("guild key should remain after oversized rejection")
            .contains_key(&connection_id));
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
        let event_type = "presence_update_reason_test";
        let closed_key = (
            String::from("guild"),
            String::from(event_type),
            String::from("closed"),
        );
        let full_key = (
            String::from("guild"),
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

        let mut subscriptions: HashMap<String, HashMap<Uuid, mpsc::Sender<String>>> =
            HashMap::from([(
                String::from("g-1:c-1"),
                HashMap::from([(full_id, full_sender), (closed_id, closed_sender)]),
            )]);
        let mut slow_connections = Vec::new();

        let delivered = dispatch_guild_payload(
            &mut subscriptions,
            "g-1",
            "payload",
            "payload".len(),
            event_type,
            &mut slow_connections,
        );

        assert_eq!(delivered, 0);
        assert_eq!(slow_connections, vec![full_id]);
        assert!(subscriptions.is_empty());
        assert_eq!(full_receiver.recv().await.as_deref(), Some("occupied"));

        let dropped = metrics_state()
            .gateway_events_dropped
            .lock()
            .expect("gateway dropped metrics mutex should not be poisoned");
        assert_eq!(dropped.get(&closed_key).copied(), Some(closed_before + 1));
        assert_eq!(dropped.get(&full_key).copied(), Some(full_before + 1));
    }
}
