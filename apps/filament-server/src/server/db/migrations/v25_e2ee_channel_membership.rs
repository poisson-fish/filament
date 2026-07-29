use sqlx::{Postgres, Transaction};

const REPLACE_RECONCILIATION_UNIQUE_CONSTRAINT_SQL: &str = "DO $$
    DECLARE
        constraint_name TEXT;
    BEGIN
        SELECT c.conname INTO constraint_name
        FROM pg_constraint c
        WHERE c.conrelid = 'e2ee_membership_reconciliations'::regclass
          AND c.contype = 'u'
          AND (
              SELECT array_agg(a.attname ORDER BY keys.ordinality)
              FROM unnest(c.conkey) WITH ORDINALITY AS keys(attnum, ordinality)
              JOIN pg_attribute a
                ON a.attrelid = c.conrelid AND a.attnum = keys.attnum
          ) = ARRAY['group_id', 'target_device_id', 'leaf_index']::name[];
        IF constraint_name IS NOT NULL THEN
            EXECUTE format(
                'ALTER TABLE e2ee_membership_reconciliations DROP CONSTRAINT %I',
                constraint_name
            );
        END IF;
    END
    $$";

const CREATE_PENDING_RECONCILIATION_UNIQUE_INDEX_SQL: &str =
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_e2ee_membership_reconciliations_unique_pending
     ON e2ee_membership_reconciliations (group_id, target_device_id, leaf_index)
     WHERE completed_epoch IS NULL";

const CREATE_CHANNEL_MEMBERSHIP_GUARD_FUNCTION_SQL: &str =
    "CREATE OR REPLACE FUNCTION filament_guard_e2ee_channel_membership()
     RETURNS TRIGGER AS $$
     BEGIN
         IF TG_OP = 'INSERT' AND EXISTS (
             SELECT 1
             FROM e2ee_channel_groups cg
             WHERE cg.guild_id = NEW.guild_id
         ) THEN
             RAISE EXCEPTION USING
                 ERRCODE = '23514',
                 CONSTRAINT = 'e2ee_channel_member_add_requires_reconciliation',
                 MESSAGE = 'encrypted channel member additions require an MLS Add flow';
         END IF;

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

const CREATE_CHANNEL_MEMBERSHIP_GUARD_TRIGGER_SQL: &str = "DO $$
    BEGIN
        IF NOT EXISTS (
            SELECT 1 FROM pg_trigger
            WHERE tgname = 'e2ee_channel_membership_guard'
              AND tgrelid = 'guild_members'::regclass
        ) THEN
            CREATE TRIGGER e2ee_channel_membership_guard
            AFTER INSERT OR DELETE ON guild_members
            FOR EACH ROW
            EXECUTE FUNCTION filament_guard_e2ee_channel_membership();
        END IF;
    END
    $$";

/// Require encrypted-channel workspace membership changes to cross the
/// acceptance-gated MLS reconciliation boundary.
pub(crate) async fn apply_e2ee_channel_membership_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    for statement in [
        REPLACE_RECONCILIATION_UNIQUE_CONSTRAINT_SQL,
        CREATE_PENDING_RECONCILIATION_UNIQUE_INDEX_SQL,
        CREATE_CHANNEL_MEMBERSHIP_GUARD_FUNCTION_SQL,
        CREATE_CHANNEL_MEMBERSHIP_GUARD_TRIGGER_SQL,
    ] {
        sqlx::query(statement).execute(&mut **tx).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn member_adds_require_mls_add_and_removals_require_signed_reconciliation() {
        assert!(CREATE_CHANNEL_MEMBERSHIP_GUARD_FUNCTION_SQL
            .contains("e2ee_channel_member_add_requires_reconciliation"));
        assert!(CREATE_CHANNEL_MEMBERSHIP_GUARD_FUNCTION_SQL
            .contains("e2ee_channel_member_remove_requires_reconciliation"));
        assert!(
            CREATE_CHANNEL_MEMBERSHIP_GUARD_FUNCTION_SQL.contains("p.external_sender_index = 0")
        );
        assert!(CREATE_CHANNEL_MEMBERSHIP_GUARD_FUNCTION_SQL.contains("r.completed_epoch IS NULL"));
        assert!(CREATE_CHANNEL_MEMBERSHIP_GUARD_TRIGGER_SQL
            .contains("AFTER INSERT OR DELETE ON guild_members"));
        assert!(CREATE_PENDING_RECONCILIATION_UNIQUE_INDEX_SQL
            .contains("WHERE completed_epoch IS NULL"));
        assert!(REPLACE_RECONCILIATION_UNIQUE_CONSTRAINT_SQL.contains("c.contype = 'u'"));
    }
}
