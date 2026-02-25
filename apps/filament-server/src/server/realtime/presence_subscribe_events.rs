use filament_core::UserId;

use crate::server::{
    gateway_events::{self, GatewayEvent},
    realtime::presence_subscribe::PresenceSubscribeResult,
};

pub(crate) struct PresenceSubscribeEvents {
    pub(crate) snapshot: GatewayEvent,
    pub(crate) online_update: Option<GatewayEvent>,
}

#[derive(Debug)]
pub(crate) struct PresenceSubscribeEventBuildError {
    pub(crate) event_type: &'static str,
    pub(crate) source: anyhow::Error,
}

pub(crate) fn build_presence_subscribe_events(
    guild_id: &str,
    user_id: UserId,
    result: PresenceSubscribeResult,
) -> Result<PresenceSubscribeEvents, PresenceSubscribeEventBuildError> {
    let snapshot =
        gateway_events::try_presence_sync(guild_id, result.snapshot_user_ids).map_err(|error| {
            PresenceSubscribeEventBuildError {
                event_type: gateway_events::PRESENCE_SYNC_EVENT,
                source: error,
            }
        })?;
    let online_update = if result.became_online {
        Some(
            gateway_events::try_presence_update(guild_id, user_id, "online").map_err(|error| {
                PresenceSubscribeEventBuildError {
                    event_type: gateway_events::PRESENCE_UPDATE_EVENT,
                    source: error,
                }
            })?,
        )
    } else {
        None
    };
    Ok(PresenceSubscribeEvents {
        snapshot,
        online_update,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use filament_core::UserId;

    use super::build_presence_subscribe_events;
    use crate::server::realtime::presence_subscribe::PresenceSubscribeResult;

    #[test]
    fn includes_online_update_for_first_online_transition() {
        let user_id = UserId::new();
        let result = PresenceSubscribeResult {
            snapshot_user_ids: HashSet::from([user_id.to_string()]),
            became_online: true,
        };

        let events =
            build_presence_subscribe_events("g1", user_id, result).expect("events should build");

        assert_eq!(events.snapshot.event_type, "presence_sync");
        assert!(events.online_update.is_some());
        let online = events.online_update.expect("online update expected");
        assert_eq!(online.event_type, "presence_update");
        assert!(online.payload.contains("\"status\":\"online\""));
    }

    #[test]
    fn omits_online_update_without_transition() {
        let user_id = UserId::new();
        let first_snapshot = UserId::new().to_string();
        let second_snapshot = UserId::new().to_string();
        let result = PresenceSubscribeResult {
            snapshot_user_ids: HashSet::from([first_snapshot.clone(), second_snapshot.clone()]),
            became_online: false,
        };

        let events =
            build_presence_subscribe_events("g1", user_id, result).expect("events should build");

        assert_eq!(events.snapshot.event_type, "presence_sync");
        assert!(events.online_update.is_none());
        assert!(events.snapshot.payload.contains(&first_snapshot));
        assert!(events.snapshot.payload.contains(&second_snapshot));
    }

    #[test]
    fn preserves_snapshot_user_membership() {
        let expected: HashSet<String> = HashSet::from([
            UserId::new().to_string(),
            UserId::new().to_string(),
            UserId::new().to_string(),
        ]);
        let result = PresenceSubscribeResult {
            snapshot_user_ids: expected.clone(),
            became_online: false,
        };

        let events = build_presence_subscribe_events("g1", UserId::new(), result)
            .expect("events should build");

        for user_id in expected {
            assert!(events.snapshot.payload.contains(&user_id));
        }
    }
}
