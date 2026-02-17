use crate::server::{
    gateway_events::{self, GatewayEvent, VoiceParticipantSnapshot},
};

pub(crate) fn build_voice_subscribe_sync_event(
    guild_id: &str,
    channel_id: &str,
    participants: Vec<VoiceParticipantSnapshot>,
    now_unix: i64,
) -> GatewayEvent {
    gateway_events::voice_participant_sync(guild_id, channel_id, participants, now_unix)
}

#[cfg(test)]
mod tests {
    use filament_core::UserId;

    use super::build_voice_subscribe_sync_event;
    use crate::server::gateway_events::VoiceParticipantSnapshot;

    #[test]
    fn builds_voice_sync_event_with_expected_payload_fields() {
        let participant = VoiceParticipantSnapshot {
            user_id: UserId::new(),
            identity: String::from("alice"),
            joined_at_unix: 10,
            updated_at_unix: 20,
            is_muted: false,
            is_deafened: false,
            is_speaking: true,
            is_video_enabled: false,
            is_screen_share_enabled: false,
        };

        let event = build_voice_subscribe_sync_event(
            "g1",
            "c1",
            vec![participant],
            123,
        );

        assert_eq!(event.event_type, "voice_participant_sync");
        assert!(event.payload.contains("\"guild_id\":\"g1\""));
        assert!(event.payload.contains("\"channel_id\":\"c1\""));
        assert!(event.payload.contains("\"synced_at_unix\":123"));
        assert!(event.payload.contains("\"identity\":\"alice\""));
    }

    #[test]
    fn supports_empty_participant_snapshot() {
        let event = build_voice_subscribe_sync_event("g1", "c1", Vec::new(), 456);

        assert_eq!(event.event_type, "voice_participant_sync");
        assert!(event.payload.contains("\"participants\":[]"));
    }
}
