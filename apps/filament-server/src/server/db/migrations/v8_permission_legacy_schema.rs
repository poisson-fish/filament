use sqlx::{Postgres, Transaction};

const CREATE_CHANNEL_ROLE_OVERRIDES_TABLE_SQL: &str =
    "CREATE TABLE IF NOT EXISTS channel_role_overrides (
                    guild_id TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
                    channel_id TEXT NOT NULL REFERENCES channels(channel_id) ON DELETE CASCADE,
                    role SMALLINT NOT NULL,
                    allow_mask BIGINT NOT NULL,
                    deny_mask BIGINT NOT NULL,
                    PRIMARY KEY(guild_id, channel_id, role)
                )";
const CREATE_GUILD_ROLES_TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS guild_roles (
                    role_id TEXT PRIMARY KEY,
                    guild_id TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
                    name TEXT NOT NULL,
                    position INTEGER NOT NULL,
                    permissions_allow_mask BIGINT NOT NULL,
                    is_system BOOLEAN NOT NULL DEFAULT FALSE,
                    system_key TEXT NULL,
                    created_at_unix BIGINT NOT NULL
                )";
const CREATE_GUILD_ROLE_MEMBERS_TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS guild_role_members (
                    guild_id TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
                    role_id TEXT NOT NULL REFERENCES guild_roles(role_id) ON DELETE CASCADE,
                    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    assigned_at_unix BIGINT NOT NULL,
                    PRIMARY KEY(guild_id, role_id, user_id)
                )";
const CREATE_CHANNEL_PERMISSION_OVERRIDES_TABLE_SQL: &str =
    "CREATE TABLE IF NOT EXISTS channel_permission_overrides (
                    guild_id TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
                    channel_id TEXT NOT NULL REFERENCES channels(channel_id) ON DELETE CASCADE,
                    target_kind SMALLINT NOT NULL,
                    target_id TEXT NOT NULL,
                    allow_mask BIGINT NOT NULL,
                    deny_mask BIGINT NOT NULL,
                    PRIMARY KEY(guild_id, channel_id, target_kind, target_id)
                )";
const CREATE_GUILD_ROLES_GUILD_POSITION_INDEX_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_guild_roles_guild_position
                    ON guild_roles(guild_id, position DESC, created_at_unix ASC)";
const CREATE_GUILD_ROLES_GUILD_SYSTEM_KEY_UNIQUE_INDEX_SQL: &str =
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_guild_roles_guild_system_key_unique
                    ON guild_roles(guild_id, system_key)
                    WHERE system_key IS NOT NULL";
const CREATE_GUILD_ROLE_MEMBERS_GUILD_USER_INDEX_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_guild_role_members_guild_user
                    ON guild_role_members(guild_id, user_id)";

pub(crate) async fn apply_permission_legacy_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    sqlx::query(CREATE_CHANNEL_ROLE_OVERRIDES_TABLE_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(CREATE_GUILD_ROLES_TABLE_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(CREATE_GUILD_ROLE_MEMBERS_TABLE_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(CREATE_CHANNEL_PERMISSION_OVERRIDES_TABLE_SQL)
        .execute(&mut **tx)
        .await?;

    sqlx::query(CREATE_GUILD_ROLES_GUILD_POSITION_INDEX_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(CREATE_GUILD_ROLES_GUILD_SYSTEM_KEY_UNIQUE_INDEX_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(CREATE_GUILD_ROLE_MEMBERS_GUILD_USER_INDEX_SQL)
        .execute(&mut **tx)
        .await?;

    let guilds: Vec<(String,)> = sqlx::query_as("SELECT guild_id FROM guilds")
        .fetch_all(&mut **tx)
        .await?;

    for (guild_id,) in guilds {
        let owner_role_id = ulid::Ulid::new().to_string();
        let mod_role_id = ulid::Ulid::new().to_string();
        let everyone_role_id = ulid::Ulid::new().to_string();
        let now = 0_i64;

        // Owner mask: 4095
        sqlx::query(
            "INSERT INTO guild_roles (role_id, guild_id, name, position, permissions_allow_mask, is_system, system_key, created_at_unix)
             VALUES ($1, $2, 'Owner', 999, 4095, true, 'owner', $3)"
        )
        .bind(&owner_role_id).bind(&guild_id).bind(now)
        .execute(&mut **tx).await?;

        // Moderator mask: 4082
        sqlx::query(
            "INSERT INTO guild_roles (role_id, guild_id, name, position, permissions_allow_mask, is_system, system_key, created_at_unix)
             VALUES ($1, $2, 'Moderator', 100, 4082, true, 'moderator', $3)"
        )
        .bind(&mod_role_id).bind(&guild_id).bind(now)
        .execute(&mut **tx).await?;

        // Everyone mask: 2304
        sqlx::query(
            "INSERT INTO guild_roles (role_id, guild_id, name, position, permissions_allow_mask, is_system, system_key, created_at_unix)
             VALUES ($1, $2, '@everyone', 0, 2304, true, 'everyone', $3)"
        )
        .bind(&everyone_role_id).bind(&guild_id).bind(now)
        .execute(&mut **tx).await?;

        // role=2 translates to Owner
        sqlx::query(
            "INSERT INTO guild_role_members (guild_id, role_id, user_id, assigned_at_unix)
             SELECT gm.guild_id, $1, gm.user_id, $2
             FROM guild_members gm
             WHERE gm.guild_id = $3 AND gm.role = 2"
        )
        .bind(&owner_role_id).bind(now).bind(&guild_id)
        .execute(&mut **tx).await?;

        // role=1 translates to Moderator
        sqlx::query(
            "INSERT INTO guild_role_members (guild_id, role_id, user_id, assigned_at_unix)
             SELECT gm.guild_id, $1, gm.user_id, $2
             FROM guild_members gm
             WHERE gm.guild_id = $3 AND gm.role = 1"
        )
        .bind(&mod_role_id).bind(now).bind(&guild_id)
        .execute(&mut **tx).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        CREATE_CHANNEL_PERMISSION_OVERRIDES_TABLE_SQL, CREATE_CHANNEL_ROLE_OVERRIDES_TABLE_SQL,
        CREATE_GUILD_ROLES_GUILD_POSITION_INDEX_SQL,
        CREATE_GUILD_ROLES_GUILD_SYSTEM_KEY_UNIQUE_INDEX_SQL, CREATE_GUILD_ROLES_TABLE_SQL,
        CREATE_GUILD_ROLE_MEMBERS_GUILD_USER_INDEX_SQL, CREATE_GUILD_ROLE_MEMBERS_TABLE_SQL,
    };

    #[test]
    fn phase_one_schema_statements_define_required_tables_and_indexes() {
        assert!(CREATE_CHANNEL_ROLE_OVERRIDES_TABLE_SQL.contains("channel_role_overrides"));
        assert!(CREATE_GUILD_ROLES_TABLE_SQL.contains("guild_roles"));
        assert!(CREATE_GUILD_ROLE_MEMBERS_TABLE_SQL.contains("guild_role_members"));
        assert!(
            CREATE_CHANNEL_PERMISSION_OVERRIDES_TABLE_SQL.contains("channel_permission_overrides")
        );
        assert!(
            CREATE_GUILD_ROLES_GUILD_POSITION_INDEX_SQL.contains("idx_guild_roles_guild_position")
        );
        assert!(CREATE_GUILD_ROLES_GUILD_SYSTEM_KEY_UNIQUE_INDEX_SQL
            .contains("idx_guild_roles_guild_system_key_unique"));
        assert!(CREATE_GUILD_ROLE_MEMBERS_GUILD_USER_INDEX_SQL
            .contains("idx_guild_role_members_guild_user"));
    }
}
