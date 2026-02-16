use filament_core::UserId;

use crate::server::core::{VoiceParticipant, VoiceParticipantsByChannel};

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

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use filament_core::UserId;

    use super::{remove_user_voice_participants, take_expired_voice_participants};
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
}