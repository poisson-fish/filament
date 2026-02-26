use std::collections::HashMap;

use filament_core::UserId;
use tokio::sync::mpsc;
use tracing::warn;
use uuid::Uuid;

use crate::server::core::{GuildConnectionIndex, Subscriptions, UserConnectionIndex};
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

pub(crate) fn dispatch_guild_payload(
    guild_connections: &mut GuildConnectionIndex,
    senders: &mut HashMap<Uuid, mpsc::Sender<String>>,
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

    let Some(connection_ids) = guild_connections.get_mut(guild_id) else {
        return 0;
    };

    let mut delivered = 0usize;
    let mut stale_connections = Vec::new();

    for connection_id in connection_ids.iter() {
        let Some(sender) = senders.get(connection_id) else {
            stale_connections.push(*connection_id);
            continue;
        };

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
        connection_ids.remove(&connection_id);
    }
    if connection_ids.is_empty() {
        guild_connections.remove(guild_id);
    }

    delivered
}

pub(crate) fn connection_ids_for_user(
    user_connections: &UserConnectionIndex,
    user_id: UserId,
) -> Vec<Uuid> {
    user_connections
        .get(&user_id)
        .into_iter()
        .flat_map(|connection_ids| connection_ids.iter().copied())
        .collect()
}

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
    use std::collections::{HashMap, HashSet};

    use filament_core::UserId;
    use tokio::sync::mpsc;
    use uuid::Uuid;

    use crate::server::metrics::{metrics_state, GATEWAY_DROP_REASON_OVERSIZED_OUTBOUND};

    use super::{
        connection_ids_for_user, dispatch_channel_payload, dispatch_gateway_payload,
        dispatch_guild_payload, dispatch_user_payload,
    };

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

    #[tokio::test]
    async fn dispatches_to_each_indexed_guild_connection_once() {
        let guild_id = "g-1";
        let first_id = Uuid::new_v4();
        let second_id = Uuid::new_v4();

        let (first_sender, mut first_receiver) = mpsc::channel::<String>(2);
        let (second_sender, mut second_receiver) = mpsc::channel::<String>(2);

        let mut guild_connections = HashMap::from([
            (String::from("g-1"), HashSet::from([first_id, second_id])),
            (String::from("g-2"), HashSet::new()),
        ]);
        let mut senders = HashMap::from([(first_id, first_sender), (second_id, second_sender)]);

        let mut slow_connections = Vec::new();
        let delivered = dispatch_guild_payload(
            &mut guild_connections,
            &mut senders,
            guild_id,
            "payload",
            "payload".len(),
            "presence_update",
            &mut slow_connections,
        );

        assert_eq!(delivered, 2);
        assert!(slow_connections.is_empty());
        assert_eq!(first_receiver.recv().await.as_deref(), Some("payload"));
        assert_eq!(second_receiver.recv().await.as_deref(), Some("payload"));
        assert!(guild_connections.contains_key("g-2"));
    }

    #[tokio::test]
    async fn prunes_closed_and_marks_full_guild_connections() {
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

        let mut guild_connections = HashMap::from([(
            String::from("g-1"),
            HashSet::from([keep_id, full_id, closed_id]),
        )]);
        let mut senders = HashMap::from([
            (keep_id, keep_sender),
            (full_id, full_sender),
            (closed_id, closed_sender),
        ]);

        let mut slow_connections = Vec::new();
        let delivered = dispatch_guild_payload(
            &mut guild_connections,
            &mut senders,
            "g-1",
            "payload",
            "payload".len(),
            "message_create",
            &mut slow_connections,
        );

        assert_eq!(delivered, 1);
        assert_eq!(slow_connections, vec![full_id]);

        let listeners = guild_connections
            .get("g-1")
            .expect("guild key should remain with keep listener");
        assert!(listeners.contains(&keep_id));
        assert!(!listeners.contains(&full_id));
        assert!(!listeners.contains(&closed_id));

        assert_eq!(full_receiver.recv().await.as_deref(), Some("occupied"));
    }

    #[tokio::test]
    async fn rejects_oversized_outbound_payload_before_guild_dispatch() {
        let connection_id = Uuid::new_v4();
        let (sender, mut receiver) = mpsc::channel::<String>(1);
        let mut guild_connections =
            HashMap::from([(String::from("g-1"), HashSet::from([connection_id]))]);
        let mut senders = HashMap::from([(connection_id, sender)]);
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
            &mut guild_connections,
            &mut senders,
            "g-1",
            payload,
            payload.len() - 1,
            event_type,
            &mut slow_connections,
        );

        assert_eq!(delivered, 0);
        assert!(slow_connections.is_empty());
        assert!(guild_connections
            .get("g-1")
            .expect("guild key should remain after oversized rejection")
            .contains(&connection_id));
        assert!(receiver.try_recv().is_err());

        let dropped = metrics_state()
            .gateway_events_dropped
            .lock()
            .expect("gateway dropped metrics mutex should not be poisoned");
        assert_eq!(dropped.get(&key).copied(), Some(before + 1));
    }

    #[tokio::test]
    async fn records_closed_and_full_queue_drop_reasons_for_guild_fanout() {
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
            .map_or((0, 0), |dropped| {
                (
                    dropped.get(&closed_key).copied().unwrap_or(0),
                    dropped.get(&full_key).copied().unwrap_or(0),
                )
            });

        let (full_sender, mut full_receiver) = mpsc::channel::<String>(1);
        full_sender
            .try_send(String::from("occupied"))
            .expect("queue should fill");
        let (closed_sender, closed_receiver) = mpsc::channel::<String>(1);
        drop(closed_receiver);

        let mut guild_connections =
            HashMap::from([(String::from("g-1"), HashSet::from([full_id, closed_id]))]);
        let mut senders = HashMap::from([(full_id, full_sender), (closed_id, closed_sender)]);
        let mut slow_connections = Vec::new();

        let delivered = dispatch_guild_payload(
            &mut guild_connections,
            &mut senders,
            "g-1",
            "payload",
            "payload".len(),
            event_type,
            &mut slow_connections,
        );

        assert_eq!(delivered, 0);
        assert_eq!(slow_connections, vec![full_id]);
        assert!(!guild_connections.contains_key("g-1"));
        assert_eq!(full_receiver.recv().await.as_deref(), Some("occupied"));

        let dropped = metrics_state()
            .gateway_events_dropped
            .lock()
            .expect("gateway dropped metrics mutex should not be poisoned");
        assert_eq!(dropped.get(&closed_key).copied(), Some(closed_before + 1));
        assert_eq!(dropped.get(&full_key).copied(), Some(full_before + 1));
    }

    #[tokio::test]
    async fn prunes_missing_sender_for_target_guild_only() {
        let target_id = Uuid::new_v4();
        let other_id = Uuid::new_v4();
        let (other_sender, _other_receiver) = mpsc::channel::<String>(1);
        let mut senders = HashMap::from([(other_id, other_sender)]);
        let mut guild_connections =
            HashMap::from([(String::from("g-target"), HashSet::from([target_id]))]);
        for index in 0..2_048 {
            guild_connections.insert(format!("g-load-{index}"), HashSet::new());
        }
        guild_connections.insert(String::from("g-other"), HashSet::from([other_id]));
        let guild_count_before = guild_connections.len();
        let mut slow_connections = Vec::new();

        let delivered = dispatch_guild_payload(
            &mut guild_connections,
            &mut senders,
            "g-target",
            "payload",
            "payload".len(),
            "presence_update",
            &mut slow_connections,
        );

        assert_eq!(delivered, 0);
        assert!(slow_connections.is_empty());
        assert!(!guild_connections.contains_key("g-target"));
        assert_eq!(guild_connections.len(), guild_count_before - 1);
        assert!(guild_connections
            .get("g-other")
            .expect("non-target guild index should remain untouched")
            .contains(&other_id));
    }

    #[test]
    fn user_fanout_returns_only_connections_for_target_user() {
        let target_user = UserId::new();
        let other_user = UserId::new();
        let target_connection_a = Uuid::new_v4();
        let target_connection_b = Uuid::new_v4();
        let other_connection = Uuid::new_v4();
        let user_connections = HashMap::from([
            (
                target_user,
                HashSet::from([target_connection_a, target_connection_b]),
            ),
            (other_user, HashSet::from([other_connection])),
        ]);

        let connection_ids = connection_ids_for_user(&user_connections, target_user);

        assert_eq!(connection_ids.len(), 2);
        assert!(connection_ids.contains(&target_connection_a));
        assert!(connection_ids.contains(&target_connection_b));
        assert!(!connection_ids.contains(&other_connection));
    }

    #[tokio::test]
    async fn user_fanout_removes_closed_and_full_connections() {
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
    async fn user_fanout_rejects_oversized_payload_before_enqueue() {
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
}
