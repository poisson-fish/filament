use tokio::sync::mpsc;
use uuid::Uuid;

use crate::server::core::Subscriptions;

pub(crate) fn insert_connection_subscription(
    subscriptions: &mut Subscriptions,
    connection_id: Uuid,
    key: String,
    outbound_tx: mpsc::Sender<String>,
) {
    subscriptions
        .entry(key)
        .or_default()
        .insert(connection_id, outbound_tx);
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use tokio::sync::mpsc;
    use uuid::Uuid;

    use super::insert_connection_subscription;

    #[test]
    fn inserts_subscription_for_new_key() {
        let connection_id = Uuid::new_v4();
        let (tx, _rx) = mpsc::channel::<String>(1);
        let mut subscriptions = HashMap::new();

        insert_connection_subscription(
            &mut subscriptions,
            connection_id,
            String::from("guild:channel"),
            tx,
        );

        let listeners = subscriptions
            .get("guild:channel")
            .expect("listener map should exist");
        assert_eq!(listeners.len(), 1);
        assert!(listeners.contains_key(&connection_id));
    }

    #[test]
    fn inserts_into_existing_key_without_removing_other_connections() {
        let first_connection = Uuid::new_v4();
        let second_connection = Uuid::new_v4();
        let (first_tx, _first_rx) = mpsc::channel::<String>(1);
        let (second_tx, _second_rx) = mpsc::channel::<String>(1);
        let mut subscriptions = HashMap::new();

        insert_connection_subscription(
            &mut subscriptions,
            first_connection,
            String::from("guild:channel"),
            first_tx,
        );
        insert_connection_subscription(
            &mut subscriptions,
            second_connection,
            String::from("guild:channel"),
            second_tx,
        );

        let listeners = subscriptions
            .get("guild:channel")
            .expect("listener map should exist");
        assert_eq!(listeners.len(), 2);
        assert!(listeners.contains_key(&first_connection));
        assert!(listeners.contains_key(&second_connection));
    }
}
