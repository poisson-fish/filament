use std::collections::{HashMap, HashSet};

use filament_core::UserId;
use uuid::Uuid;

use crate::server::core::ConnectionPresence;

pub(crate) struct PresenceSubscribeResult {
    pub(crate) snapshot_user_ids: HashSet<String>,
    pub(crate) became_online: bool,
}

pub(crate) fn apply_presence_subscribe(
    presence: &mut HashMap<Uuid, ConnectionPresence>,
    connection_id: Uuid,
    user_id: UserId,
    guild_id: &str,
) -> Option<PresenceSubscribeResult> {
    let guild = guild_id.to_owned();
    let existing = presence.get(&connection_id)?;
    let already_subscribed = existing.guild_ids.contains(&guild);
    let was_online = presence
        .values()
        .any(|entry| entry.user_id == user_id && entry.guild_ids.contains(&guild));

    if let Some(connection) = presence.get_mut(&connection_id) {
        connection.guild_ids.insert(guild.clone());
    }

    let snapshot_user_ids = presence
        .values()
        .filter(|entry| entry.guild_ids.contains(&guild))
        .map(|entry| entry.user_id.to_string())
        .collect::<HashSet<_>>();

    Some(PresenceSubscribeResult {
        snapshot_user_ids,
        became_online: !was_online && !already_subscribed,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use filament_core::UserId;
    use uuid::Uuid;

    use super::apply_presence_subscribe;
    use crate::server::core::ConnectionPresence;

    #[test]
    fn inserts_guild_and_marks_first_online_subscription() {
        let user_id = UserId::new();
        let connection_id = Uuid::new_v4();
        let mut presence = HashMap::from([(
            connection_id,
            ConnectionPresence {
                user_id,
                guild_ids: HashSet::new(),
            },
        )]);

        let result = apply_presence_subscribe(&mut presence, connection_id, user_id, "g-1")
            .expect("connection presence should exist");

        assert!(result.became_online);
        assert_eq!(result.snapshot_user_ids, HashSet::from([user_id.to_string()]));
        assert!(presence
            .get(&connection_id)
            .expect("connection should remain")
            .guild_ids
            .contains("g-1"));
    }

    #[test]
    fn does_not_mark_online_when_already_online_in_guild() {
        let user_id = UserId::new();
        let first_connection = Uuid::new_v4();
        let second_connection = Uuid::new_v4();

        let mut presence = HashMap::from([
            (
                first_connection,
                ConnectionPresence {
                    user_id,
                    guild_ids: HashSet::from([String::from("g-1")]),
                },
            ),
            (
                second_connection,
                ConnectionPresence {
                    user_id,
                    guild_ids: HashSet::new(),
                },
            ),
        ]);

        let result = apply_presence_subscribe(&mut presence, second_connection, user_id, "g-1")
            .expect("connection presence should exist");

        assert!(!result.became_online);
        assert_eq!(result.snapshot_user_ids, HashSet::from([user_id.to_string()]));
    }

    #[test]
    fn returns_none_when_connection_is_missing() {
        let mut presence = HashMap::new();

        let result = apply_presence_subscribe(
            &mut presence,
            Uuid::new_v4(),
            UserId::new(),
            "g-1",
        );

        assert!(result.is_none());
    }
}