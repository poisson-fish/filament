use filament_core::UserId;

use crate::server::gateway_events::{self, GatewayEvent};

#[derive(Debug)]
pub(crate) struct PresenceDisconnectEventBuildError {
    pub(crate) event_type: &'static str,
    pub(crate) source: anyhow::Error,
}

pub(crate) fn build_offline_presence_updates(
    offline_guild_ids: Vec<String>,
    user_id: UserId,
) -> Result<Vec<(String, GatewayEvent)>, PresenceDisconnectEventBuildError> {
    offline_guild_ids
        .into_iter()
        .map(|guild_id| {
            gateway_events::try_presence_update(&guild_id, user_id, "offline")
                .map(|event| (guild_id, event))
                .map_err(|error| PresenceDisconnectEventBuildError {
                    event_type: gateway_events::PRESENCE_UPDATE_EVENT,
                    source: error,
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use filament_core::UserId;

    use super::build_offline_presence_updates;

    #[test]
    fn builds_offline_update_for_each_guild() {
        let user_id = UserId::new();

        let updates =
            build_offline_presence_updates(vec![String::from("g1"), String::from("g2")], user_id)
                .expect("offline updates should build");

        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].0, "g1");
        assert_eq!(updates[1].0, "g2");
        assert_eq!(updates[0].1.event_type, "presence_update");
        assert!(updates[0].1.payload.contains("\"guild_id\":\"g1\""));
        assert!(updates[0]
            .1
            .payload
            .contains(&format!("\"user_id\":\"{user_id}\"")));
    }

    #[test]
    fn returns_empty_when_no_offline_guilds() {
        let updates = build_offline_presence_updates(Vec::new(), UserId::new())
            .expect("no guilds should still succeed");

        assert!(updates.is_empty());
    }
}
