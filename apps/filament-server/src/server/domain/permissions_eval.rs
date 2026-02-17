use filament_core::{ChannelPermissionOverwrite, PermissionSet, Role, UserId};
use std::collections::{HashMap, HashSet};

use crate::server::auth::now_unix;
use crate::server::core::{ChannelPermissionOverrideRecord, WorkspaceRoleRecord};
use crate::server::errors::AuthFailure;
use crate::server::permissions::{
    all_permissions, default_everyone_permissions, default_member_permissions,
    default_moderator_permissions,
    mask_permissions,
    DEFAULT_ROLE_MEMBER, DEFAULT_ROLE_MODERATOR, SYSTEM_ROLE_EVERYONE,
    SYSTEM_ROLE_WORKSPACE_OWNER,
};
use ulid::Ulid;

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

pub(crate) fn ensure_required_roles(
    roles: &mut HashMap<String, WorkspaceRoleRecord>,
) -> RoleIdSet {
    let created_at_unix = now_unix();
    let everyone = roles
        .values()
        .find(|role| role.system_key.as_deref() == Some(SYSTEM_ROLE_EVERYONE))
        .map(|role| role.role_id.clone())
        .unwrap_or_else(|| {
            let role_id = Ulid::new().to_string();
            roles.insert(
                role_id.clone(),
                WorkspaceRoleRecord {
                    role_id: role_id.clone(),
                    name: String::from("@everyone"),
                    position: 0,
                    is_system: true,
                    system_key: Some(String::from(SYSTEM_ROLE_EVERYONE)),
                    permissions_allow: default_everyone_permissions(),
                    created_at_unix,
                },
            );
            role_id
        });

    let workspace_owner = roles
        .values()
        .find(|role| role.system_key.as_deref() == Some(SYSTEM_ROLE_WORKSPACE_OWNER))
        .map(|role| role.role_id.clone())
        .unwrap_or_else(|| {
            let role_id = Ulid::new().to_string();
            roles.insert(
                role_id.clone(),
                WorkspaceRoleRecord {
                    role_id: role_id.clone(),
                    name: String::from("workspace_owner"),
                    position: 10_000,
                    is_system: true,
                    system_key: Some(String::from(SYSTEM_ROLE_WORKSPACE_OWNER)),
                    permissions_allow: all_permissions(),
                    created_at_unix,
                },
            );
            role_id
        });

    let moderator = roles
        .values()
        .find(|role| {
            role.system_key.as_deref() == Some(DEFAULT_ROLE_MODERATOR)
                || role.name.eq_ignore_ascii_case(DEFAULT_ROLE_MODERATOR)
        })
        .map(|role| role.role_id.clone())
        .unwrap_or_else(|| {
            let role_id = Ulid::new().to_string();
            roles.insert(
                role_id.clone(),
                WorkspaceRoleRecord {
                    role_id: role_id.clone(),
                    name: String::from(DEFAULT_ROLE_MODERATOR),
                    position: 100,
                    is_system: false,
                    system_key: Some(String::from(DEFAULT_ROLE_MODERATOR)),
                    permissions_allow: default_moderator_permissions(),
                    created_at_unix,
                },
            );
            role_id
        });

    let member = roles
        .values()
        .find(|role| {
            role.system_key.as_deref() == Some(DEFAULT_ROLE_MEMBER)
                || role.name.eq_ignore_ascii_case(DEFAULT_ROLE_MEMBER)
        })
        .map(|role| role.role_id.clone())
        .unwrap_or_else(|| {
            let role_id = Ulid::new().to_string();
            roles.insert(
                role_id.clone(),
                WorkspaceRoleRecord {
                    role_id: role_id.clone(),
                    name: String::from(DEFAULT_ROLE_MEMBER),
                    position: 1,
                    is_system: false,
                    system_key: Some(String::from(DEFAULT_ROLE_MEMBER)),
                    permissions_allow: default_member_permissions(),
                    created_at_unix,
                },
            );
            role_id
        });

    RoleIdSet {
        everyone,
        workspace_owner,
        member,
        moderator,
    }
}

pub(crate) fn sync_legacy_role_assignments(
    members: &HashMap<UserId, Role>,
    guild_assignments: &mut HashMap<UserId, HashSet<String>>,
    role_ids: &RoleIdSet,
) {
    guild_assignments.retain(|member, _| members.contains_key(member));
    for (member_id, legacy_role) in members {
        let assigned = guild_assignments.entry(*member_id).or_default();
        assigned.retain(|role_id| {
            role_id != &role_ids.workspace_owner
                && role_id != &role_ids.moderator
                && role_id != &role_ids.member
        });
        apply_legacy_role_assignment(
            assigned,
            *legacy_role,
            &role_ids.workspace_owner,
            &role_ids.moderator,
            &role_ids.member,
        );
    }
}

pub(crate) fn sync_legacy_channel_overrides(
    legacy_overrides: HashMap<String, HashMap<Role, ChannelPermissionOverwrite>>,
    guild_channel_overrides: &mut HashMap<String, ChannelPermissionOverrideRecord>,
    role_ids: &RoleIdSet,
) {
    for (channel_id, legacy) in legacy_overrides {
        let channel_entry = guild_channel_overrides.entry(channel_id).or_default();
        if let Some(overwrite) = legacy.get(&Role::Member).copied() {
            channel_entry
                .role_overrides
                .entry(role_ids.member.clone())
                .or_insert(overwrite);
        }
        if let Some(overwrite) = legacy.get(&Role::Moderator).copied() {
            channel_entry
                .role_overrides
                .entry(role_ids.moderator.clone())
                .or_insert(overwrite);
        }
        if let Some(overwrite) = legacy.get(&Role::Owner).copied() {
            channel_entry
                .role_overrides
                .entry(role_ids.workspace_owner.clone())
                .or_insert(overwrite);
        }
    }
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
        apply_legacy_role_assignment, ensure_required_roles,
        finalize_channel_permissions, i64_to_masked_permissions,
        merge_channel_overwrite, role_ids_from_map,
        summarize_guild_permissions, sync_legacy_channel_overrides,
        sync_legacy_role_assignments,
    };
    use crate::server::core::{ChannelPermissionOverrideRecord, WorkspaceRoleRecord};
    use crate::server::auth::now_unix;
    use crate::server::errors::AuthFailure;
    use crate::server::permissions::{
        DEFAULT_ROLE_MEMBER, DEFAULT_ROLE_MODERATOR, SYSTEM_ROLE_EVERYONE,
        SYSTEM_ROLE_WORKSPACE_OWNER,
    };
    use filament_core::{
        ChannelPermissionOverwrite, Permission, PermissionSet, Role, UserId,
    };
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
    fn ensure_required_roles_inserts_missing_system_and_default_roles() {
        let mut roles = HashMap::new();

        let role_ids = ensure_required_roles(&mut roles);
        assert_eq!(roles.len(), 4);

        let everyone = roles
            .get(&role_ids.everyone)
            .expect("everyone role should exist");
        assert_eq!(everyone.system_key.as_deref(), Some(SYSTEM_ROLE_EVERYONE));

        let owner = roles
            .get(&role_ids.workspace_owner)
            .expect("workspace owner role should exist");
        assert_eq!(
            owner.system_key.as_deref(),
            Some(SYSTEM_ROLE_WORKSPACE_OWNER)
        );

        let member = roles
            .get(&role_ids.member)
            .expect("member role should exist");
        assert!(
            member.system_key.as_deref() == Some(DEFAULT_ROLE_MEMBER)
                || member.name.eq_ignore_ascii_case(DEFAULT_ROLE_MEMBER)
        );

        let moderator = roles
            .get(&role_ids.moderator)
            .expect("moderator role should exist");
        assert!(
            moderator.system_key.as_deref() == Some(DEFAULT_ROLE_MODERATOR)
                || moderator.name.eq_ignore_ascii_case(DEFAULT_ROLE_MODERATOR)
        );
    }

    #[test]
    fn sync_legacy_role_assignments_prunes_stale_and_applies_legacy_roles() {
        let member_id = UserId::new();
        let moderator_id = UserId::new();
        let stale_id = UserId::new();

        let members = HashMap::from([
            (member_id, Role::Member),
            (moderator_id, Role::Moderator),
        ]);
        let role_ids = super::RoleIdSet {
            everyone: String::from("everyone"),
            workspace_owner: String::from("owner"),
            member: String::from("member"),
            moderator: String::from("moderator"),
        };

        let mut assignments = HashMap::from([
            (
                member_id,
                HashSet::from([
                    String::from("custom"),
                    String::from("owner"),
                    String::from("member"),
                ]),
            ),
            (stale_id, HashSet::from([String::from("member")])),
        ]);

        sync_legacy_role_assignments(&members, &mut assignments, &role_ids);

        assert!(!assignments.contains_key(&stale_id));
        let member_roles = assignments
            .get(&member_id)
            .expect("member assignment should exist");
        assert!(member_roles.contains("custom"));
        assert!(member_roles.contains("member"));
        assert!(!member_roles.contains("owner"));

        let moderator_roles = assignments
            .get(&moderator_id)
            .expect("moderator assignment should exist");
        assert!(moderator_roles.contains("moderator"));
    }

    #[test]
    fn sync_legacy_channel_overrides_maps_member_moderator_owner_roles() {
        let mut legacy_channel = HashMap::new();
        legacy_channel.insert(
            Role::Member,
            ChannelPermissionOverwrite {
                allow: permission_set(&[Permission::CreateMessage]),
                deny: PermissionSet::empty(),
            },
        );
        legacy_channel.insert(
            Role::Moderator,
            ChannelPermissionOverwrite {
                allow: permission_set(&[Permission::DeleteMessage]),
                deny: PermissionSet::empty(),
            },
        );
        legacy_channel.insert(
            Role::Owner,
            ChannelPermissionOverwrite {
                allow: permission_set(&[Permission::ManageRoles]),
                deny: PermissionSet::empty(),
            },
        );

        let legacy_overrides = HashMap::from([(String::from("channel-1"), legacy_channel)]);
        let mut channel_overrides: HashMap<String, ChannelPermissionOverrideRecord> =
            HashMap::new();
        let role_ids = super::RoleIdSet {
            everyone: String::from("everyone"),
            workspace_owner: String::from("owner"),
            member: String::from("member"),
            moderator: String::from("moderator"),
        };

        sync_legacy_channel_overrides(
            legacy_overrides,
            &mut channel_overrides,
            &role_ids,
        );

        let channel = channel_overrides
            .get("channel-1")
            .expect("channel overrides should exist");
        assert!(
            channel
                .role_overrides
                .get("member")
                .is_some_and(|overwrite| overwrite.allow.contains(Permission::CreateMessage))
        );
        assert!(
            channel
                .role_overrides
                .get("moderator")
                .is_some_and(|overwrite| overwrite.allow.contains(Permission::DeleteMessage))
        );
        assert!(
            channel
                .role_overrides
                .get("owner")
                .is_some_and(|overwrite| overwrite.allow.contains(Permission::ManageRoles))
        );
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
