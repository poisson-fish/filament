use filament_core::UserId;

use crate::server::{
    auth::channel_key,
    core::{AppState, VoiceParticipantsByChannel},
    gateway_events::{self, GatewayEvent},
    metrics::record_gateway_event_dropped,
};

use super::{
    broadcast_channel_event,
    voice_registry::{
        remove_channel_user_voice_participant_removal, remove_user_voice_participant_removals,
        take_expired_voice_participant_removals, VoiceParticipantRemoval,
    },
};

#[derive(Debug)]
pub(crate) struct VoiceCleanupEventBuildError {
    pub(crate) event_type: &'static str,
    pub(crate) source: anyhow::Error,
}

fn build_voice_removal_events(
    guild_id: &str,
    channel_id: &str,
    participant: &crate::server::core::VoiceParticipant,
    event_at_unix: i64,
) -> Result<Vec<GatewayEvent>, VoiceCleanupEventBuildError> {
    let mut events = Vec::with_capacity(participant.published_streams.len().saturating_add(1));
    for stream in &participant.published_streams {
        let event = gateway_events::try_voice_stream_unpublish(
            guild_id,
            channel_id,
            participant.user_id,
            &participant.identity,
            *stream,
            event_at_unix,
        )
        .map_err(|source| VoiceCleanupEventBuildError {
            event_type: gateway_events::VOICE_STREAM_UNPUBLISH_EVENT,
            source,
        })?;
        events.push(event);
    }
    let event = gateway_events::try_voice_participant_leave(
        guild_id,
        channel_id,
        participant.user_id,
        &participant.identity,
        event_at_unix,
    )
    .map_err(|source| VoiceCleanupEventBuildError {
        event_type: gateway_events::VOICE_PARTICIPANT_LEAVE_EVENT,
        source,
    })?;
    events.push(event);
    Ok(events)
}

fn plan_voice_removal_broadcasts(
    removals: Vec<VoiceParticipantRemoval>,
    event_at_unix: i64,
) -> Result<Vec<(String, GatewayEvent)>, VoiceCleanupEventBuildError> {
    let mut planned = Vec::new();
    for removed in removals {
        let subscription_key = channel_key(&removed.guild_id, &removed.channel_id);
        for event in build_voice_removal_events(
            &removed.guild_id,
            &removed.channel_id,
            &removed.participant,
            event_at_unix,
        )? {
            planned.push((subscription_key.clone(), event));
        }
    }
    Ok(planned)
}

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
        realtime::voice_registry::VoiceParticipantRemoval,
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

    fn participant_with_streams(streams: HashSet<VoiceStreamKind>) -> VoiceParticipant {
        VoiceParticipant {
            user_id: filament_core::UserId::new(),
            identity: String::from("voice-user"),
            joined_at_unix: 5,
            updated_at_unix: 6,
            expires_at_unix: 99,
            is_muted: false,
            is_deafened: false,
            is_speaking: false,
            is_video_enabled: false,
            is_screen_share_enabled: false,
            published_streams: streams,
        }
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

    #[test]
    fn includes_unpublish_events_then_leave_when_streams_exist() {
        let participant = participant_with_streams(HashSet::from([
            VoiceStreamKind::Microphone,
            VoiceStreamKind::Camera,
        ]));

        let events = super::build_voice_removal_events("g1", "c1", &participant, 10)
            .expect("voice removal events should serialize");

        assert_eq!(events.len(), 3);
        let unpublish_count = events
            .iter()
            .filter(|event| event.event_type == VOICE_STREAM_UNPUBLISH_EVENT)
            .count();
        assert_eq!(unpublish_count, 2);
        assert_eq!(
            events.last().map(|event| event.event_type),
            Some(VOICE_PARTICIPANT_LEAVE_EVENT)
        );
    }

    #[test]
    fn plans_channel_scoped_broadcast_pairs_for_each_removal() {
        let first = VoiceParticipantRemoval {
            guild_id: String::from("g1"),
            channel_id: String::from("c1"),
            participant: participant_with_streams(HashSet::from([VoiceStreamKind::Microphone])),
        };
        let second = VoiceParticipantRemoval {
            guild_id: String::from("g1"),
            channel_id: String::from("c2"),
            participant: participant_with_streams(HashSet::new()),
        };

        let planned = super::plan_voice_removal_broadcasts(vec![first, second], 12)
            .expect("voice removal broadcasts should serialize");

        assert_eq!(planned.len(), 3);
        assert_eq!(planned[0].0, "g1:c1");
        assert_eq!(planned[1].0, "g1:c1");
        assert_eq!(planned[2].0, "g1:c2");
        assert_eq!(planned[0].1.event_type, VOICE_STREAM_UNPUBLISH_EVENT);
        assert_eq!(planned[1].1.event_type, VOICE_PARTICIPANT_LEAVE_EVENT);
        assert_eq!(planned[2].1.event_type, VOICE_PARTICIPANT_LEAVE_EVENT);
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
