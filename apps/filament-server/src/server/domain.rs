use std::collections::{HashMap, HashSet};

use filament_core::{
    apply_channel_overwrite, base_permissions, ChannelPermissionOverwrite, Permission,
    PermissionSet, Role, UserId,
};
use sqlx::{PgPool, Row};
use ulid::Ulid;

use super::{
    auth::{now_unix, ClientIp},
    core::{AppState, AttachmentRecord, MAX_ATTACHMENTS_PER_MESSAGE, MAX_REACTION_EMOJI_CHARS},
    db::{ensure_db_schema, permission_set_from_i64, role_from_i16},
    directory_contract::IpNetwork,
    errors::AuthFailure,
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

pub(crate) async fn channel_permission_snapshot(
    state: &AppState,
    user_id: UserId,
    guild_id: &str,
    channel_id: &str,
) -> Result<(Role, PermissionSet), AuthFailure> {
    if let Some(pool) = &state.db_pool {
        if ensure_db_schema(state).await.is_err() {
            return Err(AuthFailure::Internal);
        }
        let banned = sqlx::query("SELECT 1 FROM guild_bans WHERE guild_id = $1 AND user_id = $2")
            .bind(guild_id)
            .bind(user_id.to_string())
            .fetch_optional(pool)
            .await
            .map_err(|_| AuthFailure::Internal)?
            .is_some();
        if banned {
            return Err(AuthFailure::Forbidden);
        }
        let row = sqlx::query(
            "SELECT gm.role, co.allow_mask, co.deny_mask
             FROM guild_members gm
             JOIN channels c ON c.guild_id = gm.guild_id AND c.channel_id = $3
             LEFT JOIN channel_role_overrides co
               ON co.guild_id = gm.guild_id
              AND co.channel_id = c.channel_id
              AND co.role = gm.role
             WHERE gm.guild_id = $1 AND gm.user_id = $2 AND c.channel_id = $3",
        )
        .bind(guild_id)
        .bind(user_id.to_string())
        .bind(channel_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        let Some(row) = row else {
            return Err(AuthFailure::Forbidden);
        };
        let role_value: i16 = row.try_get("role").map_err(|_| AuthFailure::Internal)?;
        let role = role_from_i16(role_value).ok_or(AuthFailure::Forbidden)?;
        let allow_mask = row.try_get::<Option<i64>, _>("allow_mask").ok().flatten();
        let deny_mask = row.try_get::<Option<i64>, _>("deny_mask").ok().flatten();
        let overwrite = if let (Some(allow), Some(deny)) = (allow_mask, deny_mask) {
            Some(ChannelPermissionOverwrite {
                allow: permission_set_from_i64(allow)?,
                deny: permission_set_from_i64(deny)?,
            })
        } else {
            None
        };
        return Ok((
            role,
            apply_channel_overwrite(base_permissions(role), overwrite),
        ));
    }

    let guilds = state.guilds.read().await;
    let Some(guild) = guilds.get(guild_id) else {
        return Err(AuthFailure::NotFound);
    };
    let Some(role) = guild.members.get(&user_id).copied() else {
        return Err(AuthFailure::Forbidden);
    };
    if guild.banned_members.contains(&user_id) {
        return Err(AuthFailure::Forbidden);
    }
    let channel = guild
        .channels
        .get(channel_id)
        .ok_or(AuthFailure::NotFound)?;
    let overwrite = channel.role_overrides.get(&role).copied();
    Ok((
        role,
        apply_channel_overwrite(base_permissions(role), overwrite),
    ))
}

pub(crate) async fn user_role_in_guild(
    state: &AppState,
    user_id: UserId,
    guild_id: &str,
) -> Result<Role, AuthFailure> {
    if let Some(pool) = &state.db_pool {
        let banned = sqlx::query("SELECT 1 FROM guild_bans WHERE guild_id = $1 AND user_id = $2")
            .bind(guild_id)
            .bind(user_id.to_string())
            .fetch_optional(pool)
            .await
            .map_err(|_| AuthFailure::Internal)?;
        if banned.is_some() {
            return Err(AuthFailure::Forbidden);
        }

        let row =
            sqlx::query("SELECT role FROM guild_members WHERE guild_id = $1 AND user_id = $2")
                .bind(guild_id)
                .bind(user_id.to_string())
                .fetch_optional(pool)
                .await
                .map_err(|_| AuthFailure::Internal)?;
        let row = row.ok_or(AuthFailure::Forbidden)?;
        let role_value: i16 = row.try_get("role").map_err(|_| AuthFailure::Internal)?;
        return role_from_i16(role_value).ok_or(AuthFailure::Forbidden);
    }

    let guilds = state.guilds.read().await;
    let guild = guilds.get(guild_id).ok_or(AuthFailure::NotFound)?;
    if guild.banned_members.contains(&user_id) {
        return Err(AuthFailure::Forbidden);
    }
    guild
        .members
        .get(&user_id)
        .copied()
        .ok_or(AuthFailure::Forbidden)
}

pub(crate) async fn member_role_in_guild(
    state: &AppState,
    user_id: UserId,
    guild_id: &str,
) -> Result<Role, AuthFailure> {
    if let Some(pool) = &state.db_pool {
        let row =
            sqlx::query("SELECT role FROM guild_members WHERE guild_id = $1 AND user_id = $2")
                .bind(guild_id)
                .bind(user_id.to_string())
                .fetch_optional(pool)
                .await
                .map_err(|_| AuthFailure::Internal)?;
        let row = row.ok_or(AuthFailure::NotFound)?;
        let role_value: i16 = row.try_get("role").map_err(|_| AuthFailure::Internal)?;
        return role_from_i16(role_value).ok_or(AuthFailure::Forbidden);
    }

    let guilds = state.guilds.read().await;
    let guild = guilds.get(guild_id).ok_or(AuthFailure::NotFound)?;
    guild
        .members
        .get(&user_id)
        .copied()
        .ok_or(AuthFailure::NotFound)
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
    use super::guild_has_active_ip_ban_for_client;
    use crate::server::{
        auth::resolve_client_ip,
        core::{AppConfig, AppState, GuildIpBanRecord},
        directory_contract::IpNetwork,
    };
    use axum::http::HeaderMap;
    use filament_core::UserId;
    use ulid::Ulid;

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
}
