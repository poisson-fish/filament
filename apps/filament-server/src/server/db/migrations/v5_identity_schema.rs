use sqlx::{Postgres, Transaction};

const CREATE_USERS_TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS users (
                    user_id TEXT PRIMARY KEY,
                    username TEXT UNIQUE NOT NULL,
                    about_markdown TEXT NOT NULL DEFAULT '',
                    avatar_object_key TEXT NULL,
                    avatar_mime_type TEXT NULL,
                    avatar_size_bytes BIGINT NULL,
                    avatar_sha256_hex TEXT NULL,
                    avatar_version BIGINT NOT NULL DEFAULT 0,
                    password_hash TEXT NOT NULL,
                    failed_logins SMALLINT NOT NULL DEFAULT 0,
                    locked_until_unix BIGINT NULL
                )";
const ADD_ABOUT_MARKDOWN_COLUMN_SQL: &str =
    "ALTER TABLE users ADD COLUMN IF NOT EXISTS about_markdown TEXT";
const BACKFILL_ABOUT_MARKDOWN_SQL: &str = "UPDATE users
                 SET about_markdown = ''
                 WHERE about_markdown IS NULL";
const ABOUT_MARKDOWN_DEFAULT_SQL: &str =
    "ALTER TABLE users ALTER COLUMN about_markdown SET DEFAULT ''";
const ABOUT_MARKDOWN_NOT_NULL_SQL: &str =
    "ALTER TABLE users ALTER COLUMN about_markdown SET NOT NULL";
const ADD_AVATAR_OBJECT_KEY_COLUMN_SQL: &str =
    "ALTER TABLE users ADD COLUMN IF NOT EXISTS avatar_object_key TEXT";
const ADD_AVATAR_MIME_TYPE_COLUMN_SQL: &str =
    "ALTER TABLE users ADD COLUMN IF NOT EXISTS avatar_mime_type TEXT";
const ADD_AVATAR_SIZE_BYTES_COLUMN_SQL: &str =
    "ALTER TABLE users ADD COLUMN IF NOT EXISTS avatar_size_bytes BIGINT";
const ADD_AVATAR_SHA256_HEX_COLUMN_SQL: &str =
    "ALTER TABLE users ADD COLUMN IF NOT EXISTS avatar_sha256_hex TEXT";
const ADD_AVATAR_VERSION_COLUMN_SQL: &str =
    "ALTER TABLE users ADD COLUMN IF NOT EXISTS avatar_version BIGINT";
const BACKFILL_AVATAR_VERSION_SQL: &str = "UPDATE users
                 SET avatar_version = 0
                 WHERE avatar_version IS NULL";
const AVATAR_VERSION_DEFAULT_SQL: &str =
    "ALTER TABLE users ALTER COLUMN avatar_version SET DEFAULT 0";
const AVATAR_VERSION_NOT_NULL_SQL: &str =
    "ALTER TABLE users ALTER COLUMN avatar_version SET NOT NULL";
const CREATE_SESSIONS_TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS sessions (
                    session_id TEXT PRIMARY KEY,
                    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    refresh_token_hash BYTEA NOT NULL,
                    expires_at_unix BIGINT NOT NULL,
                    revoked BOOLEAN NOT NULL DEFAULT FALSE
                )";
const CREATE_USED_REFRESH_TOKENS_TABLE_SQL: &str =
    "CREATE TABLE IF NOT EXISTS used_refresh_tokens (
                    token_hash BYTEA PRIMARY KEY,
                    session_id TEXT NOT NULL REFERENCES sessions(session_id) ON DELETE CASCADE,
                    used_at_unix BIGINT NOT NULL DEFAULT 0
                )";
const ADD_USED_REFRESH_TOKENS_USED_AT_COLUMN_SQL: &str =
    "ALTER TABLE used_refresh_tokens ADD COLUMN IF NOT EXISTS used_at_unix BIGINT";
const BACKFILL_USED_REFRESH_TOKENS_USED_AT_SQL: &str =
    "UPDATE used_refresh_tokens SET used_at_unix = 0 WHERE used_at_unix IS NULL";
const USED_REFRESH_TOKENS_USED_AT_NOT_NULL_SQL: &str =
    "ALTER TABLE used_refresh_tokens ALTER COLUMN used_at_unix SET NOT NULL";

pub(crate) async fn apply_identity_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    sqlx::query(CREATE_USERS_TABLE_SQL)
        .execute(&mut **tx)
        .await?;

    sqlx::query(ADD_ABOUT_MARKDOWN_COLUMN_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(BACKFILL_ABOUT_MARKDOWN_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(ABOUT_MARKDOWN_DEFAULT_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(ABOUT_MARKDOWN_NOT_NULL_SQL)
        .execute(&mut **tx)
        .await?;

    sqlx::query(ADD_AVATAR_OBJECT_KEY_COLUMN_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(ADD_AVATAR_MIME_TYPE_COLUMN_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(ADD_AVATAR_SIZE_BYTES_COLUMN_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(ADD_AVATAR_SHA256_HEX_COLUMN_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(ADD_AVATAR_VERSION_COLUMN_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(BACKFILL_AVATAR_VERSION_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(AVATAR_VERSION_DEFAULT_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(AVATAR_VERSION_NOT_NULL_SQL)
        .execute(&mut **tx)
        .await?;

    sqlx::query(CREATE_SESSIONS_TABLE_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(CREATE_USED_REFRESH_TOKENS_TABLE_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(ADD_USED_REFRESH_TOKENS_USED_AT_COLUMN_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(BACKFILL_USED_REFRESH_TOKENS_USED_AT_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(USED_REFRESH_TOKENS_USED_AT_NOT_NULL_SQL)
        .execute(&mut **tx)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ABOUT_MARKDOWN_DEFAULT_SQL, ABOUT_MARKDOWN_NOT_NULL_SQL, ADD_ABOUT_MARKDOWN_COLUMN_SQL,
        ADD_AVATAR_MIME_TYPE_COLUMN_SQL, ADD_AVATAR_OBJECT_KEY_COLUMN_SQL,
        ADD_AVATAR_SHA256_HEX_COLUMN_SQL, ADD_AVATAR_SIZE_BYTES_COLUMN_SQL,
        ADD_AVATAR_VERSION_COLUMN_SQL, ADD_USED_REFRESH_TOKENS_USED_AT_COLUMN_SQL,
        AVATAR_VERSION_DEFAULT_SQL, AVATAR_VERSION_NOT_NULL_SQL, BACKFILL_ABOUT_MARKDOWN_SQL,
        BACKFILL_AVATAR_VERSION_SQL, BACKFILL_USED_REFRESH_TOKENS_USED_AT_SQL,
        CREATE_SESSIONS_TABLE_SQL, CREATE_USED_REFRESH_TOKENS_TABLE_SQL, CREATE_USERS_TABLE_SQL,
        USED_REFRESH_TOKENS_USED_AT_NOT_NULL_SQL,
    };

    #[test]
    fn identity_schema_statements_define_required_tables_and_backfills() {
        assert!(CREATE_USERS_TABLE_SQL.contains("CREATE TABLE IF NOT EXISTS users"));
        assert!(ADD_ABOUT_MARKDOWN_COLUMN_SQL.contains("ADD COLUMN IF NOT EXISTS about_markdown"));
        assert!(BACKFILL_ABOUT_MARKDOWN_SQL.contains("SET about_markdown = ''"));
        assert!(ABOUT_MARKDOWN_DEFAULT_SQL.contains("about_markdown SET DEFAULT ''"));
        assert!(ABOUT_MARKDOWN_NOT_NULL_SQL.contains("about_markdown SET NOT NULL"));
        assert!(ADD_AVATAR_OBJECT_KEY_COLUMN_SQL.contains("avatar_object_key TEXT"));
        assert!(ADD_AVATAR_MIME_TYPE_COLUMN_SQL.contains("avatar_mime_type TEXT"));
        assert!(ADD_AVATAR_SIZE_BYTES_COLUMN_SQL.contains("avatar_size_bytes BIGINT"));
        assert!(ADD_AVATAR_SHA256_HEX_COLUMN_SQL.contains("avatar_sha256_hex TEXT"));
        assert!(ADD_AVATAR_VERSION_COLUMN_SQL.contains("avatar_version BIGINT"));
        assert!(BACKFILL_AVATAR_VERSION_SQL.contains("SET avatar_version = 0"));
        assert!(AVATAR_VERSION_DEFAULT_SQL.contains("avatar_version SET DEFAULT 0"));
        assert!(AVATAR_VERSION_NOT_NULL_SQL.contains("avatar_version SET NOT NULL"));
        assert!(CREATE_SESSIONS_TABLE_SQL.contains("CREATE TABLE IF NOT EXISTS sessions"));
        assert!(CREATE_USED_REFRESH_TOKENS_TABLE_SQL
            .contains("CREATE TABLE IF NOT EXISTS used_refresh_tokens"));
        assert!(ADD_USED_REFRESH_TOKENS_USED_AT_COLUMN_SQL
            .contains("ADD COLUMN IF NOT EXISTS used_at_unix BIGINT"));
        assert!(BACKFILL_USED_REFRESH_TOKENS_USED_AT_SQL
            .contains("SET used_at_unix = 0 WHERE used_at_unix IS NULL"));
        assert!(USED_REFRESH_TOKENS_USED_AT_NOT_NULL_SQL.contains("used_at_unix SET NOT NULL"));
    }
}
