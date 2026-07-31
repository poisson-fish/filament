use sqlx::{Postgres, Transaction};

const CREATE_E2EE_DM_PAIRS_SQL: &str = "CREATE TABLE IF NOT EXISTS e2ee_dm_pairs (
    conversation_id TEXT PRIMARY KEY REFERENCES e2ee_conversations(conversation_id) ON DELETE CASCADE,
    user_a_id       TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    user_b_id       TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    CHECK (user_a_id < user_b_id),
    FOREIGN KEY (conversation_id, user_a_id)
        REFERENCES e2ee_conversation_members(conversation_id, user_id),
    FOREIGN KEY (conversation_id, user_b_id)
        REFERENCES e2ee_conversation_members(conversation_id, user_id),
    UNIQUE (user_a_id, user_b_id)
)";

const CREATE_PAIR_VALIDATION_FUNCTION_SQL: &str =
    "CREATE OR REPLACE FUNCTION filament_validate_e2ee_dm_pair()
     RETURNS TRIGGER AS $$
     BEGIN
         IF NOT EXISTS (
             SELECT 1 FROM e2ee_conversations
             WHERE conversation_id = NEW.conversation_id
               AND conversation_crypto = 'mls_v1'
         ) OR (
             SELECT COUNT(*) FROM e2ee_conversation_members
             WHERE conversation_id = NEW.conversation_id
         ) <> 2 THEN
             RAISE EXCEPTION USING
                 ERRCODE = '23514',
                 CONSTRAINT = 'e2ee_dm_pair_membership',
                 MESSAGE = 'encrypted DM pair must map exactly two MLS members';
         END IF;
         RETURN NEW;
     END;
     $$ LANGUAGE plpgsql";

const CREATE_PAIR_VALIDATION_TRIGGER_SQL: &str = "DO $$
     BEGIN
         IF NOT EXISTS (
             SELECT 1 FROM pg_trigger
             WHERE tgname = 'e2ee_dm_pair_membership'
               AND tgrelid = 'e2ee_dm_pairs'::regclass
         ) THEN
             CREATE TRIGGER e2ee_dm_pair_membership
             BEFORE INSERT OR UPDATE ON e2ee_dm_pairs
             FOR EACH ROW
             EXECUTE FUNCTION filament_validate_e2ee_dm_pair();
         END IF;
     END
     $$";

const BACKFILL_E2EE_DM_PAIRS_SQL: &str =
    "INSERT INTO e2ee_dm_pairs (conversation_id, user_a_id, user_b_id)
     SELECT c.conversation_id, MIN(m.user_id), MAX(m.user_id)
     FROM e2ee_conversations c
     JOIN e2ee_groups g ON g.conversation_id = c.conversation_id
     JOIN e2ee_conversation_members m ON m.conversation_id = c.conversation_id
     WHERE c.conversation_crypto = 'mls_v1'
     GROUP BY c.conversation_id
     HAVING COUNT(*) = 2
     ON CONFLICT DO NOTHING";

const CREATE_NO_DOWNGRADE_FUNCTION_SQL: &str =
    "CREATE OR REPLACE FUNCTION filament_reject_e2ee_conversation_downgrade()
     RETURNS TRIGGER AS $$
     BEGIN
         IF OLD.conversation_crypto = 'mls_v1' AND NEW.conversation_crypto <> 'mls_v1' THEN
             RAISE EXCEPTION USING
                 ERRCODE = '23514',
                 CONSTRAINT = 'e2ee_conversation_no_downgrade',
                 MESSAGE = 'mls_v1 conversations cannot be downgraded';
         END IF;
         RETURN NEW;
     END;
     $$ LANGUAGE plpgsql";

const CREATE_NO_DOWNGRADE_TRIGGER_SQL: &str = "DO $$
     BEGIN
         IF NOT EXISTS (
             SELECT 1 FROM pg_trigger
             WHERE tgname = 'e2ee_conversation_no_downgrade'
               AND tgrelid = 'e2ee_conversations'::regclass
         ) THEN
             CREATE TRIGGER e2ee_conversation_no_downgrade
             BEFORE UPDATE OF conversation_crypto ON e2ee_conversations
             FOR EACH ROW
             EXECUTE FUNCTION filament_reject_e2ee_conversation_downgrade();
         END IF;
     END
     $$";

/// Apply the Phase 2 two-user conversation provisioning invariants.
///
/// The pair table prevents duplicate encrypted DMs for the same two users.
/// The trigger makes the no-silent-downgrade rule durable across future write
/// paths instead of relying on a single HTTP handler.
pub(crate) async fn apply_e2ee_conversation_provisioning_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    for statement in [
        CREATE_E2EE_DM_PAIRS_SQL,
        CREATE_PAIR_VALIDATION_FUNCTION_SQL,
        CREATE_PAIR_VALIDATION_TRIGGER_SQL,
        BACKFILL_E2EE_DM_PAIRS_SQL,
        CREATE_NO_DOWNGRADE_FUNCTION_SQL,
        CREATE_NO_DOWNGRADE_TRIGGER_SQL,
    ] {
        sqlx::query(statement).execute(&mut **tx).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_dm_pairs_are_canonical_and_unique() {
        assert!(CREATE_E2EE_DM_PAIRS_SQL.contains("CHECK (user_a_id < user_b_id)"));
        assert!(CREATE_E2EE_DM_PAIRS_SQL.contains("UNIQUE (user_a_id, user_b_id)"));
        assert!(CREATE_E2EE_DM_PAIRS_SQL.contains("ON DELETE CASCADE"));
        assert!(CREATE_E2EE_DM_PAIRS_SQL.contains("REFERENCES e2ee_conversation_members"));
        assert!(CREATE_PAIR_VALIDATION_FUNCTION_SQL.contains("conversation_crypto = 'mls_v1'"));
        assert!(CREATE_PAIR_VALIDATION_FUNCTION_SQL.contains(") <> 2"));
        assert!(BACKFILL_E2EE_DM_PAIRS_SQL.contains("HAVING COUNT(*) = 2"));
        assert!(BACKFILL_E2EE_DM_PAIRS_SQL.contains("ON CONFLICT DO NOTHING"));
    }

    #[test]
    fn database_rejects_mls_downgrades() {
        assert!(CREATE_NO_DOWNGRADE_FUNCTION_SQL.contains("OLD.conversation_crypto = 'mls_v1'"));
        assert!(CREATE_NO_DOWNGRADE_FUNCTION_SQL.contains("e2ee_conversation_no_downgrade"));
        assert!(CREATE_NO_DOWNGRADE_TRIGGER_SQL.contains("BEFORE UPDATE OF conversation_crypto"));
    }
}
