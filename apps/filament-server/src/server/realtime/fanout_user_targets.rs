use std::collections::HashMap;

use filament_core::UserId;
use uuid::Uuid;

use crate::server::core::ConnectionPresence;

pub(crate) fn connection_ids_for_user(
    presence: &HashMap<Uuid, ConnectionPresence>,
    user_id: UserId,
) -> Vec<Uuid> {
    presence
        .iter()
        .filter_map(|(connection_id, state)| (state.user_id == user_id).then_some(*connection_id))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use filament_core::UserId;
    use uuid::Uuid;

    use super::connection_ids_for_user;
    use crate::server::core::ConnectionPresence;

    #[test]
    fn returns_only_connections_for_target_user() {
        let target_user = UserId::new();
        let other_user = UserId::new();
        let target_connection_a = Uuid::new_v4();
        let target_connection_b = Uuid::new_v4();
        let other_connection = Uuid::new_v4();
        let presence = HashMap::from([
            (
                target_connection_a,
                ConnectionPresence {
                    user_id: target_user,
                    guild_ids: HashSet::new(),
                },
            ),
            (
                target_connection_b,
                ConnectionPresence {
                    user_id: target_user,
                    guild_ids: HashSet::new(),
                },
            ),
            (
                other_connection,
                ConnectionPresence {
                    user_id: other_user,
                    guild_ids: HashSet::new(),
                },
            ),
        ]);

        let connection_ids = connection_ids_for_user(&presence, target_user);

        assert_eq!(connection_ids.len(), 2);
        assert!(connection_ids.contains(&target_connection_a));
        assert!(connection_ids.contains(&target_connection_b));
        assert!(!connection_ids.contains(&other_connection));
    }

    #[test]
    fn returns_empty_when_user_has_no_connections() {
        let user_id = UserId::new();
        let other_user = UserId::new();
        let presence = HashMap::from([(
            Uuid::new_v4(),
            ConnectionPresence {
                user_id: other_user,
                guild_ids: HashSet::new(),
            },
        )]);

        let connection_ids = connection_ids_for_user(&presence, user_id);

        assert!(connection_ids.is_empty());
    }
}
