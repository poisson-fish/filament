use sqlx::{Postgres, Transaction};

const MAKE_ACK_PENDING_STATE_EXPLICIT_SQL: &str =
    "ALTER TABLE e2ee_message_acks ALTER COLUMN acked_at_unix DROP NOT NULL";

const INDEX_PENDING_DELIVERIES_BY_DEVICE_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_e2ee_message_acks_pending_device
     ON e2ee_message_acks (device_id, message_id)
     WHERE acked_at_unix IS NULL";

/// Apply the v14 per-device mailbox delivery schema.
///
/// Each message snapshots active participant devices into the ack table. A
/// nullable timestamp represents pending delivery without adding a second
/// unbounded routing table.
pub(crate) async fn apply_e2ee_mailbox_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    for statement in [
        MAKE_ACK_PENDING_STATE_EXPLICIT_SQL,
        INDEX_PENDING_DELIVERIES_BY_DEVICE_SQL,
    ] {
        sqlx::query(statement).execute(&mut **tx).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_delivery_state_and_bounded_lookup_are_schema_invariants() {
        assert!(MAKE_ACK_PENDING_STATE_EXPLICIT_SQL.contains("DROP NOT NULL"));
        assert!(INDEX_PENDING_DELIVERIES_BY_DEVICE_SQL.contains("device_id, message_id"));
        assert!(INDEX_PENDING_DELIVERIES_BY_DEVICE_SQL.contains("acked_at_unix IS NULL"));
    }
}
