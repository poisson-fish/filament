use tokio::sync::mpsc;
use uuid::Uuid;

use crate::server::core::{GuildConnectionIndex, Subscriptions};

fn guild_id_from_subscription_key(key: &str) -> Option<&str> {
    let (guild_id, _channel_id) = key.split_once(':')?;
    if guild_id.is_empty() {
        return None;
    }
    Some(guild_id)
}

pub(crate) fn insert_connection_subscription(
    subscriptions: &mut Subscriptions,
    guild_connections: &mut GuildConnectionIndex,
    connection_id: Uuid,
    key: String,
    outbound_tx: mpsc::Sender<String>,
) {
    let guild_id = guild_id_from_subscription_key(&key).map(ToOwned::to_owned);
    subscriptions
        .entry(key)
        .or_default()
        .insert(connection_id, outbound_tx);
    if let Some(guild_id) = guild_id {
        guild_connections
            .entry(guild_id)
            .or_default()
            .insert(connection_id);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use tokio::sync::mpsc;
    use uuid::Uuid;

    use super::insert_connection_subscription;
    use crate::server::core::GuildConnectionIndex;

    #[test]
    fn inserts_subscription_for_new_key() {
        let connection_id = Uuid::new_v4();
        let (tx, _rx) = mpsc::channel::<String>(1);
        let mut subscriptions = HashMap::new();
        let mut guild_connections = GuildConnectionIndex::new();

        insert_connection_subscription(
            &mut subscriptions,
            &mut guild_connections,
            connection_id,
            String::from("guild:channel"),
            tx,
        );

        let listeners = subscriptions
            .get("guild:channel")
            .expect("listener map should exist");
        assert_eq!(listeners.len(), 1);
        assert!(listeners.contains_key(&connection_id));
        assert!(guild_connections
            .get("guild")
            .expect("guild index should exist")
            .contains(&connection_id));
    }

    #[test]
    fn inserts_into_existing_key_without_removing_other_connections() {
        let first_connection = Uuid::new_v4();
        let second_connection = Uuid::new_v4();
        let (first_tx, _first_rx) = mpsc::channel::<String>(1);
        let (second_tx, _second_rx) = mpsc::channel::<String>(1);
        let mut subscriptions = HashMap::new();
        let mut guild_connections = GuildConnectionIndex::new();

        insert_connection_subscription(
            &mut subscriptions,
            &mut guild_connections,
            first_connection,
            String::from("guild:channel"),
            first_tx,
        );
        insert_connection_subscription(
            &mut subscriptions,
            &mut guild_connections,
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
        assert_eq!(
            guild_connections
                .get("guild")
                .expect("guild index should exist"),
            &HashSet::from([first_connection, second_connection])
        );
    }

    #[test]
    fn does_not_index_guild_when_key_shape_is_invalid() {
        let connection_id = Uuid::new_v4();
        let (tx, _rx) = mpsc::channel::<String>(1);
        let mut subscriptions = HashMap::new();
        let mut guild_connections = GuildConnectionIndex::new();

        insert_connection_subscription(
            &mut subscriptions,
            &mut guild_connections,
            connection_id,
            String::from("invalid-key"),
            tx,
        );

        assert!(subscriptions.contains_key("invalid-key"));
        assert!(guild_connections.is_empty());
    }
}
