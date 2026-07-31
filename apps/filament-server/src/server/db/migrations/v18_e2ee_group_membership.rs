use sqlx::{Postgres, Transaction};

const CREATE_GROUP_LEAVES_SQL: &str = "CREATE TABLE IF NOT EXISTS e2ee_group_leaves (
    group_id     TEXT NOT NULL REFERENCES e2ee_groups(group_id) ON DELETE CASCADE,
    leaf_index   INTEGER NOT NULL CHECK (leaf_index >= 0 AND leaf_index < 200),
    user_id      TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    device_id    TEXT NOT NULL REFERENCES e2ee_device_certificates(device_id),
    added_epoch  BIGINT NOT NULL CHECK (added_epoch > 0),
    PRIMARY KEY (group_id, leaf_index),
    UNIQUE (group_id, device_id)
)";

const BACKFILL_COMMITTER_LEAVES_SQL: &str =
    "INSERT INTO e2ee_group_leaves (group_id, leaf_index, user_id, device_id, added_epoch)
     SELECT k.group_id, 0, d.user_id, k.committer_device_id, 1
     FROM e2ee_commits k
     JOIN e2ee_device_certificates d ON d.device_id = k.committer_device_id
     WHERE k.epoch = 1
       AND NOT EXISTS (SELECT 1 FROM e2ee_group_leaves l WHERE l.group_id = k.group_id)
     ON CONFLICT (group_id, device_id) DO NOTHING";

const BACKFILL_WELCOME_LEAVES_SQL: &str =
    "INSERT INTO e2ee_group_leaves (group_id, leaf_index, user_id, device_id, added_epoch)
     SELECT k.group_id, 1, d.user_id, k.welcome_device_id, 1
     FROM e2ee_commits k
     JOIN e2ee_device_certificates d ON d.device_id = k.welcome_device_id
     WHERE k.epoch = 1 AND k.welcome_device_id IS NOT NULL
       AND EXISTS (
           SELECT 1 FROM e2ee_group_leaves l WHERE l.group_id = k.group_id AND l.leaf_index = 0
       )
     ON CONFLICT (group_id, device_id) DO NOTHING";

const ADD_COMMIT_MEMBERSHIP_CHANGE_SQL: &str =
    "ALTER TABLE e2ee_commits ADD COLUMN IF NOT EXISTS membership_change_json JSONB
     CHECK (membership_change_json IS NULL OR
            octet_length(membership_change_json::TEXT) <= 65536)";

const CREATE_COMMIT_WELCOME_RECIPIENTS_SQL: &str =
    "CREATE TABLE IF NOT EXISTS e2ee_commit_welcome_recipients (
    group_id   TEXT NOT NULL,
    epoch      BIGINT NOT NULL,
    device_id  TEXT NOT NULL REFERENCES e2ee_device_certificates(device_id),
    PRIMARY KEY (group_id, epoch, device_id),
    FOREIGN KEY (group_id, epoch)
        REFERENCES e2ee_commits(group_id, epoch) ON DELETE CASCADE
)";

const BACKFILL_COMMIT_WELCOME_RECIPIENTS_SQL: &str =
    "INSERT INTO e2ee_commit_welcome_recipients (group_id, epoch, device_id)
     SELECT group_id, epoch, welcome_device_id
     FROM e2ee_commits
     WHERE welcome_device_id IS NOT NULL
     ON CONFLICT (group_id, epoch, device_id) DO NOTHING";

const ALTER_PROPOSER_NULLABILITY_SQL: &str =
    "ALTER TABLE e2ee_proposals ALTER COLUMN proposer_device_id DROP NOT NULL";
const ADD_EXTERNAL_SENDER_INDEX_SQL: &str =
    "ALTER TABLE e2ee_proposals ADD COLUMN IF NOT EXISTS external_sender_index INTEGER";
const ADD_RECONCILIATION_ID_SQL: &str =
    "ALTER TABLE e2ee_proposals ADD COLUMN IF NOT EXISTS reconciliation_id TEXT";
const ADD_RECONCILIATION_DEADLINE_SQL: &str =
    "ALTER TABLE e2ee_proposals ADD COLUMN IF NOT EXISTS reconciliation_deadline_unix BIGINT";

const ADD_PROPOSAL_SOURCE_CONSTRAINT_SQL: &str = "DO $$
    BEGIN
        IF NOT EXISTS (
            SELECT 1 FROM pg_constraint WHERE conname = 'e2ee_proposals_exact_source'
        ) THEN
            ALTER TABLE e2ee_proposals ADD CONSTRAINT e2ee_proposals_exact_source
                CHECK (
                    (proposer_device_id IS NOT NULL AND external_sender_index IS NULL) OR
                    (proposer_device_id IS NULL AND external_sender_index = 0)
                );
        END IF;
    END
    $$";

const CREATE_MEMBERSHIP_RECONCILIATIONS_SQL: &str =
    "CREATE TABLE IF NOT EXISTS e2ee_membership_reconciliations (
    reconciliation_id  TEXT PRIMARY KEY,
    group_id            TEXT NOT NULL REFERENCES e2ee_groups(group_id) ON DELETE CASCADE,
    target_user_id      TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    target_device_id    TEXT NOT NULL REFERENCES e2ee_device_certificates(device_id),
    leaf_index          INTEGER NOT NULL CHECK (leaf_index >= 0 AND leaf_index < 200),
    requested_at_unix   BIGINT NOT NULL,
    deadline_unix       BIGINT NOT NULL CHECK (deadline_unix > requested_at_unix),
    completed_epoch     BIGINT CHECK (completed_epoch > 0),
    UNIQUE (group_id, target_device_id, leaf_index)
)";

const ADD_RECONCILIATION_FOREIGN_KEY_SQL: &str = "DO $$
    BEGIN
        IF NOT EXISTS (
            SELECT 1 FROM pg_constraint WHERE conname = 'e2ee_proposals_reconciliation_fk'
        ) THEN
            ALTER TABLE e2ee_proposals ADD CONSTRAINT e2ee_proposals_reconciliation_fk
                FOREIGN KEY (reconciliation_id)
                REFERENCES e2ee_membership_reconciliations(reconciliation_id)
                ON DELETE CASCADE;
        END IF;
    END
    $$";

const INDEX_PENDING_RECONCILIATIONS_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_e2ee_membership_reconciliations_pending
     ON e2ee_membership_reconciliations (group_id, deadline_unix)
     WHERE completed_epoch IS NULL";

/// Apply bounded group-DM leaf routing, multi-recipient Welcome delivery, and
/// persistent policy-removal reconciliation state.
pub(crate) async fn apply_e2ee_group_membership_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    for statement in [
        CREATE_GROUP_LEAVES_SQL,
        BACKFILL_COMMITTER_LEAVES_SQL,
        BACKFILL_WELCOME_LEAVES_SQL,
        ADD_COMMIT_MEMBERSHIP_CHANGE_SQL,
        CREATE_COMMIT_WELCOME_RECIPIENTS_SQL,
        BACKFILL_COMMIT_WELCOME_RECIPIENTS_SQL,
        ALTER_PROPOSER_NULLABILITY_SQL,
        ADD_EXTERNAL_SENDER_INDEX_SQL,
        ADD_RECONCILIATION_ID_SQL,
        ADD_RECONCILIATION_DEADLINE_SQL,
        ADD_PROPOSAL_SOURCE_CONSTRAINT_SQL,
        CREATE_MEMBERSHIP_RECONCILIATIONS_SQL,
        ADD_RECONCILIATION_FOREIGN_KEY_SQL,
        INDEX_PENDING_RECONCILIATIONS_SQL,
    ] {
        sqlx::query(statement).execute(&mut **tx).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_leaf_and_welcome_routing_are_bounded() {
        assert!(CREATE_GROUP_LEAVES_SQL.contains("leaf_index < 200"));
        assert!(CREATE_GROUP_LEAVES_SQL.contains("UNIQUE (group_id, device_id)"));
        assert!(BACKFILL_COMMITTER_LEAVES_SQL.contains("k.epoch = 1"));
        assert!(CREATE_COMMIT_WELCOME_RECIPIENTS_SQL.contains("ON DELETE CASCADE"));
        assert!(BACKFILL_COMMIT_WELCOME_RECIPIENTS_SQL.contains("welcome_device_id IS NOT NULL"));
    }

    #[test]
    fn external_proposals_have_one_typed_source_and_persistent_deadline() {
        assert!(ADD_PROPOSAL_SOURCE_CONSTRAINT_SQL.contains("e2ee_proposals_exact_source"));
        assert!(ADD_PROPOSAL_SOURCE_CONSTRAINT_SQL.contains("external_sender_index = 0"));
        assert!(CREATE_MEMBERSHIP_RECONCILIATIONS_SQL.contains("deadline_unix > requested_at_unix"));
        assert!(INDEX_PENDING_RECONCILIATIONS_SQL.contains("completed_epoch IS NULL"));
    }
}
