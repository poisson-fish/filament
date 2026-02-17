use filament_core::{ChannelKind, Role};
use sqlx::{Postgres, Transaction};

use crate::server::db::{channel_kind_to_i16, role_to_i16};

const CREATE_GUILDS_TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS guilds (
                    guild_id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    visibility SMALLINT NOT NULL DEFAULT 0,
                    created_by_user_id TEXT REFERENCES users(user_id),
                    created_at_unix BIGINT NOT NULL
                )";
const ADD_GUILD_VISIBILITY_COLUMN_SQL: &str = "ALTER TABLE guilds
                 ADD COLUMN IF NOT EXISTS visibility SMALLINT NOT NULL DEFAULT 0";
const ADD_GUILD_CREATED_BY_COLUMN_SQL: &str = "ALTER TABLE guilds
                 ADD COLUMN IF NOT EXISTS created_by_user_id TEXT REFERENCES users(user_id)";
const CREATE_GUILD_MEMBERS_TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS guild_members (
                    guild_id TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
                    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    role SMALLINT NOT NULL,
                    PRIMARY KEY(guild_id, user_id)
                )";
const BACKFILL_GUILD_CREATOR_FROM_OWNER_SQL: &str = "UPDATE guilds AS g
                 SET created_by_user_id = gm.user_id
                 FROM guild_members AS gm
                 WHERE g.created_by_user_id IS NULL
                   AND gm.guild_id = g.guild_id
                   AND gm.role = $1";
const CREATE_CHANNELS_TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS channels (
                    channel_id TEXT PRIMARY KEY,
                    guild_id TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
                    name TEXT NOT NULL,
                    kind SMALLINT NOT NULL DEFAULT 0,
                    created_at_unix BIGINT NOT NULL
                )";
const ADD_CHANNEL_KIND_COLUMN_SQL: &str =
    "ALTER TABLE channels ADD COLUMN IF NOT EXISTS kind SMALLINT";
const BACKFILL_CHANNEL_KIND_SQL: &str = "UPDATE channels SET kind = $1 WHERE kind IS NULL";
const CHANNEL_KIND_DEFAULT_SQL: &str = "ALTER TABLE channels ALTER COLUMN kind SET DEFAULT 0";
const CHANNEL_KIND_NOT_NULL_SQL: &str = "ALTER TABLE channels ALTER COLUMN kind SET NOT NULL";

pub(crate) async fn apply_workspace_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    sqlx::query(CREATE_GUILDS_TABLE_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(ADD_GUILD_VISIBILITY_COLUMN_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(ADD_GUILD_CREATED_BY_COLUMN_SQL)
        .execute(&mut **tx)
        .await?;

    sqlx::query(CREATE_GUILD_MEMBERS_TABLE_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(BACKFILL_GUILD_CREATOR_FROM_OWNER_SQL)
        .bind(role_to_i16(Role::Owner))
        .execute(&mut **tx)
        .await?;

    sqlx::query(CREATE_CHANNELS_TABLE_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(ADD_CHANNEL_KIND_COLUMN_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(BACKFILL_CHANNEL_KIND_SQL)
        .bind(channel_kind_to_i16(ChannelKind::Text))
        .execute(&mut **tx)
        .await?;
    sqlx::query(CHANNEL_KIND_DEFAULT_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(CHANNEL_KIND_NOT_NULL_SQL)
        .execute(&mut **tx)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ADD_CHANNEL_KIND_COLUMN_SQL, ADD_GUILD_CREATED_BY_COLUMN_SQL,
        ADD_GUILD_VISIBILITY_COLUMN_SQL, BACKFILL_CHANNEL_KIND_SQL,
        BACKFILL_GUILD_CREATOR_FROM_OWNER_SQL, CHANNEL_KIND_DEFAULT_SQL, CHANNEL_KIND_NOT_NULL_SQL,
        CREATE_CHANNELS_TABLE_SQL, CREATE_GUILDS_TABLE_SQL, CREATE_GUILD_MEMBERS_TABLE_SQL,
    };

    #[test]
    fn workspace_schema_statements_define_required_tables_and_backfills() {
        assert!(CREATE_GUILDS_TABLE_SQL.contains("CREATE TABLE IF NOT EXISTS guilds"));
        assert!(ADD_GUILD_VISIBILITY_COLUMN_SQL.contains("ADD COLUMN IF NOT EXISTS visibility"));
        assert!(
            ADD_GUILD_CREATED_BY_COLUMN_SQL.contains("ADD COLUMN IF NOT EXISTS created_by_user_id")
        );
        assert!(CREATE_GUILD_MEMBERS_TABLE_SQL.contains("CREATE TABLE IF NOT EXISTS guild_members"));
        assert!(
            BACKFILL_GUILD_CREATOR_FROM_OWNER_SQL.contains("SET created_by_user_id = gm.user_id")
        );
        assert!(CREATE_CHANNELS_TABLE_SQL.contains("CREATE TABLE IF NOT EXISTS channels"));
        assert!(ADD_CHANNEL_KIND_COLUMN_SQL.contains("ADD COLUMN IF NOT EXISTS kind"));
        assert!(BACKFILL_CHANNEL_KIND_SQL.contains("UPDATE channels SET kind = $1"));
        assert!(CHANNEL_KIND_DEFAULT_SQL.contains("kind SET DEFAULT 0"));
        assert!(CHANNEL_KIND_NOT_NULL_SQL.contains("kind SET NOT NULL"));
    }
}
