use std::collections::HashSet;

use filament_core::UserId;
use serde::Serialize;

use super::{
    envelope::{build_event, try_build_event},
    GatewayEvent,
};
use crate::server::core::VoiceStreamKind;

pub(crate) const PRESENCE_SYNC_EVENT: &str = "presence_sync";
pub(crate) const PRESENCE_UPDATE_EVENT: &str = "presence_update";
pub(crate) const VOICE_PARTICIPANT_SYNC_EVENT: &str = "voice_participant_sync";
pub(crate) const VOICE_PARTICIPANT_JOIN_EVENT: &str = "voice_participant_join";
pub(crate) const VOICE_PARTICIPANT_LEAVE_EVENT: &str = "voice_participant_leave";
pub(crate) const VOICE_PARTICIPANT_UPDATE_EVENT: &str = "voice_participant_update";
pub(crate) const VOICE_STREAM_PUBLISH_EVENT: &str = "voice_stream_publish";
pub(crate) const VOICE_STREAM_UNPUBLISH_EVENT: &str = "voice_stream_unpublish";

#[derive(Serialize)]
struct PresenceSyncPayload {
    guild_id: String,
    user_ids: HashSet<String>,
}

#[derive(Serialize)]
struct PresenceUpdatePayload {
    guild_id: String,
    user_id: String,
    status: &'static str,
}

#[derive(Serialize)]
struct VoiceParticipantSyncPayload {
    guild_id: String,
    channel_id: String,
    participants: Vec<VoiceParticipantPayload>,
    synced_at_unix: i64,
}

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
struct VoiceParticipantPayload {
    user_id: String,
    identity: String,
    joined_at_unix: i64,
    updated_at_unix: i64,
    is_muted: bool,
    is_deafened: bool,
    is_speaking: bool,
    is_video_enabled: bool,
    is_screen_share_enabled: bool,
}

#[derive(Serialize)]
struct VoiceParticipantJoinPayload {
    guild_id: String,
    channel_id: String,
    participant: VoiceParticipantPayload,
}

#[derive(Serialize)]
struct VoiceParticipantLeavePayload {
    guild_id: String,
    channel_id: String,
    user_id: String,
    identity: String,
    left_at_unix: i64,
}

#[derive(Serialize)]
struct VoiceParticipantUpdatePayload {
    guild_id: String,
    channel_id: String,
    user_id: String,
    identity: String,
    updated_fields: VoiceParticipantUpdatedFieldsPayload,
    updated_at_unix: i64,
}

#[derive(Serialize)]
#[allow(clippy::struct_field_names)]
struct VoiceParticipantUpdatedFieldsPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    is_muted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_deafened: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_speaking: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_video_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_screen_share_enabled: Option<bool>,
}

#[derive(Serialize)]
struct VoiceStreamPublishPayload {
    guild_id: String,
    channel_id: String,
    user_id: String,
    identity: String,
    stream: VoiceStreamKind,
    published_at_unix: i64,
}

#[derive(Serialize)]
struct VoiceStreamUnpublishPayload {
    guild_id: String,
    channel_id: String,
    user_id: String,
    identity: String,
    stream: VoiceStreamKind,
    unpublished_at_unix: i64,
}

#[derive(Clone)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct VoiceParticipantSnapshot {
    pub(crate) user_id: UserId,
    pub(crate) identity: String,
    pub(crate) joined_at_unix: i64,
    pub(crate) updated_at_unix: i64,
    pub(crate) is_muted: bool,
    pub(crate) is_deafened: bool,
    pub(crate) is_speaking: bool,
    pub(crate) is_video_enabled: bool,
    pub(crate) is_screen_share_enabled: bool,
}

impl From<VoiceParticipantSnapshot> for VoiceParticipantPayload {
    fn from(value: VoiceParticipantSnapshot) -> Self {
        Self {
            user_id: value.user_id.to_string(),
            identity: value.identity,
            joined_at_unix: value.joined_at_unix,
            updated_at_unix: value.updated_at_unix,
            is_muted: value.is_muted,
            is_deafened: value.is_deafened,
            is_speaking: value.is_speaking,
            is_video_enabled: value.is_video_enabled,
            is_screen_share_enabled: value.is_screen_share_enabled,
        }
    }
}

#[cfg(test)]
pub(crate) fn presence_sync(guild_id: &str, user_ids: HashSet<String>) -> GatewayEvent {
    try_presence_sync(guild_id, user_ids).unwrap_or_else(|error| {
        panic!("failed to build outbound gateway event {PRESENCE_SYNC_EVENT}: {error}")
    })
}

pub(crate) fn try_presence_sync(
    guild_id: &str,
    user_ids: HashSet<String>,
) -> anyhow::Result<GatewayEvent> {
    try_build_event(
        PRESENCE_SYNC_EVENT,
        PresenceSyncPayload {
            guild_id: guild_id.to_owned(),
            user_ids,
        },
    )
}

pub(crate) fn try_presence_update(
    guild_id: &str,
    user_id: UserId,
    status: &'static str,
) -> anyhow::Result<GatewayEvent> {
    try_build_event(
        PRESENCE_UPDATE_EVENT,
        PresenceUpdatePayload {
            guild_id: guild_id.to_owned(),
            user_id: user_id.to_string(),
            status,
        },
    )
}

pub(crate) fn try_voice_participant_sync(
    guild_id: &str,
    channel_id: &str,
    participants: Vec<VoiceParticipantSnapshot>,
    synced_at_unix: i64,
) -> anyhow::Result<GatewayEvent> {
    try_build_event(
        VOICE_PARTICIPANT_SYNC_EVENT,
        VoiceParticipantSyncPayload {
            guild_id: guild_id.to_owned(),
            channel_id: channel_id.to_owned(),
            participants: participants
                .into_iter()
                .map(VoiceParticipantPayload::from)
                .collect(),
            synced_at_unix,
        },
    )
}

#[cfg(test)]
pub(crate) fn voice_participant_sync(
    guild_id: &str,
    channel_id: &str,
    participants: Vec<VoiceParticipantSnapshot>,
    synced_at_unix: i64,
) -> GatewayEvent {
    try_voice_participant_sync(guild_id, channel_id, participants, synced_at_unix).unwrap_or_else(
        |error| {
            panic!("failed to build outbound gateway event {VOICE_PARTICIPANT_SYNC_EVENT}: {error}")
        },
    )
}

pub(crate) fn voice_participant_join(
    guild_id: &str,
    channel_id: &str,
    participant: VoiceParticipantSnapshot,
) -> GatewayEvent {
    build_event(
        VOICE_PARTICIPANT_JOIN_EVENT,
        VoiceParticipantJoinPayload {
            guild_id: guild_id.to_owned(),
            channel_id: channel_id.to_owned(),
            participant: VoiceParticipantPayload::from(participant),
        },
    )
}

pub(crate) fn voice_participant_leave(
    guild_id: &str,
    channel_id: &str,
    user_id: UserId,
    identity: &str,
    left_at_unix: i64,
) -> GatewayEvent {
    build_event(
        VOICE_PARTICIPANT_LEAVE_EVENT,
        VoiceParticipantLeavePayload {
            guild_id: guild_id.to_owned(),
            channel_id: channel_id.to_owned(),
            user_id: user_id.to_string(),
            identity: identity.to_owned(),
            left_at_unix,
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn voice_participant_update(
    guild_id: &str,
    channel_id: &str,
    user_id: UserId,
    identity: &str,
    is_muted: Option<bool>,
    is_deafened: Option<bool>,
    is_speaking: Option<bool>,
    is_video_enabled: Option<bool>,
    is_screen_share_enabled: Option<bool>,
    updated_at_unix: i64,
) -> GatewayEvent {
    build_event(
        VOICE_PARTICIPANT_UPDATE_EVENT,
        VoiceParticipantUpdatePayload {
            guild_id: guild_id.to_owned(),
            channel_id: channel_id.to_owned(),
            user_id: user_id.to_string(),
            identity: identity.to_owned(),
            updated_fields: VoiceParticipantUpdatedFieldsPayload {
                is_muted,
                is_deafened,
                is_speaking,
                is_video_enabled,
                is_screen_share_enabled,
            },
            updated_at_unix,
        },
    )
}

pub(crate) fn voice_stream_publish(
    guild_id: &str,
    channel_id: &str,
    user_id: UserId,
    identity: &str,
    stream: VoiceStreamKind,
    published_at_unix: i64,
) -> GatewayEvent {
    build_event(
        VOICE_STREAM_PUBLISH_EVENT,
        VoiceStreamPublishPayload {
            guild_id: guild_id.to_owned(),
            channel_id: channel_id.to_owned(),
            user_id: user_id.to_string(),
            identity: identity.to_owned(),
            stream,
            published_at_unix,
        },
    )
}

pub(crate) fn voice_stream_unpublish(
    guild_id: &str,
    channel_id: &str,
    user_id: UserId,
    identity: &str,
    stream: VoiceStreamKind,
    unpublished_at_unix: i64,
) -> GatewayEvent {
    build_event(
        VOICE_STREAM_UNPUBLISH_EVENT,
        VoiceStreamUnpublishPayload {
            guild_id: guild_id.to_owned(),
            channel_id: channel_id.to_owned(),
            user_id: user_id.to_string(),
            identity: identity.to_owned(),
            stream,
            unpublished_at_unix,
        },
    )
}

#[cfg(test)]
mod tests {
    use crate::server::core::VoiceStreamKind;
    use serde_json::Value;

    use super::*;

    fn parse_payload(event: &GatewayEvent) -> Value {
        let value: Value =
            serde_json::from_str(&event.payload).expect("gateway event payload should be valid");
        assert_eq!(value["v"], Value::from(1));
        assert_eq!(value["t"], Value::from(event.event_type));
        value["d"].clone()
    }

    fn snapshot(user_id: UserId) -> VoiceParticipantSnapshot {
        VoiceParticipantSnapshot {
            user_id,
            identity: String::from("u.identity"),
            joined_at_unix: 10,
            updated_at_unix: 11,
            is_muted: false,
            is_deafened: false,
            is_speaking: true,
            is_video_enabled: true,
            is_screen_share_enabled: false,
        }
    }

    #[test]
    fn presence_update_event_emits_status_and_user() {
        let user_id = UserId::new();
        let payload = parse_payload(
            &try_presence_update("guild-1", user_id, "online")
                .expect("presence_update should serialize"),
        );
        assert_eq!(payload["guild_id"], Value::from("guild-1"));
        assert_eq!(payload["user_id"], Value::from(user_id.to_string()));
        assert_eq!(payload["status"], Value::from("online"));
    }

    #[test]
    fn voice_participant_sync_event_emits_participant_fields() {
        let user_id = UserId::new();
        let payload = parse_payload(&voice_participant_sync(
            "guild-1",
            "channel-1",
            vec![snapshot(user_id)],
            99,
        ));
        assert_eq!(
            payload["participants"][0]["user_id"],
            Value::from(user_id.to_string())
        );
        assert_eq!(
            payload["participants"][0]["is_video_enabled"],
            Value::from(true)
        );
    }

    #[test]
    fn voice_stream_publish_event_emits_stream_kind() {
        let user_id = UserId::new();
        let payload = parse_payload(&voice_stream_publish(
            "guild-1",
            "channel-1",
            user_id,
            "u.identity",
            VoiceStreamKind::ScreenShare,
            123,
        ));
        assert_eq!(payload["stream"], Value::from("screen_share"));
        assert_eq!(payload["published_at_unix"], Value::from(123));
    }
}
