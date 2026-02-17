use std::collections::HashMap;

use filament_core::{ChannelPermissionOverwrite, Permission, PermissionSet, Role, UserId};
use sqlx::{PgPool, Row};
use ulid::Ulid;

mod moderation;
mod attachments;
mod permissions_eval;
mod reactions;

pub(crate) use attachments::{
    attachment_map_from_db_rows,
    attach_message_media,
    attachment_record_from_db_row,
    attachment_map_from_records, attachments_from_ids_in_memory,
    attachment_responses_from_db_rows,
    attachment_usage_total_from_db,
    attachment_usage_for_owner, parse_attachment_ids,
    validate_attachment_filename,
};
pub(crate) use moderation::{
    enforce_guild_ip_ban_for_request, guild_has_active_ip_ban_for_client,
};
pub(crate) use permissions_eval::{
    apply_legacy_role_assignment, ensure_required_roles,
    finalize_channel_permissions,
    guild_role_permission_inputs,
    resolve_guild_permission_summary,
    merge_legacy_channel_role_overrides,
    normalize_assigned_role_ids,
    role_ids_from_map, role_records_from_db_rows,
    summarize_in_memory_channel_overrides,
    summarize_channel_overrides,
    summarize_guild_permissions, sync_legacy_channel_overrides,
    sync_legacy_role_assignments,
};
pub(crate) use reactions::{
    attach_message_reactions, reaction_map_from_db_rows,
    reaction_summaries_from_users,
    validate_reaction_emoji,
};

use super::{
    auth::now_unix,
    core::{
        AppState, AttachmentRecord, ChannelPermissionOverrideRecord,
    },
    db::{ensure_db_schema, role_from_i16},
    errors::AuthFailure,
    permissions::{
        all_permissions, default_everyone_permissions,
    },
    types::{AttachmentPath, AttachmentResponse, ReactionResponse},
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

const OVERRIDE_TARGET_ROLE: i16 = 0;
const OVERRIDE_TARGET_MEMBER: i16 = 1;

fn is_server_owner(state: &AppState, user_id: UserId) -> bool {
    state
        .runtime
        .server_owner_user_id
        .is_some_and(|owner| owner == user_id)
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

    let role_ids = {
        let mut guild_roles = state.guild_roles.write().await;
        let roles = guild_roles.entry(guild_id.to_owned()).or_default();
        ensure_required_roles(roles)
    };

    {
        let mut role_assignments = state.guild_role_assignments.write().await;
        let guild_assignments = role_assignments.entry(guild_id.to_owned()).or_default();
        sync_legacy_role_assignments(&members, guild_assignments, &role_ids);
    }

    {
        let mut channel_overrides = state.guild_channel_permission_overrides.write().await;
        let guild_channel_overrides = channel_overrides.entry(guild_id.to_owned()).or_default();
        sync_legacy_channel_overrides(legacy_overrides, guild_channel_overrides, &role_ids);
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

    let role_inputs = role_rows
        .into_iter()
        .map(|row| {
            Ok(permissions_eval::GuildRoleDbRow {
                role_id: row.try_get("role_id").map_err(|_| AuthFailure::Internal)?,
                name: row.try_get("name").map_err(|_| AuthFailure::Internal)?,
                position: row.try_get("position").map_err(|_| AuthFailure::Internal)?,
                permissions_allow_mask: row
                    .try_get("permissions_allow_mask")
                    .map_err(|_| AuthFailure::Internal)?,
                is_system: row
                    .try_get("is_system")
                    .map_err(|_| AuthFailure::Internal)?,
                system_key: row
                    .try_get("system_key")
                    .map_err(|_| AuthFailure::Internal)?,
            })
        })
        .collect::<Result<Vec<_>, AuthFailure>>()?;

    let (roles, unknown_bits_seen, role_mask_updates) =
        role_records_from_db_rows(role_inputs)?;
    for update in role_mask_updates {
        sqlx::query(
            "UPDATE guild_roles
             SET permissions_allow_mask = $3
             WHERE guild_id = $1 AND role_id = $2",
        )
        .bind(guild_id)
        .bind(update.role_id)
        .bind(update.masked_permissions_allow)
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
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

    let assignment_role_ids = assignment_rows
        .into_iter()
        .map(|row| row.try_get("role_id").map_err(|_| AuthFailure::Internal))
        .collect::<Result<Vec<String>, AuthFailure>>()?;
    let assigned_role_ids = normalize_assigned_role_ids(
        assignment_role_ids,
        &roles,
        legacy_role,
        &role_ids,
    );

    let guild_permission_summary =
        resolve_guild_permission_summary(&roles, &assigned_role_ids, &role_ids);
    let guild_permissions = guild_permission_summary.guild_permissions;
    let is_workspace_owner = guild_permission_summary.is_workspace_owner;
    let resolved_role = guild_permission_summary.resolved_role;

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

    let override_inputs = override_rows
        .into_iter()
        .map(|row| {
            Ok(permissions_eval::ChannelOverrideDbRow {
                target_kind: row
                    .try_get("target_kind")
                    .map_err(|_| AuthFailure::Internal)?,
                target_id: row
                    .try_get("target_id")
                    .map_err(|_| AuthFailure::Internal)?,
                allow_mask: row
                    .try_get("allow_mask")
                    .map_err(|_| AuthFailure::Internal)?,
                deny_mask: row
                    .try_get("deny_mask")
                    .map_err(|_| AuthFailure::Internal)?,
            })
        })
        .collect::<Result<Vec<_>, AuthFailure>>()?;
    let override_summary = summarize_channel_overrides(
        override_inputs,
        &assigned_role_ids,
        &role_ids,
        user_id,
        OVERRIDE_TARGET_ROLE,
        OVERRIDE_TARGET_MEMBER,
    )?;
    let everyone_overwrite = override_summary.everyone_overwrite;
    let mut role_overwrite = override_summary.role_overwrite;
    let member_overwrite = override_summary.member_overwrite;
    let used_new_overrides = override_summary.used_new_overrides;
    let unknown_override_bits = override_summary.unknown_override_bits;

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

        let legacy_inputs = legacy_rows
            .into_iter()
            .map(|row| {
                Ok(permissions_eval::LegacyChannelRoleOverrideDbRow {
                    role: row.try_get("role").map_err(|_| AuthFailure::Internal)?,
                    allow_mask: row
                        .try_get("allow_mask")
                        .map_err(|_| AuthFailure::Internal)?,
                    deny_mask: row
                        .try_get("deny_mask")
                        .map_err(|_| AuthFailure::Internal)?,
                })
            })
            .collect::<Result<Vec<_>, AuthFailure>>()?;
        role_overwrite = merge_legacy_channel_role_overrides(
            role_overwrite,
            legacy_inputs,
            &assigned_role_ids,
            &role_ids,
        )?;
    }
    maybe_audit_unknown_permission_bits(
        state,
        guild_id,
        user_id,
        unknown_override_bits,
        "permissions.resolve.channel_overrides",
    )
    .await;

    let permissions = finalize_channel_permissions(
        guild_permissions,
        is_workspace_owner,
        everyone_overwrite,
        role_overwrite,
        member_overwrite,
    );

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
    apply_legacy_role_assignment(
        &mut assigned_role_ids,
        legacy_role,
        &role_ids.workspace_owner,
        &role_ids.moderator,
        &role_ids.member,
    );

    let (everyone_permissions, role_permissions) =
        guild_role_permission_inputs(roles, &role_ids.everyone);
    let guild_permission_summary = summarize_guild_permissions(
        everyone_permissions,
        &assigned_role_ids,
        &role_permissions,
        &role_ids.workspace_owner,
        &role_ids.moderator,
    );
    let guild_permissions = guild_permission_summary.guild_permissions;
    let is_workspace_owner = guild_permission_summary.is_workspace_owner;
    let resolved_role = guild_permission_summary.resolved_role;

    let Some(channel_id) = channel_id else {
        return Ok((resolved_role, guild_permissions));
    };

    let channel_overrides = state.guild_channel_permission_overrides.read().await;
    let channel_override = channel_overrides
        .get(guild_id)
        .and_then(|guild_overrides| guild_overrides.get(channel_id))
        .cloned()
        .unwrap_or_else(ChannelPermissionOverrideRecord::default);

    let override_summary = summarize_in_memory_channel_overrides(
        &channel_override,
        &assigned_role_ids,
        &role_ids,
        user_id,
    );
    let everyone_overwrite = override_summary.everyone_overwrite;
    let role_overwrite = override_summary.role_overwrite;
    let member_overwrite = override_summary.member_overwrite;

    let permissions = finalize_channel_permissions(
        guild_permissions,
        is_workspace_owner,
        everyone_overwrite,
        role_overwrite,
        member_overwrite,
    );
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
        return attachment_usage_total_from_db(total);
    }

    let attachments = state.attachments.read().await;
    Ok(attachment_usage_for_owner(attachments.values(), user_id))
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
        return attachment_record_from_db_row(attachments::AttachmentDbRow {
            attachment_id: row
                .try_get("attachment_id")
                .map_err(|_| AuthFailure::Internal)?,
            guild_id: row.try_get("guild_id").map_err(|_| AuthFailure::Internal)?,
            channel_id: row
                .try_get("channel_id")
                .map_err(|_| AuthFailure::Internal)?,
            owner_id: row.try_get("owner_id").map_err(|_| AuthFailure::Internal)?,
            filename: row.try_get("filename").map_err(|_| AuthFailure::Internal)?,
            mime_type: row.try_get("mime_type").map_err(|_| AuthFailure::Internal)?,
            size_bytes: row.try_get("size_bytes").map_err(|_| AuthFailure::Internal)?,
            sha256_hex: row.try_get("sha256_hex").map_err(|_| AuthFailure::Internal)?,
            object_key: row.try_get("object_key").map_err(|_| AuthFailure::Internal)?,
            message_id: row.try_get("message_id").map_err(|_| AuthFailure::Internal)?,
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
    let attachments = state.attachments.read().await;
    attachments_from_ids_in_memory(&attachments, attachment_ids)
}

pub(crate) fn rows_to_attachment_responses(
    rows: Vec<sqlx::postgres::PgRow>,
) -> Result<Vec<AttachmentResponse>, AuthFailure> {
    let mut attachment_rows = Vec::with_capacity(rows.len());
    for row in rows {
        attachment_rows.push(attachments::AttachmentResponseDbRow {
                attachment_id: row
                    .try_get("attachment_id")
                    .map_err(|_| AuthFailure::Internal)?,
                guild_id: row
                    .try_get("guild_id")
                    .map_err(|_| AuthFailure::Internal)?,
                channel_id: row
                    .try_get("channel_id")
                    .map_err(|_| AuthFailure::Internal)?,
                owner_id: row.try_get("owner_id").map_err(|_| AuthFailure::Internal)?,
                filename: row.try_get("filename").map_err(|_| AuthFailure::Internal)?,
                mime_type: row.try_get("mime_type").map_err(|_| AuthFailure::Internal)?,
                size_bytes: row
                    .try_get("size_bytes")
                    .map_err(|_| AuthFailure::Internal)?,
                sha256_hex: row
                    .try_get("sha256_hex")
                    .map_err(|_| AuthFailure::Internal)?,
            });
    }
    attachment_responses_from_db_rows(attachment_rows)
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

    let mut map_rows = Vec::with_capacity(rows.len());
    for row in rows {
        let message_id: Option<String> = row
            .try_get("message_id")
            .map_err(|_| AuthFailure::Internal)?;
        map_rows.push(attachments::AttachmentMapDbRow {
                message_id,
                response: attachments::AttachmentResponseDbRow {
                    attachment_id: row
                        .try_get("attachment_id")
                        .map_err(|_| AuthFailure::Internal)?,
                    guild_id: row
                        .try_get("guild_id")
                        .map_err(|_| AuthFailure::Internal)?,
                    channel_id: row
                        .try_get("channel_id")
                        .map_err(|_| AuthFailure::Internal)?,
                    owner_id: row
                        .try_get("owner_id")
                        .map_err(|_| AuthFailure::Internal)?,
                    filename: row
                        .try_get("filename")
                        .map_err(|_| AuthFailure::Internal)?,
                    mime_type: row
                        .try_get("mime_type")
                        .map_err(|_| AuthFailure::Internal)?,
                    size_bytes: row
                        .try_get("size_bytes")
                        .map_err(|_| AuthFailure::Internal)?,
                    sha256_hex: row
                        .try_get("sha256_hex")
                        .map_err(|_| AuthFailure::Internal)?,
                },
            });
    }
    attachment_map_from_db_rows(map_rows)
}

pub(crate) async fn attachment_map_for_messages_in_memory(
    state: &AppState,
    guild_id: &str,
    channel_id: Option<&str>,
    message_ids: &[String],
) -> HashMap<String, Vec<AttachmentResponse>> {
    let attachments = state.attachments.read().await;
    attachment_map_from_records(attachments.values(), guild_id, channel_id, message_ids)
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

    let mut count_rows = Vec::with_capacity(rows.len());
    for row in rows {
        count_rows.push(reactions::ReactionCountDbRow {
            message_id: row.try_get("message_id").map_err(|_| AuthFailure::Internal)?,
            emoji: row.try_get("emoji").map_err(|_| AuthFailure::Internal)?,
            count: row.try_get("count").map_err(|_| AuthFailure::Internal)?,
        });
    }
    reaction_map_from_db_rows(count_rows)
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
    use super::guild_permission_snapshot;
    use crate::server::{
        core::{AppConfig, AppState, GuildRecord, GuildVisibility},
        permissions::all_permissions,
    };
    use filament_core::{Role, UserId};
    use std::collections::{HashMap, HashSet};

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

}
