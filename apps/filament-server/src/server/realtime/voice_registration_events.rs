use filament_core::UserId;

use crate::server::{
    auth::channel_key,
    gateway_events::{self, GatewayEvent},
};

use super::{
    voice_presence::voice_snapshot_from_record,
    voice_registration::VoiceRegistrationTransition,
};

pub(crate) fn plan_voice_registration_events(
    transition: VoiceRegistrationTransition,
    guild_id: &str,
    channel_id: &str,
    user_id: UserId,
    identity: &str,
    event_at_unix: i64,
) -> Vec<(String, GatewayEvent)> {
    let mut planned = Vec::new();

    for (old_key, participant) in transition.removed {
        let Some((old_guild_id, old_channel_id)) = old_key.split_once(':') else {
            continue;
        };
        let subscription_key = channel_key(old_guild_id, old_channel_id);
        for stream in participant.published_streams {
            planned.push((
                subscription_key.clone(),
                gateway_events::voice_stream_unpublish(
                    old_guild_id,
                    old_channel_id,
                    participant.user_id,
                    &participant.identity,
                    stream,
                    event_at_unix,
                ),
            ));
        }
        planned.push((
            subscription_key,
            gateway_events::voice_participant_leave(
                old_guild_id,
                old_channel_id,
                participant.user_id,
                &participant.identity,
                event_at_unix,
            ),
        ));
    }

    let subscription_key = channel_key(guild_id, channel_id);
    if let Some(participant) = transition.joined {
        planned.push((
            subscription_key.clone(),
            gateway_events::voice_participant_join(
                guild_id,
                channel_id,
                voice_snapshot_from_record(&participant),
            ),
        ));
    }
    if let Some(participant) = transition.updated {
        planned.push((
            subscription_key.clone(),
            gateway_events::voice_participant_update(
                guild_id,
                channel_id,
                participant.user_id,
                &participant.identity,
                None,
                None,
                Some(participant.is_speaking),
                Some(participant.is_video_enabled),
                Some(participant.is_screen_share_enabled),
                participant.updated_at_unix,
            ),
        ));
    }
    for stream in transition.unpublished {
        planned.push((
            subscription_key.clone(),
            gateway_events::voice_stream_unpublish(
                guild_id,
                channel_id,
                user_id,
                identity,
                stream,
                event_at_unix,
            ),
        ));
    }
    for stream in transition.newly_published {
        planned.push((
            subscription_key.clone(),
            gateway_events::voice_stream_publish(
                guild_id,
                channel_id,
                user_id,
                identity,
                stream,
                event_at_unix,
            ),
        ));
    }

    planned
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use filament_core::UserId;

    use super::plan_voice_registration_events;
    use crate::server::{
        core::{VoiceParticipant, VoiceStreamKind},
        gateway_events::{
            VOICE_PARTICIPANT_JOIN_EVENT, VOICE_PARTICIPANT_LEAVE_EVENT,
            VOICE_PARTICIPANT_UPDATE_EVENT, VOICE_STREAM_PUBLISH_EVENT,
            VOICE_STREAM_UNPUBLISH_EVENT,
        },
        realtime::voice_registration::VoiceRegistrationTransition,
    };

    fn participant(
        user_id: UserId,
        identity: &str,
        streams: HashSet<VoiceStreamKind>,
    ) -> VoiceParticipant {
        VoiceParticipant {
            user_id,
            identity: identity.to_owned(),
            joined_at_unix: 1,
            updated_at_unix: 2,
            expires_at_unix: 3,
            is_muted: false,
            is_deafened: false,
            is_speaking: false,
            is_video_enabled: streams.contains(&VoiceStreamKind::Camera),
            is_screen_share_enabled: streams.contains(&VoiceStreamKind::ScreenShare),
            published_streams: streams,
        }
    }

    #[test]
    fn plans_removed_current_and_stream_delta_events_and_skips_malformed_old_key() {
        let current_user = UserId::new();
        let removed_user = UserId::new();
        let join_user = UserId::new();
        let update_user = UserId::new();
        let transition = VoiceRegistrationTransition {
            removed: vec![
                (
                    String::from("g1:c1"),
                    participant(
                        removed_user,
                        "removed",
                        HashSet::from([VoiceStreamKind::Microphone, VoiceStreamKind::Camera]),
                    ),
                ),
                (
                    String::from("malformed"),
                    participant(
                        removed_user,
                        "removed2",
                        HashSet::from([VoiceStreamKind::Microphone]),
                    ),
                ),
            ],
            joined: Some(participant(
                join_user,
                "joined",
                HashSet::from([VoiceStreamKind::Microphone]),
            )),
            updated: Some(participant(
                update_user,
                "updated",
                HashSet::from([VoiceStreamKind::Camera]),
            )),
            newly_published: vec![VoiceStreamKind::ScreenShare],
            unpublished: vec![VoiceStreamKind::Camera],
        };

        let planned =
            plan_voice_registration_events(transition, "g2", "c2", current_user, "current", 9);

        assert_eq!(planned.len(), 7);
        let removed_key_events = planned
            .iter()
            .filter(|(key, _)| key == "g1:c1")
            .count();
        assert_eq!(removed_key_events, 3);

        let join_count = planned
            .iter()
            .filter(|(_, event)| event.event_type == VOICE_PARTICIPANT_JOIN_EVENT)
            .count();
        assert_eq!(join_count, 1);
        let update_count = planned
            .iter()
            .filter(|(_, event)| event.event_type == VOICE_PARTICIPANT_UPDATE_EVENT)
            .count();
        assert_eq!(update_count, 1);
        let publish_count = planned
            .iter()
            .filter(|(_, event)| event.event_type == VOICE_STREAM_PUBLISH_EVENT)
            .count();
        assert_eq!(publish_count, 1);
        let unpublish_count = planned
            .iter()
            .filter(|(_, event)| event.event_type == VOICE_STREAM_UNPUBLISH_EVENT)
            .count();
        assert_eq!(unpublish_count, 3);
        let leave_count = planned
            .iter()
            .filter(|(_, event)| event.event_type == VOICE_PARTICIPANT_LEAVE_EVENT)
            .count();
        assert_eq!(leave_count, 1);
    }

    #[test]
    fn returns_empty_when_transition_has_no_changes() {
        let transition = VoiceRegistrationTransition {
            removed: Vec::new(),
            joined: None,
            updated: None,
            newly_published: Vec::new(),
            unpublished: Vec::new(),
        };

        let planned =
            plan_voice_registration_events(transition, "g1", "c1", UserId::new(), "u", 5);

        assert!(planned.is_empty());
    }
}