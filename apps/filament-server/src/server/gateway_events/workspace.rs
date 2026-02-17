use filament_core::UserId;
use serde::Serialize;

use super::{envelope::build_event, GatewayEvent};
use crate::server::core::GuildVisibility;

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

#[cfg(test)]
mod tests {
    use filament_core::{Permission, Role, UserId};
    use serde_json::Value;

    use super::*;

    fn parse_payload(event: GatewayEvent) -> Value {
        let value: Value =
            serde_json::from_str(&event.payload).expect("gateway event payload should be valid");
        assert_eq!(value["v"], Value::from(1));
        assert_eq!(value["t"], Value::from(event.event_type));
        value["d"].clone()
    }

    #[test]
    fn workspace_update_event_emits_updated_fields() {
        let payload = parse_payload(workspace_update(
            "guild-1",
            Some("Guild Prime"),
            Some(GuildVisibility::Public),
            123,
            None,
        ));
        assert_eq!(
            payload["updated_fields"]["name"],
            Value::from("Guild Prime")
        );
        assert_eq!(
            payload["updated_fields"]["visibility"],
            Value::from("public")
        );
    }

    #[test]
    fn workspace_role_create_event_emits_role_payload() {
        let user_id = UserId::new();
        let payload = parse_payload(workspace_role_create(
            "guild-1",
            "role-1",
            "ops",
            10,
            false,
            vec![Permission::ManageRoles],
            Some(user_id),
        ));
        assert_eq!(payload["role"]["role_id"], Value::from("role-1"));
        assert_eq!(payload["role"]["name"], Value::from("ops"));
    }

    #[test]
    fn workspace_channel_override_event_emits_allow_and_deny_arrays() {
        let payload = parse_payload(workspace_channel_override_update(
            "guild-1",
            "channel-1",
            Role::Moderator,
            vec![Permission::CreateMessage],
            vec![Permission::BanMember],
            50,
            None,
        ));
        assert_eq!(payload["role"], Value::from("moderator"));
        assert!(payload["updated_fields"]["allow"].is_array());
        assert!(payload["updated_fields"]["deny"].is_array());
    }
}
