use std::collections::{HashMap, HashSet};

use filament_core::{ChannelPermissionOverwrite, Permission, PermissionSet, Role, UserId};
use sqlx::{PgPool, Row};
use ulid::Ulid;

use super::{
    auth::{now_unix, ClientIp},
    core::{
        AppState, AttachmentRecord, ChannelPermissionOverrideRecord, WorkspaceRoleRecord,
        MAX_ATTACHMENTS_PER_MESSAGE, MAX_REACTION_EMOJI_CHARS,
    },
    db::{ensure_db_schema, role_from_i16},
    directory_contract::IpNetwork,
    errors::AuthFailure,
    permissions::{
        all_permissions, default_everyone_permissions, default_member_permissions,
        default_moderator_permissions, mask_permissions, membership_to_legacy_role,
        DEFAULT_ROLE_MEMBER, DEFAULT_ROLE_MODERATOR, SYSTEM_ROLE_EVERYONE,
        SYSTEM_ROLE_WORKSPACE_OWNER,
    },
    types::{AttachmentPath, AttachmentResponse, MessageResponse, ReactionResponse},
};

pub(crate) async fn user_can_write_channel(
    state: &AppState,
    user_id: UserId,
    guild_id: &str,
    channel_id: &str,
) -> bool {
    channel_permission_snapshot(state, user_id, guild_id, channel_id)
        .await
        .ok()
        .is_some_and(|(_, permissions)| permissions.contains(Permission::CreateMessage))
}

pub(crate) async fn guild_has_active_ip_ban_for_client(
    state: &AppState,
    guild_id: &str,
    client_ip: ClientIp,
) -> Result<bool, AuthFailure> {
    let Some(ip) = client_ip.ip() else {
        return Ok(false);
    };
    let now = now_unix();

    if let Some(pool) = &state.db_pool {
        let rows = sqlx::query(
            "SELECT ip_cidr
             FROM guild_ip_bans
             WHERE guild_id = $1
               AND (expires_at_unix IS NULL OR expires_at_unix > $2)
             ORDER BY created_at_unix DESC
             LIMIT $3",
        )
        .bind(guild_id)
        .bind(now)
        .bind(
            i64::try_from(state.runtime.guild_ip_ban_max_entries)
                .map_err(|_| AuthFailure::Internal)?,
        )
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        for row in rows {
            let cidr: String = row.try_get("ip_cidr").map_err(|_| AuthFailure::Internal)?;
            let Ok(network) = IpNetwork::try_from(cidr) else {
                continue;
            };
            if network.contains(ip) {
                return Ok(true);
            }
        }
        return Ok(false);
    }

    let bans = state.guild_ip_bans.read().await;
    let Some(guild_bans) = bans.get(guild_id) else {
        return Ok(false);
    };
    Ok(guild_bans.iter().any(|entry| {
        entry.expires_at_unix.is_none_or(|expires| expires > now) && entry.ip_network.contains(ip)
    }))
}

pub(crate) async fn enforce_guild_ip_ban_for_request(
    state: &AppState,
    guild_id: &str,
    user_id: UserId,
    client_ip: ClientIp,
    surface: &'static str,
) -> Result<(), AuthFailure> {
    if !guild_has_active_ip_ban_for_client(state, guild_id, client_ip).await? {
        return Ok(());
    }
    write_audit_log(
        state,
        Some(guild_id.to_owned()),
        user_id,
        Some(user_id),
        "moderation.ip_ban.hit",
        serde_json::json!({
            "surface": surface,
            "client_ip_source": client_ip.source().as_str(),
        }),
    )
    .await?;
    Err(AuthFailure::Forbidden)
}

const OVERRIDE_TARGET_ROLE: i16 = 0;
const OVERRIDE_TARGET_MEMBER: i16 = 1;

#[derive(Debug)]
struct RoleIdSet {
    everyone: String,
    workspace_owner: String,
    member: String,
    moderator: String,
}

fn is_server_owner(state: &AppState, user_id: UserId) -> bool {
    state
        .runtime
        .server_owner_user_id
        .is_some_and(|owner| owner == user_id)
}

fn i64_to_masked_permissions(value: i64) -> Result<(PermissionSet, u64), AuthFailure> {
    let raw = u64::try_from(value).map_err(|_| AuthFailure::Internal)?;
    Ok(mask_permissions(raw))
}

fn role_ids_from_map(roles: &HashMap<String, WorkspaceRoleRecord>) -> Option<RoleIdSet> {
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

fn normalize_layer(allow_bits: u64, deny_bits: u64) -> (u64, u64) {
    (allow_bits & !deny_bits, deny_bits)
}

fn apply_channel_layers(
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

async fn ensure_in_memory_permission_model_for_guild(
    state: &AppState,
    guild_id: &str,
) -> Result<(), AuthFailure> {
    let (members, legacy_overrides) = {
        let guilds = state.guilds.read().await;
        let guild = guilds.get(guild_id).ok_or(AuthFailure::NotFound)?;
        let members = guild.members.clone();
        let mut overrides: HashMap<String, HashMap<Role, ChannelPermissionOverwrite>> =
            HashMap::new();
        for (channel_id, channel) in &guild.channels {
            overrides.insert(channel_id.clone(), channel.role_overrides.clone());
        }
        (members, overrides)
    };

    let (everyone_role_id, workspace_owner_role_id, member_role_id, moderator_role_id) = {
        let mut guild_roles = state.guild_roles.write().await;
        let roles = guild_roles.entry(guild_id.to_owned()).or_default();

        let everyone_role_id = roles
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
                        created_at_unix: now_unix(),
                    },
                );
                role_id
            });

        let workspace_owner_role_id = roles
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
                        created_at_unix: now_unix(),
                    },
                );
                role_id
            });

        let moderator_role_id = roles
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
                        created_at_unix: now_unix(),
                    },
                );
                role_id
            });

        let member_role_id = roles
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
                        created_at_unix: now_unix(),
                    },
                );
                role_id
            });

        (
            everyone_role_id,
            workspace_owner_role_id,
            member_role_id,
            moderator_role_id,
        )
    };

    {
        let mut role_assignments = state.guild_role_assignments.write().await;
        let guild_assignments = role_assignments.entry(guild_id.to_owned()).or_default();
        guild_assignments.retain(|member, _| members.contains_key(member));
        for (member_id, legacy_role) in &members {
            let assigned = guild_assignments.entry(*member_id).or_default();
            assigned.retain(|role_id| {
                role_id != &workspace_owner_role_id
                    && role_id != &moderator_role_id
                    && role_id != &member_role_id
            });
            match legacy_role {
                Role::Owner => {
                    assigned.insert(workspace_owner_role_id.clone());
                }
                Role::Moderator => {
                    assigned.insert(moderator_role_id.clone());
                }
                Role::Member => {
                    assigned.insert(member_role_id.clone());
                }
            }
            // `@everyone` is implicit during resolution and does not need assignment rows.
            let _ = &everyone_role_id;
        }
    }

    {
        let mut channel_overrides = state.guild_channel_permission_overrides.write().await;
        let guild_channel_overrides = channel_overrides.entry(guild_id.to_owned()).or_default();
        for (channel_id, legacy) in legacy_overrides {
            let channel_entry = guild_channel_overrides.entry(channel_id).or_default();
            if let Some(overwrite) = legacy.get(&Role::Member).copied() {
                channel_entry
                    .role_overrides
                    .entry(member_role_id.clone())
                    .or_insert(overwrite);
            }
            if let Some(overwrite) = legacy.get(&Role::Moderator).copied() {
                channel_entry
                    .role_overrides
                    .entry(moderator_role_id.clone())
                    .or_insert(overwrite);
            }
            if let Some(overwrite) = legacy.get(&Role::Owner).copied() {
                channel_entry
                    .role_overrides
                    .entry(workspace_owner_role_id.clone())
                    .or_insert(overwrite);
            }
        }
    }

    Ok(())
}

async fn maybe_audit_unknown_permission_bits(
    state: &AppState,
    guild_id: &str,
    user_id: UserId,
    unknown_bits: u64,
    surface: &'static str,
) {
    if unknown_bits == 0 {
        return;
    }
    tracing::warn!(
        event = "permissions.unknown_bits.masked",
        guild_id = %guild_id,
        user_id = %user_id,
        surface = %surface,
        unknown_bits = format_args!("{unknown_bits:#x}")
    );
    let _ = write_audit_log(
        state,
        Some(guild_id.to_owned()),
        user_id,
        None,
        "permissions.unknown_bits.masked",
        serde_json::json!({
            "surface": surface,
            "unknown_bits": format!("{unknown_bits:#x}"),
        }),
    )
    .await;
}

#[allow(clippy::too_many_lines)]
async fn resolve_channel_permissions_db(
    state: &AppState,
    pool: &PgPool,
    user_id: UserId,
    guild_id: &str,
    channel_id: Option<&str>,
) -> Result<(Role, PermissionSet), AuthFailure> {
    if ensure_db_schema(state).await.is_err() {
        return Err(AuthFailure::Internal);
    }

    if is_server_owner(state, user_id) {
        if let Some(channel_id) = channel_id {
            let exists = sqlx::query(
                "SELECT 1
                 FROM channels
                 WHERE guild_id = $1 AND channel_id = $2",
            )
            .bind(guild_id)
            .bind(channel_id)
            .fetch_optional(pool)
            .await
            .map_err(|_| AuthFailure::Internal)?
            .is_some();
            if !exists {
                return Err(AuthFailure::NotFound);
            }
        } else {
            let exists = sqlx::query("SELECT 1 FROM guilds WHERE guild_id = $1")
                .bind(guild_id)
                .fetch_optional(pool)
                .await
                .map_err(|_| AuthFailure::Internal)?
                .is_some();
            if !exists {
                return Err(AuthFailure::NotFound);
            }
        }
        return Ok((Role::Owner, all_permissions()));
    }

    let membership_row = sqlx::query(
        "SELECT gm.role
         FROM guild_members gm
         LEFT JOIN guild_bans gb ON gb.guild_id = gm.guild_id AND gb.user_id = gm.user_id
         WHERE gm.guild_id = $1
           AND gm.user_id = $2
           AND gb.user_id IS NULL",
    )
    .bind(guild_id)
    .bind(user_id.to_string())
    .fetch_optional(pool)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let membership_row = membership_row.ok_or(AuthFailure::Forbidden)?;
    let legacy_role = role_from_i16(
        membership_row
            .try_get::<i16, _>("role")
            .map_err(|_| AuthFailure::Internal)?,
    )
    .unwrap_or(Role::Member);

    let role_rows = sqlx::query(
        "SELECT role_id, name, position, permissions_allow_mask, is_system, system_key
         FROM guild_roles
         WHERE guild_id = $1",
    )
    .bind(guild_id)
    .fetch_all(pool)
    .await
    .map_err(|_| AuthFailure::Internal)?;

    let mut roles: HashMap<String, WorkspaceRoleRecord> = HashMap::new();
    let mut unknown_bits_seen = 0_u64;
    for row in role_rows {
        let role_id: String = row.try_get("role_id").map_err(|_| AuthFailure::Internal)?;
        let permissions_allow_mask: i64 = row
            .try_get("permissions_allow_mask")
            .map_err(|_| AuthFailure::Internal)?;
        let (permissions_allow, unknown_bits) = i64_to_masked_permissions(permissions_allow_mask)?;
        if unknown_bits > 0 {
            unknown_bits_seen |= unknown_bits;
            let masked_i64 =
                i64::try_from(permissions_allow.bits()).map_err(|_| AuthFailure::Internal)?;
            sqlx::query(
                "UPDATE guild_roles
                 SET permissions_allow_mask = $3
                 WHERE guild_id = $1 AND role_id = $2",
            )
            .bind(guild_id)
            .bind(&role_id)
            .bind(masked_i64)
            .execute(pool)
            .await
            .map_err(|_| AuthFailure::Internal)?;
        }
        roles.insert(
            role_id.clone(),
            WorkspaceRoleRecord {
                role_id,
                name: row.try_get("name").map_err(|_| AuthFailure::Internal)?,
                position: row.try_get("position").map_err(|_| AuthFailure::Internal)?,
                is_system: row
                    .try_get("is_system")
                    .map_err(|_| AuthFailure::Internal)?,
                system_key: row
                    .try_get("system_key")
                    .map_err(|_| AuthFailure::Internal)?,
                permissions_allow,
                created_at_unix: 0,
            },
        );
    }
    maybe_audit_unknown_permission_bits(
        state,
        guild_id,
        user_id,
        unknown_bits_seen,
        "permissions.resolve.guild",
    )
    .await;

    let Some(role_ids) = role_ids_from_map(&roles) else {
        // Defensive fallback for legacy guilds before role backfill.
        return Ok((legacy_role, default_everyone_permissions()));
    };

    let assignment_rows = sqlx::query(
        "SELECT role_id
         FROM guild_role_members
         WHERE guild_id = $1 AND user_id = $2",
    )
    .bind(guild_id)
    .bind(user_id.to_string())
    .fetch_all(pool)
    .await
    .map_err(|_| AuthFailure::Internal)?;

    let mut assigned_role_ids = HashSet::new();
    for row in assignment_rows {
        let role_id: String = row.try_get("role_id").map_err(|_| AuthFailure::Internal)?;
        if roles.contains_key(&role_id) {
            assigned_role_ids.insert(role_id);
        }
    }

    match legacy_role {
        Role::Owner => {
            assigned_role_ids.insert(role_ids.workspace_owner.clone());
        }
        Role::Moderator => {
            assigned_role_ids.insert(role_ids.moderator.clone());
        }
        Role::Member => {
            assigned_role_ids.insert(role_ids.member.clone());
        }
    }

    let mut guild_permissions = roles
        .get(&role_ids.everyone)
        .map_or_else(default_everyone_permissions, |role| role.permissions_allow);
    for role_id in &assigned_role_ids {
        if let Some(role) = roles.get(role_id) {
            guild_permissions =
                PermissionSet::from_bits(guild_permissions.bits() | role.permissions_allow.bits());
        }
    }

    let is_workspace_owner = assigned_role_ids.contains(&role_ids.workspace_owner);
    if is_workspace_owner {
        guild_permissions = all_permissions();
    }

    let resolved_role = membership_to_legacy_role(
        &assigned_role_ids,
        &role_ids.workspace_owner,
        &role_ids.moderator,
    );

    let Some(channel_id) = channel_id else {
        return Ok((resolved_role, guild_permissions));
    };

    let channel_exists = sqlx::query(
        "SELECT 1
         FROM channels
         WHERE guild_id = $1 AND channel_id = $2",
    )
    .bind(guild_id)
    .bind(channel_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| AuthFailure::Internal)?
    .is_some();
    if !channel_exists {
        return Err(AuthFailure::NotFound);
    }

    let mut everyone_overwrite = ChannelPermissionOverwrite::default();
    let mut role_overwrite = ChannelPermissionOverwrite::default();
    let mut member_overwrite = ChannelPermissionOverwrite::default();

    let override_rows = sqlx::query(
        "SELECT target_kind, target_id, allow_mask, deny_mask
         FROM channel_permission_overrides
         WHERE guild_id = $1 AND channel_id = $2",
    )
    .bind(guild_id)
    .bind(channel_id)
    .fetch_all(pool)
    .await
    .map_err(|_| AuthFailure::Internal)?;

    let mut used_new_overrides = !override_rows.is_empty();
    let mut unknown_override_bits = 0_u64;
    for row in override_rows {
        let target_kind: i16 = row
            .try_get("target_kind")
            .map_err(|_| AuthFailure::Internal)?;
        let target_id: String = row
            .try_get("target_id")
            .map_err(|_| AuthFailure::Internal)?;
        let allow_mask: i64 = row
            .try_get("allow_mask")
            .map_err(|_| AuthFailure::Internal)?;
        let deny_mask: i64 = row
            .try_get("deny_mask")
            .map_err(|_| AuthFailure::Internal)?;
        let (allow, unknown_allow) = i64_to_masked_permissions(allow_mask)?;
        let (deny, unknown_deny) = i64_to_masked_permissions(deny_mask)?;
        unknown_override_bits |= unknown_allow | unknown_deny;
        let overwrite = ChannelPermissionOverwrite { allow, deny };

        match target_kind {
            OVERRIDE_TARGET_ROLE => {
                if target_id == role_ids.everyone {
                    everyone_overwrite = overwrite;
                } else if assigned_role_ids.contains(&target_id) {
                    role_overwrite = ChannelPermissionOverwrite {
                        allow: PermissionSet::from_bits(
                            role_overwrite.allow.bits() | overwrite.allow.bits(),
                        ),
                        deny: PermissionSet::from_bits(
                            role_overwrite.deny.bits() | overwrite.deny.bits(),
                        ),
                    };
                }
            }
            OVERRIDE_TARGET_MEMBER => {
                if target_id == user_id.to_string() {
                    member_overwrite = overwrite;
                }
            }
            _ => {
                used_new_overrides = false;
            }
        }
    }

    if !used_new_overrides {
        let legacy_rows = sqlx::query(
            "SELECT role, allow_mask, deny_mask
             FROM channel_role_overrides
             WHERE guild_id = $1 AND channel_id = $2",
        )
        .bind(guild_id)
        .bind(channel_id)
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;

        for row in legacy_rows {
            let role_value: i16 = row.try_get("role").map_err(|_| AuthFailure::Internal)?;
            let role = role_from_i16(role_value).unwrap_or(Role::Member);
            let allow_mask: i64 = row
                .try_get("allow_mask")
                .map_err(|_| AuthFailure::Internal)?;
            let deny_mask: i64 = row
                .try_get("deny_mask")
                .map_err(|_| AuthFailure::Internal)?;
            let (allow, _) = i64_to_masked_permissions(allow_mask)?;
            let (deny, _) = i64_to_masked_permissions(deny_mask)?;
            let overwrite = ChannelPermissionOverwrite { allow, deny };
            match role {
                Role::Member => {
                    if assigned_role_ids.contains(&role_ids.member) {
                        role_overwrite = ChannelPermissionOverwrite {
                            allow: PermissionSet::from_bits(
                                role_overwrite.allow.bits() | overwrite.allow.bits(),
                            ),
                            deny: PermissionSet::from_bits(
                                role_overwrite.deny.bits() | overwrite.deny.bits(),
                            ),
                        };
                    }
                }
                Role::Moderator => {
                    if assigned_role_ids.contains(&role_ids.moderator) {
                        role_overwrite = ChannelPermissionOverwrite {
                            allow: PermissionSet::from_bits(
                                role_overwrite.allow.bits() | overwrite.allow.bits(),
                            ),
                            deny: PermissionSet::from_bits(
                                role_overwrite.deny.bits() | overwrite.deny.bits(),
                            ),
                        };
                    }
                }
                Role::Owner => {
                    if assigned_role_ids.contains(&role_ids.workspace_owner) {
                        role_overwrite = ChannelPermissionOverwrite {
                            allow: PermissionSet::from_bits(
                                role_overwrite.allow.bits() | overwrite.allow.bits(),
                            ),
                            deny: PermissionSet::from_bits(
                                role_overwrite.deny.bits() | overwrite.deny.bits(),
                            ),
                        };
                    }
                }
            }
        }
    }
    maybe_audit_unknown_permission_bits(
        state,
        guild_id,
        user_id,
        unknown_override_bits,
        "permissions.resolve.channel_overrides",
    )
    .await;

    let permissions = if is_workspace_owner {
        all_permissions()
    } else {
        apply_channel_layers(
            guild_permissions,
            everyone_overwrite,
            role_overwrite,
            member_overwrite,
        )
    };

    Ok((resolved_role, permissions))
}

#[allow(clippy::too_many_lines)]
async fn resolve_channel_permissions_in_memory(
    state: &AppState,
    user_id: UserId,
    guild_id: &str,
    channel_id: Option<&str>,
) -> Result<(Role, PermissionSet), AuthFailure> {
    ensure_in_memory_permission_model_for_guild(state, guild_id).await?;

    let guilds = state.guilds.read().await;
    let guild = guilds.get(guild_id).ok_or(AuthFailure::NotFound)?;

    if is_server_owner(state, user_id) {
        if let Some(channel_id) = channel_id {
            if !guild.channels.contains_key(channel_id) {
                return Err(AuthFailure::NotFound);
            }
        }
        return Ok((Role::Owner, all_permissions()));
    }

    if guild.banned_members.contains(&user_id) {
        return Err(AuthFailure::Forbidden);
    }
    let legacy_role = guild
        .members
        .get(&user_id)
        .copied()
        .ok_or(AuthFailure::Forbidden)?;
    if let Some(channel_id) = channel_id {
        if !guild.channels.contains_key(channel_id) {
            return Err(AuthFailure::NotFound);
        }
    }
    drop(guilds);

    let guild_roles = state.guild_roles.read().await;
    let roles = guild_roles.get(guild_id).ok_or(AuthFailure::Forbidden)?;
    let role_ids = role_ids_from_map(roles).ok_or(AuthFailure::Forbidden)?;

    let guild_assignments = state.guild_role_assignments.read().await;
    let assigned_role_ids = guild_assignments
        .get(guild_id)
        .and_then(|assignments| assignments.get(&user_id).cloned())
        .unwrap_or_default();
    drop(guild_assignments);

    let mut assigned_role_ids = assigned_role_ids;
    match legacy_role {
        Role::Owner => {
            assigned_role_ids.insert(role_ids.workspace_owner.clone());
        }
        Role::Moderator => {
            assigned_role_ids.insert(role_ids.moderator.clone());
        }
        Role::Member => {
            assigned_role_ids.insert(role_ids.member.clone());
        }
    }

    let mut guild_permissions = roles
        .get(&role_ids.everyone)
        .map_or_else(default_everyone_permissions, |role| role.permissions_allow);
    for role_id in &assigned_role_ids {
        if let Some(role) = roles.get(role_id) {
            guild_permissions =
                PermissionSet::from_bits(guild_permissions.bits() | role.permissions_allow.bits());
        }
    }

    let is_workspace_owner = assigned_role_ids.contains(&role_ids.workspace_owner);
    if is_workspace_owner {
        guild_permissions = all_permissions();
    }
    let resolved_role = membership_to_legacy_role(
        &assigned_role_ids,
        &role_ids.workspace_owner,
        &role_ids.moderator,
    );

    let Some(channel_id) = channel_id else {
        return Ok((resolved_role, guild_permissions));
    };

    let channel_overrides = state.guild_channel_permission_overrides.read().await;
    let channel_override = channel_overrides
        .get(guild_id)
        .and_then(|guild_overrides| guild_overrides.get(channel_id))
        .cloned()
        .unwrap_or_else(ChannelPermissionOverrideRecord::default);

    let everyone_overwrite = channel_override
        .role_overrides
        .get(&role_ids.everyone)
        .copied()
        .unwrap_or_default();
    let mut role_overwrite = ChannelPermissionOverwrite::default();
    for role_id in &assigned_role_ids {
        if let Some(overwrite) = channel_override.role_overrides.get(role_id).copied() {
            role_overwrite = ChannelPermissionOverwrite {
                allow: PermissionSet::from_bits(
                    role_overwrite.allow.bits() | overwrite.allow.bits(),
                ),
                deny: PermissionSet::from_bits(role_overwrite.deny.bits() | overwrite.deny.bits()),
            };
        }
    }
    let member_overwrite = channel_override
        .member_overrides
        .get(&user_id)
        .copied()
        .unwrap_or_default();

    let permissions = if is_workspace_owner {
        all_permissions()
    } else {
        apply_channel_layers(
            guild_permissions,
            everyone_overwrite,
            role_overwrite,
            member_overwrite,
        )
    };
    Ok((resolved_role, permissions))
}

pub(crate) async fn channel_permission_snapshot(
    state: &AppState,
    user_id: UserId,
    guild_id: &str,
    channel_id: &str,
) -> Result<(Role, PermissionSet), AuthFailure> {
    if let Some(pool) = &state.db_pool {
        return resolve_channel_permissions_db(state, pool, user_id, guild_id, Some(channel_id))
            .await;
    }
    resolve_channel_permissions_in_memory(state, user_id, guild_id, Some(channel_id)).await
}

pub(crate) async fn guild_permission_snapshot(
    state: &AppState,
    user_id: UserId,
    guild_id: &str,
) -> Result<(Role, PermissionSet), AuthFailure> {
    if let Some(pool) = &state.db_pool {
        return resolve_channel_permissions_db(state, pool, user_id, guild_id, None).await;
    }
    resolve_channel_permissions_in_memory(state, user_id, guild_id, None).await
}

pub(crate) async fn user_role_in_guild(
    state: &AppState,
    user_id: UserId,
    guild_id: &str,
) -> Result<Role, AuthFailure> {
    guild_permission_snapshot(state, user_id, guild_id)
        .await
        .map(|(role, _)| role)
}

pub(crate) async fn member_role_in_guild(
    state: &AppState,
    user_id: UserId,
    guild_id: &str,
) -> Result<Role, AuthFailure> {
    if let Some(pool) = &state.db_pool {
        let exists = sqlx::query(
            "SELECT 1
             FROM guild_members
             WHERE guild_id = $1 AND user_id = $2",
        )
        .bind(guild_id)
        .bind(user_id.to_string())
        .fetch_optional(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?
        .is_some();
        if !exists {
            return Err(AuthFailure::NotFound);
        }
        return guild_permission_snapshot(state, user_id, guild_id)
            .await
            .map(|(role, _)| role);
    }

    let guilds = state.guilds.read().await;
    let guild = guilds.get(guild_id).ok_or(AuthFailure::NotFound)?;
    if !guild.members.contains_key(&user_id) {
        return Err(AuthFailure::NotFound);
    }
    drop(guilds);
    guild_permission_snapshot(state, user_id, guild_id)
        .await
        .map(|(role, _)| role)
}

pub(crate) async fn attachment_usage_for_user(
    state: &AppState,
    user_id: UserId,
) -> Result<u64, AuthFailure> {
    if let Some(pool) = &state.db_pool {
        let row = sqlx::query(
            "SELECT COALESCE(SUM(size_bytes)::BIGINT, 0) AS total FROM attachments WHERE owner_id = $1",
        )
        .bind(user_id.to_string())
        .fetch_one(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        let total: i64 = row.try_get("total").map_err(|_| AuthFailure::Internal)?;
        return u64::try_from(total).map_err(|_| AuthFailure::Internal);
    }

    let usage = state
        .attachments
        .read()
        .await
        .values()
        .filter(|record| record.owner_id == user_id)
        .map(|record| record.size_bytes)
        .sum();
    Ok(usage)
}

pub(crate) async fn find_attachment(
    state: &AppState,
    path: &AttachmentPath,
) -> Result<AttachmentRecord, AuthFailure> {
    if let Some(pool) = &state.db_pool {
        let row = sqlx::query(
            "SELECT attachment_id, guild_id, channel_id, owner_id, filename, mime_type, size_bytes, sha256_hex, object_key, message_id
             FROM attachments
             WHERE attachment_id = $1 AND guild_id = $2 AND channel_id = $3",
        )
        .bind(&path.attachment_id)
        .bind(&path.guild_id)
        .bind(&path.channel_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        let row = row.ok_or(AuthFailure::NotFound)?;
        let owner_id: String = row.try_get("owner_id").map_err(|_| AuthFailure::Internal)?;
        let size_bytes: i64 = row
            .try_get("size_bytes")
            .map_err(|_| AuthFailure::Internal)?;
        return Ok(AttachmentRecord {
            attachment_id: row
                .try_get("attachment_id")
                .map_err(|_| AuthFailure::Internal)?,
            guild_id: row.try_get("guild_id").map_err(|_| AuthFailure::Internal)?,
            channel_id: row
                .try_get("channel_id")
                .map_err(|_| AuthFailure::Internal)?,
            owner_id: UserId::try_from(owner_id).map_err(|_| AuthFailure::Internal)?,
            filename: row.try_get("filename").map_err(|_| AuthFailure::Internal)?,
            mime_type: row
                .try_get("mime_type")
                .map_err(|_| AuthFailure::Internal)?,
            size_bytes: u64::try_from(size_bytes).map_err(|_| AuthFailure::Internal)?,
            sha256_hex: row
                .try_get("sha256_hex")
                .map_err(|_| AuthFailure::Internal)?,
            object_key: row
                .try_get("object_key")
                .map_err(|_| AuthFailure::Internal)?,
            message_id: row
                .try_get("message_id")
                .map_err(|_| AuthFailure::Internal)?,
        });
    }
    state
        .attachments
        .read()
        .await
        .get(&path.attachment_id)
        .filter(|record| record.guild_id == path.guild_id && record.channel_id == path.channel_id)
        .cloned()
        .ok_or(AuthFailure::NotFound)
}

pub(crate) fn parse_attachment_ids(value: Vec<String>) -> Result<Vec<String>, AuthFailure> {
    if value.len() > MAX_ATTACHMENTS_PER_MESSAGE {
        return Err(AuthFailure::InvalidRequest);
    }

    let mut deduped = Vec::with_capacity(value.len());
    let mut seen = HashSet::with_capacity(value.len());
    for attachment_id in value {
        if Ulid::from_string(&attachment_id).is_err() {
            return Err(AuthFailure::InvalidRequest);
        }
        if seen.insert(attachment_id.clone()) {
            deduped.push(attachment_id);
        }
    }
    Ok(deduped)
}

pub(crate) async fn bind_message_attachments_db(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    attachment_ids: &[String],
    message_id: &str,
    guild_id: &str,
    channel_id: &str,
    owner_id: UserId,
) -> Result<(), AuthFailure> {
    if attachment_ids.is_empty() {
        return Ok(());
    }

    let update_result = sqlx::query(
        "UPDATE attachments
         SET message_id = $1
         WHERE attachment_id = ANY($2::text[])
           AND guild_id = $3
           AND channel_id = $4
           AND owner_id = $5
           AND message_id IS NULL",
    )
    .bind(message_id)
    .bind(attachment_ids)
    .bind(guild_id)
    .bind(channel_id)
    .bind(owner_id.to_string())
    .execute(&mut **tx)
    .await
    .map_err(|_| AuthFailure::Internal)?;

    let updated =
        usize::try_from(update_result.rows_affected()).map_err(|_| AuthFailure::Internal)?;
    if updated != attachment_ids.len() {
        return Err(AuthFailure::InvalidRequest);
    }
    Ok(())
}

pub(crate) async fn fetch_attachments_for_message_db(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &str,
    channel_id: &str,
    message_id: &str,
) -> Result<Vec<AttachmentResponse>, AuthFailure> {
    let rows = sqlx::query(
        "SELECT attachment_id, guild_id, channel_id, owner_id, filename, mime_type, size_bytes, sha256_hex
         FROM attachments
         WHERE guild_id = $1 AND channel_id = $2 AND message_id = $3
         ORDER BY created_at_unix ASC, attachment_id ASC",
    )
    .bind(guild_id)
    .bind(channel_id)
    .bind(message_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    rows_to_attachment_responses(rows)
}

pub(crate) async fn attachments_for_message_in_memory(
    state: &AppState,
    attachment_ids: &[String],
) -> Result<Vec<AttachmentResponse>, AuthFailure> {
    if attachment_ids.is_empty() {
        return Ok(Vec::new());
    }
    let attachments = state.attachments.read().await;
    let mut out = Vec::with_capacity(attachment_ids.len());
    for attachment_id in attachment_ids {
        let Some(record) = attachments.get(attachment_id) else {
            return Err(AuthFailure::InvalidRequest);
        };
        out.push(AttachmentResponse {
            attachment_id: record.attachment_id.clone(),
            guild_id: record.guild_id.clone(),
            channel_id: record.channel_id.clone(),
            owner_id: record.owner_id.to_string(),
            filename: record.filename.clone(),
            mime_type: record.mime_type.clone(),
            size_bytes: record.size_bytes,
            sha256_hex: record.sha256_hex.clone(),
        });
    }
    Ok(out)
}

pub(crate) fn rows_to_attachment_responses(
    rows: Vec<sqlx::postgres::PgRow>,
) -> Result<Vec<AttachmentResponse>, AuthFailure> {
    let mut attachments = Vec::with_capacity(rows.len());
    for row in rows {
        let size_bytes: i64 = row
            .try_get("size_bytes")
            .map_err(|_| AuthFailure::Internal)?;
        attachments.push(AttachmentResponse {
            attachment_id: row
                .try_get("attachment_id")
                .map_err(|_| AuthFailure::Internal)?,
            guild_id: row.try_get("guild_id").map_err(|_| AuthFailure::Internal)?,
            channel_id: row
                .try_get("channel_id")
                .map_err(|_| AuthFailure::Internal)?,
            owner_id: row.try_get("owner_id").map_err(|_| AuthFailure::Internal)?,
            filename: row.try_get("filename").map_err(|_| AuthFailure::Internal)?,
            mime_type: row
                .try_get("mime_type")
                .map_err(|_| AuthFailure::Internal)?,
            size_bytes: u64::try_from(size_bytes).map_err(|_| AuthFailure::Internal)?,
            sha256_hex: row
                .try_get("sha256_hex")
                .map_err(|_| AuthFailure::Internal)?,
        });
    }
    Ok(attachments)
}

pub(crate) async fn attachment_map_for_messages_db(
    pool: &PgPool,
    guild_id: &str,
    channel_id: Option<&str>,
    message_ids: &[String],
) -> Result<HashMap<String, Vec<AttachmentResponse>>, AuthFailure> {
    if message_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = if let Some(channel_id) = channel_id {
        sqlx::query(
            "SELECT attachment_id, guild_id, channel_id, owner_id, filename, mime_type, size_bytes, sha256_hex, message_id
             FROM attachments
             WHERE guild_id = $1 AND channel_id = $2 AND message_id = ANY($3::text[])
             ORDER BY created_at_unix ASC, attachment_id ASC",
        )
        .bind(guild_id)
        .bind(channel_id)
        .bind(message_ids)
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?
    } else {
        sqlx::query(
            "SELECT attachment_id, guild_id, channel_id, owner_id, filename, mime_type, size_bytes, sha256_hex, message_id
             FROM attachments
             WHERE guild_id = $1 AND message_id = ANY($2::text[])
             ORDER BY created_at_unix ASC, attachment_id ASC",
        )
        .bind(guild_id)
        .bind(message_ids)
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?
    };

    let mut by_message: HashMap<String, Vec<AttachmentResponse>> = HashMap::new();
    for row in rows {
        let message_id: Option<String> = row
            .try_get("message_id")
            .map_err(|_| AuthFailure::Internal)?;
        let Some(message_id) = message_id else {
            continue;
        };
        let size_bytes: i64 = row
            .try_get("size_bytes")
            .map_err(|_| AuthFailure::Internal)?;
        by_message
            .entry(message_id)
            .or_default()
            .push(AttachmentResponse {
                attachment_id: row
                    .try_get("attachment_id")
                    .map_err(|_| AuthFailure::Internal)?,
                guild_id: row.try_get("guild_id").map_err(|_| AuthFailure::Internal)?,
                channel_id: row
                    .try_get("channel_id")
                    .map_err(|_| AuthFailure::Internal)?,
                owner_id: row.try_get("owner_id").map_err(|_| AuthFailure::Internal)?,
                filename: row.try_get("filename").map_err(|_| AuthFailure::Internal)?,
                mime_type: row
                    .try_get("mime_type")
                    .map_err(|_| AuthFailure::Internal)?,
                size_bytes: u64::try_from(size_bytes).map_err(|_| AuthFailure::Internal)?,
                sha256_hex: row
                    .try_get("sha256_hex")
                    .map_err(|_| AuthFailure::Internal)?,
            });
    }
    Ok(by_message)
}

pub(crate) async fn attachment_map_for_messages_in_memory(
    state: &AppState,
    guild_id: &str,
    channel_id: Option<&str>,
    message_ids: &[String],
) -> HashMap<String, Vec<AttachmentResponse>> {
    if message_ids.is_empty() {
        return HashMap::new();
    }
    let wanted: HashSet<&str> = message_ids.iter().map(String::as_str).collect();
    let attachments = state.attachments.read().await;
    let mut by_message: HashMap<String, Vec<AttachmentResponse>> = HashMap::new();
    for record in attachments.values() {
        let Some(message_id) = record.message_id.as_deref() else {
            continue;
        };
        if record.guild_id != guild_id {
            continue;
        }
        if channel_id.is_some_and(|cid| cid != record.channel_id) {
            continue;
        }
        if !wanted.contains(message_id) {
            continue;
        }
        by_message
            .entry(message_id.to_owned())
            .or_default()
            .push(AttachmentResponse {
                attachment_id: record.attachment_id.clone(),
                guild_id: record.guild_id.clone(),
                channel_id: record.channel_id.clone(),
                owner_id: record.owner_id.to_string(),
                filename: record.filename.clone(),
                mime_type: record.mime_type.clone(),
                size_bytes: record.size_bytes,
                sha256_hex: record.sha256_hex.clone(),
            });
    }
    for values in by_message.values_mut() {
        values.sort_by(|a, b| a.attachment_id.cmp(&b.attachment_id));
    }
    by_message
}

pub(crate) fn attach_message_media(
    messages: &mut [MessageResponse],
    attachment_map: &HashMap<String, Vec<AttachmentResponse>>,
) {
    for message in messages {
        message.attachments = attachment_map
            .get(&message.message_id)
            .cloned()
            .unwrap_or_default();
    }
}

pub(crate) fn attach_message_reactions(
    messages: &mut [MessageResponse],
    reaction_map: &HashMap<String, Vec<ReactionResponse>>,
) {
    for message in messages {
        message.reactions = reaction_map
            .get(&message.message_id)
            .cloned()
            .unwrap_or_default();
    }
}

pub(crate) fn reaction_summaries_from_users(
    reactions: &HashMap<String, HashSet<UserId>>,
) -> Vec<ReactionResponse> {
    let mut summaries = Vec::with_capacity(reactions.len());
    for (emoji, users) in reactions {
        summaries.push(ReactionResponse {
            emoji: emoji.clone(),
            count: users.len(),
        });
    }
    summaries.sort_by(|left, right| left.emoji.cmp(&right.emoji));
    summaries
}

pub(crate) async fn reaction_map_for_messages_db(
    pool: &PgPool,
    guild_id: &str,
    channel_id: Option<&str>,
    message_ids: &[String],
) -> Result<HashMap<String, Vec<ReactionResponse>>, AuthFailure> {
    if message_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = if let Some(channel_id) = channel_id {
        sqlx::query(
            "SELECT message_id, emoji, COUNT(*) AS count
             FROM message_reactions
             WHERE guild_id = $1 AND channel_id = $2 AND message_id = ANY($3::text[])
             GROUP BY message_id, emoji",
        )
        .bind(guild_id)
        .bind(channel_id)
        .bind(message_ids)
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?
    } else {
        sqlx::query(
            "SELECT message_id, emoji, COUNT(*) AS count
             FROM message_reactions
             WHERE guild_id = $1 AND message_id = ANY($2::text[])
             GROUP BY message_id, emoji",
        )
        .bind(guild_id)
        .bind(message_ids)
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?
    };

    let mut by_message: HashMap<String, Vec<ReactionResponse>> = HashMap::new();
    for row in rows {
        let message_id: String = row
            .try_get("message_id")
            .map_err(|_| AuthFailure::Internal)?;
        let emoji: String = row.try_get("emoji").map_err(|_| AuthFailure::Internal)?;
        let count: i64 = row.try_get("count").map_err(|_| AuthFailure::Internal)?;
        by_message
            .entry(message_id)
            .or_default()
            .push(ReactionResponse {
                emoji,
                count: usize::try_from(count).map_err(|_| AuthFailure::Internal)?,
            });
    }
    for reactions in by_message.values_mut() {
        reactions.sort_by(|left, right| left.emoji.cmp(&right.emoji));
    }
    Ok(by_message)
}

pub(crate) fn validate_attachment_filename(value: String) -> Result<String, AuthFailure> {
    if value.is_empty() || value.len() > 128 {
        return Err(AuthFailure::InvalidRequest);
    }
    if value.contains('/') || value.contains('\\') || value.contains('\0') {
        return Err(AuthFailure::InvalidRequest);
    }
    Ok(value)
}

pub(crate) fn validate_reaction_emoji(value: &str) -> Result<(), AuthFailure> {
    if value.is_empty() || value.chars().count() > MAX_REACTION_EMOJI_CHARS {
        return Err(AuthFailure::InvalidRequest);
    }
    if value.chars().any(char::is_whitespace) {
        return Err(AuthFailure::InvalidRequest);
    }
    Ok(())
}

pub(crate) async fn write_audit_log(
    state: &AppState,
    guild_id: Option<String>,
    actor_user_id: UserId,
    target_user_id: Option<UserId>,
    action: &str,
    details_json: serde_json::Value,
) -> Result<(), AuthFailure> {
    let audit_id = Ulid::new().to_string();
    let created_at_unix = now_unix();
    if let Some(pool) = &state.db_pool {
        sqlx::query(
            "INSERT INTO audit_logs (audit_id, guild_id, actor_user_id, target_user_id, action, details_json, created_at_unix)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(audit_id)
        .bind(guild_id)
        .bind(actor_user_id.to_string())
        .bind(target_user_id.map(|value| value.to_string()))
        .bind(action)
        .bind(details_json.to_string())
        .bind(created_at_unix)
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        return Ok(());
    }

    state.audit_logs.write().await.push(serde_json::json!({
        "audit_id": audit_id,
        "guild_id": guild_id,
        "actor_user_id": actor_user_id.to_string(),
        "target_user_id": target_user_id.map(|value| value.to_string()),
        "action": action,
        "details": details_json,
        "created_at_unix": created_at_unix,
    }));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        apply_channel_layers, guild_has_active_ip_ban_for_client, guild_permission_snapshot,
    };
    use crate::server::{
        auth::resolve_client_ip,
        core::{AppConfig, AppState, GuildIpBanRecord, GuildRecord, GuildVisibility},
        directory_contract::IpNetwork,
        permissions::all_permissions,
    };
    use axum::http::HeaderMap;
    use filament_core::{ChannelPermissionOverwrite, Permission, PermissionSet, Role, UserId};
    use std::collections::{HashMap, HashSet};
    use ulid::Ulid;

    fn permission_set(values: &[Permission]) -> PermissionSet {
        let mut set = PermissionSet::empty();
        for value in values {
            set.insert(*value);
        }
        set
    }

    #[tokio::test]
    async fn guild_ip_ban_matching_handles_ipv4_and_ipv6_host_observations() {
        let state = AppState::new(&AppConfig::default()).expect("state initializes");
        let guild_id = String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV");
        let source_user_id = UserId::new();
        let now = crate::server::auth::now_unix();

        state.guild_ip_bans.write().await.insert(
            guild_id.clone(),
            vec![
                GuildIpBanRecord {
                    ban_id: Ulid::new().to_string(),
                    ip_network: IpNetwork::host("203.0.113.41".parse().expect("ipv4 parses")),
                    source_user_id: Some(source_user_id),
                    reason: String::from("ipv4 host"),
                    created_at_unix: now,
                    expires_at_unix: None,
                },
                GuildIpBanRecord {
                    ban_id: Ulid::new().to_string(),
                    ip_network: IpNetwork::host("2001:db8::42".parse().expect("ipv6 parses")),
                    source_user_id: Some(source_user_id),
                    reason: String::from("ipv6 host"),
                    created_at_unix: now,
                    expires_at_unix: None,
                },
            ],
        );

        let headers = HeaderMap::new();
        let ipv4_client = resolve_client_ip(
            &headers,
            Some("203.0.113.41".parse().expect("peer ip parses")),
            &[],
        );
        let ipv6_client = resolve_client_ip(
            &headers,
            Some("2001:db8::42".parse().expect("peer ip parses")),
            &[],
        );
        let other_client = resolve_client_ip(
            &headers,
            Some("198.51.100.91".parse().expect("peer ip parses")),
            &[],
        );

        assert!(
            guild_has_active_ip_ban_for_client(&state, &guild_id, ipv4_client)
                .await
                .expect("ipv4 check succeeds")
        );
        assert!(
            guild_has_active_ip_ban_for_client(&state, &guild_id, ipv6_client)
                .await
                .expect("ipv6 check succeeds")
        );
        assert!(
            !guild_has_active_ip_ban_for_client(&state, &guild_id, other_client)
                .await
                .expect("non-matching check succeeds")
        );
    }

    #[tokio::test]
    async fn server_owner_bypass_grants_all_permissions_without_membership() {
        let server_owner = UserId::new();
        let guild_creator = UserId::new();
        let mut config = AppConfig::default();
        config.server_owner_user_id = Some(server_owner);
        let state = AppState::new(&config).expect("state initializes");
        let guild_id = String::from("01ARZ3NDEKTSV4RRFFQ69G5FFF");

        state.guilds.write().await.insert(
            guild_id.clone(),
            GuildRecord {
                name: String::from("phase7"),
                visibility: GuildVisibility::Private,
                created_by_user_id: guild_creator,
                members: HashMap::new(),
                banned_members: HashSet::new(),
                channels: HashMap::new(),
            },
        );

        let (resolved_role, permissions) =
            guild_permission_snapshot(&state, server_owner, &guild_id)
                .await
                .expect("server owner permission resolution should succeed");
        assert_eq!(resolved_role, Role::Owner);
        assert_eq!(permissions.bits(), all_permissions().bits());
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
}
