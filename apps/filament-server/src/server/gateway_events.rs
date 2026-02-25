mod connection;
mod envelope;
mod friend;
mod message_channel;
mod presence_voice;
mod profile;
mod workspace;

#[cfg(test)]
pub(crate) const EMITTED_EVENT_TYPES: &[&str] = &[
    connection::READY_EVENT,
    connection::SUBSCRIBED_EVENT,
    message_channel::MESSAGE_CREATE_EVENT,
    message_channel::MESSAGE_UPDATE_EVENT,
    message_channel::MESSAGE_DELETE_EVENT,
    message_channel::MESSAGE_REACTION_EVENT,
    message_channel::CHANNEL_CREATE_EVENT,
    presence_voice::PRESENCE_SYNC_EVENT,
    presence_voice::PRESENCE_UPDATE_EVENT,
    presence_voice::VOICE_PARTICIPANT_SYNC_EVENT,
    presence_voice::VOICE_PARTICIPANT_JOIN_EVENT,
    presence_voice::VOICE_PARTICIPANT_LEAVE_EVENT,
    presence_voice::VOICE_PARTICIPANT_UPDATE_EVENT,
    presence_voice::VOICE_STREAM_PUBLISH_EVENT,
    presence_voice::VOICE_STREAM_UNPUBLISH_EVENT,
    workspace::WORKSPACE_UPDATE_EVENT,
    workspace::WORKSPACE_MEMBER_ADD_EVENT,
    workspace::WORKSPACE_MEMBER_UPDATE_EVENT,
    workspace::WORKSPACE_MEMBER_REMOVE_EVENT,
    workspace::WORKSPACE_MEMBER_BAN_EVENT,
    workspace::WORKSPACE_ROLE_CREATE_EVENT,
    workspace::WORKSPACE_ROLE_UPDATE_EVENT,
    workspace::WORKSPACE_ROLE_DELETE_EVENT,
    workspace::WORKSPACE_ROLE_REORDER_EVENT,
    workspace::WORKSPACE_ROLE_ASSIGNMENT_ADD_EVENT,
    workspace::WORKSPACE_ROLE_ASSIGNMENT_REMOVE_EVENT,
    workspace::WORKSPACE_CHANNEL_OVERRIDE_UPDATE_EVENT,
    workspace::WORKSPACE_CHANNEL_ROLE_OVERRIDE_UPDATE_EVENT,
    workspace::WORKSPACE_CHANNEL_PERMISSION_OVERRIDE_UPDATE_EVENT,
    workspace::WORKSPACE_IP_BAN_SYNC_EVENT,
    profile::PROFILE_UPDATE_EVENT,
    profile::PROFILE_AVATAR_UPDATE_EVENT,
    friend::FRIEND_REQUEST_CREATE_EVENT,
    friend::FRIEND_REQUEST_UPDATE_EVENT,
    friend::FRIEND_REQUEST_DELETE_EVENT,
    friend::FRIEND_REMOVE_EVENT,
];

pub(crate) use connection::{try_ready, try_subscribed, READY_EVENT, SUBSCRIBED_EVENT};
pub(crate) use envelope::GatewayEvent;
#[cfg(test)]
pub(crate) use friend::friend_request_delete;
#[cfg(test)]
pub(crate) use friend::{friend_remove, friend_request_create, friend_request_update};
pub(crate) use friend::{
    try_friend_remove, try_friend_request_create, try_friend_request_delete,
    try_friend_request_update, FRIEND_REMOVE_EVENT, FRIEND_REQUEST_CREATE_EVENT,
    FRIEND_REQUEST_DELETE_EVENT, FRIEND_REQUEST_UPDATE_EVENT,
};
#[cfg(test)]
pub(crate) use message_channel::channel_create;
#[cfg(test)]
pub(crate) use message_channel::message_reaction;
pub(crate) use message_channel::{
    try_channel_create, try_message_create, try_message_delete, try_message_reaction,
    try_message_update, CHANNEL_CREATE_EVENT, MESSAGE_CREATE_EVENT, MESSAGE_DELETE_EVENT,
    MESSAGE_REACTION_EVENT, MESSAGE_UPDATE_EVENT,
};
#[cfg(test)]
pub(crate) use presence_voice::presence_sync;
pub(crate) use presence_voice::{
    try_presence_sync, try_presence_update, try_voice_participant_join,
    try_voice_participant_leave, try_voice_participant_sync, try_voice_participant_update,
    try_voice_stream_publish, try_voice_stream_unpublish, VoiceParticipantSnapshot,
    PRESENCE_SYNC_EVENT, PRESENCE_UPDATE_EVENT, VOICE_PARTICIPANT_JOIN_EVENT,
    VOICE_PARTICIPANT_LEAVE_EVENT, VOICE_PARTICIPANT_SYNC_EVENT, VOICE_PARTICIPANT_UPDATE_EVENT,
    VOICE_STREAM_PUBLISH_EVENT, VOICE_STREAM_UNPUBLISH_EVENT,
};
#[cfg(test)]
pub(crate) use presence_voice::{
    voice_participant_join, voice_participant_leave, voice_participant_sync,
    voice_participant_update, voice_stream_publish, voice_stream_unpublish,
};
pub(crate) use profile::{
    try_profile_avatar_update, try_profile_update, PROFILE_AVATAR_UPDATE_EVENT,
    PROFILE_UPDATE_EVENT,
};
#[cfg(test)]
pub(crate) use workspace::workspace_role_reorder;
#[cfg(test)]
pub(crate) use workspace::workspace_role_update;
pub(crate) use workspace::{
    try_workspace_channel_override_update, try_workspace_channel_permission_override_update,
    try_workspace_channel_permission_override_update_legacy,
    try_workspace_channel_role_override_update, try_workspace_ip_ban_sync,
    try_workspace_member_add, try_workspace_member_ban, try_workspace_member_remove,
    try_workspace_member_update, try_workspace_role_assignment_add,
    try_workspace_role_assignment_remove, try_workspace_role_create, try_workspace_role_delete,
    try_workspace_role_reorder, try_workspace_role_update, try_workspace_update,
    WorkspaceChannelOverrideFieldsPayload, WORKSPACE_IP_BAN_SYNC_EVENT, WORKSPACE_MEMBER_ADD_EVENT,
    WORKSPACE_MEMBER_BAN_EVENT, WORKSPACE_MEMBER_REMOVE_EVENT, WORKSPACE_MEMBER_UPDATE_EVENT,
    WORKSPACE_ROLE_ASSIGNMENT_ADD_EVENT, WORKSPACE_ROLE_ASSIGNMENT_REMOVE_EVENT,
    WORKSPACE_ROLE_CREATE_EVENT, WORKSPACE_ROLE_DELETE_EVENT, WORKSPACE_ROLE_REORDER_EVENT,
    WORKSPACE_ROLE_UPDATE_EVENT, WORKSPACE_UPDATE_EVENT,
};
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

        let ready_event = try_ready(user_id).expect("ready event should serialize");
        let ready_payload = parse_event(&ready_event);
        assert_eq!(ready_payload["user_id"], Value::from(user_id.to_string()));

        let subscribed_event = try_subscribed("g", "c").expect("subscribed event should serialize");
        let subscribed_payload = parse_event(&subscribed_event);
        assert_eq!(subscribed_payload["guild_id"], Value::from("g"));
        assert_eq!(subscribed_payload["channel_id"], Value::from("c"));

        let message_create_payload =
            parse_event(&try_message_create(&message).expect("message_create should serialize"));
        assert_eq!(
            message_create_payload["message_id"],
            Value::from(message.message_id)
        );

        let message_reaction_payload = parse_event(&message_reaction("g", "c", "m", "👍", 2));
        assert_eq!(message_reaction_payload["count"], Value::from(2));

        let message_update_payload = parse_event(
            &try_message_update(
                "g",
                "c",
                "m",
                "updated",
                &[MarkdownToken::Text {
                    text: String::from("updated"),
                }],
                11,
            )
            .expect("message_update should serialize"),
        );
        assert_eq!(
            message_update_payload["updated_fields"]["content"],
            Value::from("updated")
        );
        assert_eq!(message_update_payload["updated_at_unix"], Value::from(11));

        let message_delete_payload = parse_event(
            &try_message_delete("g", "c", "m", 12).expect("message_delete should serialize"),
        );
        assert_eq!(message_delete_payload["deleted_at_unix"], Value::from(12));

        let channel_create_payload = parse_event(&channel_create("g", &channel));
        assert_eq!(
            channel_create_payload["channel"]["name"],
            Value::from("general")
        );

        let presence_sync_payload = parse_event(
            &try_presence_sync(
                "g",
                [user_id.to_string(), friend_id.to_string()]
                    .into_iter()
                    .collect(),
            )
            .expect("presence_sync should serialize"),
        );
        assert!(presence_sync_payload["user_ids"].is_array());

        let presence_update_payload = parse_event(
            &try_presence_update("g", user_id, "online").expect("presence_update should serialize"),
        );
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

        let workspace_update_payload = parse_event(
            &try_workspace_update(
                "g",
                Some("Guild Prime"),
                Some(crate::server::core::GuildVisibility::Public),
                13,
                Some(user_id),
            )
            .expect("workspace_update should serialize"),
        );
        assert_eq!(
            workspace_update_payload["updated_fields"]["name"],
            Value::from("Guild Prime")
        );
        assert_eq!(
            workspace_update_payload["updated_fields"]["visibility"],
            Value::from("public")
        );

        let workspace_member_add_payload = parse_event(
            &try_workspace_member_add("g", friend_id, Role::Member, 14, Some(user_id))
                .expect("workspace_member_add should serialize"),
        );
        assert_eq!(workspace_member_add_payload["role"], Value::from("member"));

        let workspace_member_update_payload = parse_event(
            &try_workspace_member_update("g", friend_id, Some(Role::Moderator), 15, Some(user_id))
                .expect("workspace_member_update should serialize"),
        );
        assert_eq!(
            workspace_member_update_payload["updated_fields"]["role"],
            Value::from("moderator")
        );

        let workspace_member_remove_payload = parse_event(
            &try_workspace_member_remove("g", friend_id, "kick", 16, Some(user_id))
                .expect("workspace_member_remove should serialize"),
        );
        assert_eq!(
            workspace_member_remove_payload["reason"],
            Value::from("kick")
        );

        let workspace_member_ban_payload = parse_event(
            &try_workspace_member_ban("g", friend_id, 17, Some(user_id))
                .expect("workspace_member_ban should serialize"),
        );
        assert_eq!(
            workspace_member_ban_payload["banned_at_unix"],
            Value::from(17)
        );

        let workspace_role_create_payload = parse_event(
            &try_workspace_role_create(
                "g",
                "role-1",
                "ops",
                90,
                false,
                vec![Permission::ManageRoles],
                Some(String::from("#00AAFF")),
                Some(user_id),
            )
            .expect("workspace_role_create should serialize"),
        );
        assert_eq!(
            workspace_role_create_payload["role"]["name"],
            Value::from("ops")
        );
        assert_eq!(
            workspace_role_create_payload["role"]["color_hex"],
            Value::from("#00AAFF")
        );

        let workspace_role_update_payload = parse_event(&workspace_role_update(
            "g",
            "role-1",
            Some("ops-v2"),
            Some(vec![
                Permission::ManageRoles,
                Permission::ManageChannelOverrides,
            ]),
            Some(Some(String::from("#3366CC"))),
            18,
            Some(user_id),
        ));
        assert_eq!(
            workspace_role_update_payload["updated_fields"]["name"],
            Value::from("ops-v2")
        );
        assert_eq!(
            workspace_role_update_payload["updated_fields"]["color_hex"],
            Value::from("#3366CC")
        );

        let workspace_role_delete_payload = parse_event(
            &try_workspace_role_delete("g", "role-1", 19, Some(user_id))
                .expect("workspace_role_delete should serialize"),
        );
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

        let workspace_assignment_add_payload = parse_event(
            &try_workspace_role_assignment_add("g", friend_id, "role-1", 21, Some(user_id))
                .expect("workspace_role_assignment_add should serialize"),
        );
        assert_eq!(
            workspace_assignment_add_payload["assigned_at_unix"],
            Value::from(21)
        );

        let workspace_assignment_remove_payload = parse_event(
            &try_workspace_role_assignment_remove("g", friend_id, "role-1", 22, Some(user_id))
                .expect("workspace_role_assignment_remove should serialize"),
        );
        assert_eq!(
            workspace_assignment_remove_payload["removed_at_unix"],
            Value::from(22)
        );

        let workspace_override_payload = parse_event(
            &try_workspace_channel_override_update(
                "g",
                "c",
                Role::Moderator,
                vec![Permission::CreateMessage],
                vec![Permission::BanMember],
                23,
                Some(user_id),
            )
            .expect("workspace_channel_override_update should serialize"),
        );
        assert_eq!(workspace_override_payload["role"], Value::from("moderator"));
        assert!(workspace_override_payload["updated_fields"]["allow"].is_array());
        assert!(workspace_override_payload["updated_fields"]["deny"].is_array());

        let workspace_role_override_payload = parse_event(
            &try_workspace_channel_role_override_update(
                "g",
                "c",
                Role::Moderator,
                WorkspaceChannelOverrideFieldsPayload::new(
                    vec![Permission::CreateMessage],
                    vec![Permission::BanMember],
                ),
                24,
                Some(user_id),
            )
            .expect("workspace_channel_role_override_update should serialize"),
        );
        assert_eq!(
            workspace_role_override_payload["role"],
            Value::from("moderator")
        );
        assert!(workspace_role_override_payload["updated_fields"]["allow"].is_array());
        assert!(workspace_role_override_payload["updated_fields"]["deny"].is_array());

        let workspace_permission_override_payload = parse_event(
            &try_workspace_channel_permission_override_update(
                "g",
                "c",
                crate::server::types::PermissionOverrideTargetKind::Member,
                &friend_id.to_string(),
                WorkspaceChannelOverrideFieldsPayload::new(
                    vec![Permission::CreateMessage],
                    vec![Permission::BanMember],
                ),
                25,
                Some(user_id),
            )
            .expect("workspace_channel_permission_override_update should serialize"),
        );
        assert_eq!(
            workspace_permission_override_payload["target_kind"],
            Value::from("member")
        );
        assert_eq!(
            workspace_permission_override_payload["target_id"],
            Value::from(friend_id.to_string())
        );
        assert!(workspace_permission_override_payload["updated_fields"]["allow"].is_array());
        assert!(workspace_permission_override_payload["updated_fields"]["deny"].is_array());

        let workspace_ip_ban_payload = parse_event(
            &try_workspace_ip_ban_sync("g", "upsert", 2, 26, Some(user_id))
                .expect("workspace_ip_ban_sync should serialize"),
        );
        assert_eq!(
            workspace_ip_ban_payload["summary"]["changed_count"],
            Value::from(2)
        );
        assert!(!contains_ip_field(&workspace_ip_ban_payload));

        let profile_update_payload = parse_event(
            &try_profile_update(
                &user_id.to_string(),
                Some("alice"),
                Some("about"),
                Some(&[MarkdownToken::Text {
                    text: String::from("about"),
                }]),
                26,
            )
            .expect("profile_update should serialize"),
        );
        assert_eq!(
            profile_update_payload["updated_fields"]["username"],
            Value::from("alice")
        );

        let profile_avatar_payload = parse_event(
            &try_profile_avatar_update(&user_id.to_string(), 3, 27)
                .expect("profile_avatar_update should serialize"),
        );
        assert_eq!(profile_avatar_payload["avatar_version"], Value::from(3));

        let friend_request_create_payload = parse_event(&friend_request_create(
            "req-1",
            &user_id.to_string(),
            "alice",
            &friend_id.to_string(),
            "bob",
            28,
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
            29,
            30,
            Some(user_id),
        ));
        assert_eq!(
            friend_request_update_payload["state"],
            Value::from("accepted")
        );

        let friend_request_delete_payload =
            parse_event(&friend_request_delete("req-1", 31, Some(user_id)));
        assert_eq!(
            friend_request_delete_payload["deleted_at_unix"],
            Value::from(31)
        );

        let friend_remove_payload = parse_event(&friend_remove(
            &user_id.to_string(),
            &friend_id.to_string(),
            32,
            Some(user_id),
        ));
        assert_eq!(friend_remove_payload["removed_at_unix"], Value::from(32));
    }
}
