use filament_core::UserId;

use crate::server::{
    gateway_events::GatewayEvent,
    realtime::{
        presence_disconnect::DisconnectPresenceOutcome,
        presence_disconnect_events::build_offline_presence_updates,
    },
};

pub(crate) struct DisconnectFollowups {
    pub(crate) remove_voice_participants: bool,
    pub(crate) offline_updates: Vec<(String, GatewayEvent)>,
}

pub(crate) fn plan_disconnect_followups(
    outcome: DisconnectPresenceOutcome,
    user_id: UserId,
) -> DisconnectFollowups {
    DisconnectFollowups {
        remove_voice_participants: !outcome.user_has_other_connections,
        offline_updates: build_offline_presence_updates(outcome.offline_guilds, user_id),
    }
}

#[cfg(test)]
mod tests {
    use filament_core::UserId;

    use super::plan_disconnect_followups;
    use crate::server::realtime::presence_disconnect::DisconnectPresenceOutcome;

    #[test]
    fn plans_voice_cleanup_and_offline_updates_when_user_goes_offline() {
        let user_id = UserId::new();
        let outcome = DisconnectPresenceOutcome {
            user_has_other_connections: false,
            offline_guilds: vec![String::from("g1")],
        };

        let followups = plan_disconnect_followups(outcome, user_id);

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

        let followups = plan_disconnect_followups(outcome, user_id);

        assert!(!followups.remove_voice_participants);
        assert!(followups.offline_updates.is_empty());
    }
}
