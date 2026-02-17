use filament_core::UserId;

use crate::server::{
    core::VoiceParticipantsByChannel,
    gateway_events::GatewayEvent,
    realtime::{
        voice_cleanup_events::plan_voice_removal_broadcasts,
        voice_registry::{
            remove_user_voice_participant_removals, take_expired_voice_participant_removals,
        },
    },
};

pub(crate) fn expired_voice_removal_broadcasts(
    voice: &mut VoiceParticipantsByChannel,
    now_unix: i64,
) -> Vec<(String, GatewayEvent)> {
    let removed = take_expired_voice_participant_removals(voice, now_unix);
    plan_voice_removal_broadcasts(removed, now_unix)
}

pub(crate) fn disconnected_user_voice_removal_broadcasts(
    voice: &mut VoiceParticipantsByChannel,
    user_id: UserId,
    disconnected_at_unix: i64,
) -> Vec<(String, GatewayEvent)> {
    let removed = remove_user_voice_participant_removals(voice, user_id);
    plan_voice_removal_broadcasts(removed, disconnected_at_unix)
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use filament_core::UserId;

    use super::{disconnected_user_voice_removal_broadcasts, expired_voice_removal_broadcasts};
    use crate::server::{
        core::{VoiceParticipant, VoiceParticipantsByChannel, VoiceStreamKind},
        gateway_events::{VOICE_PARTICIPANT_LEAVE_EVENT, VOICE_STREAM_UNPUBLISH_EVENT},
    };

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
    fn expired_cleanup_plans_voice_removal_events_for_parseable_channel_keys() {
        let expiring_user = UserId::new();
        let mut voice: VoiceParticipantsByChannel = HashMap::from([(
            String::from("g1:c1"),
            HashMap::from([(expiring_user, participant(expiring_user, "alice", 5))]),
        )]);

        let planned = expired_voice_removal_broadcasts(&mut voice, 10);

        assert_eq!(planned.len(), 2);
        assert_eq!(planned[0].0, "g1:c1");
        assert_eq!(planned[1].0, "g1:c1");
        assert_eq!(planned[0].1.event_type, VOICE_STREAM_UNPUBLISH_EVENT);
        assert_eq!(planned[1].1.event_type, VOICE_PARTICIPANT_LEAVE_EVENT);
        assert!(voice.is_empty());
    }

    #[test]
    fn expired_cleanup_drops_malformed_channel_key_fail_closed() {
        let expiring_user = UserId::new();
        let mut voice: VoiceParticipantsByChannel = HashMap::from([(
            String::from("invalid"),
            HashMap::from([(expiring_user, participant(expiring_user, "alice", 5))]),
        )]);

        let planned = expired_voice_removal_broadcasts(&mut voice, 10);

        assert!(planned.is_empty());
        assert!(voice.is_empty());
    }

    #[test]
    fn disconnected_cleanup_plans_voice_removal_events_for_target_user_only() {
        let target_user = UserId::new();
        let other_user = UserId::new();
        let mut voice: VoiceParticipantsByChannel = HashMap::from([(
            String::from("g1:c1"),
            HashMap::from([
                (target_user, participant(target_user, "alice", 30)),
                (other_user, participant(other_user, "bob", 30)),
            ]),
        )]);

        let planned = disconnected_user_voice_removal_broadcasts(&mut voice, target_user, 10);

        assert_eq!(planned.len(), 2);
        assert_eq!(planned[0].0, "g1:c1");
        assert_eq!(planned[1].0, "g1:c1");
        assert_eq!(planned[0].1.event_type, VOICE_STREAM_UNPUBLISH_EVENT);
        assert_eq!(planned[1].1.event_type, VOICE_PARTICIPANT_LEAVE_EVENT);
        assert_eq!(voice["g1:c1"].len(), 1);
        assert!(voice["g1:c1"].contains_key(&other_user));
    }

    #[test]
    fn disconnected_cleanup_drops_malformed_channel_key_fail_closed() {
        let target_user = UserId::new();
        let mut voice: VoiceParticipantsByChannel = HashMap::from([(
            String::from("invalid"),
            HashMap::from([(target_user, participant(target_user, "alice", 30))]),
        )]);

        let planned = disconnected_user_voice_removal_broadcasts(&mut voice, target_user, 10);

        assert!(planned.is_empty());
        assert!(voice.is_empty());
    }
}
