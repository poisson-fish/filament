use crate::server::core::VoiceParticipantsByChannel;
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

pub(crate) fn collect_voice_snapshots(
    voice: &VoiceParticipantsByChannel,
    channel_key: &str,
) -> Vec<VoiceParticipantSnapshot> {
    let mut snapshots = Vec::new();
    if let Some(channel_participants) = voice.get(channel_key) {
        snapshots.extend(channel_participants.values().map(voice_snapshot_from_record));
    }
    snapshots.sort_by(|a, b| {
        a.joined_at_unix
            .cmp(&b.joined_at_unix)
            .then(a.identity.cmp(&b.identity))
    });
    snapshots
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use filament_core::UserId;

    use super::{collect_voice_snapshots, voice_channel_key, voice_snapshot_from_record};
    use crate::server::core::{VoiceParticipant, VoiceParticipantsByChannel, VoiceStreamKind};

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

    #[test]
    fn collect_voice_snapshots_returns_sorted_channel_snapshot() {
        let first_user = UserId::new();
        let second_user = UserId::new();
        let mut voice: VoiceParticipantsByChannel = HashMap::new();
        voice.insert(
            String::from("g-main:c-lobby"),
            HashMap::from([
                (
                    first_user,
                    VoiceParticipant {
                        user_id: first_user,
                        identity: String::from("zeta"),
                        joined_at_unix: 20,
                        updated_at_unix: 20,
                        expires_at_unix: 50,
                        is_muted: false,
                        is_deafened: false,
                        is_speaking: false,
                        is_video_enabled: false,
                        is_screen_share_enabled: false,
                        published_streams: HashSet::from([VoiceStreamKind::Microphone]),
                    },
                ),
                (
                    second_user,
                    VoiceParticipant {
                        user_id: second_user,
                        identity: String::from("alpha"),
                        joined_at_unix: 10,
                        updated_at_unix: 10,
                        expires_at_unix: 50,
                        is_muted: false,
                        is_deafened: false,
                        is_speaking: false,
                        is_video_enabled: false,
                        is_screen_share_enabled: false,
                        published_streams: HashSet::from([VoiceStreamKind::Camera]),
                    },
                ),
            ]),
        );

        let snapshots = collect_voice_snapshots(&voice, "g-main:c-lobby");

        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].user_id, second_user);
        assert_eq!(snapshots[1].user_id, first_user);
    }

    #[test]
    fn collect_voice_snapshots_returns_empty_for_missing_channel() {
        let voice: VoiceParticipantsByChannel = HashMap::new();
        let snapshots = collect_voice_snapshots(&voice, "g-main:c-missing");
        assert!(snapshots.is_empty());
    }
}