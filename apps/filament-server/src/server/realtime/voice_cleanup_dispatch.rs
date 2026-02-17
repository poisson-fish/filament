use filament_core::UserId;

use crate::server::core::AppState;

use super::{
    broadcast_channel_event,
    voice_cleanup_registry::{
        disconnected_user_voice_removal_broadcasts, expired_voice_removal_broadcasts,
    },
};

pub(crate) async fn broadcast_expired_voice_removals(state: &AppState, now_unix: i64) {
    let planned = {
        let mut voice = state.realtime_registry.voice_participants().write().await;
        expired_voice_removal_broadcasts(&mut voice, now_unix)
    };

    for (channel_subscription_key, event) in planned {
        broadcast_channel_event(state, &channel_subscription_key, &event).await;
    }
}

pub(crate) async fn broadcast_disconnected_user_voice_removals(
    state: &AppState,
    user_id: UserId,
    disconnected_at_unix: i64,
) {
    let planned = {
        let mut voice = state.realtime_registry.voice_participants().write().await;
        disconnected_user_voice_removal_broadcasts(&mut voice, user_id, disconnected_at_unix)
    };

    for (channel_subscription_key, event) in planned {
        broadcast_channel_event(state, &channel_subscription_key, &event).await;
    }
}

#[cfg(test)]
mod tests {
    use crate::server::gateway_events::{self, GatewayEvent};

    fn sample_planned() -> Vec<(String, GatewayEvent)> {
        vec![
            (
                String::from("voice:guild-1:channel-1"),
                gateway_events::voice_participant_leave(
                    "guild-1",
                    "channel-1",
                    filament_core::UserId::new(),
                    "alice",
                    123,
                ),
            ),
            (
                String::from("voice:guild-1:channel-1"),
                gateway_events::voice_participant_update(
                    "guild-1",
                    "channel-1",
                    filament_core::UserId::new(),
                    "alice",
                    Some(false),
                    Some(false),
                    Some(true),
                    Some(false),
                    Some(false),
                    123,
                ),
            ),
        ]
    }

    fn planned_event_count(planned: &[(String, GatewayEvent)]) -> usize {
        planned.len()
    }

    #[test]
    fn reports_zero_for_empty_voice_cleanup_plan() {
        assert_eq!(planned_event_count(&[]), 0);
    }

    #[test]
    fn reports_number_of_planned_voice_cleanup_events() {
        let planned = sample_planned();

        assert_eq!(planned_event_count(&planned), 2);
    }
}
