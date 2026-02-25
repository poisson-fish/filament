use uuid::Uuid;

use crate::server::core::{GuildConnectionIndex, Subscriptions, UserConnectionIndex};

pub(crate) fn remove_connection_from_subscription_indexes(
    subscriptions: &mut Subscriptions,
    guild_connections: &mut GuildConnectionIndex,
    user_connections: &mut UserConnectionIndex,
    connection_id: Uuid,
) {
    subscriptions.retain(|_, listeners| {
        listeners.remove(&connection_id);
        !listeners.is_empty()
    });
    guild_connections.retain(|_, connection_ids| {
        connection_ids.remove(&connection_id);
        !connection_ids.is_empty()
    });
    user_connections.retain(|_, connection_ids| {
        connection_ids.remove(&connection_id);
        !connection_ids.is_empty()
    });
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use filament_core::UserId;
    use tokio::sync::mpsc;
    use uuid::Uuid;

    use super::remove_connection_from_subscription_indexes;
    use crate::server::core::{GuildConnectionIndex, Subscriptions, UserConnectionIndex};

    #[test]
    fn removes_connection_and_prunes_empty_entries_across_all_indexes() {
        let target = Uuid::new_v4();
        let keep = Uuid::new_v4();
        let target_user = UserId::new();
        let mixed_user = UserId::new();
        let (target_tx, _) = mpsc::channel::<String>(1);
        let (keep_tx, _) = mpsc::channel::<String>(1);

        let mut subscriptions: Subscriptions = HashMap::from([
            (String::from("g1:c1"), HashMap::from([(target, target_tx)])),
            (String::from("g1:c2"), HashMap::from([(keep, keep_tx)])),
        ]);
        let mut guild_connections: GuildConnectionIndex = HashMap::from([
            (String::from("g1"), HashSet::from([target, keep])),
            (String::from("g2"), HashSet::from([target])),
        ]);
        let mut user_connections: UserConnectionIndex = HashMap::from([
            (target_user, HashSet::from([target])),
            (mixed_user, HashSet::from([target, keep])),
        ]);

        remove_connection_from_subscription_indexes(
            &mut subscriptions,
            &mut guild_connections,
            &mut user_connections,
            target,
        );

        assert!(!subscriptions.contains_key("g1:c1"));
        assert!(subscriptions.contains_key("g1:c2"));
        assert!(!guild_connections.contains_key("g2"));
        assert!(guild_connections
            .get("g1")
            .expect("mixed guild should remain")
            .contains(&keep));
        assert!(!user_connections.contains_key(&target_user));
        assert!(user_connections
            .get(&mixed_user)
            .expect("mixed user should remain")
            .contains(&keep));
    }

    #[test]
    fn retains_entries_when_other_connections_remain() {
        let target = Uuid::new_v4();
        let keep = Uuid::new_v4();
        let mixed_user = UserId::new();
        let (target_tx, _) = mpsc::channel::<String>(1);
        let (keep_tx, _) = mpsc::channel::<String>(1);

        let mut subscriptions: Subscriptions = HashMap::from([(
            String::from("g1:c1"),
            HashMap::from([(target, target_tx), (keep, keep_tx)]),
        )]);
        let mut guild_connections: GuildConnectionIndex =
            HashMap::from([(String::from("g1"), HashSet::from([target, keep]))]);
        let mut user_connections: UserConnectionIndex =
            HashMap::from([(mixed_user, HashSet::from([target, keep]))]);

        remove_connection_from_subscription_indexes(
            &mut subscriptions,
            &mut guild_connections,
            &mut user_connections,
            target,
        );

        let listeners = subscriptions
            .get("g1:c1")
            .expect("entry should be retained for remaining listeners");
        assert_eq!(listeners.len(), 1);
        assert!(listeners.contains_key(&keep));
        assert!(guild_connections
            .get("g1")
            .expect("guild should remain")
            .contains(&keep));
        assert!(user_connections
            .get(&mixed_user)
            .expect("user should remain")
            .contains(&keep));
    }
}
