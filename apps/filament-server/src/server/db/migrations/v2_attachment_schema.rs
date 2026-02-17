use sqlx::{Postgres, Transaction};

const CREATE_ATTACHMENTS_TABLE_SQL: &str =
    "CREATE TABLE IF NOT EXISTS attachments (
                    attachment_id TEXT PRIMARY KEY,
                    guild_id TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
                    channel_id TEXT NOT NULL REFERENCES channels(channel_id) ON DELETE CASCADE,
                    owner_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    filename TEXT NOT NULL,
                    mime_type TEXT NOT NULL,
                    size_bytes BIGINT NOT NULL,
                    sha256_hex TEXT NOT NULL,
                    object_key TEXT NOT NULL UNIQUE,
                    created_at_unix BIGINT NOT NULL
                )";
const ADD_OBJECT_KEY_COLUMN_SQL: &str =
    "ALTER TABLE attachments ADD COLUMN IF NOT EXISTS object_key TEXT";
const BACKFILL_OBJECT_KEY_SQL: &str =
    "UPDATE attachments
                 SET object_key = CONCAT('attachments/', attachment_id)
                 WHERE object_key IS NULL OR object_key = ''";
const OBJECT_KEY_NOT_NULL_SQL: &str =
    "ALTER TABLE attachments ALTER COLUMN object_key SET NOT NULL";
const OBJECT_KEY_UNIQUE_INDEX_SQL: &str =
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_attachments_object_key_unique
                    ON attachments(object_key)";
const ADD_CREATED_AT_UNIX_COLUMN_SQL: &str =
    "ALTER TABLE attachments ADD COLUMN IF NOT EXISTS created_at_unix BIGINT";
const BACKFILL_CREATED_AT_UNIX_SQL: &str =
    "UPDATE attachments
                 SET created_at_unix = 0
                 WHERE created_at_unix IS NULL";
const CREATED_AT_UNIX_NOT_NULL_SQL: &str =
    "ALTER TABLE attachments ALTER COLUMN created_at_unix SET NOT NULL";
const ATTACHMENTS_OWNER_INDEX_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_attachments_owner
                    ON attachments(owner_id)";
const ADD_MESSAGE_ID_COLUMN_SQL: &str =
    "ALTER TABLE attachments
                 ADD COLUMN IF NOT EXISTS message_id TEXT NULL REFERENCES messages(message_id) ON DELETE SET NULL";
const ATTACHMENTS_MESSAGE_INDEX_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_attachments_message
                    ON attachments(message_id)";

pub(crate) async fn apply_attachment_schema(tx: &mut Transaction<'_, Postgres>) -> Result<(), sqlx::Error> {
    sqlx::query(CREATE_ATTACHMENTS_TABLE_SQL)
        .execute(&mut **tx)
        .await?;

    sqlx::query(ADD_OBJECT_KEY_COLUMN_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(BACKFILL_OBJECT_KEY_SQL).execute(&mut **tx).await?;
    sqlx::query(OBJECT_KEY_NOT_NULL_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(OBJECT_KEY_UNIQUE_INDEX_SQL)
        .execute(&mut **tx)
        .await?;

    sqlx::query(ADD_CREATED_AT_UNIX_COLUMN_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(BACKFILL_CREATED_AT_UNIX_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(CREATED_AT_UNIX_NOT_NULL_SQL)
        .execute(&mut **tx)
        .await?;

    sqlx::query(ATTACHMENTS_OWNER_INDEX_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(ADD_MESSAGE_ID_COLUMN_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(ATTACHMENTS_MESSAGE_INDEX_SQL)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ADD_CREATED_AT_UNIX_COLUMN_SQL, ADD_MESSAGE_ID_COLUMN_SQL, ADD_OBJECT_KEY_COLUMN_SQL,
        ATTACHMENTS_MESSAGE_INDEX_SQL, ATTACHMENTS_OWNER_INDEX_SQL, BACKFILL_CREATED_AT_UNIX_SQL,
        BACKFILL_OBJECT_KEY_SQL, CREATE_ATTACHMENTS_TABLE_SQL, CREATED_AT_UNIX_NOT_NULL_SQL,
        OBJECT_KEY_NOT_NULL_SQL, OBJECT_KEY_UNIQUE_INDEX_SQL,
    };

    #[test]
    fn attachment_schema_migration_statements_include_fail_closed_backfill_guards() {
        assert!(CREATE_ATTACHMENTS_TABLE_SQL.contains("CREATE TABLE IF NOT EXISTS attachments"));
        assert!(ADD_OBJECT_KEY_COLUMN_SQL.contains("ADD COLUMN IF NOT EXISTS object_key"));
        assert!(BACKFILL_OBJECT_KEY_SQL.contains("CONCAT('attachments/', attachment_id)"));
        assert!(OBJECT_KEY_NOT_NULL_SQL.contains("object_key SET NOT NULL"));
        assert!(OBJECT_KEY_UNIQUE_INDEX_SQL.contains("idx_attachments_object_key_unique"));

        assert!(ADD_CREATED_AT_UNIX_COLUMN_SQL.contains("created_at_unix BIGINT"));
        assert!(BACKFILL_CREATED_AT_UNIX_SQL.contains("SET created_at_unix = 0"));
        assert!(CREATED_AT_UNIX_NOT_NULL_SQL.contains("created_at_unix SET NOT NULL"));
        assert!(ATTACHMENTS_OWNER_INDEX_SQL.contains("idx_attachments_owner"));
        assert!(ADD_MESSAGE_ID_COLUMN_SQL.contains("ADD COLUMN IF NOT EXISTS message_id"));
        assert!(ATTACHMENTS_MESSAGE_INDEX_SQL.contains("idx_attachments_message"));
    }
}