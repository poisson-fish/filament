use sqlx::{Postgres, Transaction};

const ADD_ENCRYPTED_CHANNEL_POLICY_SQL: &str =
    "ALTER TABLE guilds ADD COLUMN IF NOT EXISTS encrypted_channel_policy SMALLINT";
const BACKFILL_ENCRYPTED_CHANNEL_POLICY_SQL: &str =
    "UPDATE guilds SET encrypted_channel_policy = 0 WHERE encrypted_channel_policy IS NULL";
const ENCRYPTED_CHANNEL_POLICY_DEFAULT_SQL: &str =
    "ALTER TABLE guilds ALTER COLUMN encrypted_channel_policy SET DEFAULT 0";
const ENCRYPTED_CHANNEL_POLICY_NOT_NULL_SQL: &str =
    "ALTER TABLE guilds ALTER COLUMN encrypted_channel_policy SET NOT NULL";
const ADD_ENCRYPTED_CHANNEL_POLICY_CONSTRAINT_SQL: &str = "DO $$
    BEGIN
        IF NOT EXISTS (
            SELECT 1 FROM pg_constraint
            WHERE conname = 'guilds_encrypted_channel_policy'
        ) THEN
            ALTER TABLE guilds ADD CONSTRAINT guilds_encrypted_channel_policy
                CHECK (encrypted_channel_policy IN (0, 1, 2));
        END IF;
    END
    $$";

const CREATE_POLICY_TRANSITION_FUNCTION_SQL: &str =
    "CREATE OR REPLACE FUNCTION filament_guard_encrypted_channel_policy_change()
     RETURNS TRIGGER AS $$
     BEGIN
         IF OLD.encrypted_channel_policy <> NEW.encrypted_channel_policy
            AND EXISTS (
                SELECT 1 FROM channels
                WHERE guild_id = NEW.guild_id
                  AND channel_type = 1
            )
         THEN
             RAISE EXCEPTION USING
                 ERRCODE = '23514',
                 CONSTRAINT = 'encrypted_channel_policy_requires_reconciliation',
                 MESSAGE = 'encrypted channel policy cannot change while encrypted channels exist';
         END IF;
         RETURN NEW;
     END;
     $$ LANGUAGE plpgsql";
const CREATE_POLICY_TRANSITION_TRIGGER_SQL: &str = "DO $$
    BEGIN
        IF NOT EXISTS (
            SELECT 1 FROM pg_trigger
            WHERE tgname = 'guilds_encrypted_channel_policy_reconciliation'
              AND tgrelid = 'guilds'::regclass
        ) THEN
            CREATE TRIGGER guilds_encrypted_channel_policy_reconciliation
            BEFORE UPDATE OF encrypted_channel_policy ON guilds
            FOR EACH ROW
            EXECUTE FUNCTION filament_guard_encrypted_channel_policy_change();
        END IF;
    END
    $$";

/// Install the fail-closed workspace gate for Phase 6 encrypted channels.
///
/// Existing workspaces are explicitly backfilled to `disabled`. Until policy
/// reconciliation is implemented, changing policy while an encrypted channel
/// exists is forbidden at the database boundary so moderator membership
/// requirements cannot be weakened or stranded by a handler regression.
pub(crate) async fn apply_e2ee_channel_policy_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    for statement in [
        ADD_ENCRYPTED_CHANNEL_POLICY_SQL,
        BACKFILL_ENCRYPTED_CHANNEL_POLICY_SQL,
        ENCRYPTED_CHANNEL_POLICY_DEFAULT_SQL,
        ENCRYPTED_CHANNEL_POLICY_NOT_NULL_SQL,
        ADD_ENCRYPTED_CHANNEL_POLICY_CONSTRAINT_SQL,
        CREATE_POLICY_TRANSITION_FUNCTION_SQL,
        CREATE_POLICY_TRANSITION_TRIGGER_SQL,
    ] {
        sqlx::query(statement).execute(&mut **tx).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_policy_is_bounded_and_defaults_disabled() {
        assert!(BACKFILL_ENCRYPTED_CHANNEL_POLICY_SQL.contains("encrypted_channel_policy = 0"));
        assert!(ENCRYPTED_CHANNEL_POLICY_DEFAULT_SQL.contains("SET DEFAULT 0"));
        assert!(ADD_ENCRYPTED_CHANNEL_POLICY_CONSTRAINT_SQL
            .contains("encrypted_channel_policy IN (0, 1, 2)"));
    }

    #[test]
    fn policy_transition_requires_no_encrypted_channels() {
        assert!(CREATE_POLICY_TRANSITION_FUNCTION_SQL.contains("channel_type = 1"));
        assert!(CREATE_POLICY_TRANSITION_FUNCTION_SQL
            .contains("encrypted_channel_policy_requires_reconciliation"));
        assert!(CREATE_POLICY_TRANSITION_TRIGGER_SQL
            .contains("BEFORE UPDATE OF encrypted_channel_policy ON guilds"));
    }
}
