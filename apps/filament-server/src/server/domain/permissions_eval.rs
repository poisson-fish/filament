use filament_core::{ChannelPermissionOverwrite, PermissionSet, Role, UserId};
use std::collections::{HashMap, HashSet};

use crate::server::auth::now_unix;
use crate::server::core::{ChannelPermissionOverrideRecord, WorkspaceRoleRecord};
use crate::server::errors::AuthFailure;
use crate::server::permissions::{
    all_permissions, default_everyone_permissions, default_member_permissions,
    default_moderator_permissions, mask_permissions, DEFAULT_ROLE_MEMBER, DEFAULT_ROLE_MODERATOR,
    SYSTEM_ROLE_EVERYONE, SYSTEM_ROLE_WORKSPACE_OWNER,
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

#[derive(Debug)]
pub(crate) struct GuildRoleDbRow {
    pub(crate) role_id: String,
    pub(crate) name: String,
    pub(crate) position: i32,
    pub(crate) permissions_allow_mask: i64,
    pub(crate) is_system: bool,
    pub(crate) system_key: Option<String>,
}

#[derive(Debug)]
pub(crate) struct RoleMaskUpdate {
    pub(crate) role_id: String,
    pub(crate) masked_permissions_allow: i64,
}

#[derive(Debug)]
pub(crate) struct ChannelOverrideDbRow {
    pub(crate) target_kind: i16,
    pub(crate) target_id: String,
    pub(crate) allow_mask: i64,
    pub(crate) deny_mask: i64,
}

#[derive(Debug)]
pub(crate) struct LegacyChannelRoleOverrideDbRow {
    pub(crate) role: i16,
    pub(crate) allow_mask: i64,
    pub(crate) deny_mask: i64,
}

#[derive(Debug)]
pub(crate) struct ChannelOverrideSummary {
    pub(crate) everyone_overwrite: ChannelPermissionOverwrite,
    pub(crate) role_overwrite: ChannelPermissionOverwrite,
    pub(crate) member_overwrite: ChannelPermissionOverwrite,
    pub(crate) used_new_overrides: bool,
    pub(crate) unknown_override_bits: u64,
}

#[derive(Debug)]
#[allow(clippy::struct_field_names)]
pub(crate) struct InMemoryChannelOverrideSummary {
    pub(crate) everyone_overwrite: ChannelPermissionOverwrite,
    pub(crate) role_overwrite: ChannelPermissionOverwrite,
    pub(crate) member_overwrite: ChannelPermissionOverwrite,
}

pub(crate) fn role_ids_from_map(roles: &HashMap<String, WorkspaceRoleRecord>) -> Option<RoleIdSet> {
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

#[allow(clippy::too_many_lines)]
pub(crate) fn ensure_required_roles(roles: &mut HashMap<String, WorkspaceRoleRecord>) -> RoleIdSet {
    let created_at_unix = now_unix();

    let everyone_existing = roles
        .values()
        .find(|role| role.system_key.as_deref() == Some(SYSTEM_ROLE_EVERYONE))
        .map(|role| role.role_id.clone());
    let everyone = if let Some(role_id) = everyone_existing {
        role_id
    } else {
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
                color_hex: None,
                created_at_unix,
            },
        );
        role_id
    };

    let workspace_owner_existing = roles
        .values()
        .find(|role| role.system_key.as_deref() == Some(SYSTEM_ROLE_WORKSPACE_OWNER))
        .map(|role| role.role_id.clone());
    let workspace_owner = if let Some(role_id) = workspace_owner_existing {
        role_id
    } else {
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
                color_hex: None,
                created_at_unix,
            },
        );
        role_id
    };

    let moderator_existing = roles
        .values()
        .find(|role| {
            role.system_key.as_deref() == Some(DEFAULT_ROLE_MODERATOR)
                || role.name.eq_ignore_ascii_case(DEFAULT_ROLE_MODERATOR)
        })
        .map(|role| role.role_id.clone());
    let moderator = if let Some(role_id) = moderator_existing {
        role_id
    } else {
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
                color_hex: None,
                created_at_unix,
            },
        );
        role_id
    };

    let member_existing = roles
        .values()
        .find(|role| {
            role.system_key.as_deref() == Some(DEFAULT_ROLE_MEMBER)
                || role.name.eq_ignore_ascii_case(DEFAULT_ROLE_MEMBER)
        })
        .map(|role| role.role_id.clone());
    let member = if let Some(role_id) = member_existing {
        role_id
    } else {
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
                color_hex: None,
                created_at_unix,
            },
        );
        role_id
    };

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

pub(crate) fn i64_to_masked_permissions(value: i64) -> Result<(PermissionSet, u64), AuthFailure> {
    let raw = u64::try_from(value).map_err(|_| AuthFailure::Internal)?;
    Ok(mask_permissions(raw))
}

#[allow(clippy::type_complexity)]
pub(crate) fn role_records_from_db_rows(
    rows: Vec<GuildRoleDbRow>,
) -> Result<
    (
        HashMap<String, WorkspaceRoleRecord>,
        u64,
        Vec<RoleMaskUpdate>,
    ),
    AuthFailure,
> {
    let mut roles: HashMap<String, WorkspaceRoleRecord> = HashMap::new();
    let mut unknown_bits_seen = 0_u64;
    let mut mask_updates = Vec::new();

    for row in rows {
        let (permissions_allow, unknown_bits) =
            i64_to_masked_permissions(row.permissions_allow_mask)?;
        if unknown_bits > 0 {
            unknown_bits_seen |= unknown_bits;
            mask_updates.push(RoleMaskUpdate {
                role_id: row.role_id.clone(),
                masked_permissions_allow: i64::try_from(permissions_allow.bits())
                    .map_err(|_| AuthFailure::Internal)?,
            });
        }

        roles.insert(
            row.role_id.clone(),
            WorkspaceRoleRecord {
                role_id: row.role_id,
                name: row.name,
                position: row.position,
                is_system: row.is_system,
                system_key: row.system_key,
                permissions_allow,
                color_hex: None,
                created_at_unix: 0,
            },
        );
    }

    Ok((roles, unknown_bits_seen, mask_updates))
}

pub(crate) fn normalize_assigned_role_ids(
    assignment_role_ids: Vec<String>,
    roles: &HashMap<String, WorkspaceRoleRecord>,
    legacy_role: Role,
    role_ids: &RoleIdSet,
) -> HashSet<String> {
    let mut assigned_role_ids = HashSet::new();
    for role_id in assignment_role_ids {
        if roles.contains_key(&role_id) {
            assigned_role_ids.insert(role_id);
        }
    }

    apply_legacy_role_assignment(
        &mut assigned_role_ids,
        legacy_role,
        &role_ids.workspace_owner,
        &role_ids.moderator,
        &role_ids.member,
    );
    assigned_role_ids
}

pub(crate) fn guild_role_permission_inputs(
    roles: &HashMap<String, WorkspaceRoleRecord>,
    everyone_role_id: &str,
) -> (PermissionSet, HashMap<String, PermissionSet>) {
    let everyone_permissions = roles
        .get(everyone_role_id)
        .map_or_else(default_everyone_permissions, |role| role.permissions_allow);
    let role_permissions = roles
        .iter()
        .map(|(role_id, role)| (role_id.clone(), role.permissions_allow))
        .collect();
    (everyone_permissions, role_permissions)
}

pub(crate) fn summarize_channel_overrides(
    rows: Vec<ChannelOverrideDbRow>,
    assigned_role_ids: &HashSet<String>,
    role_ids: &RoleIdSet,
    user_id: UserId,
    override_target_role: i16,
    override_target_member: i16,
) -> Result<ChannelOverrideSummary, AuthFailure> {
    let mut everyone_overwrite = ChannelPermissionOverwrite::default();
    let mut role_overwrite = ChannelPermissionOverwrite::default();
    let mut member_overwrite = ChannelPermissionOverwrite::default();
    let mut used_new_overrides = !rows.is_empty();
    let mut unknown_override_bits = 0_u64;

    for row in rows {
        let (allow, unknown_allow) = i64_to_masked_permissions(row.allow_mask)?;
        let (deny, unknown_deny) = i64_to_masked_permissions(row.deny_mask)?;
        unknown_override_bits |= unknown_allow | unknown_deny;
        let overwrite = ChannelPermissionOverwrite { allow, deny };

        match row.target_kind {
            value if value == override_target_role => {
                if row.target_id == role_ids.everyone {
                    everyone_overwrite = overwrite;
                } else if assigned_role_ids.contains(&row.target_id) {
                    role_overwrite = merge_channel_overwrite(role_overwrite, overwrite);
                }
            }
            value if value == override_target_member => {
                if row.target_id == user_id.to_string() {
                    member_overwrite = overwrite;
                }
            }
            _ => {
                used_new_overrides = false;
            }
        }
    }

    Ok(ChannelOverrideSummary {
        everyone_overwrite,
        role_overwrite,
        member_overwrite,
        used_new_overrides,
        unknown_override_bits,
    })
}

pub(crate) fn summarize_in_memory_channel_overrides(
    channel_override: &ChannelPermissionOverrideRecord,
    assigned_role_ids: &HashSet<String>,
    role_ids: &RoleIdSet,
    user_id: UserId,
) -> InMemoryChannelOverrideSummary {
    let everyone_overwrite = channel_override
        .role_overrides
        .get(&role_ids.everyone)
        .copied()
        .unwrap_or_default();
    let role_overwrite =
        merge_assigned_role_overrides(&channel_override.role_overrides, assigned_role_ids);
    let member_overwrite = channel_override
        .member_overrides
        .get(&user_id)
        .copied()
        .unwrap_or_default();

    InMemoryChannelOverrideSummary {
        everyone_overwrite,
        role_overwrite,
        member_overwrite,
    }
}

pub(crate) fn summarize_in_memory_guild_permissions(
    roles: &HashMap<String, WorkspaceRoleRecord>,
    assigned_role_ids: &HashSet<String>,
    legacy_role: Role,
    role_ids: &RoleIdSet,
) -> GuildPermissionSummary {
    let mut normalized_assigned_role_ids = assigned_role_ids.clone();
    apply_legacy_role_assignment(
        &mut normalized_assigned_role_ids,
        legacy_role,
        &role_ids.workspace_owner,
        &role_ids.moderator,
        &role_ids.member,
    );
    resolve_guild_permission_summary(roles, &normalized_assigned_role_ids, role_ids)
}

pub(crate) fn resolve_in_memory_channel_permissions(
    guild_permissions: PermissionSet,
    is_workspace_owner: bool,
    channel_override: &ChannelPermissionOverrideRecord,
    assigned_role_ids: &HashSet<String>,
    role_ids: &RoleIdSet,
    user_id: UserId,
) -> PermissionSet {
    let override_summary = summarize_in_memory_channel_overrides(
        channel_override,
        assigned_role_ids,
        role_ids,
        user_id,
    );
    finalize_channel_permissions(
        guild_permissions,
        is_workspace_owner,
        override_summary.everyone_overwrite,
        override_summary.role_overwrite,
        override_summary.member_overwrite,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_db_channel_permissions(
    guild_permissions: PermissionSet,
    is_workspace_owner: bool,
    override_rows: Vec<ChannelOverrideDbRow>,
    legacy_override_rows: Option<Vec<LegacyChannelRoleOverrideDbRow>>,
    assigned_role_ids: &HashSet<String>,
    role_ids: &RoleIdSet,
    user_id: UserId,
    override_target_role: i16,
    override_target_member: i16,
) -> Result<(PermissionSet, u64), AuthFailure> {
    let override_summary = summarize_channel_overrides(
        override_rows,
        assigned_role_ids,
        role_ids,
        user_id,
        override_target_role,
        override_target_member,
    )?;

    let role_overwrite = if override_summary.used_new_overrides {
        override_summary.role_overwrite
    } else {
        merge_legacy_channel_role_overrides(
            override_summary.role_overwrite,
            legacy_override_rows.unwrap_or_default(),
            assigned_role_ids,
            role_ids,
        )?
    };

    let permissions = finalize_channel_permissions(
        guild_permissions,
        is_workspace_owner,
        override_summary.everyone_overwrite,
        role_overwrite,
        override_summary.member_overwrite,
    );
    Ok((permissions, override_summary.unknown_override_bits))
}

pub(crate) fn merge_legacy_channel_role_overrides(
    base_role_overwrite: ChannelPermissionOverwrite,
    rows: Vec<LegacyChannelRoleOverrideDbRow>,
    assigned_role_ids: &HashSet<String>,
    role_ids: &RoleIdSet,
) -> Result<ChannelPermissionOverwrite, AuthFailure> {
    let mut role_overwrite = base_role_overwrite;

    for row in rows {
        let role = crate::server::db::role_from_i16(row.role).unwrap_or(Role::Member);
        let (allow, _) = i64_to_masked_permissions(row.allow_mask)?;
        let (deny, _) = i64_to_masked_permissions(row.deny_mask)?;
        let overwrite = ChannelPermissionOverwrite { allow, deny };
        match role {
            Role::Member => {
                if assigned_role_ids.contains(&role_ids.member) {
                    role_overwrite = merge_channel_overwrite(role_overwrite, overwrite);
                }
            }
            Role::Moderator => {
                if assigned_role_ids.contains(&role_ids.moderator) {
                    role_overwrite = merge_channel_overwrite(role_overwrite, overwrite);
                }
            }
            Role::Owner => {
                if assigned_role_ids.contains(&role_ids.workspace_owner) {
                    role_overwrite = merge_channel_overwrite(role_overwrite, overwrite);
                }
            }
        }
    }

    Ok(role_overwrite)
}

pub(crate) fn merge_assigned_role_overrides(
    role_overrides: &HashMap<String, ChannelPermissionOverwrite>,
    assigned_role_ids: &HashSet<String>,
) -> ChannelPermissionOverwrite {
    let mut role_overwrite = ChannelPermissionOverwrite::default();
    for role_id in assigned_role_ids {
        if let Some(overwrite) = role_overrides.get(role_id).copied() {
            role_overwrite = merge_channel_overwrite(role_overwrite, overwrite);
        }
    }
    role_overwrite
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

pub(crate) fn resolve_guild_permission_summary(
    roles: &HashMap<String, WorkspaceRoleRecord>,
    assigned_role_ids: &HashSet<String>,
    role_ids: &RoleIdSet,
) -> GuildPermissionSummary {
    let (everyone_permissions, role_permissions) =
        guild_role_permission_inputs(roles, &role_ids.everyone);
    summarize_guild_permissions(
        everyone_permissions,
        assigned_role_ids,
        &role_permissions,
        &role_ids.workspace_owner,
        &role_ids.moderator,
    )
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
        aggregate_guild_permissions, apply_channel_layers, apply_legacy_role_assignment,
        ensure_required_roles, finalize_channel_permissions, guild_role_permission_inputs,
        i64_to_masked_permissions, merge_assigned_role_overrides, merge_channel_overwrite,
        merge_legacy_channel_role_overrides, normalize_assigned_role_ids,
        resolve_db_channel_permissions, resolve_guild_permission_summary,
        resolve_in_memory_channel_permissions, role_ids_from_map, role_records_from_db_rows,
        summarize_channel_overrides, summarize_guild_permissions,
        summarize_in_memory_channel_overrides, summarize_in_memory_guild_permissions,
        sync_legacy_channel_overrides, sync_legacy_role_assignments,
    };
    use crate::server::auth::now_unix;
    use crate::server::core::{ChannelPermissionOverrideRecord, WorkspaceRoleRecord};
    use crate::server::errors::AuthFailure;
    use crate::server::permissions::{
        DEFAULT_ROLE_MEMBER, DEFAULT_ROLE_MODERATOR, SYSTEM_ROLE_EVERYONE,
        SYSTEM_ROLE_WORKSPACE_OWNER,
    };
    use filament_core::{ChannelPermissionOverwrite, Permission, PermissionSet, Role, UserId};
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

        let members = HashMap::from([(member_id, Role::Member), (moderator_id, Role::Moderator)]);
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

        sync_legacy_channel_overrides(legacy_overrides, &mut channel_overrides, &role_ids);

        let channel = channel_overrides
            .get("channel-1")
            .expect("channel overrides should exist");
        assert!(channel
            .role_overrides
            .get("member")
            .is_some_and(|overwrite| overwrite.allow.contains(Permission::CreateMessage)));
        assert!(channel
            .role_overrides
            .get("moderator")
            .is_some_and(|overwrite| overwrite.allow.contains(Permission::DeleteMessage)));
        assert!(channel
            .role_overrides
            .get("owner")
            .is_some_and(|overwrite| overwrite.allow.contains(Permission::ManageRoles)));
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
        assert!(summary
            .guild_permissions
            .contains(Permission::CreateMessage));
    }

    #[test]
    fn resolve_guild_permission_summary_uses_everyone_and_assigned_role_masks() {
        let created_at_unix = now_unix();
        let roles = HashMap::from([
            (
                String::from("everyone"),
                WorkspaceRoleRecord {
                    role_id: String::from("everyone"),
                    name: String::from("@everyone"),
                    position: 0,
                    is_system: true,
                    system_key: Some(String::from(SYSTEM_ROLE_EVERYONE)),
                    permissions_allow: permission_set(&[Permission::DeleteMessage]),
                    color_hex: None,
                    created_at_unix,
                },
            ),
            (
                String::from("member"),
                WorkspaceRoleRecord {
                    role_id: String::from("member"),
                    name: String::from("member"),
                    position: 1,
                    is_system: false,
                    system_key: Some(String::from(DEFAULT_ROLE_MEMBER)),
                    permissions_allow: permission_set(&[Permission::CreateMessage]),
                    color_hex: None,
                    created_at_unix,
                },
            ),
            (
                String::from("moderator"),
                WorkspaceRoleRecord {
                    role_id: String::from("moderator"),
                    name: String::from("moderator"),
                    position: 2,
                    is_system: false,
                    system_key: Some(String::from(DEFAULT_ROLE_MODERATOR)),
                    permissions_allow: permission_set(&[Permission::ManageRoles]),
                    color_hex: None,
                    created_at_unix,
                },
            ),
            (
                String::from("workspace-owner"),
                WorkspaceRoleRecord {
                    role_id: String::from("workspace-owner"),
                    name: String::from("workspace_owner"),
                    position: 10_000,
                    is_system: true,
                    system_key: Some(String::from(SYSTEM_ROLE_WORKSPACE_OWNER)),
                    permissions_allow: PermissionSet::empty(),
                    color_hex: None,
                    created_at_unix,
                },
            ),
        ]);

        let role_ids = super::RoleIdSet {
            everyone: String::from("everyone"),
            workspace_owner: String::from("workspace-owner"),
            member: String::from("member"),
            moderator: String::from("moderator"),
        };
        let assigned = HashSet::from([String::from("member")]);

        let summary = resolve_guild_permission_summary(&roles, &assigned, &role_ids);
        assert_eq!(summary.resolved_role, Role::Member);
        assert!(!summary.is_workspace_owner);
        assert!(summary
            .guild_permissions
            .contains(Permission::DeleteMessage));
        assert!(summary
            .guild_permissions
            .contains(Permission::CreateMessage));
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
                color_hex: None,
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
                color_hex: None,
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
                color_hex: None,
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
                color_hex: None,
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

    #[test]
    fn role_records_from_db_rows_masks_unknown_bits_and_plans_updates() {
        let rows = vec![
            super::GuildRoleDbRow {
                role_id: String::from("member"),
                name: String::from("member"),
                position: 1,
                permissions_allow_mask: 0,
                is_system: false,
                system_key: Some(String::from(DEFAULT_ROLE_MEMBER)),
            },
            super::GuildRoleDbRow {
                role_id: String::from("owner"),
                name: String::from("owner"),
                position: 10_000,
                permissions_allow_mask: i64::MAX,
                is_system: true,
                system_key: Some(String::from(SYSTEM_ROLE_WORKSPACE_OWNER)),
            },
        ];

        let (roles, unknown_bits, updates) =
            role_records_from_db_rows(rows).expect("rows should map");

        assert_eq!(roles.len(), 2);
        assert!(unknown_bits > 0);
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].role_id, "owner");
        assert!(updates[0].masked_permissions_allow >= 0);
    }

    #[test]
    fn normalize_assigned_role_ids_filters_unknown_and_applies_legacy() {
        let role_ids = super::RoleIdSet {
            everyone: String::from("everyone"),
            workspace_owner: String::from("owner"),
            member: String::from("member"),
            moderator: String::from("moderator"),
        };
        let created_at_unix = now_unix();
        let roles = HashMap::from([
            (
                String::from("member"),
                WorkspaceRoleRecord {
                    role_id: String::from("member"),
                    name: String::from("member"),
                    position: 1,
                    is_system: false,
                    system_key: Some(String::from(DEFAULT_ROLE_MEMBER)),
                    permissions_allow: PermissionSet::empty(),
                    color_hex: None,
                    created_at_unix,
                },
            ),
            (
                String::from("custom"),
                WorkspaceRoleRecord {
                    role_id: String::from("custom"),
                    name: String::from("custom"),
                    position: 2,
                    is_system: false,
                    system_key: None,
                    permissions_allow: PermissionSet::empty(),
                    color_hex: None,
                    created_at_unix,
                },
            ),
        ]);

        let normalized = normalize_assigned_role_ids(
            vec![String::from("custom"), String::from("missing")],
            &roles,
            Role::Member,
            &role_ids,
        );

        assert!(normalized.contains("custom"));
        assert!(normalized.contains("member"));
        assert!(!normalized.contains("missing"));
    }

    #[test]
    fn guild_role_permission_inputs_returns_everyone_default_and_role_map() {
        let created_at_unix = now_unix();
        let roles = HashMap::from([
            (
                String::from("everyone"),
                WorkspaceRoleRecord {
                    role_id: String::from("everyone"),
                    name: String::from("@everyone"),
                    position: 0,
                    is_system: true,
                    system_key: Some(String::from(SYSTEM_ROLE_EVERYONE)),
                    permissions_allow: permission_set(&[Permission::DeleteMessage]),
                    color_hex: None,
                    created_at_unix,
                },
            ),
            (
                String::from("member"),
                WorkspaceRoleRecord {
                    role_id: String::from("member"),
                    name: String::from("member"),
                    position: 1,
                    is_system: false,
                    system_key: Some(String::from(DEFAULT_ROLE_MEMBER)),
                    permissions_allow: permission_set(&[Permission::CreateMessage]),
                    color_hex: None,
                    created_at_unix,
                },
            ),
        ]);

        let (everyone_permissions, role_permissions) =
            guild_role_permission_inputs(&roles, "everyone");
        assert!(everyone_permissions.contains(Permission::DeleteMessage));
        assert!(role_permissions
            .get("member")
            .is_some_and(|set| set.contains(Permission::CreateMessage)));
    }

    #[test]
    fn summarize_channel_overrides_merges_roles_and_member_targets() {
        let role_ids = super::RoleIdSet {
            everyone: String::from("everyone"),
            workspace_owner: String::from("owner"),
            member: String::from("member"),
            moderator: String::from("moderator"),
        };
        let user_id = UserId::new();
        let assigned = HashSet::from([String::from("member")]);
        let everyone_allow = i64::try_from(permission_set(&[Permission::DeleteMessage]).bits())
            .expect("test mask should fit i64");
        let role_allow = i64::try_from(permission_set(&[Permission::CreateMessage]).bits())
            .expect("test mask should fit i64");
        let member_deny = i64::try_from(permission_set(&[Permission::CreateMessage]).bits())
            .expect("test mask should fit i64");
        let rows = vec![
            super::ChannelOverrideDbRow {
                target_kind: 0,
                target_id: String::from("everyone"),
                allow_mask: everyone_allow,
                deny_mask: 0,
            },
            super::ChannelOverrideDbRow {
                target_kind: 0,
                target_id: String::from("member"),
                allow_mask: role_allow,
                deny_mask: 0,
            },
            super::ChannelOverrideDbRow {
                target_kind: 1,
                target_id: user_id.to_string(),
                allow_mask: 0,
                deny_mask: member_deny,
            },
        ];

        let summary = summarize_channel_overrides(rows, &assigned, &role_ids, user_id, 0, 1)
            .expect("overrides should summarize");

        assert!(summary
            .everyone_overwrite
            .allow
            .contains(Permission::DeleteMessage));
        assert!(summary
            .role_overwrite
            .allow
            .contains(Permission::CreateMessage));
        assert!(summary
            .member_overwrite
            .deny
            .contains(Permission::CreateMessage));
        assert!(summary.used_new_overrides);
    }

    #[test]
    fn merge_legacy_channel_role_overrides_applies_only_assigned_legacy_roles() {
        let role_ids = super::RoleIdSet {
            everyone: String::from("everyone"),
            workspace_owner: String::from("owner"),
            member: String::from("member"),
            moderator: String::from("moderator"),
        };
        let assigned = HashSet::from([String::from("member"), String::from("moderator")]);
        let member_allow = i64::try_from(permission_set(&[Permission::CreateMessage]).bits())
            .expect("test mask should fit i64");
        let moderator_allow = i64::try_from(permission_set(&[Permission::DeleteMessage]).bits())
            .expect("test mask should fit i64");
        let owner_allow = i64::try_from(permission_set(&[Permission::ManageRoles]).bits())
            .expect("test mask should fit i64");

        let merged = merge_legacy_channel_role_overrides(
            ChannelPermissionOverwrite::default(),
            vec![
                super::LegacyChannelRoleOverrideDbRow {
                    role: 0,
                    allow_mask: member_allow,
                    deny_mask: 0,
                },
                super::LegacyChannelRoleOverrideDbRow {
                    role: 1,
                    allow_mask: moderator_allow,
                    deny_mask: 0,
                },
                super::LegacyChannelRoleOverrideDbRow {
                    role: 2,
                    allow_mask: owner_allow,
                    deny_mask: 0,
                },
            ],
            &assigned,
            &role_ids,
        )
        .expect("legacy role overrides should merge");

        assert!(merged.allow.contains(Permission::CreateMessage));
        assert!(merged.allow.contains(Permission::DeleteMessage));
        assert!(!merged.allow.contains(Permission::ManageRoles));
    }

    #[test]
    fn merge_legacy_channel_role_overrides_rejects_negative_masks_fail_closed() {
        let role_ids = super::RoleIdSet {
            everyone: String::from("everyone"),
            workspace_owner: String::from("owner"),
            member: String::from("member"),
            moderator: String::from("moderator"),
        };
        let assigned = HashSet::from([String::from("member")]);

        assert!(matches!(
            merge_legacy_channel_role_overrides(
                ChannelPermissionOverwrite::default(),
                vec![super::LegacyChannelRoleOverrideDbRow {
                    role: 0,
                    allow_mask: -1,
                    deny_mask: 0,
                }],
                &assigned,
                &role_ids,
            ),
            Err(AuthFailure::Internal)
        ));
    }

    #[test]
    fn merge_assigned_role_overrides_combines_only_assigned_entries() {
        let mut role_overrides = HashMap::new();
        role_overrides.insert(
            String::from("member"),
            ChannelPermissionOverwrite {
                allow: permission_set(&[Permission::CreateMessage]),
                deny: PermissionSet::empty(),
            },
        );
        role_overrides.insert(
            String::from("moderator"),
            ChannelPermissionOverwrite {
                allow: permission_set(&[Permission::DeleteMessage]),
                deny: PermissionSet::empty(),
            },
        );
        role_overrides.insert(
            String::from("owner"),
            ChannelPermissionOverwrite {
                allow: permission_set(&[Permission::ManageRoles]),
                deny: PermissionSet::empty(),
            },
        );

        let assigned_role_ids = HashSet::from([String::from("member"), String::from("moderator")]);
        let merged = merge_assigned_role_overrides(&role_overrides, &assigned_role_ids);

        assert!(merged.allow.contains(Permission::CreateMessage));
        assert!(merged.allow.contains(Permission::DeleteMessage));
        assert!(!merged.allow.contains(Permission::ManageRoles));
    }

    #[test]
    fn summarize_in_memory_channel_overrides_selects_everyone_role_and_member_layers() {
        let role_ids = super::RoleIdSet {
            everyone: String::from("everyone"),
            workspace_owner: String::from("owner"),
            member: String::from("member"),
            moderator: String::from("moderator"),
        };
        let user_id = UserId::new();
        let mut channel_override = ChannelPermissionOverrideRecord::default();
        channel_override.role_overrides.insert(
            String::from("everyone"),
            ChannelPermissionOverwrite {
                allow: permission_set(&[Permission::DeleteMessage]),
                deny: PermissionSet::empty(),
            },
        );
        channel_override.role_overrides.insert(
            String::from("member"),
            ChannelPermissionOverwrite {
                allow: permission_set(&[Permission::CreateMessage]),
                deny: PermissionSet::empty(),
            },
        );
        channel_override.member_overrides.insert(
            user_id,
            ChannelPermissionOverwrite {
                allow: PermissionSet::empty(),
                deny: permission_set(&[Permission::CreateMessage]),
            },
        );

        let assigned = HashSet::from([String::from("member")]);
        let summary =
            summarize_in_memory_channel_overrides(&channel_override, &assigned, &role_ids, user_id);

        assert!(summary
            .everyone_overwrite
            .allow
            .contains(Permission::DeleteMessage));
        assert!(summary
            .role_overwrite
            .allow
            .contains(Permission::CreateMessage));
        assert!(summary
            .member_overwrite
            .deny
            .contains(Permission::CreateMessage));
    }

    #[test]
    fn summarize_in_memory_guild_permissions_applies_legacy_role_before_resolution() {
        let role_ids = super::RoleIdSet {
            everyone: String::from("everyone"),
            workspace_owner: String::from("owner"),
            member: String::from("member"),
            moderator: String::from("moderator"),
        };
        let mut roles = HashMap::new();
        roles.insert(
            role_ids.everyone.clone(),
            WorkspaceRoleRecord {
                role_id: role_ids.everyone.clone(),
                name: String::from("@everyone"),
                position: 0,
                is_system: true,
                system_key: Some(String::from(SYSTEM_ROLE_EVERYONE)),
                permissions_allow: permission_set(&[]),
                color_hex: None,
                created_at_unix: now_unix(),
            },
        );
        roles.insert(
            role_ids.member.clone(),
            WorkspaceRoleRecord {
                role_id: role_ids.member.clone(),
                name: String::from(DEFAULT_ROLE_MEMBER),
                position: 1,
                is_system: false,
                system_key: Some(String::from(DEFAULT_ROLE_MEMBER)),
                permissions_allow: permission_set(&[Permission::CreateMessage]),
                color_hex: None,
                created_at_unix: now_unix(),
            },
        );
        roles.insert(
            role_ids.moderator.clone(),
            WorkspaceRoleRecord {
                role_id: role_ids.moderator.clone(),
                name: String::from(DEFAULT_ROLE_MODERATOR),
                position: 2,
                is_system: false,
                system_key: Some(String::from(DEFAULT_ROLE_MODERATOR)),
                permissions_allow: permission_set(&[Permission::DeleteMessage]),
                color_hex: None,
                created_at_unix: now_unix(),
            },
        );
        roles.insert(
            role_ids.workspace_owner.clone(),
            WorkspaceRoleRecord {
                role_id: role_ids.workspace_owner.clone(),
                name: String::from("workspace_owner"),
                position: 10,
                is_system: true,
                system_key: Some(String::from(SYSTEM_ROLE_WORKSPACE_OWNER)),
                permissions_allow: crate::server::permissions::all_permissions(),
                color_hex: None,
                created_at_unix: now_unix(),
            },
        );

        let summary = summarize_in_memory_guild_permissions(
            &roles,
            &HashSet::new(),
            Role::Moderator,
            &role_ids,
        );

        assert_eq!(summary.resolved_role, Role::Moderator);
        assert!(summary
            .guild_permissions
            .contains(Permission::DeleteMessage));
    }

    #[test]
    fn resolve_in_memory_channel_permissions_applies_override_layers() {
        let role_ids = super::RoleIdSet {
            everyone: String::from("everyone"),
            workspace_owner: String::from("owner"),
            member: String::from("member"),
            moderator: String::from("moderator"),
        };
        let mut assigned = HashSet::new();
        assigned.insert(role_ids.workspace_owner.clone());
        assigned.insert(role_ids.member.clone());
        assigned.insert(role_ids.moderator.clone());

        let user_id = UserId::new();
        let mut channel_override = ChannelPermissionOverrideRecord::default();
        channel_override.role_overrides.insert(
            role_ids.everyone.clone(),
            ChannelPermissionOverwrite {
                allow: permission_set(&[]),
                deny: permission_set(&[Permission::CreateMessage]),
            },
        );
        channel_override.role_overrides.insert(
            role_ids.member.clone(),
            ChannelPermissionOverwrite {
                allow: permission_set(&[]),
                deny: permission_set(&[Permission::DeleteMessage]),
            },
        );
        channel_override.member_overrides.insert(
            user_id,
            ChannelPermissionOverwrite {
                allow: permission_set(&[Permission::CreateMessage]),
                deny: permission_set(&[]),
            },
        );

        let resolved = resolve_in_memory_channel_permissions(
            permission_set(&[Permission::CreateMessage, Permission::DeleteMessage]),
            false,
            &channel_override,
            &assigned,
            &role_ids,
            user_id,
        );

        assert!(resolved.contains(Permission::CreateMessage));
        assert!(!resolved.contains(Permission::DeleteMessage));
    }

    #[test]
    fn resolve_db_channel_permissions_falls_back_to_legacy_overrides_when_new_are_invalid() {
        let role_ids = super::RoleIdSet {
            everyone: String::from("everyone"),
            workspace_owner: String::from("owner"),
            member: String::from("member"),
            moderator: String::from("moderator"),
        };
        let mut assigned = HashSet::new();
        assigned.insert(role_ids.member.clone());

        let (resolved, unknown_bits) = resolve_db_channel_permissions(
            permission_set(&[Permission::CreateMessage]),
            false,
            vec![super::ChannelOverrideDbRow {
                target_kind: 99,
                target_id: String::from("ignored"),
                allow_mask: i64::try_from(PermissionSet::empty().bits()).expect("fits"),
                deny_mask: i64::try_from(PermissionSet::empty().bits()).expect("fits"),
            }],
            Some(vec![
                super::LegacyChannelRoleOverrideDbRow {
                    role: 0,
                    allow_mask: i64::try_from(PermissionSet::empty().bits()).expect("fits"),
                    deny_mask: i64::try_from(permission_set(&[Permission::CreateMessage]).bits())
                        .expect("fits"),
                },
                super::LegacyChannelRoleOverrideDbRow {
                    role: 1,
                    allow_mask: i64::try_from(PermissionSet::empty().bits()).expect("fits"),
                    deny_mask: i64::try_from(permission_set(&[Permission::CreateMessage]).bits())
                        .expect("fits"),
                },
                super::LegacyChannelRoleOverrideDbRow {
                    role: 2,
                    allow_mask: i64::try_from(PermissionSet::empty().bits()).expect("fits"),
                    deny_mask: i64::try_from(permission_set(&[Permission::CreateMessage]).bits())
                        .expect("fits"),
                },
            ]),
            &assigned,
            &role_ids,
            UserId::new(),
            0,
            1,
        )
        .expect("resolution should succeed");

        assert!(!resolved.contains(Permission::CreateMessage));
        assert_eq!(unknown_bits, 0);
    }
}
