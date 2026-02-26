use sqlx::{Postgres, Transaction};

const ADD_BANNER_OBJECT_KEY_COLUMN_SQL: &str =
    "ALTER TABLE users ADD COLUMN IF NOT EXISTS banner_object_key TEXT";
const ADD_BANNER_MIME_TYPE_COLUMN_SQL: &str =
    "ALTER TABLE users ADD COLUMN IF NOT EXISTS banner_mime_type TEXT";
const ADD_BANNER_SIZE_BYTES_COLUMN_SQL: &str =
    "ALTER TABLE users ADD COLUMN IF NOT EXISTS banner_size_bytes BIGINT";
const ADD_BANNER_SHA256_HEX_COLUMN_SQL: &str =
    "ALTER TABLE users ADD COLUMN IF NOT EXISTS banner_sha256_hex TEXT";
const ADD_BANNER_VERSION_COLUMN_SQL: &str =
    "ALTER TABLE users ADD COLUMN IF NOT EXISTS banner_version BIGINT";
const BACKFILL_BANNER_VERSION_SQL: &str = "UPDATE users
                 SET banner_version = 0
                 WHERE banner_version IS NULL";
const BANNER_VERSION_DEFAULT_SQL: &str =
    "ALTER TABLE users ALTER COLUMN banner_version SET DEFAULT 0";
const BANNER_VERSION_NOT_NULL_SQL: &str =
    "ALTER TABLE users ALTER COLUMN banner_version SET NOT NULL";

pub(crate) async fn apply_profile_banner_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    sqlx::query(ADD_BANNER_OBJECT_KEY_COLUMN_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(ADD_BANNER_MIME_TYPE_COLUMN_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(ADD_BANNER_SIZE_BYTES_COLUMN_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(ADD_BANNER_SHA256_HEX_COLUMN_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(ADD_BANNER_VERSION_COLUMN_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(BACKFILL_BANNER_VERSION_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(BANNER_VERSION_DEFAULT_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(BANNER_VERSION_NOT_NULL_SQL)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ADD_BANNER_MIME_TYPE_COLUMN_SQL, ADD_BANNER_OBJECT_KEY_COLUMN_SQL,
        ADD_BANNER_SHA256_HEX_COLUMN_SQL, ADD_BANNER_SIZE_BYTES_COLUMN_SQL,
        ADD_BANNER_VERSION_COLUMN_SQL, BACKFILL_BANNER_VERSION_SQL, BANNER_VERSION_DEFAULT_SQL,
        BANNER_VERSION_NOT_NULL_SQL,
    };

    #[test]
    fn profile_banner_schema_statements_cover_columns_and_backfill() {
        assert!(ADD_BANNER_OBJECT_KEY_COLUMN_SQL.contains("banner_object_key TEXT"));
        assert!(ADD_BANNER_MIME_TYPE_COLUMN_SQL.contains("banner_mime_type TEXT"));
        assert!(ADD_BANNER_SIZE_BYTES_COLUMN_SQL.contains("banner_size_bytes BIGINT"));
        assert!(ADD_BANNER_SHA256_HEX_COLUMN_SQL.contains("banner_sha256_hex TEXT"));
        assert!(ADD_BANNER_VERSION_COLUMN_SQL.contains("banner_version BIGINT"));
        assert!(BACKFILL_BANNER_VERSION_SQL.contains("SET banner_version = 0"));
        assert!(BANNER_VERSION_DEFAULT_SQL.contains("banner_version SET DEFAULT 0"));
        assert!(BANNER_VERSION_NOT_NULL_SQL.contains("banner_version SET NOT NULL"));
    }
}
