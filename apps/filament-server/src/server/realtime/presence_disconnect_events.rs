use filament_core::UserId;

use crate::server::gateway_events::{self, GatewayEvent};

pub(crate) fn build_offline_presence_updates(
    offline_guild_ids: Vec<String>,
    user_id: UserId,
) -> Vec<(String, GatewayEvent)> {
    offline_guild_ids
        .into_iter()
        .map(|guild_id| {
            let event = gateway_events::presence_update(&guild_id, user_id, "offline");
            (guild_id, event)
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
            build_offline_presence_updates(vec![String::from("g1"), String::from("g2")], user_id);

        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].0, "g1");
        assert_eq!(updates[1].0, "g2");
        assert_eq!(updates[0].1.event_type, "presence_update");
        assert!(updates[0].1.payload.contains("\"guild_id\":\"g1\""));
        assert!(updates[0]
            .1
            .payload
            .contains(&format!("\"user_id\":\"{}\"", user_id)));
    }

    #[test]
    fn returns_empty_when_no_offline_guilds() {
        let updates = build_offline_presence_updates(Vec::new(), UserId::new());

        assert!(updates.is_empty());
    }
}
