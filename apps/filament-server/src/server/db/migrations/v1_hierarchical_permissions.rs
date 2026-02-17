use filament_core::Role;
use sqlx::{Postgres, Row, Transaction};
use ulid::Ulid;

use crate::server::{
    db::role_from_i16,
    permissions::{
        all_permissions, default_everyone_permissions, default_member_permissions,
        default_moderator_permissions, DEFAULT_ROLE_MEMBER, DEFAULT_ROLE_MODERATOR,
        SYSTEM_ROLE_EVERYONE, SYSTEM_ROLE_WORKSPACE_OWNER,
    },
};

const TARGET_KIND_ROLE: i16 = 0;

fn now_unix() -> i64 {
    let now = std::time::SystemTime::now();
    let secs = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_secs(0))
        .as_secs();
    i64::try_from(secs).unwrap_or(i64::MAX)
}

#[derive(Debug)]
struct SeededGuildRoleIds {
    workspace_owner_role_id: String,
    member_role_id: String,
    moderator_role_id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TargetRoleKey {
    WorkspaceOwner,
    Member,
    Moderator,
}

fn target_role_key_for_legacy_role_value(role_value: i16) -> TargetRoleKey {
    match role_from_i16(role_value).unwrap_or(Role::Member) {
        Role::Owner => TargetRoleKey::WorkspaceOwner,
        Role::Member => TargetRoleKey::Member,
        Role::Moderator => TargetRoleKey::Moderator,
    }
}

fn target_role_id_by_key<'a>(role_ids: &'a SeededGuildRoleIds, key: TargetRoleKey) -> &'a str {
    match key {
        TargetRoleKey::WorkspaceOwner => &role_ids.workspace_owner_role_id,
        TargetRoleKey::Member => &role_ids.member_role_id,
        TargetRoleKey::Moderator => &role_ids.moderator_role_id,
    }
}

async fn ensure_seed_roles_for_guild(
    tx: &mut Transaction<'_, Postgres>,
    guild_id: &str,
) -> Result<SeededGuildRoleIds, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT role_id, name, system_key
         FROM guild_roles
         WHERE guild_id = $1",
    )
    .bind(guild_id)
    .fetch_all(&mut **tx)
    .await?;

    let mut everyone_role_id = None;
    let mut workspace_owner_role_id = None;
    let mut member_role_id = None;
    let mut moderator_role_id = None;

    for row in rows {
        let role_id: String = row.try_get("role_id")?;
        let name: String = row.try_get("name")?;
        let system_key = row.try_get::<Option<String>, _>("system_key")?;
        if system_key.as_deref() == Some(SYSTEM_ROLE_EVERYONE) {
            everyone_role_id = Some(role_id.clone());
        }
        if system_key.as_deref() == Some(SYSTEM_ROLE_WORKSPACE_OWNER) {
            workspace_owner_role_id = Some(role_id.clone());
        }
        if system_key.as_deref() == Some(DEFAULT_ROLE_MEMBER)
            || name.eq_ignore_ascii_case(DEFAULT_ROLE_MEMBER)
        {
            member_role_id = Some(role_id.clone());
        }
        if system_key.as_deref() == Some(DEFAULT_ROLE_MODERATOR)
            || name.eq_ignore_ascii_case(DEFAULT_ROLE_MODERATOR)
        {
            moderator_role_id = Some(role_id);
        }
    }

    if everyone_role_id.is_none() {
        let role_id = Ulid::new().to_string();
        sqlx::query(
            "INSERT INTO guild_roles
                (role_id, guild_id, name, position, permissions_allow_mask, is_system, system_key, created_at_unix)
             VALUES
                ($1, $2, $3, $4, $5, TRUE, $6, $7)",
        )
        .bind(&role_id)
        .bind(guild_id)
        .bind("@everyone")
        .bind(0_i32)
        .bind(i64::try_from(default_everyone_permissions().bits()).unwrap_or(i64::MAX))
        .bind(SYSTEM_ROLE_EVERYONE)
        .bind(now_unix())
        .execute(&mut **tx)
        .await?;
        everyone_role_id = Some(role_id);
    }

    if workspace_owner_role_id.is_none() {
        let role_id = Ulid::new().to_string();
        sqlx::query(
            "INSERT INTO guild_roles
                (role_id, guild_id, name, position, permissions_allow_mask, is_system, system_key, created_at_unix)
             VALUES
                ($1, $2, $3, $4, $5, TRUE, $6, $7)",
        )
        .bind(&role_id)
        .bind(guild_id)
        .bind("workspace_owner")
        .bind(10_000_i32)
        .bind(i64::try_from(all_permissions().bits()).unwrap_or(i64::MAX))
        .bind(SYSTEM_ROLE_WORKSPACE_OWNER)
        .bind(now_unix())
        .execute(&mut **tx)
        .await?;
        workspace_owner_role_id = Some(role_id);
    }

    if moderator_role_id.is_none() {
        let role_id = Ulid::new().to_string();
        sqlx::query(
            "INSERT INTO guild_roles
                (role_id, guild_id, name, position, permissions_allow_mask, is_system, system_key, created_at_unix)
             VALUES
                ($1, $2, $3, $4, $5, FALSE, $6, $7)",
        )
        .bind(&role_id)
        .bind(guild_id)
        .bind(DEFAULT_ROLE_MODERATOR)
        .bind(100_i32)
        .bind(i64::try_from(default_moderator_permissions().bits()).unwrap_or(i64::MAX))
        .bind(DEFAULT_ROLE_MODERATOR)
        .bind(now_unix())
        .execute(&mut **tx)
        .await?;
        moderator_role_id = Some(role_id);
    }

    if member_role_id.is_none() {
        let role_id = Ulid::new().to_string();
        sqlx::query(
            "INSERT INTO guild_roles
                (role_id, guild_id, name, position, permissions_allow_mask, is_system, system_key, created_at_unix)
             VALUES
                ($1, $2, $3, $4, $5, FALSE, $6, $7)",
        )
        .bind(&role_id)
        .bind(guild_id)
        .bind(DEFAULT_ROLE_MEMBER)
        .bind(1_i32)
        .bind(i64::try_from(default_member_permissions().bits()).unwrap_or(i64::MAX))
        .bind(DEFAULT_ROLE_MEMBER)
        .bind(now_unix())
        .execute(&mut **tx)
        .await?;
        member_role_id = Some(role_id);
    }

    let _everyone_role_id = everyone_role_id.expect("everyone role should be set");
    Ok(SeededGuildRoleIds {
        workspace_owner_role_id: workspace_owner_role_id
            .expect("workspace owner role should be set"),
        member_role_id: member_role_id.expect("member role should be set"),
        moderator_role_id: moderator_role_id.expect("moderator role should be set"),
    })
}

async fn backfill_role_assignments_for_guild(
    tx: &mut Transaction<'_, Postgres>,
    guild_id: &str,
    created_by_user_id: Option<&str>,
    role_ids: &SeededGuildRoleIds,
) -> Result<(), sqlx::Error> {
    let members = sqlx::query("SELECT user_id, role FROM guild_members WHERE guild_id = $1")
        .bind(guild_id)
        .fetch_all(&mut **tx)
        .await?;

    for row in members {
        let user_id: String = row.try_get("user_id")?;
        let role_value: i16 = row.try_get("role")?;
        let role_id = target_role_id_by_key(
            role_ids,
            target_role_key_for_legacy_role_value(role_value),
        );
        sqlx::query(
            "INSERT INTO guild_role_members (guild_id, role_id, user_id, assigned_at_unix)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (guild_id, role_id, user_id) DO NOTHING",
        )
        .bind(guild_id)
        .bind(role_id)
        .bind(user_id)
        .bind(now_unix())
        .execute(&mut **tx)
        .await?;
    }

    if let Some(owner_user_id) = created_by_user_id {
        sqlx::query(
            "INSERT INTO guild_role_members (guild_id, role_id, user_id, assigned_at_unix)
             SELECT $1, $2, $3, $4
             WHERE EXISTS (
                 SELECT 1 FROM guild_members WHERE guild_id = $1 AND user_id = $3
             )
             ON CONFLICT (guild_id, role_id, user_id) DO NOTHING",
        )
        .bind(guild_id)
        .bind(&role_ids.workspace_owner_role_id)
        .bind(owner_user_id)
        .bind(now_unix())
        .execute(&mut **tx)
        .await?;
    }

    let owner_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*)
         FROM guild_role_members grm
         JOIN guild_roles gr ON gr.role_id = grm.role_id
         WHERE grm.guild_id = $1
           AND gr.system_key = $2",
    )
    .bind(guild_id)
    .bind(SYSTEM_ROLE_WORKSPACE_OWNER)
    .fetch_one(&mut **tx)
    .await?;
    if owner_count == 0 {
        sqlx::query(
            "INSERT INTO guild_role_members (guild_id, role_id, user_id, assigned_at_unix)
             SELECT $1, $2, gm.user_id, $3
             FROM guild_members gm
             WHERE gm.guild_id = $1
             ORDER BY gm.role DESC, gm.user_id ASC
             LIMIT 1
             ON CONFLICT (guild_id, role_id, user_id) DO NOTHING",
        )
        .bind(guild_id)
        .bind(&role_ids.workspace_owner_role_id)
        .bind(now_unix())
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

async fn backfill_channel_role_overrides_for_guild(
    tx: &mut Transaction<'_, Postgres>,
    guild_id: &str,
    role_ids: &SeededGuildRoleIds,
) -> Result<(), sqlx::Error> {
    let rows = sqlx::query(
        "SELECT channel_id, role, allow_mask, deny_mask
         FROM channel_role_overrides
         WHERE guild_id = $1",
    )
    .bind(guild_id)
    .fetch_all(&mut **tx)
    .await?;

    for row in rows {
        let channel_id: String = row.try_get("channel_id")?;
        let role_value: i16 = row.try_get("role")?;
        let target_role_id = target_role_id_by_key(
            role_ids,
            target_role_key_for_legacy_role_value(role_value),
        );
        let allow_mask: i64 = row.try_get("allow_mask")?;
        let deny_mask: i64 = row.try_get("deny_mask")?;

        sqlx::query(
            "INSERT INTO channel_permission_overrides
                (guild_id, channel_id, target_kind, target_id, allow_mask, deny_mask)
             VALUES
                ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (guild_id, channel_id, target_kind, target_id)
             DO UPDATE SET allow_mask = EXCLUDED.allow_mask, deny_mask = EXCLUDED.deny_mask",
        )
        .bind(guild_id)
        .bind(channel_id)
        .bind(TARGET_KIND_ROLE)
        .bind(target_role_id)
        .bind(allow_mask)
        .bind(deny_mask)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

pub(crate) async fn seed_hierarchical_permissions_for_new_guild(
    tx: &mut Transaction<'_, Postgres>,
    guild_id: &str,
    creator_user_id: &str,
) -> Result<(), sqlx::Error> {
    let role_ids = ensure_seed_roles_for_guild(tx, guild_id).await?;
    backfill_role_assignments_for_guild(tx, guild_id, Some(creator_user_id), &role_ids).await?;
    Ok(())
}

pub(crate) async fn backfill_hierarchical_permission_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    let guild_rows = sqlx::query("SELECT guild_id, created_by_user_id FROM guilds")
        .fetch_all(&mut **tx)
        .await?;

    for row in guild_rows {
        let guild_id: String = row.try_get("guild_id")?;
        let created_by_user_id = row.try_get::<Option<String>, _>("created_by_user_id")?;
        let role_ids = ensure_seed_roles_for_guild(tx, &guild_id).await?;
        backfill_role_assignments_for_guild(
            tx,
            &guild_id,
            created_by_user_id.as_deref(),
            &role_ids,
        )
        .await?;
        backfill_channel_role_overrides_for_guild(tx, &guild_id, &role_ids).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{target_role_key_for_legacy_role_value, TargetRoleKey};

    #[test]
    fn target_role_key_defaults_to_member_for_unknown_legacy_role_values() {
        assert_eq!(
            target_role_key_for_legacy_role_value(99),
            TargetRoleKey::Member
        );
    }

    #[test]
    fn target_role_key_maps_all_supported_legacy_role_values() {
        assert_eq!(
            target_role_key_for_legacy_role_value(2),
            TargetRoleKey::WorkspaceOwner
        );
        assert_eq!(
            target_role_key_for_legacy_role_value(1),
            TargetRoleKey::Moderator
        );
        assert_eq!(
            target_role_key_for_legacy_role_value(0),
            TargetRoleKey::Member
        );
    }
}