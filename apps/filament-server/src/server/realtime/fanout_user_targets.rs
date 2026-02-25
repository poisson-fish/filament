use filament_core::UserId;
use uuid::Uuid;

use crate::server::core::UserConnectionIndex;

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

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use filament_core::UserId;
    use uuid::Uuid;

    use super::connection_ids_for_user;

    #[test]
    fn returns_only_connections_for_target_user() {
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

    #[test]
    fn returns_empty_when_user_has_no_connections() {
        let user_id = UserId::new();
        let other_user = UserId::new();
        let user_connections = HashMap::from([(other_user, HashSet::from([Uuid::new_v4()]))]);

        let connection_ids = connection_ids_for_user(&user_connections, user_id);

        assert!(connection_ids.is_empty());
    }
}
