use sqlx::{Postgres, Transaction};

const ADD_ROTATION_SEQUENCE_SQL: &str = "ALTER TABLE e2ee_root_identities
    ADD COLUMN IF NOT EXISTS rotation_sequence BIGINT NOT NULL DEFAULT 0";

const ADD_ROTATION_SEQUENCE_LIMIT_SQL: &str = "DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'e2ee_root_identity_rotation_sequence_range'
    ) THEN
        ALTER TABLE e2ee_root_identities
            ADD CONSTRAINT e2ee_root_identity_rotation_sequence_range
            CHECK (rotation_sequence >= 0 AND rotation_sequence <= 100);
    END IF;
END $$";

const CREATE_ROOT_ROTATIONS_SQL: &str = "CREATE TABLE IF NOT EXISTS e2ee_root_identity_rotations (
    user_id                 TEXT NOT NULL REFERENCES e2ee_root_identities(user_id) ON DELETE CASCADE,
    sequence                BIGINT NOT NULL CHECK (sequence > 0 AND sequence <= 100),
    previous_root_key_pub   BYTEA NOT NULL CHECK (octet_length(previous_root_key_pub) = 32),
    new_root_key_pub        BYTEA NOT NULL CHECK (octet_length(new_root_key_pub) = 32),
    previous_root_signature BYTEA NOT NULL CHECK (octet_length(previous_root_signature) = 64),
    new_root_signature      BYTEA NOT NULL CHECK (octet_length(new_root_signature) = 64),
    rotating_device_id      TEXT NOT NULL,
    rotated_at_unix         BIGINT NOT NULL,
    PRIMARY KEY (user_id, sequence),
    CHECK (previous_root_key_pub <> new_root_key_pub)
)";

const INDEX_ROOT_ROTATIONS_BY_TIME_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_e2ee_root_rotations_time
     ON e2ee_root_identity_rotations (user_id, rotated_at_unix)";

/// Add bounded, append-only root-identity continuity history.
pub(crate) async fn apply_e2ee_root_rotation_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    for statement in [
        ADD_ROTATION_SEQUENCE_SQL,
        ADD_ROTATION_SEQUENCE_LIMIT_SQL,
        CREATE_ROOT_ROTATIONS_SQL,
        INDEX_ROOT_ROTATIONS_BY_TIME_SQL,
    ] {
        sqlx::query(statement).execute(&mut **tx).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_schema_bounds_history_and_crypto_fields() {
        assert!(ADD_ROTATION_SEQUENCE_LIMIT_SQL.contains("rotation_sequence <= 100"));
        assert!(CREATE_ROOT_ROTATIONS_SQL.contains("sequence > 0 AND sequence <= 100"));
        assert!(CREATE_ROOT_ROTATIONS_SQL.contains("octet_length(previous_root_key_pub) = 32"));
        assert!(CREATE_ROOT_ROTATIONS_SQL.contains("octet_length(new_root_signature) = 64"));
        assert!(CREATE_ROOT_ROTATIONS_SQL.contains("previous_root_key_pub <> new_root_key_pub"));
    }
}
