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
                    color_hex TEXT NULL,
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
const LEGACY_PERMISSION_SCHEMA_STATEMENTS: [&str; 7] = [
    CREATE_CHANNEL_ROLE_OVERRIDES_TABLE_SQL,
    CREATE_GUILD_ROLES_TABLE_SQL,
    CREATE_GUILD_ROLE_MEMBERS_TABLE_SQL,
    CREATE_CHANNEL_PERMISSION_OVERRIDES_TABLE_SQL,
    CREATE_GUILD_ROLES_GUILD_POSITION_INDEX_SQL,
    CREATE_GUILD_ROLES_GUILD_SYSTEM_KEY_UNIQUE_INDEX_SQL,
    CREATE_GUILD_ROLE_MEMBERS_GUILD_USER_INDEX_SQL,
];

pub(crate) async fn apply_permission_legacy_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    for statement in LEGACY_PERMISSION_SCHEMA_STATEMENTS {
        sqlx::query(statement).execute(&mut **tx).await?;
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
        LEGACY_PERMISSION_SCHEMA_STATEMENTS,
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

    #[test]
    fn phase_one_schema_migration_stays_schema_only() {
        assert_eq!(LEGACY_PERMISSION_SCHEMA_STATEMENTS.len(), 7);
        assert!(LEGACY_PERMISSION_SCHEMA_STATEMENTS
            .into_iter()
            .all(|statement| statement.trim_start().starts_with("CREATE ")));
    }
}
