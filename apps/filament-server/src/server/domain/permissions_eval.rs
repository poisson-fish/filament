use filament_core::{ChannelPermissionOverwrite, PermissionSet, Role};
use std::collections::{HashMap, HashSet};

use crate::server::core::WorkspaceRoleRecord;
use crate::server::errors::AuthFailure;
use crate::server::permissions::{
    mask_permissions,
    DEFAULT_ROLE_MEMBER, DEFAULT_ROLE_MODERATOR, SYSTEM_ROLE_EVERYONE,
    SYSTEM_ROLE_WORKSPACE_OWNER,
};

#[derive(Debug)]
pub(crate) struct RoleIdSet {
    pub(crate) everyone: String,
    pub(crate) workspace_owner: String,
    pub(crate) member: String,
    pub(crate) moderator: String,
}

#[derive(Debug)]
pub(crate) struct GuildPermissionSummary {
    pub(crate) resolved_role: Role,
    pub(crate) guild_permissions: PermissionSet,
    pub(crate) is_workspace_owner: bool,
}

pub(crate) fn role_ids_from_map(
    roles: &HashMap<String, WorkspaceRoleRecord>,
) -> Option<RoleIdSet> {
    let everyone = roles
        .values()
        .find(|role| role.system_key.as_deref() == Some(SYSTEM_ROLE_EVERYONE))
        .map(|role| role.role_id.clone())?;
    let workspace_owner = roles
        .values()
        .find(|role| role.system_key.as_deref() == Some(SYSTEM_ROLE_WORKSPACE_OWNER))
        .map(|role| role.role_id.clone())?;
    let member = roles
        .values()
        .find(|role| {
            role.system_key.as_deref() == Some(DEFAULT_ROLE_MEMBER)
                || role.name.eq_ignore_ascii_case(DEFAULT_ROLE_MEMBER)
        })
        .map(|role| role.role_id.clone())?;
    let moderator = roles
        .values()
        .find(|role| {
            role.system_key.as_deref() == Some(DEFAULT_ROLE_MODERATOR)
                || role.name.eq_ignore_ascii_case(DEFAULT_ROLE_MODERATOR)
        })
        .map(|role| role.role_id.clone())?;
    Some(RoleIdSet {
        everyone,
        workspace_owner,
        member,
        moderator,
    })
}

pub(crate) fn i64_to_masked_permissions(
    value: i64,
) -> Result<(PermissionSet, u64), AuthFailure> {
    let raw = u64::try_from(value).map_err(|_| AuthFailure::Internal)?;
    Ok(mask_permissions(raw))
}

pub(crate) fn apply_channel_layers(
    base: PermissionSet,
    everyone: ChannelPermissionOverwrite,
    role_aggregate: ChannelPermissionOverwrite,
    member: ChannelPermissionOverwrite,
) -> PermissionSet {
    let mut bits = base.bits();
    let (everyone_allow, everyone_deny) =
        normalize_layer(everyone.allow.bits(), everyone.deny.bits());
    bits &= !everyone_deny;
    bits |= everyone_allow;

    let (role_allow, role_deny) =
        normalize_layer(role_aggregate.allow.bits(), role_aggregate.deny.bits());
    bits &= !role_deny;
    bits |= role_allow;

    let (member_allow, member_deny) = normalize_layer(member.allow.bits(), member.deny.bits());
    bits &= !member_deny;
    bits |= member_allow;
    PermissionSet::from_bits(bits)
}

pub(crate) fn merge_channel_overwrite(
    aggregate: ChannelPermissionOverwrite,
    overwrite: ChannelPermissionOverwrite,
) -> ChannelPermissionOverwrite {
    ChannelPermissionOverwrite {
        allow: PermissionSet::from_bits(aggregate.allow.bits() | overwrite.allow.bits()),
        deny: PermissionSet::from_bits(aggregate.deny.bits() | overwrite.deny.bits()),
    }
}

fn normalize_layer(allow_bits: u64, deny_bits: u64) -> (u64, u64) {
    (allow_bits & !deny_bits, deny_bits)
}

pub(crate) fn apply_legacy_role_assignment(
    assigned_role_ids: &mut HashSet<String>,
    legacy_role: Role,
    workspace_owner_role_id: &str,
    moderator_role_id: &str,
    member_role_id: &str,
) {
    match legacy_role {
        Role::Owner => {
            assigned_role_ids.insert(workspace_owner_role_id.to_owned());
        }
        Role::Moderator => {
            assigned_role_ids.insert(moderator_role_id.to_owned());
        }
        Role::Member => {
            assigned_role_ids.insert(member_role_id.to_owned());
        }
    }
}

pub(crate) fn aggregate_guild_permissions(
    everyone_permissions: PermissionSet,
    assigned_role_ids: &HashSet<String>,
    role_permissions: &HashMap<String, PermissionSet>,
) -> PermissionSet {
    let mut guild_permissions = everyone_permissions;
    for role_id in assigned_role_ids {
        if let Some(allow_set) = role_permissions.get(role_id) {
            guild_permissions =
                PermissionSet::from_bits(guild_permissions.bits() | allow_set.bits());
        }
    }
    guild_permissions
}

pub(crate) fn summarize_guild_permissions(
    everyone_permissions: PermissionSet,
    assigned_role_ids: &HashSet<String>,
    role_permissions: &HashMap<String, PermissionSet>,
    workspace_owner_role_id: &str,
    moderator_role_id: &str,
) -> GuildPermissionSummary {
    let mut guild_permissions =
        aggregate_guild_permissions(everyone_permissions, assigned_role_ids, role_permissions);

    let is_workspace_owner = assigned_role_ids.contains(workspace_owner_role_id);
    if is_workspace_owner {
        guild_permissions = crate::server::permissions::all_permissions();
    }

    let resolved_role = crate::server::permissions::membership_to_legacy_role(
        assigned_role_ids,
        workspace_owner_role_id,
        moderator_role_id,
    );

    GuildPermissionSummary {
        resolved_role,
        guild_permissions,
        is_workspace_owner,
    }
}

pub(crate) fn finalize_channel_permissions(
    guild_permissions: PermissionSet,
    is_workspace_owner: bool,
    everyone_overwrite: ChannelPermissionOverwrite,
    role_overwrite: ChannelPermissionOverwrite,
    member_overwrite: ChannelPermissionOverwrite,
) -> PermissionSet {
    if is_workspace_owner {
        return crate::server::permissions::all_permissions();
    }
    apply_channel_layers(
        guild_permissions,
        everyone_overwrite,
        role_overwrite,
        member_overwrite,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        aggregate_guild_permissions, apply_channel_layers,
        apply_legacy_role_assignment, finalize_channel_permissions,
        i64_to_masked_permissions, merge_channel_overwrite, role_ids_from_map,
        summarize_guild_permissions,
    };
    use crate::server::core::WorkspaceRoleRecord;
    use crate::server::auth::now_unix;
    use crate::server::errors::AuthFailure;
    use filament_core::{ChannelPermissionOverwrite, Permission, PermissionSet, Role};
    use std::collections::{HashMap, HashSet};

    fn permission_set(values: &[Permission]) -> PermissionSet {
        let mut set = PermissionSet::empty();
        for value in values {
            set.insert(*value);
        }
        set
    }

    #[test]
    fn apply_channel_layers_follows_locked_precedence() {
        let base = permission_set(&[Permission::CreateMessage, Permission::DeleteMessage]);
        let everyone = ChannelPermissionOverwrite {
            allow: PermissionSet::empty(),
            deny: permission_set(&[Permission::CreateMessage]),
        };
        let roles = ChannelPermissionOverwrite {
            allow: permission_set(&[Permission::CreateMessage]),
            deny: permission_set(&[Permission::DeleteMessage]),
        };
        let member = ChannelPermissionOverwrite {
            allow: permission_set(&[Permission::DeleteMessage]),
            deny: permission_set(&[Permission::CreateMessage]),
        };

        let resolved = apply_channel_layers(base, everyone, roles, member);
        assert!(!resolved.contains(Permission::CreateMessage));
        assert!(resolved.contains(Permission::DeleteMessage));
    }

    #[test]
    fn apply_channel_layers_prefers_deny_when_same_layer_conflicts() {
        let base = permission_set(&[Permission::CreateMessage]);
        let member = ChannelPermissionOverwrite {
            allow: permission_set(&[Permission::CreateMessage]),
            deny: permission_set(&[Permission::CreateMessage]),
        };

        let resolved = apply_channel_layers(
            base,
            ChannelPermissionOverwrite::default(),
            ChannelPermissionOverwrite::default(),
            member,
        );
        assert!(!resolved.contains(Permission::CreateMessage));
    }

    #[test]
    fn apply_legacy_role_assignment_inserts_expected_role_id() {
        let mut assigned = HashSet::new();
        apply_legacy_role_assignment(&mut assigned, Role::Moderator, "owner", "mod", "member");

        assert!(assigned.contains("mod"));
        assert!(!assigned.contains("owner"));
        assert!(!assigned.contains("member"));
    }

    #[test]
    fn aggregate_guild_permissions_unions_assigned_role_masks() {
        let everyone = permission_set(&[Permission::ManageRoles]);
        let mut assigned = HashSet::new();
        assigned.insert(String::from("member"));
        assigned.insert(String::from("moderator"));

        let mut role_permissions = HashMap::new();
        role_permissions.insert(
            String::from("member"),
            permission_set(&[Permission::CreateMessage]),
        );
        role_permissions.insert(
            String::from("moderator"),
            permission_set(&[Permission::DeleteMessage]),
        );

        let aggregated = aggregate_guild_permissions(everyone, &assigned, &role_permissions);
        assert!(aggregated.contains(Permission::ManageRoles));
        assert!(aggregated.contains(Permission::CreateMessage));
        assert!(aggregated.contains(Permission::DeleteMessage));
    }

    #[test]
    fn summarize_guild_permissions_marks_workspace_owner_and_grants_all() {
        let everyone = permission_set(&[Permission::CreateMessage]);
        let mut assigned = HashSet::new();
        assigned.insert(String::from("owner"));

        let mut role_permissions = HashMap::new();
        role_permissions.insert(String::from("owner"), PermissionSet::empty());

        let summary = summarize_guild_permissions(
            everyone,
            &assigned,
            &role_permissions,
            "owner",
            "moderator",
        );

        assert!(summary.is_workspace_owner);
        assert_eq!(summary.resolved_role, Role::Owner);
        assert_eq!(
            summary.guild_permissions.bits(),
            crate::server::permissions::all_permissions().bits()
        );
    }

    #[test]
    fn summarize_guild_permissions_uses_aggregated_permissions_for_non_owner() {
        let everyone = permission_set(&[Permission::ManageRoles]);
        let mut assigned = HashSet::new();
        assigned.insert(String::from("member"));

        let mut role_permissions = HashMap::new();
        role_permissions.insert(
            String::from("member"),
            permission_set(&[Permission::CreateMessage]),
        );

        let summary = summarize_guild_permissions(
            everyone,
            &assigned,
            &role_permissions,
            "owner",
            "moderator",
        );

        assert!(!summary.is_workspace_owner);
        assert_eq!(summary.resolved_role, Role::Member);
        assert!(summary.guild_permissions.contains(Permission::ManageRoles));
        assert!(summary.guild_permissions.contains(Permission::CreateMessage));
    }

    #[test]
    fn finalize_channel_permissions_returns_all_for_workspace_owner() {
        let resolved = finalize_channel_permissions(
            permission_set(&[Permission::CreateMessage]),
            true,
            ChannelPermissionOverwrite::default(),
            ChannelPermissionOverwrite::default(),
            ChannelPermissionOverwrite::default(),
        );

        assert_eq!(
            resolved.bits(),
            crate::server::permissions::all_permissions().bits()
        );
    }

    #[test]
    fn finalize_channel_permissions_applies_layers_for_non_owner() {
        let base = permission_set(&[Permission::CreateMessage]);
        let everyone = ChannelPermissionOverwrite {
            allow: PermissionSet::empty(),
            deny: permission_set(&[Permission::CreateMessage]),
        };

        let resolved = finalize_channel_permissions(
            base,
            false,
            everyone,
            ChannelPermissionOverwrite::default(),
            ChannelPermissionOverwrite::default(),
        );

        assert!(!resolved.contains(Permission::CreateMessage));
    }

    #[test]
    fn role_ids_from_map_extracts_expected_system_roles() {
        let created_at_unix = now_unix();
        let mut roles = HashMap::new();
        roles.insert(
            String::from("r-everyone"),
            WorkspaceRoleRecord {
                role_id: String::from("r-everyone"),
                name: String::from("@everyone"),
                position: 0,
                is_system: true,
                system_key: Some(String::from("everyone")),
                permissions_allow: PermissionSet::empty(),
                created_at_unix,
            },
        );
        roles.insert(
            String::from("r-owner"),
            WorkspaceRoleRecord {
                role_id: String::from("r-owner"),
                name: String::from("workspace_owner"),
                position: 10_000,
                is_system: true,
                system_key: Some(String::from("workspace_owner")),
                permissions_allow: PermissionSet::empty(),
                created_at_unix,
            },
        );
        roles.insert(
            String::from("r-member"),
            WorkspaceRoleRecord {
                role_id: String::from("r-member"),
                name: String::from("member"),
                position: 1,
                is_system: false,
                system_key: Some(String::from("member")),
                permissions_allow: PermissionSet::empty(),
                created_at_unix,
            },
        );
        roles.insert(
            String::from("r-mod"),
            WorkspaceRoleRecord {
                role_id: String::from("r-mod"),
                name: String::from("moderator"),
                position: 2,
                is_system: false,
                system_key: Some(String::from("moderator")),
                permissions_allow: PermissionSet::empty(),
                created_at_unix,
            },
        );

        let ids = role_ids_from_map(&roles).expect("role ids should be resolved");
        assert_eq!(ids.everyone, "r-everyone");
        assert_eq!(ids.workspace_owner, "r-owner");
        assert_eq!(ids.member, "r-member");
        assert_eq!(ids.moderator, "r-mod");
    }

    #[test]
    fn i64_to_masked_permissions_rejects_negative_input_fail_closed() {
        assert!(matches!(
            i64_to_masked_permissions(-1),
            Err(AuthFailure::Internal)
        ));
    }

    #[test]
    fn i64_to_masked_permissions_masks_unknown_bits() {
        let unknown_bit = 1_u64 << 60;
        let mut known = PermissionSet::empty();
        known.insert(Permission::CreateMessage);
        let raw_bits = known.bits() | unknown_bit;
        let raw_i64 = i64::try_from(raw_bits).expect("test bits should fit i64");

        let (masked, unknown) = i64_to_masked_permissions(raw_i64)
            .expect("masking should succeed for non-negative values");

        assert!(masked.contains(Permission::CreateMessage));
        assert_eq!(unknown, unknown_bit);
    }

    #[test]
    fn merge_channel_overwrite_unions_allow_and_deny_masks() {
        let left = ChannelPermissionOverwrite {
            allow: permission_set(&[Permission::CreateMessage]),
            deny: permission_set(&[Permission::DeleteMessage]),
        };
        let right = ChannelPermissionOverwrite {
            allow: permission_set(&[Permission::DeleteMessage]),
            deny: permission_set(&[Permission::CreateMessage]),
        };

        let merged = merge_channel_overwrite(left, right);
        assert!(merged.allow.contains(Permission::CreateMessage));
        assert!(merged.allow.contains(Permission::DeleteMessage));
        assert!(merged.deny.contains(Permission::CreateMessage));
        assert!(merged.deny.contains(Permission::DeleteMessage));
    }
}
