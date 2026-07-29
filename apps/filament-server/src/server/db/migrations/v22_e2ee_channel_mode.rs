use sqlx::{Postgres, Transaction};

const ADD_CHANNEL_TYPE_SQL: &str =
    "ALTER TABLE channels ADD COLUMN IF NOT EXISTS channel_type SMALLINT";
const BACKFILL_CHANNEL_TYPE_SQL: &str =
    "UPDATE channels SET channel_type = 0 WHERE channel_type IS NULL";
const CHANNEL_TYPE_DEFAULT_SQL: &str =
    "ALTER TABLE channels ALTER COLUMN channel_type SET DEFAULT 0";
const CHANNEL_TYPE_NOT_NULL_SQL: &str =
    "ALTER TABLE channels ALTER COLUMN channel_type SET NOT NULL";
const ADD_CHANNEL_TYPE_CONSTRAINT_SQL: &str = "DO $$
    BEGIN
        IF NOT EXISTS (
            SELECT 1 FROM pg_constraint WHERE conname = 'channels_channel_type'
        ) THEN
            ALTER TABLE channels ADD CONSTRAINT channels_channel_type
                CHECK (channel_type IN (0, 1));
        END IF;
    END
    $$";

const CREATE_IMMUTABLE_CHANNEL_TYPE_FUNCTION_SQL: &str =
    "CREATE OR REPLACE FUNCTION filament_reject_channel_type_change()
     RETURNS TRIGGER AS $$
     BEGIN
         IF OLD.channel_type <> NEW.channel_type THEN
             RAISE EXCEPTION USING
                 ERRCODE = '23514',
                 CONSTRAINT = 'channels_channel_type_immutable',
                 MESSAGE = 'channel confidentiality mode is immutable';
         END IF;
         RETURN NEW;
     END;
     $$ LANGUAGE plpgsql";
const CREATE_IMMUTABLE_CHANNEL_TYPE_TRIGGER_SQL: &str = "DO $$
    BEGIN
        IF NOT EXISTS (
            SELECT 1 FROM pg_trigger
            WHERE tgname = 'channels_channel_type_immutable'
              AND tgrelid = 'channels'::regclass
        ) THEN
            CREATE TRIGGER channels_channel_type_immutable
            BEFORE UPDATE OF channel_type ON channels
            FOR EACH ROW
            EXECUTE FUNCTION filament_reject_channel_type_change();
        END IF;
    END
    $$";

const CREATE_PLAINTEXT_CHANNEL_STORAGE_FUNCTION_SQL: &str =
    "CREATE OR REPLACE FUNCTION filament_require_plaintext_channel_storage()
     RETURNS TRIGGER AS $$
     BEGIN
         IF NOT EXISTS (
             SELECT 1 FROM channels
             WHERE channel_id = NEW.channel_id
               AND guild_id = NEW.guild_id
               AND channel_type = 0
         ) THEN
             RAISE EXCEPTION USING
                 ERRCODE = '23514',
                 CONSTRAINT = 'plaintext_storage_requires_plaintext_channel',
                 MESSAGE = 'plaintext rows are forbidden for encrypted channels';
         END IF;
         RETURN NEW;
     END;
     $$ LANGUAGE plpgsql";
const CREATE_PLAINTEXT_MESSAGE_TRIGGER_SQL: &str = "DO $$
    BEGIN
        IF NOT EXISTS (
            SELECT 1 FROM pg_trigger
            WHERE tgname = 'messages_require_plaintext_channel'
              AND tgrelid = 'messages'::regclass
        ) THEN
            CREATE TRIGGER messages_require_plaintext_channel
            BEFORE INSERT OR UPDATE OF guild_id, channel_id ON messages
            FOR EACH ROW
            EXECUTE FUNCTION filament_require_plaintext_channel_storage();
        END IF;
    END
    $$";
const CREATE_PLAINTEXT_ATTACHMENT_TRIGGER_SQL: &str = "DO $$
    BEGIN
        IF NOT EXISTS (
            SELECT 1 FROM pg_trigger
            WHERE tgname = 'attachments_require_plaintext_channel'
              AND tgrelid = 'attachments'::regclass
        ) THEN
            CREATE TRIGGER attachments_require_plaintext_channel
            BEFORE INSERT OR UPDATE OF guild_id, channel_id ON attachments
            FOR EACH ROW
            EXECUTE FUNCTION filament_require_plaintext_channel_storage();
        END IF;
    END
    $$";

/// Install the immutable Phase 6 channel confidentiality boundary.
///
/// Existing channels are explicitly backfilled as plaintext. Encrypted
/// channels must be inserted with their final mode by the future atomic MLS
/// provisioning path, and ordinary message/attachment tables cannot accept
/// rows for them even if a handler regresses.
pub(crate) async fn apply_e2ee_channel_mode_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    for statement in [
        ADD_CHANNEL_TYPE_SQL,
        BACKFILL_CHANNEL_TYPE_SQL,
        CHANNEL_TYPE_DEFAULT_SQL,
        CHANNEL_TYPE_NOT_NULL_SQL,
        ADD_CHANNEL_TYPE_CONSTRAINT_SQL,
        CREATE_IMMUTABLE_CHANNEL_TYPE_FUNCTION_SQL,
        CREATE_IMMUTABLE_CHANNEL_TYPE_TRIGGER_SQL,
        CREATE_PLAINTEXT_CHANNEL_STORAGE_FUNCTION_SQL,
        CREATE_PLAINTEXT_MESSAGE_TRIGGER_SQL,
        CREATE_PLAINTEXT_ATTACHMENT_TRIGGER_SQL,
    ] {
        sqlx::query(statement).execute(&mut **tx).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_type_is_bounded_backfilled_and_immutable() {
        assert!(BACKFILL_CHANNEL_TYPE_SQL.contains("channel_type = 0"));
        assert!(ADD_CHANNEL_TYPE_CONSTRAINT_SQL.contains("channel_type IN (0, 1)"));
        assert!(CREATE_IMMUTABLE_CHANNEL_TYPE_TRIGGER_SQL.contains("BEFORE UPDATE OF channel_type"));
        assert!(
            CREATE_IMMUTABLE_CHANNEL_TYPE_FUNCTION_SQL.contains("channels_channel_type_immutable")
        );
    }

    #[test]
    fn ordinary_storage_rejects_encrypted_or_cross_guild_channels() {
        assert!(CREATE_PLAINTEXT_CHANNEL_STORAGE_FUNCTION_SQL.contains("channel_type = 0"));
        assert!(CREATE_PLAINTEXT_CHANNEL_STORAGE_FUNCTION_SQL.contains("guild_id = NEW.guild_id"));
        assert!(CREATE_PLAINTEXT_MESSAGE_TRIGGER_SQL
            .contains("BEFORE INSERT OR UPDATE OF guild_id, channel_id ON messages"));
        assert!(CREATE_PLAINTEXT_ATTACHMENT_TRIGGER_SQL
            .contains("BEFORE INSERT OR UPDATE OF guild_id, channel_id ON attachments"));
    }
}
