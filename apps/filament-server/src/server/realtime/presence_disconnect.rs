use std::collections::HashMap;

use uuid::Uuid;

use crate::server::core::ConnectionPresence;

pub(crate) struct DisconnectPresenceOutcome {
    pub(crate) user_has_other_connections: bool,
    pub(crate) offline_guilds: Vec<String>,
}

pub(crate) fn compute_disconnect_presence_outcome(
    remaining: &HashMap<Uuid, ConnectionPresence>,
    removed_presence: &ConnectionPresence,
) -> DisconnectPresenceOutcome {
    let user_has_other_connections = remaining
        .values()
        .any(|entry| entry.user_id == removed_presence.user_id);

    let mut offline_guilds = Vec::new();
    for guild_id in &removed_presence.guild_ids {
        let still_online = remaining.values().any(|entry| {
            entry.user_id == removed_presence.user_id && entry.guild_ids.contains(guild_id)
        });
        if !still_online {
            offline_guilds.push(guild_id.clone());
        }
    }

    DisconnectPresenceOutcome {
        user_has_other_connections,
        offline_guilds,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use filament_core::UserId;
    use uuid::Uuid;

    use super::compute_disconnect_presence_outcome;
    use crate::server::core::ConnectionPresence;

    #[test]
    fn reports_all_removed_guilds_offline_without_other_connections() {
        let user_id = UserId::new();
        let removed_presence = ConnectionPresence {
            user_id,
            guild_ids: HashSet::from([String::from("g-1"), String::from("g-2")]),
        };

        let remaining = HashMap::new();
        let outcome = compute_disconnect_presence_outcome(&remaining, &removed_presence);

        assert!(!outcome.user_has_other_connections);
        assert_eq!(
            outcome.offline_guilds.into_iter().collect::<HashSet<_>>(),
            HashSet::from([String::from("g-1"), String::from("g-2")])
        );
    }

    #[test]
    fn keeps_guild_online_when_user_has_other_subscribed_connection() {
        let user_id = UserId::new();
        let removed_presence = ConnectionPresence {
            user_id,
            guild_ids: HashSet::from([String::from("g-1"), String::from("g-2")]),
        };
        let remaining_connection = Uuid::new_v4();

        let remaining = HashMap::from([(
            remaining_connection,
            ConnectionPresence {
                user_id,
                guild_ids: HashSet::from([String::from("g-1")]),
            },
        )]);

        let outcome = compute_disconnect_presence_outcome(&remaining, &removed_presence);

        assert!(outcome.user_has_other_connections);
        assert_eq!(outcome.offline_guilds, vec![String::from("g-2")]);
    }

    #[test]
    fn ignores_other_users_when_computing_presence() {
        let user_id = UserId::new();
        let removed_presence = ConnectionPresence {
            user_id,
            guild_ids: HashSet::from([String::from("g-1")]),
        };
        let remaining_connection = Uuid::new_v4();

        let remaining = HashMap::from([(
            remaining_connection,
            ConnectionPresence {
                user_id: UserId::new(),
                guild_ids: HashSet::from([String::from("g-1")]),
            },
        )]);

        let outcome = compute_disconnect_presence_outcome(&remaining, &removed_presence);

        assert!(!outcome.user_has_other_connections);
        assert_eq!(outcome.offline_guilds, vec![String::from("g-1")]);
    }
}
