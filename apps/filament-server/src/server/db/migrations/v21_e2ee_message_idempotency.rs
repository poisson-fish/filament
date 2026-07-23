use sqlx::{Postgres, Transaction};

const CREATE_MESSAGE_RECEIPTS_SQL: &str = "CREATE TABLE IF NOT EXISTS e2ee_message_receipts (
    group_id            TEXT NOT NULL REFERENCES e2ee_groups(group_id) ON DELETE CASCADE,
    sender_device_id    TEXT NOT NULL REFERENCES e2ee_device_certificates(device_id),
    ciphertext_sha256   BYTEA NOT NULL CHECK (octet_length(ciphertext_sha256) = 32),
    request_sha256      BYTEA NOT NULL CHECK (octet_length(request_sha256) = 32),
    message_id          TEXT NOT NULL CHECK (char_length(message_id) = 26),
    created_at_unix     BIGINT NOT NULL CHECK (created_at_unix >= 0),
    expires_at_unix     BIGINT NOT NULL CHECK (expires_at_unix > created_at_unix),
    PRIMARY KEY (group_id, sender_device_id, ciphertext_sha256)
)";

const INDEX_MESSAGE_RECEIPTS_EXPIRY_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_e2ee_message_receipts_expiry
     ON e2ee_message_receipts (expires_at_unix, group_id, sender_device_id)";

/// Retain a bounded fingerprint and response for each accepted MLS message.
///
/// Receipts intentionally do not reference the transient message row:
/// all-device acknowledgments may hard-delete ciphertext before a sender can
/// reconcile a lost response. The ciphertext fingerprint also prevents the
/// same MLS `PrivateMessage` from being replayed with altered routing fields.
pub(crate) async fn apply_e2ee_message_idempotency_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    for statement in [
        CREATE_MESSAGE_RECEIPTS_SQL,
        INDEX_MESSAGE_RECEIPTS_EXPIRY_SQL,
    ] {
        sqlx::query(statement).execute(&mut **tx).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_receipts_are_fixed_bounded_and_expiry_indexed() {
        assert!(CREATE_MESSAGE_RECEIPTS_SQL
            .contains("PRIMARY KEY (group_id, sender_device_id, ciphertext_sha256)"));
        assert!(CREATE_MESSAGE_RECEIPTS_SQL.contains("octet_length(ciphertext_sha256) = 32"));
        assert!(CREATE_MESSAGE_RECEIPTS_SQL.contains("octet_length(request_sha256) = 32"));
        assert!(CREATE_MESSAGE_RECEIPTS_SQL.contains("char_length(message_id) = 26"));
        assert!(CREATE_MESSAGE_RECEIPTS_SQL.contains("ON DELETE CASCADE"));
        assert!(INDEX_MESSAGE_RECEIPTS_EXPIRY_SQL.contains("expires_at_unix"));
    }
}
