use sqlx::{Postgres, Transaction};

const CREATE_E2EE_CHANNEL_GROUPS_SQL: &str = "CREATE TABLE IF NOT EXISTS e2ee_channel_groups (
    channel_id       TEXT PRIMARY KEY REFERENCES channels(channel_id) ON DELETE CASCADE,
    guild_id         TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
    conversation_id  TEXT NOT NULL UNIQUE
        REFERENCES e2ee_conversations(conversation_id) ON DELETE CASCADE,
    group_id         TEXT NOT NULL UNIQUE REFERENCES e2ee_groups(group_id) ON DELETE CASCADE,
    provisioned_at_unix BIGINT NOT NULL,
    UNIQUE (guild_id, channel_id)
)";

const CREATE_CHANNEL_GROUP_VALIDATION_FUNCTION_SQL: &str =
    "CREATE OR REPLACE FUNCTION filament_validate_e2ee_channel_group()
     RETURNS TRIGGER AS $$
     BEGIN
         IF NOT EXISTS (
             SELECT 1
             FROM channels c
             JOIN guilds w ON w.guild_id = c.guild_id
             WHERE c.channel_id = NEW.channel_id
               AND c.guild_id = NEW.guild_id
               AND c.channel_type = 1
               AND w.encrypted_channel_policy <> 0
         ) THEN
             RAISE EXCEPTION USING
                 ERRCODE = '23514',
                 CONSTRAINT = 'e2ee_channel_group_requires_encrypted_channel',
                 MESSAGE = 'encrypted channel mapping requires enabled immutable channel mode';
         END IF;

         IF NOT EXISTS (
             SELECT 1
             FROM e2ee_groups g
             JOIN e2ee_conversations c ON c.conversation_id = g.conversation_id
             WHERE g.group_id = NEW.group_id
               AND g.conversation_id = NEW.conversation_id
               AND c.conversation_crypto = 'mls_v1'
         ) THEN
             RAISE EXCEPTION USING
                 ERRCODE = '23514',
                 CONSTRAINT = 'e2ee_channel_group_requires_mls',
                 MESSAGE = 'encrypted channel mapping requires its exact MLS group';
         END IF;

         IF EXISTS (
             SELECT user_id
             FROM guild_members
             WHERE guild_id = NEW.guild_id
             EXCEPT
             SELECT user_id
             FROM e2ee_conversation_members
             WHERE conversation_id = NEW.conversation_id
         ) OR EXISTS (
             SELECT user_id
             FROM e2ee_conversation_members
             WHERE conversation_id = NEW.conversation_id
             EXCEPT
             SELECT user_id
             FROM guild_members
             WHERE guild_id = NEW.guild_id
         ) THEN
             RAISE EXCEPTION USING
                 ERRCODE = '23514',
                 CONSTRAINT = 'e2ee_channel_group_exact_audience',
                 MESSAGE = 'initial encrypted channel audience must equal workspace membership';
         END IF;

         IF EXISTS (
             SELECT 1
             FROM guild_members gm
             WHERE gm.guild_id = NEW.guild_id
               AND NOT EXISTS (
                   SELECT 1
                   FROM guild_role_members grm
                   JOIN guild_roles gr
                     ON gr.guild_id = grm.guild_id AND gr.role_id = grm.role_id
                   WHERE grm.guild_id = gm.guild_id
                     AND grm.user_id = gm.user_id
                     AND (gr.permissions_allow_mask & 256) <> 0
               )
         ) THEN
             RAISE EXCEPTION USING
                 ERRCODE = '23514',
                 CONSTRAINT = 'e2ee_channel_group_authorized_audience',
                 MESSAGE = 'every initial encrypted channel member must be authorized';
         END IF;

         IF EXISTS (
             SELECT m.user_id
             FROM e2ee_conversation_members m
             WHERE m.conversation_id = NEW.conversation_id
               AND NOT EXISTS (
                   SELECT 1
                   FROM e2ee_group_leaves l
                   WHERE l.group_id = NEW.group_id
                     AND l.user_id = m.user_id
               )
         ) THEN
             RAISE EXCEPTION USING
                 ERRCODE = '23514',
                 CONSTRAINT = 'e2ee_channel_group_member_leaf',
                 MESSAGE = 'every encrypted channel member requires an MLS leaf';
         END IF;
         RETURN NEW;
     END;
     $$ LANGUAGE plpgsql";

const CREATE_CHANNEL_GROUP_VALIDATION_TRIGGER_SQL: &str = "DO $$
    BEGIN
        IF NOT EXISTS (
            SELECT 1 FROM pg_trigger
            WHERE tgname = 'e2ee_channel_group_validate'
              AND tgrelid = 'e2ee_channel_groups'::regclass
        ) THEN
            CREATE TRIGGER e2ee_channel_group_validate
            BEFORE INSERT OR UPDATE ON e2ee_channel_groups
            FOR EACH ROW
            EXECUTE FUNCTION filament_validate_e2ee_channel_group();
        END IF;
    END
    $$";

const CREATE_CHANNEL_GROUP_IMMUTABILITY_FUNCTION_SQL: &str =
    "CREATE OR REPLACE FUNCTION filament_reject_e2ee_channel_group_change()
     RETURNS TRIGGER AS $$
     BEGIN
         RAISE EXCEPTION USING
             ERRCODE = '23514',
             CONSTRAINT = 'e2ee_channel_group_immutable',
             MESSAGE = 'encrypted channel MLS binding is immutable';
     END;
     $$ LANGUAGE plpgsql";

const CREATE_CHANNEL_GROUP_IMMUTABILITY_TRIGGER_SQL: &str = "DO $$
    BEGIN
        IF NOT EXISTS (
            SELECT 1 FROM pg_trigger
            WHERE tgname = 'e2ee_channel_group_immutable'
              AND tgrelid = 'e2ee_channel_groups'::regclass
        ) THEN
            CREATE TRIGGER e2ee_channel_group_immutable
            BEFORE UPDATE ON e2ee_channel_groups
            FOR EACH ROW
            EXECUTE FUNCTION filament_reject_e2ee_channel_group_change();
        END IF;
    END
    $$";

const INDEX_CHANNEL_GROUP_GUILD_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_e2ee_channel_groups_guild
     ON e2ee_channel_groups (guild_id, channel_id)";

/// Bind encrypted workspace channels to their initial MLS state.
///
/// The validation trigger intentionally requires exact workspace-wide
/// membership for the first provisioning slice. Narrower permission audiences
/// remain unavailable until authorization reconciliation can update MLS and
/// routing state in one acceptance-gated operation.
pub(crate) async fn apply_e2ee_channel_group_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    for statement in [
        CREATE_E2EE_CHANNEL_GROUPS_SQL,
        CREATE_CHANNEL_GROUP_VALIDATION_FUNCTION_SQL,
        CREATE_CHANNEL_GROUP_VALIDATION_TRIGGER_SQL,
        CREATE_CHANNEL_GROUP_IMMUTABILITY_FUNCTION_SQL,
        CREATE_CHANNEL_GROUP_IMMUTABILITY_TRIGGER_SQL,
        INDEX_CHANNEL_GROUP_GUILD_SQL,
    ] {
        sqlx::query(statement).execute(&mut **tx).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_channel_binding_is_exact_and_immutable() {
        assert!(CREATE_E2EE_CHANNEL_GROUPS_SQL.contains("channel_id       TEXT PRIMARY KEY"));
        assert!(CREATE_E2EE_CHANNEL_GROUPS_SQL.contains("conversation_id  TEXT NOT NULL UNIQUE"));
        assert!(CREATE_E2EE_CHANNEL_GROUPS_SQL.contains("group_id         TEXT NOT NULL UNIQUE"));
        assert!(CREATE_CHANNEL_GROUP_VALIDATION_FUNCTION_SQL.contains("c.channel_type = 1"));
        assert!(CREATE_CHANNEL_GROUP_VALIDATION_FUNCTION_SQL
            .contains("w.encrypted_channel_policy <> 0"));
        assert!(CREATE_CHANNEL_GROUP_VALIDATION_FUNCTION_SQL.contains("EXCEPT"));
        assert!(CREATE_CHANNEL_GROUP_VALIDATION_FUNCTION_SQL
            .contains("e2ee_channel_group_authorized_audience"));
        assert!(CREATE_CHANNEL_GROUP_VALIDATION_FUNCTION_SQL.contains("e2ee_group_leaves"));
        assert!(CREATE_CHANNEL_GROUP_IMMUTABILITY_TRIGGER_SQL
            .contains("BEFORE UPDATE ON e2ee_channel_groups"));
    }

    #[test]
    fn encrypted_channel_binding_contains_no_plaintext_storage() {
        assert!(!CREATE_E2EE_CHANNEL_GROUPS_SQL.contains("content"));
        assert!(!CREATE_E2EE_CHANNEL_GROUPS_SQL.contains("ciphertext"));
        assert!(!CREATE_E2EE_CHANNEL_GROUPS_SQL.contains("welcome_blob"));
    }
}
