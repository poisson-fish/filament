use std::collections::HashSet;

use filament_core::UserId;

use crate::server::{
    core::{VoiceParticipant, VoiceParticipantsByChannel, VoiceStreamKind},
    errors::AuthFailure,
};

pub(crate) struct VoiceRegistrationTransition {
    pub(crate) removed: Vec<(String, VoiceParticipant)>,
    pub(crate) joined: Option<VoiceParticipant>,
    pub(crate) updated: Option<VoiceParticipant>,
    pub(crate) newly_published: Vec<VoiceStreamKind>,
    pub(crate) unpublished: Vec<VoiceStreamKind>,
}

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
        existing.identity = identity.to_owned();
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

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use filament_core::UserId;

    use super::apply_voice_registration_transition;
    use crate::server::{
        core::{VoiceParticipant, VoiceParticipantsByChannel, VoiceStreamKind},
        errors::AuthFailure,
    };

    fn participant(user_id: UserId, identity: &str, streams: HashSet<VoiceStreamKind>) -> VoiceParticipant {
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
                participant(existing_user, "existing", HashSet::from([VoiceStreamKind::Microphone])),
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
                participant(existing_user, "existing", HashSet::from([VoiceStreamKind::Microphone])),
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
                    participant(target_user, "old", HashSet::from([VoiceStreamKind::Microphone])),
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
        assert!(channels.get("g1:c1").is_none());
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
}
