use sqlx::{Postgres, Transaction};

const CREATE_ROOT_IDENTITIES_SQL: &str = "CREATE TABLE IF NOT EXISTS e2ee_root_identities (
    user_id         TEXT PRIMARY KEY REFERENCES users(user_id) ON DELETE CASCADE,
    root_key_pub    BYTEA NOT NULL CHECK (octet_length(root_key_pub) = 32),
    created_at_unix BIGINT NOT NULL
)";

const CREATE_DEVICE_CERTIFICATES_SQL: &str = "CREATE TABLE IF NOT EXISTS e2ee_device_certificates (
    device_id          TEXT PRIMARY KEY,
    user_id            TEXT NOT NULL REFERENCES e2ee_root_identities(user_id),
    device_sig_pubkey  BYTEA NOT NULL CHECK (octet_length(device_sig_pubkey) = 32),
    root_key_sig       BYTEA NOT NULL CHECK (octet_length(root_key_sig) = 64),
    root_key_pub       BYTEA NOT NULL CHECK (octet_length(root_key_pub) = 32),
    created_at_unix    BIGINT NOT NULL,
    tombstoned_at_unix BIGINT,
    UNIQUE (user_id, device_id)
)";

const INDEX_DEVICE_CERTS_BY_USER_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_e2ee_device_certs_user ON e2ee_device_certificates (user_id)";

const UNIQUE_DEVICE_ID_SQL: &str =
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_e2ee_device_id_unique ON e2ee_device_certificates (device_id)";

const CREATE_KEYPACKAGES_SQL: &str = "CREATE TABLE IF NOT EXISTS e2ee_keypackages (
    device_id          TEXT NOT NULL REFERENCES e2ee_device_certificates(device_id) ON DELETE CASCADE,
    key_package_hash   TEXT NOT NULL CHECK (char_length(key_package_hash) = 64),
    key_package_blob   BYTEA NOT NULL CHECK (
        octet_length(key_package_blob) > 0 AND octet_length(key_package_blob) <= 4096
    ),
    is_last_resort     BOOLEAN NOT NULL DEFAULT FALSE,
    claimed_at_unix    BIGINT,
    created_at_unix    BIGINT NOT NULL,
    PRIMARY KEY (device_id, key_package_hash)
)";

const INDEX_KEYPACKAGES_BY_DEVICE_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_e2ee_keypackages_device ON e2ee_keypackages (device_id)";

const INDEX_KEYPACKAGES_UNCLAIMED_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_e2ee_keypackages_unclaimed
    ON e2ee_keypackages (device_id, is_last_resort, created_at_unix)
    WHERE claimed_at_unix IS NULL";

const UNIQUE_LAST_RESORT_SQL: &str = "CREATE UNIQUE INDEX IF NOT EXISTS idx_e2ee_one_last_resort
    ON e2ee_keypackages (device_id)
    WHERE is_last_resort AND claimed_at_unix IS NULL";

const CREATE_AUDIT_LOG_SQL: &str = "CREATE TABLE IF NOT EXISTS e2ee_audit_log (
    id              BIGSERIAL PRIMARY KEY,
    action          TEXT NOT NULL,
    user_id         TEXT,
    device_id       TEXT,
    metadata_json   JSONB,
    created_at_unix BIGINT NOT NULL
)";

const INDEX_AUDIT_BY_USER_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_e2ee_audit_user ON e2ee_audit_log (user_id)";

const INDEX_AUDIT_BY_TIME_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_e2ee_audit_time ON e2ee_audit_log (created_at_unix)";

/// Apply the E2EE identity schema (migration v12).
///
/// Creates tables for root identities, device certificates, `KeyPackage`
/// pools, and public audit metadata. MLS blobs remain opaque `BYTEA` values.
pub(crate) async fn apply_e2ee_identity_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    for statement in [
        CREATE_ROOT_IDENTITIES_SQL,
        CREATE_DEVICE_CERTIFICATES_SQL,
        INDEX_DEVICE_CERTS_BY_USER_SQL,
        UNIQUE_DEVICE_ID_SQL,
        CREATE_KEYPACKAGES_SQL,
        INDEX_KEYPACKAGES_BY_DEVICE_SQL,
        INDEX_KEYPACKAGES_UNCLAIMED_SQL,
        UNIQUE_LAST_RESORT_SQL,
        CREATE_AUDIT_LOG_SQL,
        INDEX_AUDIT_BY_USER_SQL,
        INDEX_AUDIT_BY_TIME_SQL,
    ] {
        sqlx::query(statement).execute(&mut **tx).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_and_device_schema_lock_crypto_lengths_and_identity() {
        assert!(CREATE_ROOT_IDENTITIES_SQL.contains("octet_length(root_key_pub) = 32"));
        assert!(CREATE_ROOT_IDENTITIES_SQL.contains("REFERENCES users(user_id) ON DELETE CASCADE"));
        assert!(CREATE_DEVICE_CERTIFICATES_SQL.contains("device_id          TEXT PRIMARY KEY"));
        assert!(CREATE_DEVICE_CERTIFICATES_SQL.contains("octet_length(device_sig_pubkey) = 32"));
        assert!(CREATE_DEVICE_CERTIFICATES_SQL.contains("octet_length(root_key_sig) = 64"));
        assert!(CREATE_DEVICE_CERTIFICATES_SQL.contains("REFERENCES e2ee_root_identities"));
    }

    #[test]
    fn keypackage_schema_enforces_opaque_blob_and_single_use_constraints() {
        assert!(CREATE_KEYPACKAGES_SQL.contains("octet_length(key_package_blob) <= 4096"));
        assert!(CREATE_KEYPACKAGES_SQL.contains("ON DELETE CASCADE"));
        assert!(UNIQUE_LAST_RESORT_SQL.contains("WHERE is_last_resort AND claimed_at_unix IS NULL"));
        assert!(INDEX_KEYPACKAGES_UNCLAIMED_SQL.contains("claimed_at_unix IS NULL"));
    }

    #[test]
    fn audit_log_uses_structured_public_metadata() {
        assert!(CREATE_AUDIT_LOG_SQL.contains("metadata_json   JSONB"));
        assert!(INDEX_AUDIT_BY_USER_SQL.contains("idx_e2ee_audit_user"));
        assert!(INDEX_AUDIT_BY_TIME_SQL.contains("idx_e2ee_audit_time"));
    }
}
