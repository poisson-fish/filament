use sqlx::{Postgres, Transaction};

const CREATE_E2EE_ATTACHMENT_BLOBS_SQL: &str = "CREATE TABLE IF NOT EXISTS e2ee_attachment_blobs (
    attachment_id       TEXT PRIMARY KEY,
    group_id            TEXT NOT NULL REFERENCES e2ee_groups(group_id) ON DELETE CASCADE,
    owner_user_id       TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    uploader_device_id  TEXT NOT NULL REFERENCES e2ee_device_certificates(device_id),
    ciphertext_blob     BYTEA NOT NULL CHECK (
        octet_length(ciphertext_blob) IN (65536, 262144, 1048576, 4194304, 16777216, 33554432)
    ),
    created_at_unix     BIGINT NOT NULL,
    expires_at_unix     BIGINT NOT NULL CHECK (expires_at_unix > created_at_unix)
)";

const CREATE_E2EE_ATTACHMENT_DELIVERIES_SQL: &str =
    "CREATE TABLE IF NOT EXISTS e2ee_attachment_deliveries (
    attachment_id  TEXT NOT NULL REFERENCES e2ee_attachment_blobs(attachment_id) ON DELETE CASCADE,
    device_id      TEXT NOT NULL REFERENCES e2ee_device_certificates(device_id),
    acked_at_unix  BIGINT,
    PRIMARY KEY (attachment_id, device_id)
)";

const INDEX_E2EE_ATTACHMENTS_EXPIRY_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_e2ee_attachment_blobs_expiry
     ON e2ee_attachment_blobs (expires_at_unix, attachment_id)";

const INDEX_E2EE_ATTACHMENTS_OWNER_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_e2ee_attachment_blobs_owner
     ON e2ee_attachment_blobs (owner_user_id, attachment_id)";

const INDEX_PENDING_E2EE_ATTACHMENT_DELIVERIES_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_e2ee_attachment_deliveries_pending_device
     ON e2ee_attachment_deliveries (device_id, attachment_id)
     WHERE acked_at_unix IS NULL";

/// Apply bounded opaque attachment storage and per-device transient delivery state.
pub(crate) async fn apply_e2ee_attachment_mailbox_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    for statement in [
        CREATE_E2EE_ATTACHMENT_BLOBS_SQL,
        CREATE_E2EE_ATTACHMENT_DELIVERIES_SQL,
        INDEX_E2EE_ATTACHMENTS_EXPIRY_SQL,
        INDEX_E2EE_ATTACHMENTS_OWNER_SQL,
        INDEX_PENDING_E2EE_ATTACHMENT_DELIVERIES_SQL,
    ] {
        sqlx::query(statement).execute(&mut **tx).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attachment_schema_is_opaque_bucketed_quota_indexed_and_transient() {
        assert!(CREATE_E2EE_ATTACHMENT_BLOBS_SQL.contains("ciphertext_blob     BYTEA"));
        assert!(CREATE_E2EE_ATTACHMENT_BLOBS_SQL.contains(
            "octet_length(ciphertext_blob) IN (65536, 262144, 1048576, 4194304, 16777216, 33554432)"
        ));
        assert!(CREATE_E2EE_ATTACHMENT_BLOBS_SQL.contains("expires_at_unix > created_at_unix"));
        assert!(!CREATE_E2EE_ATTACHMENT_BLOBS_SQL.contains("filename"));
        assert!(!CREATE_E2EE_ATTACHMENT_BLOBS_SQL.contains("mime_type"));
        assert!(!CREATE_E2EE_ATTACHMENT_BLOBS_SQL.contains("content_hash"));
        assert!(CREATE_E2EE_ATTACHMENT_DELIVERIES_SQL.contains("ON DELETE CASCADE"));
        assert!(INDEX_E2EE_ATTACHMENTS_OWNER_SQL.contains("owner_user_id"));
        assert!(INDEX_PENDING_E2EE_ATTACHMENT_DELIVERIES_SQL.contains("acked_at_unix IS NULL"));
    }
}
