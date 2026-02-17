use filament_core::UserId;
use serde::Serialize;

mod connection;
mod envelope;
mod message_channel;
mod presence_voice;

use super::core::GuildVisibility;

pub(crate) use envelope::GatewayEvent;
use envelope::build_event;
pub(crate) use connection::{ready, subscribed};
pub(crate) use message_channel::{
    channel_create,
    message_create,
    message_delete,
    message_reaction,
    message_update,
};
pub(crate) use presence_voice::{
    presence_sync,
    presence_update,
    voice_participant_join,
    voice_participant_leave,
    voice_participant_sync,
    voice_participant_update,
    voice_stream_publish,
    voice_stream_unpublish,
    VoiceParticipantSnapshot,
};
#[cfg(test)]
pub(crate) use presence_voice::{
    VOICE_PARTICIPANT_JOIN_EVENT,
    VOICE_PARTICIPANT_LEAVE_EVENT,
    VOICE_PARTICIPANT_UPDATE_EVENT,
    VOICE_STREAM_PUBLISH_EVENT,
    VOICE_STREAM_UNPUBLISH_EVENT,
};
pub(crate) const WORKSPACE_UPDATE_EVENT: &str = "workspace_update";
pub(crate) const WORKSPACE_MEMBER_ADD_EVENT: &str = "workspace_member_add";
pub(crate) const WORKSPACE_MEMBER_UPDATE_EVENT: &str = "workspace_member_update";
pub(crate) const WORKSPACE_MEMBER_REMOVE_EVENT: &str = "workspace_member_remove";
pub(crate) const WORKSPACE_MEMBER_BAN_EVENT: &str = "workspace_member_ban";
pub(crate) const WORKSPACE_ROLE_CREATE_EVENT: &str = "workspace_role_create";
pub(crate) const WORKSPACE_ROLE_UPDATE_EVENT: &str = "workspace_role_update";
pub(crate) const WORKSPACE_ROLE_DELETE_EVENT: &str = "workspace_role_delete";
pub(crate) const WORKSPACE_ROLE_REORDER_EVENT: &str = "workspace_role_reorder";
pub(crate) const WORKSPACE_ROLE_ASSIGNMENT_ADD_EVENT: &str = "workspace_role_assignment_add";
pub(crate) const WORKSPACE_ROLE_ASSIGNMENT_REMOVE_EVENT: &str = "workspace_role_assignment_remove";
pub(crate) const WORKSPACE_CHANNEL_OVERRIDE_UPDATE_EVENT: &str =
    "workspace_channel_override_update";
pub(crate) const WORKSPACE_IP_BAN_SYNC_EVENT: &str = "workspace_ip_ban_sync";
pub(crate) const PROFILE_UPDATE_EVENT: &str = "profile_update";
pub(crate) const PROFILE_AVATAR_UPDATE_EVENT: &str = "profile_avatar_update";
pub(crate) const FRIEND_REQUEST_CREATE_EVENT: &str = "friend_request_create";
pub(crate) const FRIEND_REQUEST_UPDATE_EVENT: &str = "friend_request_update";
pub(crate) const FRIEND_REQUEST_DELETE_EVENT: &str = "friend_request_delete";
pub(crate) const FRIEND_REMOVE_EVENT: &str = "friend_remove";

#[derive(Serialize)]
struct WorkspaceUpdatePayload<'a> {
    guild_id: &'a str,
    updated_fields: WorkspaceUpdateFieldsPayload<'a>,
    updated_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceUpdateFieldsPayload<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    visibility: Option<GuildVisibility>,
}

#[derive(Serialize)]
struct WorkspaceMemberAddPayload {
    guild_id: String,
    user_id: String,
    role: filament_core::Role,
    joined_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceMemberUpdatePayload {
    guild_id: String,
    user_id: String,
    updated_fields: WorkspaceMemberUpdateFieldsPayload,
    updated_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceMemberUpdateFieldsPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<filament_core::Role>,
}

#[derive(Serialize)]
struct WorkspaceMemberRemovePayload {
    guild_id: String,
    user_id: String,
    reason: &'static str,
    removed_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceMemberBanPayload {
    guild_id: String,
    user_id: String,
    banned_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceRolePayload {
    role_id: String,
    name: String,
    position: i32,
    is_system: bool,
    permissions: Vec<filament_core::Permission>,
}

#[derive(Serialize)]
struct WorkspaceRoleCreatePayload {
    guild_id: String,
    role: WorkspaceRolePayload,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceRoleUpdatePayload {
    guild_id: String,
    role_id: String,
    updated_fields: WorkspaceRoleUpdateFieldsPayload,
    updated_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceRoleUpdateFieldsPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    permissions: Option<Vec<filament_core::Permission>>,
}

#[derive(Serialize)]
struct WorkspaceRoleDeletePayload {
    guild_id: String,
    role_id: String,
    deleted_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceRoleReorderPayload {
    guild_id: String,
    role_ids: Vec<String>,
    updated_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceRoleAssignmentPayload {
    guild_id: String,
    user_id: String,
    role_id: String,
    assigned_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceRoleAssignmentRemovePayload {
    guild_id: String,
    user_id: String,
    role_id: String,
    removed_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceChannelOverrideUpdatePayload {
    guild_id: String,
    channel_id: String,
    role: filament_core::Role,
    updated_fields: WorkspaceChannelOverrideFieldsPayload,
    updated_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceChannelOverrideFieldsPayload {
    allow: Vec<filament_core::Permission>,
    deny: Vec<filament_core::Permission>,
}

#[derive(Serialize)]
struct WorkspaceIpBanSyncPayload {
    guild_id: String,
    summary: WorkspaceIpBanSyncSummaryPayload,
    updated_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceIpBanSyncSummaryPayload {
    action: &'static str,
    changed_count: usize,
}

#[derive(Serialize)]
struct ProfileUpdatePayload<'a> {
    user_id: &'a str,
    updated_fields: ProfileUpdateFieldsPayload<'a>,
    updated_at_unix: i64,
}

#[derive(Serialize)]
struct ProfileUpdateFieldsPayload<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    about_markdown: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    about_markdown_tokens: Option<&'a [filament_core::MarkdownToken]>,
}

#[derive(Serialize)]
struct ProfileAvatarUpdatePayload<'a> {
    user_id: &'a str,
    avatar_version: i64,
    updated_at_unix: i64,
}

#[derive(Serialize)]
struct FriendRequestCreatePayload<'a> {
    request_id: &'a str,
    sender_user_id: &'a str,
    sender_username: &'a str,
    recipient_user_id: &'a str,
    recipient_username: &'a str,
    created_at_unix: i64,
}

#[derive(Serialize)]
struct FriendRequestUpdatePayload<'a> {
    request_id: &'a str,
    state: &'static str,
    user_id: &'a str,
    friend_user_id: &'a str,
    friend_username: &'a str,
    friendship_created_at_unix: i64,
    updated_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct FriendRequestDeletePayload<'a> {
    request_id: &'a str,
    deleted_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct FriendRemovePayload<'a> {
    user_id: &'a str,
    friend_user_id: &'a str,
    removed_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

pub(crate) fn workspace_update(
    guild_id: &str,
    name: Option<&str>,
    visibility: Option<GuildVisibility>,
    updated_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        WORKSPACE_UPDATE_EVENT,
        WorkspaceUpdatePayload {
            guild_id,
            updated_fields: WorkspaceUpdateFieldsPayload { name, visibility },
            updated_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn workspace_member_add(
    guild_id: &str,
    user_id: UserId,
    role: filament_core::Role,
    joined_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        WORKSPACE_MEMBER_ADD_EVENT,
        WorkspaceMemberAddPayload {
            guild_id: guild_id.to_owned(),
            user_id: user_id.to_string(),
            role,
            joined_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn workspace_member_update(
    guild_id: &str,
    user_id: UserId,
    role: Option<filament_core::Role>,
    updated_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        WORKSPACE_MEMBER_UPDATE_EVENT,
        WorkspaceMemberUpdatePayload {
            guild_id: guild_id.to_owned(),
            user_id: user_id.to_string(),
            updated_fields: WorkspaceMemberUpdateFieldsPayload { role },
            updated_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn workspace_member_remove(
    guild_id: &str,
    user_id: UserId,
    reason: &'static str,
    removed_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        WORKSPACE_MEMBER_REMOVE_EVENT,
        WorkspaceMemberRemovePayload {
            guild_id: guild_id.to_owned(),
            user_id: user_id.to_string(),
            reason,
            removed_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn workspace_member_ban(
    guild_id: &str,
    user_id: UserId,
    banned_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        WORKSPACE_MEMBER_BAN_EVENT,
        WorkspaceMemberBanPayload {
            guild_id: guild_id.to_owned(),
            user_id: user_id.to_string(),
            banned_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn workspace_role_create(
    guild_id: &str,
    role_id: &str,
    name: &str,
    position: i32,
    is_system: bool,
    permissions: Vec<filament_core::Permission>,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        WORKSPACE_ROLE_CREATE_EVENT,
        WorkspaceRoleCreatePayload {
            guild_id: guild_id.to_owned(),
            role: WorkspaceRolePayload {
                role_id: role_id.to_owned(),
                name: name.to_owned(),
                position,
                is_system,
                permissions,
            },
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn workspace_role_update(
    guild_id: &str,
    role_id: &str,
    name: Option<&str>,
    permissions: Option<Vec<filament_core::Permission>>,
    updated_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        WORKSPACE_ROLE_UPDATE_EVENT,
        WorkspaceRoleUpdatePayload {
            guild_id: guild_id.to_owned(),
            role_id: role_id.to_owned(),
            updated_fields: WorkspaceRoleUpdateFieldsPayload {
                name: name.map(ToOwned::to_owned),
                permissions,
            },
            updated_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn workspace_role_delete(
    guild_id: &str,
    role_id: &str,
    deleted_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        WORKSPACE_ROLE_DELETE_EVENT,
        WorkspaceRoleDeletePayload {
            guild_id: guild_id.to_owned(),
            role_id: role_id.to_owned(),
            deleted_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn workspace_role_reorder(
    guild_id: &str,
    role_ids: Vec<String>,
    updated_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        WORKSPACE_ROLE_REORDER_EVENT,
        WorkspaceRoleReorderPayload {
            guild_id: guild_id.to_owned(),
            role_ids,
            updated_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn workspace_role_assignment_add(
    guild_id: &str,
    user_id: UserId,
    role_id: &str,
    assigned_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        WORKSPACE_ROLE_ASSIGNMENT_ADD_EVENT,
        WorkspaceRoleAssignmentPayload {
            guild_id: guild_id.to_owned(),
            user_id: user_id.to_string(),
            role_id: role_id.to_owned(),
            assigned_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn workspace_role_assignment_remove(
    guild_id: &str,
    user_id: UserId,
    role_id: &str,
    removed_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        WORKSPACE_ROLE_ASSIGNMENT_REMOVE_EVENT,
        WorkspaceRoleAssignmentRemovePayload {
            guild_id: guild_id.to_owned(),
            user_id: user_id.to_string(),
            role_id: role_id.to_owned(),
            removed_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn workspace_channel_override_update(
    guild_id: &str,
    channel_id: &str,
    role: filament_core::Role,
    allow: Vec<filament_core::Permission>,
    deny: Vec<filament_core::Permission>,
    updated_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        WORKSPACE_CHANNEL_OVERRIDE_UPDATE_EVENT,
        WorkspaceChannelOverrideUpdatePayload {
            guild_id: guild_id.to_owned(),
            channel_id: channel_id.to_owned(),
            role,
            updated_fields: WorkspaceChannelOverrideFieldsPayload { allow, deny },
            updated_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn workspace_ip_ban_sync(
    guild_id: &str,
    action: &'static str,
    changed_count: usize,
    updated_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        WORKSPACE_IP_BAN_SYNC_EVENT,
        WorkspaceIpBanSyncPayload {
            guild_id: guild_id.to_owned(),
            summary: WorkspaceIpBanSyncSummaryPayload {
                action,
                changed_count,
            },
            updated_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn profile_update(
    user_id: &str,
    username: Option<&str>,
    about_markdown: Option<&str>,
    about_markdown_tokens: Option<&[filament_core::MarkdownToken]>,
    updated_at_unix: i64,
) -> GatewayEvent {
    build_event(
        PROFILE_UPDATE_EVENT,
        ProfileUpdatePayload {
            user_id,
            updated_fields: ProfileUpdateFieldsPayload {
                username,
                about_markdown,
                about_markdown_tokens,
            },
            updated_at_unix,
        },
    )
}

pub(crate) fn profile_avatar_update(
    user_id: &str,
    avatar_version: i64,
    updated_at_unix: i64,
) -> GatewayEvent {
    build_event(
        PROFILE_AVATAR_UPDATE_EVENT,
        ProfileAvatarUpdatePayload {
            user_id,
            avatar_version,
            updated_at_unix,
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn friend_request_create(
    request_id: &str,
    sender_user_id: &str,
    sender_username: &str,
    recipient_user_id: &str,
    recipient_username: &str,
    created_at_unix: i64,
) -> GatewayEvent {
    build_event(
        FRIEND_REQUEST_CREATE_EVENT,
        FriendRequestCreatePayload {
            request_id,
            sender_user_id,
            sender_username,
            recipient_user_id,
            recipient_username,
            created_at_unix,
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn friend_request_update(
    request_id: &str,
    user_id: &str,
    friend_user_id: &str,
    friend_username: &str,
    friendship_created_at_unix: i64,
    updated_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        FRIEND_REQUEST_UPDATE_EVENT,
        FriendRequestUpdatePayload {
            request_id,
            state: "accepted",
            user_id,
            friend_user_id,
            friend_username,
            friendship_created_at_unix,
            updated_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn friend_request_delete(
    request_id: &str,
    deleted_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        FRIEND_REQUEST_DELETE_EVENT,
        FriendRequestDeletePayload {
            request_id,
            deleted_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn friend_remove(
    user_id: &str,
    friend_user_id: &str,
    removed_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        FRIEND_REMOVE_EVENT,
        FriendRemovePayload {
            user_id,
            friend_user_id,
            removed_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

#[cfg(test)]
mod tests {
    use filament_core::{ChannelKind, MarkdownToken, Permission, Role, UserId};
    use serde_json::Value;

    use super::*;
    use crate::server::core::VoiceStreamKind;
    use crate::server::types::{ChannelResponse, MessageResponse};

    fn parse_event(event: &GatewayEvent) -> Value {
        let value: Value =
            serde_json::from_str(&event.payload).expect("gateway event payload should be json");
        assert_eq!(value["v"], Value::from(1));
        assert_eq!(value["t"], Value::from(event.event_type));
        assert!(value["d"].is_object());
        value["d"].clone()
    }

    fn contains_ip_field(value: &Value) -> bool {
        match value {
            Value::Object(map) => map.iter().any(|(key, nested)| {
                key == "ip"
                    || key == "ip_cidr"
                    || key == "ip_network"
                    || key == "source_ip"
                    || key == "address"
                    || contains_ip_field(nested)
            }),
            Value::Array(values) => values.iter().any(contains_ip_field),
            _ => false,
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn event_builders_emit_contract_payloads() {
        let user_id = UserId::new();
        let friend_id = UserId::new();
        let message = MessageResponse {
            message_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAX"),
            guild_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            channel_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAW"),
            author_id: user_id.to_string(),
            content: String::from("hello"),
            markdown_tokens: vec![MarkdownToken::Text {
                text: String::from("hello"),
            }],
            attachments: Vec::new(),
            reactions: Vec::new(),
            created_at_unix: 10,
        };
        let channel = ChannelResponse {
            channel_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAZ"),
            name: String::from("general"),
            kind: ChannelKind::Text,
        };

        let ready_payload = parse_event(&ready(user_id));
        assert_eq!(ready_payload["user_id"], Value::from(user_id.to_string()));

        let subscribed_payload = parse_event(&subscribed("g", "c"));
        assert_eq!(subscribed_payload["guild_id"], Value::from("g"));
        assert_eq!(subscribed_payload["channel_id"], Value::from("c"));

        let message_create_payload = parse_event(&message_create(&message));
        assert_eq!(
            message_create_payload["message_id"],
            Value::from(message.message_id)
        );

        let message_reaction_payload = parse_event(&message_reaction("g", "c", "m", "👍", 2));
        assert_eq!(message_reaction_payload["count"], Value::from(2));

        let message_update_payload = parse_event(&message_update(
            "g",
            "c",
            "m",
            "updated",
            &[MarkdownToken::Text {
                text: String::from("updated"),
            }],
            11,
        ));
        assert_eq!(
            message_update_payload["updated_fields"]["content"],
            Value::from("updated")
        );
        assert_eq!(message_update_payload["updated_at_unix"], Value::from(11));

        let message_delete_payload = parse_event(&message_delete("g", "c", "m", 12));
        assert_eq!(message_delete_payload["deleted_at_unix"], Value::from(12));

        let channel_create_payload = parse_event(&channel_create("g", &channel));
        assert_eq!(
            channel_create_payload["channel"]["name"],
            Value::from("general")
        );

        let presence_sync_payload = parse_event(&presence_sync(
            "g",
            [user_id.to_string(), friend_id.to_string()]
                .into_iter()
                .collect(),
        ));
        assert!(presence_sync_payload["user_ids"].is_array());

        let presence_update_payload = parse_event(&presence_update("g", user_id, "online"));
        assert_eq!(presence_update_payload["status"], Value::from("online"));

        let voice_sync_payload = parse_event(&voice_participant_sync(
            "g",
            "c",
            vec![VoiceParticipantSnapshot {
                user_id,
                identity: String::from("u.identity"),
                joined_at_unix: 13,
                updated_at_unix: 13,
                is_muted: false,
                is_deafened: false,
                is_speaking: false,
                is_video_enabled: true,
                is_screen_share_enabled: false,
            }],
            13,
        ));
        assert_eq!(voice_sync_payload["guild_id"], Value::from("g"));
        assert_eq!(voice_sync_payload["channel_id"], Value::from("c"));
        assert_eq!(
            voice_sync_payload["participants"][0]["user_id"],
            Value::from(user_id.to_string())
        );
        assert_eq!(
            voice_sync_payload["participants"][0]["is_video_enabled"],
            Value::from(true)
        );

        let voice_join_payload = parse_event(&voice_participant_join(
            "g",
            "c",
            VoiceParticipantSnapshot {
                user_id,
                identity: String::from("u.identity"),
                joined_at_unix: 14,
                updated_at_unix: 14,
                is_muted: false,
                is_deafened: false,
                is_speaking: false,
                is_video_enabled: false,
                is_screen_share_enabled: true,
            },
        ));
        assert_eq!(
            voice_join_payload["participant"]["identity"],
            Value::from("u.identity")
        );
        let voice_leave_payload = parse_event(&voice_participant_leave(
            "g",
            "c",
            user_id,
            "u.identity",
            15,
        ));
        assert_eq!(voice_leave_payload["left_at_unix"], Value::from(15));

        let voice_update_payload = parse_event(&voice_participant_update(
            "g",
            "c",
            user_id,
            "u.identity",
            Some(true),
            Some(false),
            Some(true),
            Some(true),
            Some(false),
            16,
        ));
        assert_eq!(
            voice_update_payload["updated_fields"]["is_speaking"],
            Value::from(true)
        );

        let voice_publish_payload = parse_event(&voice_stream_publish(
            "g",
            "c",
            user_id,
            "u.identity",
            VoiceStreamKind::Camera,
            17,
        ));
        assert_eq!(voice_publish_payload["stream"], Value::from("camera"));
        let voice_unpublish_payload = parse_event(&voice_stream_unpublish(
            "g",
            "c",
            user_id,
            "u.identity",
            VoiceStreamKind::ScreenShare,
            18,
        ));
        assert_eq!(
            voice_unpublish_payload["stream"],
            Value::from("screen_share")
        );

        let workspace_update_payload = parse_event(&workspace_update(
            "g",
            Some("Guild Prime"),
            Some(crate::server::core::GuildVisibility::Public),
            13,
            Some(user_id),
        ));
        assert_eq!(
            workspace_update_payload["updated_fields"]["name"],
            Value::from("Guild Prime")
        );
        assert_eq!(
            workspace_update_payload["updated_fields"]["visibility"],
            Value::from("public")
        );

        let workspace_member_add_payload = parse_event(&workspace_member_add(
            "g",
            friend_id,
            Role::Member,
            14,
            Some(user_id),
        ));
        assert_eq!(workspace_member_add_payload["role"], Value::from("member"));

        let workspace_member_update_payload = parse_event(&workspace_member_update(
            "g",
            friend_id,
            Some(Role::Moderator),
            15,
            Some(user_id),
        ));
        assert_eq!(
            workspace_member_update_payload["updated_fields"]["role"],
            Value::from("moderator")
        );

        let workspace_member_remove_payload = parse_event(&workspace_member_remove(
            "g",
            friend_id,
            "kick",
            16,
            Some(user_id),
        ));
        assert_eq!(
            workspace_member_remove_payload["reason"],
            Value::from("kick")
        );

        let workspace_member_ban_payload =
            parse_event(&workspace_member_ban("g", friend_id, 17, Some(user_id)));
        assert_eq!(
            workspace_member_ban_payload["banned_at_unix"],
            Value::from(17)
        );

        let workspace_role_create_payload = parse_event(&workspace_role_create(
            "g",
            "role-1",
            "ops",
            90,
            false,
            vec![Permission::ManageRoles],
            Some(user_id),
        ));
        assert_eq!(
            workspace_role_create_payload["role"]["name"],
            Value::from("ops")
        );

        let workspace_role_update_payload = parse_event(&workspace_role_update(
            "g",
            "role-1",
            Some("ops-v2"),
            Some(vec![
                Permission::ManageRoles,
                Permission::ManageChannelOverrides,
            ]),
            18,
            Some(user_id),
        ));
        assert_eq!(
            workspace_role_update_payload["updated_fields"]["name"],
            Value::from("ops-v2")
        );

        let workspace_role_delete_payload =
            parse_event(&workspace_role_delete("g", "role-1", 19, Some(user_id)));
        assert_eq!(
            workspace_role_delete_payload["deleted_at_unix"],
            Value::from(19)
        );

        let workspace_role_reorder_payload = parse_event(&workspace_role_reorder(
            "g",
            vec![String::from("role-1"), String::from("role-2")],
            20,
            Some(user_id),
        ));
        assert_eq!(
            workspace_role_reorder_payload["role_ids"][0],
            Value::from("role-1")
        );

        let workspace_assignment_add_payload = parse_event(&workspace_role_assignment_add(
            "g",
            friend_id,
            "role-1",
            21,
            Some(user_id),
        ));
        assert_eq!(
            workspace_assignment_add_payload["assigned_at_unix"],
            Value::from(21)
        );

        let workspace_assignment_remove_payload = parse_event(&workspace_role_assignment_remove(
            "g",
            friend_id,
            "role-1",
            22,
            Some(user_id),
        ));
        assert_eq!(
            workspace_assignment_remove_payload["removed_at_unix"],
            Value::from(22)
        );

        let workspace_override_payload = parse_event(&workspace_channel_override_update(
            "g",
            "c",
            Role::Moderator,
            vec![Permission::CreateMessage],
            vec![Permission::BanMember],
            23,
            Some(user_id),
        ));
        assert_eq!(workspace_override_payload["role"], Value::from("moderator"));
        assert!(workspace_override_payload["updated_fields"]["allow"].is_array());
        assert!(workspace_override_payload["updated_fields"]["deny"].is_array());

        let workspace_ip_ban_payload =
            parse_event(&workspace_ip_ban_sync("g", "upsert", 2, 24, Some(user_id)));
        assert_eq!(
            workspace_ip_ban_payload["summary"]["changed_count"],
            Value::from(2)
        );
        assert!(!contains_ip_field(&workspace_ip_ban_payload));

        let profile_update_payload = parse_event(&profile_update(
            &user_id.to_string(),
            Some("alice"),
            Some("about"),
            Some(&[MarkdownToken::Text {
                text: String::from("about"),
            }]),
            25,
        ));
        assert_eq!(
            profile_update_payload["updated_fields"]["username"],
            Value::from("alice")
        );

        let profile_avatar_payload =
            parse_event(&profile_avatar_update(&user_id.to_string(), 3, 26));
        assert_eq!(profile_avatar_payload["avatar_version"], Value::from(3));

        let friend_request_create_payload = parse_event(&friend_request_create(
            "req-1",
            &user_id.to_string(),
            "alice",
            &friend_id.to_string(),
            "bob",
            27,
        ));
        assert_eq!(
            friend_request_create_payload["recipient_username"],
            Value::from("bob")
        );

        let friend_request_update_payload = parse_event(&friend_request_update(
            "req-1",
            &user_id.to_string(),
            &friend_id.to_string(),
            "bob",
            28,
            29,
            Some(user_id),
        ));
        assert_eq!(
            friend_request_update_payload["state"],
            Value::from("accepted")
        );

        let friend_request_delete_payload =
            parse_event(&friend_request_delete("req-1", 30, Some(user_id)));
        assert_eq!(
            friend_request_delete_payload["deleted_at_unix"],
            Value::from(30)
        );

        let friend_remove_payload = parse_event(&friend_remove(
            &user_id.to_string(),
            &friend_id.to_string(),
            31,
            Some(user_id),
        ));
        assert_eq!(friend_remove_payload["removed_at_unix"], Value::from(31));
    }
}
