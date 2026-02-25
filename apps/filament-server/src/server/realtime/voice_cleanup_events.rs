use crate::server::{
    auth::channel_key,
    core::VoiceParticipant,
    gateway_events::{self, GatewayEvent},
    realtime::voice_registry::VoiceParticipantRemoval,
};

#[derive(Debug)]
pub(crate) struct VoiceCleanupEventBuildError {
    pub(crate) event_type: &'static str,
    pub(crate) source: anyhow::Error,
}

pub(crate) fn build_voice_removal_events(
    guild_id: &str,
    channel_id: &str,
    participant: &VoiceParticipant,
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

pub(crate) fn plan_voice_removal_broadcasts(
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use filament_core::UserId;

    use super::{build_voice_removal_events, plan_voice_removal_broadcasts};
    use crate::server::{
        core::{VoiceParticipant, VoiceStreamKind},
        gateway_events::{VOICE_PARTICIPANT_LEAVE_EVENT, VOICE_STREAM_UNPUBLISH_EVENT},
        realtime::voice_registry::VoiceParticipantRemoval,
    };

    fn participant(published_streams: HashSet<VoiceStreamKind>) -> VoiceParticipant {
        VoiceParticipant {
            user_id: UserId::new(),
            identity: String::from("voice-user"),
            joined_at_unix: 5,
            updated_at_unix: 6,
            expires_at_unix: 99,
            is_muted: false,
            is_deafened: false,
            is_speaking: false,
            is_video_enabled: false,
            is_screen_share_enabled: false,
            published_streams,
        }
    }

    #[test]
    fn includes_unpublish_events_then_leave_when_streams_exist() {
        let participant = participant(HashSet::from([
            VoiceStreamKind::Microphone,
            VoiceStreamKind::Camera,
        ]));

        let events = build_voice_removal_events("g1", "c1", &participant, 10)
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
    fn includes_only_leave_when_no_published_streams() {
        let participant = participant(HashSet::new());

        let events = build_voice_removal_events("g1", "c1", &participant, 10)
            .expect("voice removal events should serialize");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, VOICE_PARTICIPANT_LEAVE_EVENT);
    }

    #[test]
    fn plans_channel_scoped_broadcast_pairs_for_each_removal() {
        let first = VoiceParticipantRemoval {
            guild_id: String::from("g1"),
            channel_id: String::from("c1"),
            participant: participant(HashSet::from([VoiceStreamKind::Microphone])),
        };
        let second = VoiceParticipantRemoval {
            guild_id: String::from("g1"),
            channel_id: String::from("c2"),
            participant: participant(HashSet::new()),
        };

        let planned = plan_voice_removal_broadcasts(vec![first, second], 12)
            .expect("voice removal broadcasts should serialize");

        assert_eq!(planned.len(), 3);
        assert_eq!(planned[0].0, "g1:c1");
        assert_eq!(planned[1].0, "g1:c1");
        assert_eq!(planned[2].0, "g1:c2");
        assert_eq!(planned[0].1.event_type, VOICE_STREAM_UNPUBLISH_EVENT);
        assert_eq!(planned[1].1.event_type, VOICE_PARTICIPANT_LEAVE_EVENT);
        assert_eq!(planned[2].1.event_type, VOICE_PARTICIPANT_LEAVE_EVENT);
    }
}
