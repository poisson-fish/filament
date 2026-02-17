use filament_core::UserId;

use crate::server::core::{VoiceParticipant, VoiceParticipantsByChannel};

pub(crate) struct VoiceParticipantRemoval {
    pub(crate) guild_id: String,
    pub(crate) channel_id: String,
    pub(crate) participant: VoiceParticipant,
}

pub(crate) fn take_expired_voice_participant_removals(
    voice: &mut VoiceParticipantsByChannel,
    now_unix: i64,
) -> Vec<VoiceParticipantRemoval> {
    take_expired_voice_participants(voice, now_unix)
        .into_iter()
        .filter_map(|(channel_key, participant)| {
            let (guild_id, channel_id) = channel_key.split_once(':')?;
            Some(VoiceParticipantRemoval {
                guild_id: guild_id.to_owned(),
                channel_id: channel_id.to_owned(),
                participant,
            })
        })
        .collect()
}

pub(crate) fn take_expired_voice_participants(
    voice: &mut VoiceParticipantsByChannel,
    now_unix: i64,
) -> Vec<(String, VoiceParticipant)> {
    let mut removed = Vec::new();
    voice.retain(|channel_key, participants| {
        participants.retain(|_, participant| {
            if participant.expires_at_unix > now_unix {
                return true;
            }
            removed.push((channel_key.clone(), participant.clone()));
            false
        });
        !participants.is_empty()
    });
    removed
}

pub(crate) fn remove_user_voice_participants(
    voice: &mut VoiceParticipantsByChannel,
    user_id: UserId,
) -> Vec<(String, VoiceParticipant)> {
    let mut removed = Vec::new();
    voice.retain(|channel_key, participants| {
        if let Some(participant) = participants.remove(&user_id) {
            removed.push((channel_key.clone(), participant));
        }
        !participants.is_empty()
    });
    removed
}

pub(crate) fn remove_channel_user_voice_participant(
    voice: &mut VoiceParticipantsByChannel,
    guild_id: &str,
    channel_id: &str,
    user_id: UserId,
) -> Option<(String, VoiceParticipant)> {
    let channel_key = format!("{guild_id}:{channel_id}");
    let participants = voice.get_mut(&channel_key)?;
    let participant = participants.remove(&user_id)?;
    if participants.is_empty() {
        voice.remove(&channel_key);
    }
    Some((channel_key, participant))
}

pub(crate) fn remove_user_voice_participant_removals(
    voice: &mut VoiceParticipantsByChannel,
    user_id: UserId,
) -> Vec<VoiceParticipantRemoval> {
    remove_user_voice_participants(voice, user_id)
        .into_iter()
        .filter_map(|(channel_key, participant)| {
            let (guild_id, channel_id) = channel_key.split_once(':')?;
            Some(VoiceParticipantRemoval {
                guild_id: guild_id.to_owned(),
                channel_id: channel_id.to_owned(),
                participant,
            })
        })
        .collect()
}

pub(crate) fn remove_channel_user_voice_participant_removal(
    voice: &mut VoiceParticipantsByChannel,
    guild_id: &str,
    channel_id: &str,
    user_id: UserId,
) -> Option<VoiceParticipantRemoval> {
    remove_channel_user_voice_participant(voice, guild_id, channel_id, user_id).map(
        |(_, participant)| VoiceParticipantRemoval {
            guild_id: guild_id.to_owned(),
            channel_id: channel_id.to_owned(),
            participant,
        },
    )
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use filament_core::UserId;

    use super::{
        remove_channel_user_voice_participant, remove_channel_user_voice_participant_removal,
        remove_user_voice_participant_removals, remove_user_voice_participants,
        take_expired_voice_participant_removals, take_expired_voice_participants,
    };
    use crate::server::core::{VoiceParticipant, VoiceParticipantsByChannel, VoiceStreamKind};

    fn participant(user_id: UserId, identity: &str, expires_at_unix: i64) -> VoiceParticipant {
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
    fn expired_participants_are_removed_and_empty_channels_pruned() {
        let keep_user = UserId::new();
        let expired_user = UserId::new();
        let mut voice: VoiceParticipantsByChannel = HashMap::from([
            (
                String::from("g1:c1"),
                HashMap::from([
                    (keep_user, participant(keep_user, "keep", 20)),
                    (expired_user, participant(expired_user, "expired", 10)),
                ]),
            ),
            (
                String::from("g1:c2"),
                HashMap::from([(expired_user, participant(expired_user, "expired-only", 10))]),
            ),
        ]);

        let removed = take_expired_voice_participants(&mut voice, 10);

        assert_eq!(removed.len(), 2);
        assert!(removed.iter().any(|(key, _)| key == "g1:c1"));
        assert!(removed.iter().any(|(key, _)| key == "g1:c2"));
        assert!(voice.contains_key("g1:c1"));
        assert!(!voice.contains_key("g1:c2"));
        assert_eq!(voice["g1:c1"].len(), 1);
        assert!(voice["g1:c1"].contains_key(&keep_user));
    }

    #[test]
    fn removing_user_participants_prunes_empty_channels() {
        let target_user = UserId::new();
        let other_user = UserId::new();
        let mut voice: VoiceParticipantsByChannel = HashMap::from([
            (
                String::from("g2:c1"),
                HashMap::from([
                    (target_user, participant(target_user, "target", 100)),
                    (other_user, participant(other_user, "other", 100)),
                ]),
            ),
            (
                String::from("g2:c2"),
                HashMap::from([(target_user, participant(target_user, "target-only", 100))]),
            ),
        ]);

        let removed = remove_user_voice_participants(&mut voice, target_user);

        assert_eq!(removed.len(), 2);
        assert!(voice.contains_key("g2:c1"));
        assert!(!voice.contains_key("g2:c2"));
        assert_eq!(voice["g2:c1"].len(), 1);
        assert!(voice["g2:c1"].contains_key(&other_user));
    }

    #[test]
    fn expired_removals_include_only_parseable_channel_keys() {
        let parseable_user = UserId::new();
        let invalid_user = UserId::new();
        let mut voice: VoiceParticipantsByChannel = HashMap::from([
            (
                String::from("g3:c1"),
                HashMap::from([(parseable_user, participant(parseable_user, "parseable", 10))]),
            ),
            (
                String::from("invalid"),
                HashMap::from([(invalid_user, participant(invalid_user, "invalid", 10))]),
            ),
        ]);

        let removals = take_expired_voice_participant_removals(&mut voice, 10);

        assert_eq!(removals.len(), 1);
        assert_eq!(removals[0].guild_id, "g3");
        assert_eq!(removals[0].channel_id, "c1");
        assert_eq!(removals[0].participant.user_id, parseable_user);
        assert!(voice.is_empty());
    }

    #[test]
    fn user_removals_include_only_parseable_channel_keys() {
        let parseable_user = UserId::new();
        let invalid_user = UserId::new();
        let mut voice: VoiceParticipantsByChannel = HashMap::from([
            (
                String::from("g4:c1"),
                HashMap::from([(parseable_user, participant(parseable_user, "parseable", 99))]),
            ),
            (
                String::from("invalid"),
                HashMap::from([(parseable_user, participant(parseable_user, "invalid", 99))]),
            ),
            (
                String::from("g4:c2"),
                HashMap::from([(invalid_user, participant(invalid_user, "other", 99))]),
            ),
        ]);

        let removals = remove_user_voice_participant_removals(&mut voice, parseable_user);

        assert_eq!(removals.len(), 1);
        assert_eq!(removals[0].guild_id, "g4");
        assert_eq!(removals[0].channel_id, "c1");
        assert_eq!(removals[0].participant.user_id, parseable_user);
        assert!(voice.contains_key("g4:c2"));
        assert!(!voice.contains_key("g4:c1"));
        assert!(!voice.contains_key("invalid"));
    }

    #[test]
    fn channel_user_removal_removes_only_target_channel_participant() {
        let target_user = UserId::new();
        let other_user = UserId::new();
        let mut voice: VoiceParticipantsByChannel = HashMap::from([
            (
                String::from("g5:c1"),
                HashMap::from([
                    (target_user, participant(target_user, "target", 120)),
                    (other_user, participant(other_user, "other", 120)),
                ]),
            ),
            (
                String::from("g5:c2"),
                HashMap::from([(target_user, participant(target_user, "target-2", 120))]),
            ),
        ]);

        let removed = remove_channel_user_voice_participant(&mut voice, "g5", "c1", target_user)
            .expect("participant should be removed from the targeted channel");

        assert_eq!(removed.0, "g5:c1");
        assert_eq!(removed.1.user_id, target_user);
        assert_eq!(voice["g5:c1"].len(), 1);
        assert!(voice["g5:c1"].contains_key(&other_user));
        assert_eq!(voice["g5:c2"].len(), 1);
        assert!(voice["g5:c2"].contains_key(&target_user));
    }

    #[test]
    fn channel_user_removal_returns_scoped_removal_payload() {
        let target_user = UserId::new();
        let mut voice: VoiceParticipantsByChannel = HashMap::from([(
            String::from("g6:c1"),
            HashMap::from([(target_user, participant(target_user, "target", 120))]),
        )]);

        let removed = remove_channel_user_voice_participant_removal(
            &mut voice,
            "g6",
            "c1",
            target_user,
        )
        .expect("participant should be removed from scoped channel");

        assert_eq!(removed.guild_id, "g6");
        assert_eq!(removed.channel_id, "c1");
        assert_eq!(removed.participant.user_id, target_user);
        assert!(voice.is_empty());
    }
}
