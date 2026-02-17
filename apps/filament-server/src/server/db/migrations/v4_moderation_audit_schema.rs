use sqlx::{Postgres, Transaction};

const CREATE_USER_IP_OBSERVATIONS_TABLE_SQL: &str =
    "CREATE TABLE IF NOT EXISTS user_ip_observations (
                    observation_id TEXT PRIMARY KEY,
                    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    ip_cidr TEXT NOT NULL,
                    first_seen_at_unix BIGINT NOT NULL,
                    last_seen_at_unix BIGINT NOT NULL
                )";
const CREATE_USER_IP_OBSERVATIONS_UNIQUE_INDEX_SQL: &str =
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_user_ip_observations_user_ip_unique
                    ON user_ip_observations(user_id, ip_cidr)";
const CREATE_GUILD_IP_BANS_TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS guild_ip_bans (
                    ban_id TEXT PRIMARY KEY,
                    guild_id TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
                    ip_cidr TEXT NOT NULL,
                    source_user_id TEXT NULL REFERENCES users(user_id) ON DELETE SET NULL,
                    reason TEXT NOT NULL,
                    created_by_user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    created_at_unix BIGINT NOT NULL,
                    expires_at_unix BIGINT NULL
                )";
const CREATE_USER_IP_OBSERVATIONS_USER_LAST_SEEN_INDEX_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_user_ip_observations_user_last_seen
                    ON user_ip_observations(user_id, last_seen_at_unix DESC)";
const CREATE_GUILD_IP_BANS_GUILD_CREATED_INDEX_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_guild_ip_bans_guild_created
                    ON guild_ip_bans(guild_id, created_at_unix DESC)";
const CREATE_AUDIT_LOGS_TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS audit_logs (
                    audit_id TEXT PRIMARY KEY,
                    guild_id TEXT NULL,
                    actor_user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    target_user_id TEXT NULL,
                    action TEXT NOT NULL,
                    details_json TEXT NOT NULL,
                    created_at_unix BIGINT NOT NULL
                )";
const CREATE_AUDIT_LOGS_GUILD_CREATED_INDEX_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_audit_logs_guild_created
                    ON audit_logs(guild_id, created_at_unix DESC)";
const CREATE_AUDIT_LOGS_GUILD_ACTION_CREATED_INDEX_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_audit_logs_guild_action_created
                    ON audit_logs(guild_id, action, created_at_unix DESC)";

pub(crate) async fn apply_moderation_audit_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    sqlx::query(CREATE_USER_IP_OBSERVATIONS_TABLE_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(CREATE_USER_IP_OBSERVATIONS_UNIQUE_INDEX_SQL)
        .execute(&mut **tx)
        .await?;

    sqlx::query(CREATE_GUILD_IP_BANS_TABLE_SQL)
        .execute(&mut **tx)
        .await?;

    sqlx::query(CREATE_AUDIT_LOGS_TABLE_SQL)
        .execute(&mut **tx)
        .await?;

    sqlx::query(CREATE_USER_IP_OBSERVATIONS_USER_LAST_SEEN_INDEX_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(CREATE_GUILD_IP_BANS_GUILD_CREATED_INDEX_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(CREATE_AUDIT_LOGS_GUILD_CREATED_INDEX_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(CREATE_AUDIT_LOGS_GUILD_ACTION_CREATED_INDEX_SQL)
        .execute(&mut **tx)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        CREATE_AUDIT_LOGS_GUILD_ACTION_CREATED_INDEX_SQL,
        CREATE_AUDIT_LOGS_GUILD_CREATED_INDEX_SQL, CREATE_AUDIT_LOGS_TABLE_SQL,
        CREATE_GUILD_IP_BANS_GUILD_CREATED_INDEX_SQL, CREATE_GUILD_IP_BANS_TABLE_SQL,
        CREATE_USER_IP_OBSERVATIONS_TABLE_SQL, CREATE_USER_IP_OBSERVATIONS_UNIQUE_INDEX_SQL,
        CREATE_USER_IP_OBSERVATIONS_USER_LAST_SEEN_INDEX_SQL,
    };

    #[test]
    fn moderation_audit_schema_statements_define_required_tables_and_indexes() {
        assert!(CREATE_USER_IP_OBSERVATIONS_TABLE_SQL.contains("user_ip_observations"));
        assert!(CREATE_GUILD_IP_BANS_TABLE_SQL.contains("guild_ip_bans"));
        assert!(CREATE_AUDIT_LOGS_TABLE_SQL.contains("CREATE TABLE IF NOT EXISTS audit_logs"));
        assert!(CREATE_USER_IP_OBSERVATIONS_UNIQUE_INDEX_SQL
            .contains("idx_user_ip_observations_user_ip_unique"));
        assert!(CREATE_USER_IP_OBSERVATIONS_USER_LAST_SEEN_INDEX_SQL
            .contains("idx_user_ip_observations_user_last_seen"));
        assert!(CREATE_GUILD_IP_BANS_GUILD_CREATED_INDEX_SQL
            .contains("idx_guild_ip_bans_guild_created"));
        assert!(CREATE_AUDIT_LOGS_GUILD_CREATED_INDEX_SQL.contains("idx_audit_logs_guild_created"));
        assert!(CREATE_AUDIT_LOGS_GUILD_ACTION_CREATED_INDEX_SQL
            .contains("idx_audit_logs_guild_action_created"));
    }
}
