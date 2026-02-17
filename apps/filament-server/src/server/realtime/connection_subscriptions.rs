use uuid::Uuid;

use crate::server::core::Subscriptions;

pub(crate) fn remove_connection_from_subscriptions(
    subscriptions: &mut Subscriptions,
    connection_id: Uuid,
) {
    subscriptions.retain(|_, listeners| {
        listeners.remove(&connection_id);
        !listeners.is_empty()
    });
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use tokio::sync::mpsc;
    use uuid::Uuid;

    use super::remove_connection_from_subscriptions;
    use crate::server::core::Subscriptions;

    #[test]
    fn removes_connection_and_drops_empty_subscription_entries() {
        let target = Uuid::new_v4();
        let keep = Uuid::new_v4();
        let (target_tx, _) = mpsc::channel::<String>(1);
        let (keep_tx, _) = mpsc::channel::<String>(1);

        let mut subscriptions: Subscriptions = HashMap::from([
            (String::from("g1:c1"), HashMap::from([(target, target_tx)])),
            (String::from("g1:c2"), HashMap::from([(keep, keep_tx)])),
        ]);

        remove_connection_from_subscriptions(&mut subscriptions, target);

        assert!(!subscriptions.contains_key("g1:c1"));
        assert!(subscriptions.contains_key("g1:c2"));
    }

    #[test]
    fn retains_subscription_entry_when_other_connections_remain() {
        let target = Uuid::new_v4();
        let keep = Uuid::new_v4();
        let (target_tx, _) = mpsc::channel::<String>(1);
        let (keep_tx, _) = mpsc::channel::<String>(1);

        let mut subscriptions: Subscriptions = HashMap::from([(
            String::from("g1:c1"),
            HashMap::from([(target, target_tx), (keep, keep_tx)]),
        )]);

        remove_connection_from_subscriptions(&mut subscriptions, target);

        let listeners = subscriptions
            .get("g1:c1")
            .expect("entry should be retained for remaining listeners");
        assert_eq!(listeners.len(), 1);
        assert!(listeners.contains_key(&keep));
    }
}
