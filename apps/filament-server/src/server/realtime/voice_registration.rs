use std::collections::HashSet;

use filament_core::UserId;

use crate::server::{
    auth::channel_key,
    core::{VoiceParticipant, VoiceParticipantsByChannel, VoiceStreamKind},
    errors::AuthFailure,
    gateway_events::{self, GatewayEvent},
};

pub(crate) struct VoiceRegistrationTransition {
    pub(crate) removed: Vec<(String, VoiceParticipant)>,
    pub(crate) joined: Option<VoiceParticipant>,
    pub(crate) updated: Option<VoiceParticipant>,
    pub(crate) newly_published: Vec<VoiceStreamKind>,
    pub(crate) unpublished: Vec<VoiceStreamKind>,
}

#[derive(Debug)]
pub(crate) struct VoiceRegistrationEventBuildError {
    pub(crate) event_type: &'static str,
    pub(crate) source: anyhow::Error,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_voice_registration_transition(
    channels: &mut VoiceParticipantsByChannel,
    key: &str,
    user_id: UserId,
    identity: &str,
    publish_streams: &[VoiceStreamKind],
    expires_at_unix: i64,
    now_unix: i64,
    max_tracked_channels: usize,
    max_participants_per_channel: usize,
) -> Result<VoiceRegistrationTransition, AuthFailure> {
    let mut removed = Vec::new();
    let mut joined = None;
    let mut updated = None;
    let mut newly_published = Vec::new();
    let mut unpublished = Vec::new();

    for (existing_key, participants) in channels.iter_mut() {
        if existing_key == key {
            continue;
        }
        if let Some(existing) = participants.remove(&user_id) {
            removed.push((existing_key.clone(), existing));
        }
    }
    channels.retain(|_, participants| !participants.is_empty());

    if !channels.contains_key(key) && channels.len() >= max_tracked_channels {
        return Err(AuthFailure::RateLimited);
    }

    let channel_participants = channels.entry(key.to_owned()).or_default();
    if !channel_participants.contains_key(&user_id)
        && channel_participants.len() >= max_participants_per_channel
    {
        return Err(AuthFailure::RateLimited);
    }

    let next_streams: HashSet<VoiceStreamKind> = publish_streams.iter().copied().collect();
    let next_video = next_streams.contains(&VoiceStreamKind::Camera);
    let next_screen = next_streams.contains(&VoiceStreamKind::ScreenShare);
    if let Some(existing) = channel_participants.get_mut(&user_id) {
        let prev_streams = existing.published_streams.clone();
        for stream in next_streams.difference(&prev_streams) {
            newly_published.push(*stream);
        }
        for stream in prev_streams.difference(&next_streams) {
            unpublished.push(*stream);
        }
        identity.clone_into(&mut existing.identity);
        existing.updated_at_unix = now_unix;
        existing.expires_at_unix = expires_at_unix;
        existing.is_video_enabled = next_video;
        existing.is_screen_share_enabled = next_screen;
        existing.published_streams = next_streams;
        updated = Some(existing.clone());
    } else {
        let participant = VoiceParticipant {
            user_id,
            identity: identity.to_owned(),
            joined_at_unix: now_unix,
            updated_at_unix: now_unix,
            expires_at_unix,
            is_muted: false,
            is_deafened: false,
            is_speaking: false,
            is_video_enabled: next_video,
            is_screen_share_enabled: next_screen,
            published_streams: next_streams.clone(),
        };
        joined = Some(participant.clone());
        newly_published.extend(next_streams);
        channel_participants.insert(user_id, participant);
    }

    Ok(VoiceRegistrationTransition {
        removed,
        joined,
        updated,
        newly_published,
        unpublished,
    })
}

pub(crate) fn plan_voice_registration_events(
    transition: VoiceRegistrationTransition,
    guild_id: &str,
    channel_id: &str,
    user_id: UserId,
    identity: &str,
    event_at_unix: i64,
) -> Result<Vec<(String, GatewayEvent)>, VoiceRegistrationEventBuildError> {
    let mut planned = Vec::new();

    for (old_key, participant) in transition.removed {
        let Some((old_guild_id, old_channel_id)) = old_key.split_once(':') else {
            continue;
        };
        let subscription_key = channel_key(old_guild_id, old_channel_id);
        for stream in participant.published_streams {
            let event = gateway_events::try_voice_stream_unpublish(
                old_guild_id,
                old_channel_id,
                participant.user_id,
                &participant.identity,
                stream,
                event_at_unix,
            )
            .map_err(|source| VoiceRegistrationEventBuildError {
                event_type: gateway_events::VOICE_STREAM_UNPUBLISH_EVENT,
                source,
            })?;
            planned.push((subscription_key.clone(), event));
        }
        let event = gateway_events::try_voice_participant_leave(
            old_guild_id,
            old_channel_id,
            participant.user_id,
            &participant.identity,
            event_at_unix,
        )
        .map_err(|source| VoiceRegistrationEventBuildError {
            event_type: gateway_events::VOICE_PARTICIPANT_LEAVE_EVENT,
            source,
        })?;
        planned.push((subscription_key, event));
    }

    let subscription_key = channel_key(guild_id, channel_id);
    if let Some(participant) = transition.joined {
        let event = gateway_events::try_voice_participant_join(
            guild_id,
            channel_id,
            super::voice_presence::voice_snapshot_from_record(&participant),
        )
        .map_err(|source| VoiceRegistrationEventBuildError {
            event_type: gateway_events::VOICE_PARTICIPANT_JOIN_EVENT,
            source,
        })?;
        planned.push((subscription_key.clone(), event));
    }
    if let Some(participant) = transition.updated {
        let event = gateway_events::try_voice_participant_update(
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
        )
        .map_err(|source| VoiceRegistrationEventBuildError {
            event_type: gateway_events::VOICE_PARTICIPANT_UPDATE_EVENT,
            source,
        })?;
        planned.push((subscription_key.clone(), event));
    }
    for stream in transition.unpublished {
        let event = gateway_events::try_voice_stream_unpublish(
            guild_id,
            channel_id,
            user_id,
            identity,
            stream,
            event_at_unix,
        )
        .map_err(|source| VoiceRegistrationEventBuildError {
            event_type: gateway_events::VOICE_STREAM_UNPUBLISH_EVENT,
            source,
        })?;
        planned.push((subscription_key.clone(), event));
    }
    for stream in transition.newly_published {
        let event = gateway_events::try_voice_stream_publish(
            guild_id,
            channel_id,
            user_id,
            identity,
            stream,
            event_at_unix,
        )
        .map_err(|source| VoiceRegistrationEventBuildError {
            event_type: gateway_events::VOICE_STREAM_PUBLISH_EVENT,
            source,
        })?;
        planned.push((subscription_key.clone(), event));
    }

    Ok(planned)
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use filament_core::UserId;

    use super::{apply_voice_registration_transition, plan_voice_registration_events};
    use crate::server::{
        core::{VoiceParticipant, VoiceParticipantsByChannel, VoiceStreamKind},
        errors::AuthFailure,
        gateway_events::{
            VOICE_PARTICIPANT_JOIN_EVENT, VOICE_PARTICIPANT_LEAVE_EVENT,
            VOICE_PARTICIPANT_UPDATE_EVENT, VOICE_STREAM_PUBLISH_EVENT,
            VOICE_STREAM_UNPUBLISH_EVENT,
        },
    };

    fn participant(
        user_id: UserId,
        identity: &str,
        streams: HashSet<VoiceStreamKind>,
    ) -> VoiceParticipant {
        VoiceParticipant {
            user_id,
            identity: identity.to_owned(),
            joined_at_unix: 10,
            updated_at_unix: 10,
            expires_at_unix: 100,
            is_muted: false,
            is_deafened: false,
            is_speaking: false,
            is_video_enabled: streams.contains(&VoiceStreamKind::Camera),
            is_screen_share_enabled: streams.contains(&VoiceStreamKind::ScreenShare),
            published_streams: streams,
        }
    }

    #[test]
    fn rejects_when_max_tracked_channels_reached_for_new_channel() {
        let existing_user = UserId::new();
        let joining_user = UserId::new();
        let mut channels: VoiceParticipantsByChannel = HashMap::from([(
            String::from("g1:c1"),
            HashMap::from([(
                existing_user,
                participant(
                    existing_user,
                    "existing",
                    HashSet::from([VoiceStreamKind::Microphone]),
                ),
            )]),
        )]);

        let result = apply_voice_registration_transition(
            &mut channels,
            "g1:c2",
            joining_user,
            "joining",
            &[VoiceStreamKind::Microphone],
            200,
            20,
            1,
            10,
        );

        assert!(matches!(result, Err(AuthFailure::RateLimited)));
    }

    #[test]
    fn rejects_when_channel_participant_cap_reached_for_new_user() {
        let existing_user = UserId::new();
        let joining_user = UserId::new();
        let mut channels: VoiceParticipantsByChannel = HashMap::from([(
            String::from("g1:c1"),
            HashMap::from([(
                existing_user,
                participant(
                    existing_user,
                    "existing",
                    HashSet::from([VoiceStreamKind::Microphone]),
                ),
            )]),
        )]);

        let result = apply_voice_registration_transition(
            &mut channels,
            "g1:c1",
            joining_user,
            "joining",
            &[VoiceStreamKind::Microphone],
            200,
            20,
            10,
            1,
        );

        assert!(matches!(result, Err(AuthFailure::RateLimited)));
    }

    #[test]
    fn moves_user_from_previous_channel_and_marks_joined_streams() {
        let target_user = UserId::new();
        let mut channels: VoiceParticipantsByChannel = HashMap::from([
            (
                String::from("g1:c1"),
                HashMap::from([(
                    target_user,
                    participant(
                        target_user,
                        "old",
                        HashSet::from([VoiceStreamKind::Microphone]),
                    ),
                )]),
            ),
            (String::from("g1:c2"), HashMap::new()),
        ]);

        let result = apply_voice_registration_transition(
            &mut channels,
            "g1:c2",
            target_user,
            "new",
            &[VoiceStreamKind::Microphone, VoiceStreamKind::Camera],
            300,
            33,
            10,
            10,
        )
        .expect("transition should succeed");

        assert_eq!(result.removed.len(), 1);
        assert_eq!(result.removed[0].0, "g1:c1");
        assert!(result.updated.is_none());
        assert!(result.joined.is_some());
        assert_eq!(result.newly_published.len(), 2);
        assert!(!channels.contains_key("g1:c1"));
        assert!(channels
            .get("g1:c2")
            .and_then(|participants| participants.get(&target_user))
            .is_some());
    }

    #[test]
    fn updates_existing_participant_and_tracks_stream_deltas() {
        let user_id = UserId::new();
        let mut channels: VoiceParticipantsByChannel = HashMap::from([(
            String::from("g1:c1"),
            HashMap::from([(
                user_id,
                participant(
                    user_id,
                    "before",
                    HashSet::from([VoiceStreamKind::Microphone, VoiceStreamKind::ScreenShare]),
                ),
            )]),
        )]);

        let result = apply_voice_registration_transition(
            &mut channels,
            "g1:c1",
            user_id,
            "after",
            &[VoiceStreamKind::Microphone, VoiceStreamKind::Camera],
            500,
            44,
            10,
            10,
        )
        .expect("transition should succeed");

        assert!(result.joined.is_none());
        assert!(result.updated.is_some());
        assert_eq!(result.newly_published, vec![VoiceStreamKind::Camera]);
        assert_eq!(result.unpublished, vec![VoiceStreamKind::ScreenShare]);
        let updated = channels
            .get("g1:c1")
            .and_then(|participants| participants.get(&user_id))
            .expect("participant should exist");
        assert_eq!(updated.identity, "after");
        assert_eq!(updated.updated_at_unix, 44);
        assert!(updated.published_streams.contains(&VoiceStreamKind::Camera));
        assert!(!updated
            .published_streams
            .contains(&VoiceStreamKind::ScreenShare));
    }

    #[test]
    fn plans_removed_current_and_stream_delta_events_and_skips_malformed_old_key() {
        let current_user = UserId::new();
        let removed_user = UserId::new();
        let join_user = UserId::new();
        let update_user = UserId::new();
        let transition = super::VoiceRegistrationTransition {
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
            plan_voice_registration_events(transition, "g2", "c2", current_user, "current", 9)
                .expect("voice registration events should serialize");

        assert_eq!(planned.len(), 7);
        let removed_key_events = planned.iter().filter(|(key, _)| key == "g1:c1").count();
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
        let transition = super::VoiceRegistrationTransition {
            removed: Vec::new(),
            joined: None,
            updated: None,
            newly_published: Vec::new(),
            unpublished: Vec::new(),
        };

        let planned = plan_voice_registration_events(transition, "g1", "c1", UserId::new(), "u", 5)
            .expect("voice registration events should serialize");

        assert!(planned.is_empty());
    }
}
