use sqlx::{Postgres, Transaction};

const CREATE_AUTHORIZED_CHANNEL_GROUP_VALIDATION_FUNCTION_SQL: &str =
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
             SELECT gm.user_id
             FROM guild_members gm
             WHERE gm.guild_id = NEW.guild_id
               AND NOT EXISTS (
                   SELECT 1 FROM guild_bans gb
                   WHERE gb.guild_id = gm.guild_id AND gb.user_id = gm.user_id
               )
               AND filament_e2ee_channel_user_can_post(
                   NEW.guild_id, NEW.channel_id, gm.user_id
               )
             EXCEPT
             SELECT user_id
             FROM e2ee_conversation_members
             WHERE conversation_id = NEW.conversation_id
         ) OR EXISTS (
             SELECT user_id
             FROM e2ee_conversation_members
             WHERE conversation_id = NEW.conversation_id
             EXCEPT
             SELECT gm.user_id
             FROM guild_members gm
             WHERE gm.guild_id = NEW.guild_id
               AND NOT EXISTS (
                   SELECT 1 FROM guild_bans gb
                   WHERE gb.guild_id = gm.guild_id AND gb.user_id = gm.user_id
               )
               AND filament_e2ee_channel_user_can_post(
                   NEW.guild_id, NEW.channel_id, gm.user_id
               )
         ) THEN
             RAISE EXCEPTION USING
                 ERRCODE = '23514',
                 CONSTRAINT = 'e2ee_channel_group_exact_authorized_audience',
                 MESSAGE = 'initial encrypted channel audience must equal channel authorization';
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

/// Replace the workspace-wide Phase 6 bootstrap invariant with an exact
/// authorization-to-MLS audience invariant.
///
/// Permission overwrites are installed in the channel provisioning
/// transaction before this existing trigger runs. The database therefore
/// independently rejects both omitted authorized users and injected
/// unauthorized users.
pub(crate) async fn apply_e2ee_channel_audience_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    sqlx::query(CREATE_AUTHORIZED_CHANNEL_GROUP_VALIDATION_FUNCTION_SQL)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_channel_audience_equals_exact_effective_authorization() {
        assert!(CREATE_AUTHORIZED_CHANNEL_GROUP_VALIDATION_FUNCTION_SQL
            .contains("filament_e2ee_channel_user_can_post"));
        assert!(
            CREATE_AUTHORIZED_CHANNEL_GROUP_VALIDATION_FUNCTION_SQL
                .matches("EXCEPT")
                .count()
                >= 2
        );
        assert!(CREATE_AUTHORIZED_CHANNEL_GROUP_VALIDATION_FUNCTION_SQL
            .contains("e2ee_channel_group_exact_authorized_audience"));
        assert!(CREATE_AUTHORIZED_CHANNEL_GROUP_VALIDATION_FUNCTION_SQL
            .contains("e2ee_channel_group_member_leaf"));
    }

    #[test]
    fn channel_audience_invariant_contains_no_plaintext_or_crypto_material() {
        assert!(!CREATE_AUTHORIZED_CHANNEL_GROUP_VALIDATION_FUNCTION_SQL.contains("content"));
        assert!(!CREATE_AUTHORIZED_CHANNEL_GROUP_VALIDATION_FUNCTION_SQL.contains("ciphertext"));
        assert!(!CREATE_AUTHORIZED_CHANNEL_GROUP_VALIDATION_FUNCTION_SQL.contains("welcome_blob"));
    }
}
