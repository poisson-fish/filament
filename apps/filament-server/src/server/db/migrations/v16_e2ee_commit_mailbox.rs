use sqlx::{Postgres, Transaction};

const ADD_WELCOME_DEVICE_SQL: &str =
    "ALTER TABLE e2ee_commits ADD COLUMN IF NOT EXISTS welcome_device_id TEXT
     REFERENCES e2ee_device_certificates(device_id)";

const ADD_WELCOME_TARGET_PAIR_CONSTRAINT_SQL: &str = "DO $$
     BEGIN
         IF NOT EXISTS (
             SELECT 1 FROM pg_constraint
             WHERE conname = 'e2ee_commits_welcome_target_pair'
         ) THEN
             ALTER TABLE e2ee_commits ADD CONSTRAINT e2ee_commits_welcome_target_pair
                 CHECK ((welcome_blob IS NULL) = (welcome_device_id IS NULL)) NOT VALID;
         END IF;
     END
     $$";

const CREATE_E2EE_COMMIT_DELIVERIES_SQL: &str =
    "CREATE TABLE IF NOT EXISTS e2ee_commit_deliveries (
    group_id       TEXT NOT NULL,
    epoch          BIGINT NOT NULL,
    device_id      TEXT NOT NULL REFERENCES e2ee_device_certificates(device_id),
    acked_at_unix  BIGINT,
    PRIMARY KEY (group_id, epoch, device_id),
    FOREIGN KEY (group_id, epoch)
        REFERENCES e2ee_commits(group_id, epoch) ON DELETE CASCADE
)";

const INDEX_PENDING_COMMIT_DELIVERIES_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_e2ee_commit_deliveries_pending_device
     ON e2ee_commit_deliveries (device_id, group_id, epoch)
     WHERE acked_at_unix IS NULL";

const BACKFILL_EXISTING_COMMIT_DELIVERIES_SQL: &str =
    "INSERT INTO e2ee_commit_deliveries (group_id, epoch, device_id, acked_at_unix)
     SELECT k.group_id, k.epoch, d.device_id,
            CASE WHEN d.device_id = k.committer_device_id THEN k.created_at_unix ELSE NULL END
     FROM e2ee_commits k
     JOIN e2ee_groups g ON g.group_id = k.group_id
     JOIN e2ee_conversation_members m ON m.conversation_id = g.conversation_id
     JOIN e2ee_device_certificates d
       ON d.user_id = m.user_id AND d.tombstoned_at_unix IS NULL
     ON CONFLICT (group_id, epoch, device_id) DO NOTHING";

/// Apply device-bound, transient delivery state for commits and Welcomes.
///
/// Legacy commits are backfilled as commit-only deliveries because their
/// Welcome target was not previously persisted and must not be guessed.
pub(crate) async fn apply_e2ee_commit_mailbox_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    for statement in [
        ADD_WELCOME_DEVICE_SQL,
        ADD_WELCOME_TARGET_PAIR_CONSTRAINT_SQL,
        CREATE_E2EE_COMMIT_DELIVERIES_SQL,
        INDEX_PENDING_COMMIT_DELIVERIES_SQL,
        BACKFILL_EXISTING_COMMIT_DELIVERIES_SQL,
    ] {
        sqlx::query(statement).execute(&mut **tx).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn welcome_target_and_commit_delivery_are_database_bound() {
        assert!(ADD_WELCOME_DEVICE_SQL.contains("welcome_device_id"));
        assert!(ADD_WELCOME_TARGET_PAIR_CONSTRAINT_SQL
            .contains("(welcome_blob IS NULL) = (welcome_device_id IS NULL)"));
        assert!(ADD_WELCOME_TARGET_PAIR_CONSTRAINT_SQL.contains("NOT VALID"));
        assert!(CREATE_E2EE_COMMIT_DELIVERIES_SQL.contains("ON DELETE CASCADE"));
        assert!(
            CREATE_E2EE_COMMIT_DELIVERIES_SQL.contains("PRIMARY KEY (group_id, epoch, device_id)")
        );
        assert!(INDEX_PENDING_COMMIT_DELIVERIES_SQL.contains("acked_at_unix IS NULL"));
    }

    #[test]
    fn legacy_backfill_never_guesses_welcome_recipients() {
        assert!(BACKFILL_EXISTING_COMMIT_DELIVERIES_SQL.contains("committer_device_id"));
        assert!(!BACKFILL_EXISTING_COMMIT_DELIVERIES_SQL.contains("welcome_device_id"));
    }
}
