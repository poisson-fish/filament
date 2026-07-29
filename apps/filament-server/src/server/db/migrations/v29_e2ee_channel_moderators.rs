use sqlx::{Postgres, Transaction};

const CREATE_MODERATOR_MEMBERSHIP_PREDICATE_SQL: &str =
    "CREATE OR REPLACE FUNCTION filament_e2ee_user_in_all_encrypted_channels(
         checked_guild_id TEXT,
         checked_user_id TEXT
     )
     RETURNS BOOLEAN AS $$
     SELECT NOT EXISTS (
         SELECT 1
         FROM channels c
         LEFT JOIN e2ee_channel_groups cg ON cg.channel_id = c.channel_id
         WHERE c.guild_id = checked_guild_id
           AND c.channel_type = 1
           AND (
               cg.group_id IS NULL
               OR NOT EXISTS (
                   SELECT 1
                   FROM e2ee_group_leaves l
                   WHERE l.group_id = cg.group_id
                     AND l.user_id = checked_user_id
               )
           )
     )
     $$ LANGUAGE SQL STABLE";

const CREATE_MODERATOR_POLICY_GUARD_FUNCTION_SQL: &str =
    "CREATE OR REPLACE FUNCTION filament_guard_e2ee_moderator_policy()
     RETURNS TRIGGER AS $$
     DECLARE
         affected_guild_id TEXT;
     BEGIN
         affected_guild_id := CASE
             WHEN TG_OP = 'DELETE' THEN OLD.guild_id
             ELSE NEW.guild_id
         END;

         IF EXISTS (
             SELECT 1
             FROM guilds g
             WHERE g.guild_id = affected_guild_id
               AND g.encrypted_channel_policy = 1
               AND EXISTS (
                   SELECT 1
                   FROM guild_members gm
                   WHERE gm.guild_id = g.guild_id
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
                             g.guild_id, gm.user_id
                         )
                         OR EXISTS (
                             SELECT 1
                             FROM e2ee_channel_groups cg
                             JOIN e2ee_group_leaves l
                               ON l.group_id = cg.group_id
                              AND l.user_id = gm.user_id
                             WHERE cg.guild_id = g.guild_id
                               AND NOT filament_e2ee_channel_user_can_post(
                                   cg.guild_id, cg.channel_id, gm.user_id
                               )
                         )
                     )
               )
         ) THEN
             RAISE EXCEPTION USING
                 ERRCODE = '23514',
                 CONSTRAINT = 'e2ee_moderator_membership_required',
                 MESSAGE = 'moderators must remain authorized members of every encrypted channel';
         END IF;
         RETURN NULL;
     END;
     $$ LANGUAGE plpgsql";

const CREATE_MODERATOR_POLICY_GUARD_TRIGGERS_SQL: &str = "DO $$
    BEGIN
        IF NOT EXISTS (
            SELECT 1 FROM pg_trigger
            WHERE tgname = 'e2ee_moderator_role_guard'
              AND tgrelid = 'guild_roles'::regclass
        ) THEN
            CREATE CONSTRAINT TRIGGER e2ee_moderator_role_guard
            AFTER UPDATE OR DELETE ON guild_roles
            DEFERRABLE INITIALLY DEFERRED
            FOR EACH ROW
            EXECUTE FUNCTION filament_guard_e2ee_moderator_policy();
        END IF;

        IF NOT EXISTS (
            SELECT 1 FROM pg_trigger
            WHERE tgname = 'e2ee_moderator_assignment_guard'
              AND tgrelid = 'guild_role_members'::regclass
        ) THEN
            CREATE CONSTRAINT TRIGGER e2ee_moderator_assignment_guard
            AFTER INSERT OR UPDATE OR DELETE ON guild_role_members
            DEFERRABLE INITIALLY DEFERRED
            FOR EACH ROW
            EXECUTE FUNCTION filament_guard_e2ee_moderator_policy();
        END IF;

        IF NOT EXISTS (
            SELECT 1 FROM pg_trigger
            WHERE tgname = 'e2ee_moderator_legacy_role_guard'
              AND tgrelid = 'guild_members'::regclass
        ) THEN
            CREATE CONSTRAINT TRIGGER e2ee_moderator_legacy_role_guard
            AFTER INSERT OR UPDATE OR DELETE ON guild_members
            DEFERRABLE INITIALLY DEFERRED
            FOR EACH ROW
            EXECUTE FUNCTION filament_guard_e2ee_moderator_policy();
        END IF;

        IF NOT EXISTS (
            SELECT 1 FROM pg_trigger
            WHERE tgname = 'e2ee_moderator_override_guard'
              AND tgrelid = 'channel_permission_overrides'::regclass
        ) THEN
            CREATE CONSTRAINT TRIGGER e2ee_moderator_override_guard
            AFTER INSERT OR UPDATE OR DELETE ON channel_permission_overrides
            DEFERRABLE INITIALLY DEFERRED
            FOR EACH ROW
            EXECUTE FUNCTION filament_guard_e2ee_moderator_policy();
        END IF;

        IF NOT EXISTS (
            SELECT 1 FROM pg_trigger
            WHERE tgname = 'e2ee_moderator_legacy_override_guard'
              AND tgrelid = 'channel_role_overrides'::regclass
        ) THEN
            CREATE CONSTRAINT TRIGGER e2ee_moderator_legacy_override_guard
            AFTER INSERT OR UPDATE OR DELETE ON channel_role_overrides
            DEFERRABLE INITIALLY DEFERRED
            FOR EACH ROW
            EXECUTE FUNCTION filament_guard_e2ee_moderator_policy();
        END IF;
    END
    $$";

const CREATE_MODERATOR_LEAF_GUARD_FUNCTION_SQL: &str =
    "CREATE OR REPLACE FUNCTION filament_guard_e2ee_moderator_leaf()
     RETURNS TRIGGER AS $$
     DECLARE
         checked_group_id TEXT;
         checked_user_id TEXT;
     BEGIN
         checked_group_id := CASE
             WHEN TG_OP = 'DELETE' THEN OLD.group_id
             ELSE NEW.group_id
         END;
         checked_user_id := CASE
             WHEN TG_OP = 'DELETE' THEN OLD.user_id
             ELSE NEW.user_id
         END;

         IF EXISTS (
             SELECT 1
             FROM e2ee_channel_groups cg
             JOIN guilds g ON g.guild_id = cg.guild_id
             JOIN guild_members gm
               ON gm.guild_id = cg.guild_id
              AND gm.user_id = checked_user_id
             WHERE cg.group_id = checked_group_id
               AND g.encrypted_channel_policy = 1
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
               AND NOT EXISTS (
                   SELECT 1
                   FROM e2ee_group_leaves l
                   WHERE l.group_id = checked_group_id
                     AND l.user_id = checked_user_id
               )
         ) THEN
             RAISE EXCEPTION USING
                 ERRCODE = '23514',
                 CONSTRAINT = 'e2ee_moderator_membership_required',
                 MESSAGE = 'moderators must retain an MLS leaf in every encrypted channel';
         END IF;
         RETURN NULL;
     END;
     $$ LANGUAGE plpgsql";

const CREATE_MODERATOR_LEAF_GUARD_TRIGGER_SQL: &str = "DO $$
    BEGIN
        IF NOT EXISTS (
            SELECT 1 FROM pg_trigger
            WHERE tgname = 'e2ee_moderator_leaf_guard'
              AND tgrelid = 'e2ee_group_leaves'::regclass
        ) THEN
            CREATE CONSTRAINT TRIGGER e2ee_moderator_leaf_guard
            AFTER INSERT OR UPDATE OR DELETE ON e2ee_group_leaves
            DEFERRABLE INITIALLY DEFERRED
            FOR EACH ROW
            EXECUTE FUNCTION filament_guard_e2ee_moderator_leaf();
        END IF;
    END
    $$";

/// Enforce the visible-moderator invariant for
/// `require_moderator_membership` encrypted channels.
///
/// A moderator promotion is accepted only after the target already has an MLS
/// leaf in every encrypted channel. Permission mutations cannot make a visible
/// moderator ineligible for a channel, and an MLS commit cannot remove their
/// final leaf while the moderator role remains assigned.
pub(crate) async fn apply_e2ee_channel_moderator_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    for statement in [
        CREATE_MODERATOR_MEMBERSHIP_PREDICATE_SQL,
        CREATE_MODERATOR_POLICY_GUARD_FUNCTION_SQL,
        CREATE_MODERATOR_POLICY_GUARD_TRIGGERS_SQL,
        CREATE_MODERATOR_LEAF_GUARD_FUNCTION_SQL,
        CREATE_MODERATOR_LEAF_GUARD_TRIGGER_SQL,
    ] {
        sqlx::query(statement).execute(&mut **tx).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moderator_membership_requires_every_encrypted_channel_leaf() {
        assert!(CREATE_MODERATOR_MEMBERSHIP_PREDICATE_SQL.contains("channel_type = 1"));
        assert!(CREATE_MODERATOR_MEMBERSHIP_PREDICATE_SQL.contains("cg.group_id IS NULL"));
        assert!(CREATE_MODERATOR_MEMBERSHIP_PREDICATE_SQL.contains("e2ee_group_leaves"));
        assert!(CREATE_MODERATOR_POLICY_GUARD_FUNCTION_SQL.contains("encrypted_channel_policy = 1"));
        assert!(CREATE_MODERATOR_POLICY_GUARD_FUNCTION_SQL.contains("gr.system_key = 'moderator'"));
        assert!(CREATE_MODERATOR_POLICY_GUARD_FUNCTION_SQL
            .contains("e2ee_moderator_membership_required"));
    }

    #[test]
    fn moderator_authorization_and_leaf_removal_fail_closed() {
        assert!(CREATE_MODERATOR_POLICY_GUARD_FUNCTION_SQL
            .contains("filament_e2ee_channel_user_can_post"));
        assert!(
            CREATE_MODERATOR_POLICY_GUARD_TRIGGERS_SQL.contains("DEFERRABLE INITIALLY DEFERRED")
        );
        assert!(CREATE_MODERATOR_POLICY_GUARD_TRIGGERS_SQL.contains("channel_permission_overrides"));
        assert!(CREATE_MODERATOR_LEAF_GUARD_TRIGGER_SQL
            .contains("AFTER INSERT OR UPDATE OR DELETE ON e2ee_group_leaves"));
        assert!(
            CREATE_MODERATOR_LEAF_GUARD_FUNCTION_SQL.contains("e2ee_moderator_membership_required")
        );
    }
}
