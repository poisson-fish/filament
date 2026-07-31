use sqlx::{Postgres, Transaction};

const CREATE_RECONCILIATION_MONITOR_INDEX_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_e2ee_membership_reconciliations_deadline_pending
     ON e2ee_membership_reconciliations (deadline_unix, reconciliation_id)
     WHERE completed_epoch IS NULL";

/// Give the bounded oldest-first reconciliation monitor a deadline-first
/// partial index so its `LIMIT` does not require sorting every pending row.
pub(crate) async fn apply_e2ee_reconciliation_monitor_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    sqlx::query(CREATE_RECONCILIATION_MONITOR_INDEX_SQL)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitor_index_is_deadline_first_and_pending_only() {
        assert!(
            CREATE_RECONCILIATION_MONITOR_INDEX_SQL.contains("(deadline_unix, reconciliation_id)")
        );
        assert!(CREATE_RECONCILIATION_MONITOR_INDEX_SQL.contains("WHERE completed_epoch IS NULL"));
    }
}
