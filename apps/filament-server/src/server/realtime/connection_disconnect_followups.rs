use filament_core::UserId;

use crate::server::{
    gateway_events::GatewayEvent,
    realtime::{
        presence_disconnect::DisconnectPresenceOutcome,
        presence_disconnect_events::{
            build_offline_presence_updates, PresenceDisconnectEventBuildError,
        },
    },
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

pub(crate) fn plan_disconnect_followups(
    outcome: DisconnectPresenceOutcome,
    user_id: UserId,
) -> Result<DisconnectFollowups, DisconnectFollowupsBuildError> {
    let offline_updates = build_offline_presence_updates(outcome.offline_guilds, user_id).map_err(
        |PresenceDisconnectEventBuildError { event_type, source }| DisconnectFollowupsBuildError {
            event_type,
            source,
        },
    )?;
    Ok(DisconnectFollowups {
        remove_voice_participants: !outcome.user_has_other_connections,
        offline_updates,
    })
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
}
