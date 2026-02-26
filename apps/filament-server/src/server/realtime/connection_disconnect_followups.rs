use std::collections::HashMap;

use filament_core::UserId;
use uuid::Uuid;

use crate::server::{
    core::ConnectionPresence,
    gateway_events::{self, GatewayEvent},
};

pub(crate) struct DisconnectFollowups {
    pub(crate) remove_voice_participants: bool,
    pub(crate) offline_updates: Vec<(String, GatewayEvent)>,
}

#[derive(Debug)]
pub(crate) struct DisconnectFollowupsBuildError {
    pub(crate) event_type: &'static str,
    pub(crate) source: anyhow::Error,
}

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

fn build_offline_presence_updates(
    offline_guild_ids: Vec<String>,
    user_id: UserId,
) -> Result<Vec<(String, GatewayEvent)>, DisconnectFollowupsBuildError> {
    offline_guild_ids
        .into_iter()
        .map(|guild_id| {
            gateway_events::try_presence_update(&guild_id, user_id, "offline")
                .map(|event| (guild_id, event))
                .map_err(|source| DisconnectFollowupsBuildError {
                    event_type: gateway_events::PRESENCE_UPDATE_EVENT,
                    source,
                })
        })
        .collect()
}

pub(crate) fn plan_disconnect_followups(
    outcome: DisconnectPresenceOutcome,
    user_id: UserId,
) -> Result<DisconnectFollowups, DisconnectFollowupsBuildError> {
    let offline_updates = build_offline_presence_updates(outcome.offline_guilds, user_id)?;
    Ok(DisconnectFollowups {
        remove_voice_participants: !outcome.user_has_other_connections,
        offline_updates,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use filament_core::UserId;
    use uuid::Uuid;

    use super::{
        compute_disconnect_presence_outcome, plan_disconnect_followups, DisconnectPresenceOutcome,
    };
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

    #[test]
    fn plans_voice_cleanup_and_offline_updates_when_user_goes_offline() {
        let user_id = UserId::new();
        let outcome = DisconnectPresenceOutcome {
            user_has_other_connections: false,
            offline_guilds: vec![String::from("g1")],
        };

        let followups =
            plan_disconnect_followups(outcome, user_id).expect("followups should build");

        assert!(followups.remove_voice_participants);
        assert_eq!(followups.offline_updates.len(), 1);
        assert_eq!(followups.offline_updates[0].0, "g1");
    }

    #[test]
    fn skips_voice_cleanup_when_other_connections_exist() {
        let user_id = UserId::new();
        let outcome = DisconnectPresenceOutcome {
            user_has_other_connections: true,
            offline_guilds: Vec::new(),
        };

        let followups =
            plan_disconnect_followups(outcome, user_id).expect("followups should build");

        assert!(!followups.remove_voice_participants);
        assert!(followups.offline_updates.is_empty());
    }

    #[test]
    fn builds_offline_update_for_each_guild() {
        let user_id = UserId::new();
        let outcome = DisconnectPresenceOutcome {
            user_has_other_connections: false,
            offline_guilds: vec![String::from("g1"), String::from("g2")],
        };

        let followups =
            plan_disconnect_followups(outcome, user_id).expect("followups should build");

        assert_eq!(followups.offline_updates.len(), 2);
        assert_eq!(followups.offline_updates[0].0, "g1");
        assert_eq!(followups.offline_updates[1].0, "g2");
        assert_eq!(followups.offline_updates[0].1.event_type, "presence_update");
        assert!(followups.offline_updates[0]
            .1
            .payload
            .contains("\"guild_id\":\"g1\""));
        assert!(followups.offline_updates[0]
            .1
            .payload
            .contains(&format!("\"user_id\":\"{user_id}\"")));
    }
}
