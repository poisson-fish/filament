use sqlx::{Postgres, Transaction};

const CREATE_COMMIT_RECEIPTS_SQL: &str = "CREATE TABLE IF NOT EXISTS e2ee_commit_receipts (
    group_id          TEXT NOT NULL REFERENCES e2ee_groups(group_id) ON DELETE CASCADE,
    epoch             BIGINT NOT NULL CHECK (epoch > 0),
    committer_device_id TEXT NOT NULL REFERENCES e2ee_device_certificates(device_id),
    request_sha256    BYTEA NOT NULL CHECK (octet_length(request_sha256) = 32),
    expires_at_unix   BIGINT NOT NULL CHECK (expires_at_unix > 0),
    PRIMARY KEY (group_id, epoch)
)";

const INDEX_COMMIT_RECEIPTS_EXPIRY_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_e2ee_commit_receipts_expiry
     ON e2ee_commit_receipts (expires_at_unix, group_id, epoch)";

/// Retain a bounded fingerprint of each accepted commit request.
///
/// Receipts intentionally do not reference the transient commit row: mailbox
/// acknowledgments may hard-delete that row before a client can reconcile a
/// lost response. The mailbox TTL sweeper independently deletes receipts.
pub(crate) async fn apply_e2ee_commit_idempotency_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    for statement in [CREATE_COMMIT_RECEIPTS_SQL, INDEX_COMMIT_RECEIPTS_EXPIRY_SQL] {
        sqlx::query(statement).execute(&mut **tx).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_receipts_are_fixed_bounded_and_expiry_indexed() {
        assert!(CREATE_COMMIT_RECEIPTS_SQL.contains("PRIMARY KEY (group_id, epoch)"));
        assert!(CREATE_COMMIT_RECEIPTS_SQL.contains("octet_length(request_sha256) = 32"));
        assert!(CREATE_COMMIT_RECEIPTS_SQL.contains("ON DELETE CASCADE"));
        assert!(INDEX_COMMIT_RECEIPTS_EXPIRY_SQL.contains("expires_at_unix"));
    }
}
