use sqlx::{Postgres, Transaction};

const REPLACE_CHANNEL_MEMBERSHIP_GUARD_FUNCTION_SQL: &str =
    "CREATE OR REPLACE FUNCTION filament_guard_e2ee_channel_membership()
     RETURNS TRIGGER AS $$
     BEGIN
         IF TG_OP = 'DELETE'
            AND EXISTS (SELECT 1 FROM guilds g WHERE g.guild_id = OLD.guild_id)
            AND EXISTS (
                SELECT 1
                FROM e2ee_channel_groups cg
                JOIN e2ee_group_leaves l
                  ON l.group_id = cg.group_id
                 AND l.user_id = OLD.user_id
                WHERE cg.guild_id = OLD.guild_id
                  AND NOT EXISTS (
                      SELECT 1
                      FROM e2ee_membership_reconciliations r
                      JOIN e2ee_proposals p
                        ON p.reconciliation_id = r.reconciliation_id
                      WHERE r.group_id = l.group_id
                        AND r.target_user_id = l.user_id
                        AND r.target_device_id = l.device_id
                        AND r.leaf_index = l.leaf_index
                        AND r.completed_epoch IS NULL
                        AND p.external_sender_index = 0
                  )
            )
         THEN
             RAISE EXCEPTION USING
                 ERRCODE = '23514',
                 CONSTRAINT = 'e2ee_channel_member_remove_requires_reconciliation',
                 MESSAGE = 'encrypted channel member removal requires signed MLS Remove proposals';
         END IF;

         IF TG_OP = 'DELETE' THEN
             RETURN OLD;
         END IF;
         RETURN NEW;
     END;
     $$ LANGUAGE plpgsql";

/// Separate structural workspace membership from encrypted-channel membership.
///
/// A workspace join grants no MLS leaf and no ciphertext delivery. An existing
/// encrypted-channel member must still submit an authenticated, capability-
/// checked MLS Add commit with a recipient-bound Welcome. Structural removals
/// retain the signed Remove reconciliation guard.
pub(crate) async fn apply_e2ee_channel_workspace_join_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    sqlx::query(REPLACE_CHANNEL_MEMBERSHIP_GUARD_FUNCTION_SQL)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_join_does_not_create_or_require_an_mls_leaf() {
        assert!(!REPLACE_CHANNEL_MEMBERSHIP_GUARD_FUNCTION_SQL
            .contains("e2ee_channel_member_add_requires_reconciliation"));
        assert!(!REPLACE_CHANNEL_MEMBERSHIP_GUARD_FUNCTION_SQL.contains("TG_OP = 'INSERT'"));
        assert!(!REPLACE_CHANNEL_MEMBERSHIP_GUARD_FUNCTION_SQL
            .contains("INSERT INTO e2ee_group_leaves"));
        assert!(!REPLACE_CHANNEL_MEMBERSHIP_GUARD_FUNCTION_SQL
            .contains("INSERT INTO e2ee_conversation_members"));
    }

    #[test]
    fn workspace_removal_still_requires_exact_signed_reconciliation() {
        assert!(REPLACE_CHANNEL_MEMBERSHIP_GUARD_FUNCTION_SQL
            .contains("e2ee_channel_member_remove_requires_reconciliation"));
        assert!(
            REPLACE_CHANNEL_MEMBERSHIP_GUARD_FUNCTION_SQL.contains("p.external_sender_index = 0")
        );
        assert!(REPLACE_CHANNEL_MEMBERSHIP_GUARD_FUNCTION_SQL.contains("r.completed_epoch IS NULL"));
    }
}
