use sqlx::{Postgres, Transaction};

const REPLACE_POLICY_TRANSITION_FUNCTION_SQL: &str =
    "CREATE OR REPLACE FUNCTION filament_guard_encrypted_channel_policy_change()
     RETURNS TRIGGER AS $$
     BEGIN
         IF OLD.encrypted_channel_policy = NEW.encrypted_channel_policy
            OR NOT EXISTS (
                SELECT 1 FROM channels
                WHERE guild_id = NEW.guild_id
                  AND channel_type = 1
            )
         THEN
             RETURN NEW;
         END IF;

         IF NEW.encrypted_channel_policy = 0 THEN
             RAISE EXCEPTION USING
                 ERRCODE = '23514',
                 CONSTRAINT = 'encrypted_channel_policy_requires_reconciliation',
                 MESSAGE = 'encrypted channel policy cannot be disabled while encrypted channels exist';
         END IF;

         IF NEW.encrypted_channel_policy = 1
            AND (
                EXISTS (
                    SELECT 1
                    FROM channels c
                    LEFT JOIN e2ee_channel_groups cg ON cg.channel_id = c.channel_id
                    WHERE c.guild_id = NEW.guild_id
                      AND c.channel_type = 1
                      AND cg.group_id IS NULL
                )
                OR EXISTS (
                    SELECT 1
                    FROM guild_members gm
                    WHERE gm.guild_id = NEW.guild_id
                      AND (
                          gm.role = 1
                          OR EXISTS (
                              SELECT 1
                              FROM guild_role_members grm
                              JOIN guild_roles gr
                                ON gr.guild_id = grm.guild_id
                               AND gr.role_id = grm.role_id
                              WHERE grm.guild_id = gm.guild_id
                                AND grm.user_id = gm.user_id
                                AND gr.system_key = 'moderator'
                          )
                      )
                      AND (
                          NOT filament_e2ee_user_in_all_encrypted_channels(
                              NEW.guild_id, gm.user_id
                          )
                          OR EXISTS (
                              SELECT 1
                              FROM e2ee_channel_groups cg
                              JOIN e2ee_group_leaves l
                                ON l.group_id = cg.group_id
                               AND l.user_id = gm.user_id
                              WHERE cg.guild_id = NEW.guild_id
                                AND NOT filament_e2ee_channel_user_can_post(
                                    cg.guild_id, cg.channel_id, gm.user_id
                                )
                          )
                      )
                )
            )
         THEN
             RAISE EXCEPTION USING
                 ERRCODE = '23514',
                 CONSTRAINT = 'e2ee_moderator_membership_required',
                 MESSAGE = 'moderators must be authorized MLS members before policy tightening';
         END IF;

         RETURN NEW;
     END;
     $$ LANGUAGE plpgsql";

/// Replace the temporary blanket policy freeze with invariant-checked
/// transitions.
///
/// Relaxing the visible-moderator policy to `unrestricted` does not change the
/// MLS audience and is safe. Tightening it requires every moderator to already
/// be an authorized member of every encrypted channel. Disabling encryption
/// remains forbidden while an encrypted channel exists because channel mode is
/// immutable.
pub(crate) async fn apply_e2ee_channel_policy_transition_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    sqlx::query(REPLACE_POLICY_TRANSITION_FUNCTION_SQL)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_transition_state_machine_is_fail_closed() {
        assert!(REPLACE_POLICY_TRANSITION_FUNCTION_SQL.contains("NEW.encrypted_channel_policy = 0"));
        assert!(REPLACE_POLICY_TRANSITION_FUNCTION_SQL
            .contains("encrypted_channel_policy_requires_reconciliation"));
        assert!(REPLACE_POLICY_TRANSITION_FUNCTION_SQL.contains("NEW.encrypted_channel_policy = 1"));
        assert!(REPLACE_POLICY_TRANSITION_FUNCTION_SQL
            .contains("filament_e2ee_user_in_all_encrypted_channels"));
        assert!(
            REPLACE_POLICY_TRANSITION_FUNCTION_SQL.contains("filament_e2ee_channel_user_can_post")
        );
        assert!(
            REPLACE_POLICY_TRANSITION_FUNCTION_SQL.contains("e2ee_moderator_membership_required")
        );
    }

    #[test]
    fn unrestricted_transition_has_no_membership_mutation() {
        assert!(!REPLACE_POLICY_TRANSITION_FUNCTION_SQL.contains("UPDATE e2ee_group_leaves"));
        assert!(!REPLACE_POLICY_TRANSITION_FUNCTION_SQL.contains("DELETE FROM e2ee_group_leaves"));
        assert!(!REPLACE_POLICY_TRANSITION_FUNCTION_SQL.contains("INSERT INTO e2ee_group_leaves"));
    }
}
