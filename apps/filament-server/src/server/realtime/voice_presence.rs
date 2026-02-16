use crate::server::{
    core::VoiceParticipant,
    gateway_events::{self, VoiceParticipantSnapshot},
};

pub(crate) fn voice_channel_key(guild_id: &str, channel_id: &str) -> String {
    format!("{guild_id}:{channel_id}")
}

pub(crate) fn voice_snapshot_from_record(
    participant: &VoiceParticipant,
) -> VoiceParticipantSnapshot {
    gateway_events::VoiceParticipantSnapshot {
        user_id: participant.user_id,
        identity: participant.identity.clone(),
        joined_at_unix: participant.joined_at_unix,
        updated_at_unix: participant.updated_at_unix,
        is_muted: participant.is_muted,
        is_deafened: participant.is_deafened,
        is_speaking: participant.is_speaking,
        is_video_enabled: participant.is_video_enabled,
        is_screen_share_enabled: participant.is_screen_share_enabled,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use filament_core::UserId;

    use super::{voice_channel_key, voice_snapshot_from_record};
    use crate::server::core::{VoiceParticipant, VoiceStreamKind};

    #[test]
    fn voice_channel_key_uses_guild_and_channel_namespace() {
        assert_eq!(voice_channel_key("g-main", "c-lobby"), "g-main:c-lobby");
    }

    #[test]
    fn voice_snapshot_preserves_voice_state_fields() {
        let user_id = UserId::new();
        let participant = VoiceParticipant {
            user_id,
            identity: String::from("alice"),
            joined_at_unix: 10,
            updated_at_unix: 42,
            expires_at_unix: 99,
            is_muted: true,
            is_deafened: false,
            is_speaking: true,
            is_video_enabled: true,
            is_screen_share_enabled: false,
            published_streams: HashSet::from([VoiceStreamKind::Camera]),
        };

        let snapshot = voice_snapshot_from_record(&participant);
        assert_eq!(snapshot.user_id, user_id);
        assert_eq!(snapshot.identity, "alice");
        assert_eq!(snapshot.joined_at_unix, 10);
        assert_eq!(snapshot.updated_at_unix, 42);
        assert!(snapshot.is_muted);
        assert!(!snapshot.is_deafened);
        assert!(snapshot.is_speaking);
        assert!(snapshot.is_video_enabled);
        assert!(!snapshot.is_screen_share_enabled);
    }
}