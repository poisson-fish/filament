use sqlx::{Postgres, Transaction};

const CREATE_E2EE_PROPOSALS_SQL: &str = "CREATE TABLE IF NOT EXISTS e2ee_proposals (
    proposal_id       TEXT PRIMARY KEY,
    group_id          TEXT NOT NULL REFERENCES e2ee_groups(group_id) ON DELETE CASCADE,
    epoch             BIGINT NOT NULL CHECK (epoch >= 0),
    proposer_device_id TEXT NOT NULL REFERENCES e2ee_device_certificates(device_id),
    proposal_blob     BYTEA NOT NULL CHECK (
        octet_length(proposal_blob) > 0 AND octet_length(proposal_blob) <= 65536
    ),
    created_at_unix   BIGINT NOT NULL,
    expires_at_unix   BIGINT NOT NULL CHECK (expires_at_unix > created_at_unix)
)";

const CREATE_E2EE_PROPOSAL_DELIVERIES_SQL: &str =
    "CREATE TABLE IF NOT EXISTS e2ee_proposal_deliveries (
    proposal_id   TEXT NOT NULL REFERENCES e2ee_proposals(proposal_id) ON DELETE CASCADE,
    device_id     TEXT NOT NULL REFERENCES e2ee_device_certificates(device_id),
    acked_at_unix BIGINT,
    PRIMARY KEY (proposal_id, device_id)
)";

const INDEX_PROPOSALS_BY_GROUP_SQL: &str = "CREATE INDEX IF NOT EXISTS idx_e2ee_proposals_group
     ON e2ee_proposals (group_id, proposal_id)";

const INDEX_PROPOSALS_EXPIRY_SQL: &str = "CREATE INDEX IF NOT EXISTS idx_e2ee_proposals_expiry
     ON e2ee_proposals (expires_at_unix)";

const INDEX_PENDING_PROPOSAL_DELIVERIES_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_e2ee_proposal_deliveries_pending_device
     ON e2ee_proposal_deliveries (device_id, proposal_id)
     WHERE acked_at_unix IS NULL";

/// Apply bounded, transient per-device delivery state for opaque MLS proposals.
pub(crate) async fn apply_e2ee_proposal_mailbox_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    for statement in [
        CREATE_E2EE_PROPOSALS_SQL,
        CREATE_E2EE_PROPOSAL_DELIVERIES_SQL,
        INDEX_PROPOSALS_BY_GROUP_SQL,
        INDEX_PROPOSALS_EXPIRY_SQL,
        INDEX_PENDING_PROPOSAL_DELIVERIES_SQL,
    ] {
        sqlx::query(statement).execute(&mut **tx).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposal_schema_is_opaque_bounded_and_transient() {
        assert!(CREATE_E2EE_PROPOSALS_SQL.contains("octet_length(proposal_blob) <= 65536"));
        assert!(CREATE_E2EE_PROPOSALS_SQL.contains("expires_at_unix > created_at_unix"));
        assert!(CREATE_E2EE_PROPOSAL_DELIVERIES_SQL.contains("ON DELETE CASCADE"));
        assert!(
            CREATE_E2EE_PROPOSAL_DELIVERIES_SQL.contains("PRIMARY KEY (proposal_id, device_id)")
        );
        assert!(INDEX_PENDING_PROPOSAL_DELIVERIES_SQL.contains("acked_at_unix IS NULL"));
        assert!(!CREATE_E2EE_PROPOSALS_SQL.contains("proposal_kind"));
    }
}
