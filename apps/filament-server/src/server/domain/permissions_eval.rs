use filament_core::{ChannelPermissionOverwrite, PermissionSet, Role};
use std::collections::{HashMap, HashSet};

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

#[cfg(test)]
mod tests {
    use super::{
        aggregate_guild_permissions, apply_channel_layers,
        apply_legacy_role_assignment,
    };
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
}
