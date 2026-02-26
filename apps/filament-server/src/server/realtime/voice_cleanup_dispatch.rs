use filament_core::UserId;

use crate::server::{
    core::{AppState, VoiceParticipantsByChannel},
    gateway_events::GatewayEvent,
    metrics::record_gateway_event_dropped,
};

use super::{
    broadcast_channel_event,
    voice_cleanup_events::{plan_voice_removal_broadcasts, VoiceCleanupEventBuildError},
    voice_registry::{
        remove_channel_user_voice_participant_removal, remove_user_voice_participant_removals,
        take_expired_voice_participant_removals,
    },
};

fn expired_voice_removal_broadcasts(
    voice: &mut VoiceParticipantsByChannel,
    now_unix: i64,
) -> Result<Vec<(String, GatewayEvent)>, VoiceCleanupEventBuildError> {
    let removed = take_expired_voice_participant_removals(voice, now_unix);
    plan_voice_removal_broadcasts(removed, now_unix)
}

fn disconnected_user_voice_removal_broadcasts(
    voice: &mut VoiceParticipantsByChannel,
    user_id: UserId,
    disconnected_at_unix: i64,
) -> Result<Vec<(String, GatewayEvent)>, VoiceCleanupEventBuildError> {
    let removed = remove_user_voice_participant_removals(voice, user_id);
    plan_voice_removal_broadcasts(removed, disconnected_at_unix)
}

pub(crate) fn channel_user_voice_removal_broadcasts(
    voice: &mut VoiceParticipantsByChannel,
    guild_id: &str,
    channel_id: &str,
    user_id: UserId,
    removed_at_unix: i64,
) -> Result<Vec<(String, GatewayEvent)>, VoiceCleanupEventBuildError> {
    let removed =
        remove_channel_user_voice_participant_removal(voice, guild_id, channel_id, user_id)
            .into_iter()
            .collect::<Vec<_>>();
    plan_voice_removal_broadcasts(removed, removed_at_unix)
}

pub(crate) async fn broadcast_expired_voice_removals(state: &AppState, now_unix: i64) {
    let planned_result = {
        let mut voice = state.realtime_registry.voice_participants().write().await;
        expired_voice_removal_broadcasts(&mut voice, now_unix)
    };
    let planned = match planned_result {
        Ok(planned) => planned,
        Err(error) => {
            tracing::warn!(
                event = "gateway.voice_cleanup.serialize_failed",
                event_type = error.event_type,
                error = %error.source
            );
            record_gateway_event_dropped("channel", error.event_type, "serialize_error");
            return;
        }
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
    let planned_result = {
        let mut voice = state.realtime_registry.voice_participants().write().await;
        disconnected_user_voice_removal_broadcasts(&mut voice, user_id, disconnected_at_unix)
    };
    let planned = match planned_result {
        Ok(planned) => planned,
        Err(error) => {
            tracing::warn!(
                event = "gateway.voice_cleanup.serialize_failed",
                user_id = %user_id,
                event_type = error.event_type,
                error = %error.source
            );
            record_gateway_event_dropped("channel", error.event_type, "serialize_error");
            return;
        }
    };

    for (channel_subscription_key, event) in planned {
        broadcast_channel_event(state, &channel_subscription_key, &event).await;
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use crate::server::{
        core::{VoiceParticipant, VoiceParticipantsByChannel, VoiceStreamKind},
        gateway_events::{
            self, GatewayEvent, VOICE_PARTICIPANT_LEAVE_EVENT, VOICE_STREAM_UNPUBLISH_EVENT,
        },
    };

    fn sample_planned() -> Vec<(String, GatewayEvent)> {
        vec![
            (
                String::from("voice:guild-1:channel-1"),
                gateway_events::try_voice_participant_leave(
                    "guild-1",
                    "channel-1",
                    filament_core::UserId::new(),
                    "alice",
                    123,
                )
                .expect("voice_participant_leave event should serialize"),
            ),
            (
                String::from("voice:guild-1:channel-1"),
                gateway_events::try_voice_participant_update(
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
                )
                .expect("voice_participant_update event should serialize"),
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

    fn participant(
        user_id: filament_core::UserId,
        identity: &str,
        expires_at_unix: i64,
    ) -> VoiceParticipant {
        VoiceParticipant {
            user_id,
            identity: identity.to_owned(),
            joined_at_unix: 1,
            updated_at_unix: 1,
            expires_at_unix,
            is_muted: false,
            is_deafened: false,
            is_speaking: false,
            is_video_enabled: false,
            is_screen_share_enabled: false,
            published_streams: HashSet::from([VoiceStreamKind::Microphone]),
        }
    }

    #[test]
    fn expired_cleanup_plans_voice_removal_events_for_parseable_channel_keys() {
        let expiring_user = filament_core::UserId::new();
        let mut voice: VoiceParticipantsByChannel = HashMap::from([(
            String::from("g1:c1"),
            HashMap::from([(expiring_user, participant(expiring_user, "alice", 5))]),
        )]);

        let planned = super::expired_voice_removal_broadcasts(&mut voice, 10)
            .expect("expired cleanup events should serialize");

        assert_eq!(planned.len(), 2);
        assert_eq!(planned[0].0, "g1:c1");
        assert_eq!(planned[1].0, "g1:c1");
        assert_eq!(planned[0].1.event_type, VOICE_STREAM_UNPUBLISH_EVENT);
        assert_eq!(planned[1].1.event_type, VOICE_PARTICIPANT_LEAVE_EVENT);
        assert!(voice.is_empty());
    }

    #[test]
    fn disconnected_cleanup_plans_voice_removal_events_for_target_user_only() {
        let target_user = filament_core::UserId::new();
        let other_user = filament_core::UserId::new();
        let mut voice: VoiceParticipantsByChannel = HashMap::from([(
            String::from("g1:c1"),
            HashMap::from([
                (target_user, participant(target_user, "alice", 30)),
                (other_user, participant(other_user, "bob", 30)),
            ]),
        )]);

        let planned =
            super::disconnected_user_voice_removal_broadcasts(&mut voice, target_user, 10)
                .expect("disconnected cleanup events should serialize");

        assert_eq!(planned.len(), 2);
        assert_eq!(planned[0].0, "g1:c1");
        assert_eq!(planned[1].0, "g1:c1");
        assert_eq!(planned[0].1.event_type, VOICE_STREAM_UNPUBLISH_EVENT);
        assert_eq!(planned[1].1.event_type, VOICE_PARTICIPANT_LEAVE_EVENT);
        assert_eq!(voice["g1:c1"].len(), 1);
        assert!(voice["g1:c1"].contains_key(&other_user));
    }

    #[test]
    fn channel_scoped_cleanup_removes_target_user_only_from_target_channel() {
        let target_user = filament_core::UserId::new();
        let other_user = filament_core::UserId::new();
        let mut voice: VoiceParticipantsByChannel = HashMap::from([
            (
                String::from("g2:c1"),
                HashMap::from([
                    (target_user, participant(target_user, "alice", 50)),
                    (other_user, participant(other_user, "bob", 50)),
                ]),
            ),
            (
                String::from("g2:c2"),
                HashMap::from([(target_user, participant(target_user, "alice-other", 50))]),
            ),
        ]);

        let planned =
            super::channel_user_voice_removal_broadcasts(&mut voice, "g2", "c1", target_user, 10)
                .expect("channel-scoped cleanup events should serialize");

        assert_eq!(planned.len(), 2);
        assert_eq!(planned[0].0, "g2:c1");
        assert_eq!(planned[1].0, "g2:c1");
        assert_eq!(planned[0].1.event_type, VOICE_STREAM_UNPUBLISH_EVENT);
        assert_eq!(planned[1].1.event_type, VOICE_PARTICIPANT_LEAVE_EVENT);
        assert_eq!(voice["g2:c1"].len(), 1);
        assert!(voice["g2:c1"].contains_key(&other_user));
        assert_eq!(voice["g2:c2"].len(), 1);
        assert!(voice["g2:c2"].contains_key(&target_user));
    }
}
