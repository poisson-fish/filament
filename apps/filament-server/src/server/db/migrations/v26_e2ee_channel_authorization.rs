use sqlx::{Postgres, Transaction};

const CREATE_CHANNEL_POST_AUTHORIZATION_FUNCTION_SQL: &str =
    "CREATE OR REPLACE FUNCTION filament_e2ee_channel_user_can_post(
         checked_guild_id TEXT,
         checked_channel_id TEXT,
         checked_user_id TEXT
     )
     RETURNS BOOLEAN AS $$
     WITH role_ids AS (
         SELECT
             MAX(role_id) FILTER (WHERE system_key = 'everyone') AS everyone_role_id,
             MAX(role_id) FILTER (WHERE system_key = 'workspace_owner') AS owner_role_id
         FROM guild_roles
         WHERE guild_id = checked_guild_id
     ),
     effective_role_members AS (
         SELECT grm.guild_id, grm.role_id, grm.user_id
         FROM guild_role_members grm
         WHERE grm.guild_id = checked_guild_id
           AND grm.user_id = checked_user_id
         UNION
         SELECT gm.guild_id, gr.role_id, gm.user_id
         FROM guild_members gm
         JOIN guild_roles gr ON gr.guild_id = gm.guild_id
         WHERE gm.guild_id = checked_guild_id
           AND gm.user_id = checked_user_id
           AND (
               (gm.role = 2 AND gr.system_key = 'workspace_owner')
               OR (
                   gm.role = 1
                   AND (
                       gr.system_key = 'moderator'
                       OR LOWER(gr.name) = 'moderator'
                   )
               )
               OR (
                   gm.role = 0
                   AND (
                       gr.system_key = 'member'
                       OR LOWER(gr.name) = 'member'
                   )
               )
           )
     ),
     permission_state AS (
         SELECT
             EXISTS (
                 SELECT 1
                 FROM guild_members gm
                 WHERE gm.guild_id = checked_guild_id
                   AND gm.user_id = checked_user_id
             ) AS is_member,
             EXISTS (
                 SELECT 1
                 FROM effective_role_members grm
                 CROSS JOIN role_ids ids
                 WHERE grm.guild_id = checked_guild_id
                   AND grm.user_id = checked_user_id
                   AND grm.role_id = ids.owner_role_id
             ) AS is_owner,
             (
                 EXISTS (
                     SELECT 1
                     FROM guild_roles gr
                     CROSS JOIN role_ids ids
                     WHERE gr.guild_id = checked_guild_id
                       AND gr.role_id = ids.everyone_role_id
                       AND (gr.permissions_allow_mask & 256) <> 0
                 )
                 OR EXISTS (
                     SELECT 1
                     FROM effective_role_members grm
                     JOIN guild_roles gr
                       ON gr.guild_id = grm.guild_id AND gr.role_id = grm.role_id
                     WHERE grm.guild_id = checked_guild_id
                       AND grm.user_id = checked_user_id
                       AND (gr.permissions_allow_mask & 256) <> 0
                 )
             ) AS base_allowed,
             EXISTS (
                 SELECT 1
                 FROM channel_permission_overrides o
                 CROSS JOIN role_ids ids
                 WHERE o.guild_id = checked_guild_id
                   AND o.channel_id = checked_channel_id
                   AND o.target_kind = 0
                   AND o.target_id = ids.everyone_role_id
                   AND (o.allow_mask & 256) <> 0
             ) AS everyone_allow,
             EXISTS (
                 SELECT 1
                 FROM channel_permission_overrides o
                 CROSS JOIN role_ids ids
                 WHERE o.guild_id = checked_guild_id
                   AND o.channel_id = checked_channel_id
                   AND o.target_kind = 0
                   AND o.target_id = ids.everyone_role_id
                   AND (o.deny_mask & 256) <> 0
             ) AS everyone_deny,
             EXISTS (
                 SELECT 1
                 FROM channel_permission_overrides o
                 JOIN effective_role_members grm
                   ON grm.guild_id = o.guild_id AND grm.role_id = o.target_id
                 CROSS JOIN role_ids ids
                 WHERE o.guild_id = checked_guild_id
                   AND o.channel_id = checked_channel_id
                   AND o.target_kind = 0
                   AND o.target_id <> ids.everyone_role_id
                   AND grm.user_id = checked_user_id
                   AND (o.allow_mask & 256) <> 0
             ) AS role_allow,
             EXISTS (
                 SELECT 1
                 FROM channel_permission_overrides o
                 JOIN effective_role_members grm
                   ON grm.guild_id = o.guild_id AND grm.role_id = o.target_id
                 CROSS JOIN role_ids ids
                 WHERE o.guild_id = checked_guild_id
                   AND o.channel_id = checked_channel_id
                   AND o.target_kind = 0
                   AND o.target_id <> ids.everyone_role_id
                   AND grm.user_id = checked_user_id
                   AND (o.deny_mask & 256) <> 0
             ) AS role_deny,
             (
                 (
                     NOT EXISTS (
                         SELECT 1
                         FROM channel_permission_overrides o
                         WHERE o.guild_id = checked_guild_id
                           AND o.channel_id = checked_channel_id
                     )
                     OR EXISTS (
                         SELECT 1
                         FROM channel_permission_overrides o
                         WHERE o.guild_id = checked_guild_id
                           AND o.channel_id = checked_channel_id
                           AND o.target_kind NOT IN (0, 1)
                     )
                 )
                 AND EXISTS (
                     SELECT 1
                     FROM channel_role_overrides o
                     JOIN guild_members gm
                       ON gm.guild_id = o.guild_id AND gm.role = o.role
                     WHERE o.guild_id = checked_guild_id
                       AND o.channel_id = checked_channel_id
                       AND gm.user_id = checked_user_id
                       AND (o.allow_mask & 256) <> 0
                 )
             ) AS legacy_role_allow,
             (
                 (
                     NOT EXISTS (
                         SELECT 1
                         FROM channel_permission_overrides o
                         WHERE o.guild_id = checked_guild_id
                           AND o.channel_id = checked_channel_id
                     )
                     OR EXISTS (
                         SELECT 1
                         FROM channel_permission_overrides o
                         WHERE o.guild_id = checked_guild_id
                           AND o.channel_id = checked_channel_id
                           AND o.target_kind NOT IN (0, 1)
                     )
                 )
                 AND EXISTS (
                     SELECT 1
                     FROM channel_role_overrides o
                     JOIN guild_members gm
                       ON gm.guild_id = o.guild_id AND gm.role = o.role
                     WHERE o.guild_id = checked_guild_id
                       AND o.channel_id = checked_channel_id
                       AND gm.user_id = checked_user_id
                       AND (o.deny_mask & 256) <> 0
                 )
             ) AS legacy_role_deny,
             EXISTS (
                 SELECT 1
                 FROM channel_permission_overrides o
                 WHERE o.guild_id = checked_guild_id
                   AND o.channel_id = checked_channel_id
                   AND o.target_kind = 1
                   AND o.target_id = checked_user_id
                   AND (o.allow_mask & 256) <> 0
             ) AS member_allow,
             EXISTS (
                 SELECT 1
                 FROM channel_permission_overrides o
                 WHERE o.guild_id = checked_guild_id
                   AND o.channel_id = checked_channel_id
                   AND o.target_kind = 1
                   AND o.target_id = checked_user_id
                   AND (o.deny_mask & 256) <> 0
             ) AS member_deny
     )
     SELECT is_member AND (
         is_owner
         OR (
             (
                 (
                     (base_allowed OR everyone_allow)
                     AND NOT everyone_deny
                 ) OR role_allow OR legacy_role_allow
             ) AND NOT (role_deny OR legacy_role_deny)
             OR member_allow
         ) AND NOT member_deny
     )
     FROM permission_state
     $$ LANGUAGE SQL STABLE";

const CREATE_CHANNEL_AUTHORIZATION_GUARD_FUNCTION_SQL: &str =
    "CREATE OR REPLACE FUNCTION filament_guard_e2ee_channel_authorization()
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
             FROM e2ee_channel_groups cg
             JOIN e2ee_group_leaves l ON l.group_id = cg.group_id
             WHERE cg.guild_id = affected_guild_id
               AND NOT filament_e2ee_channel_user_can_post(
                   cg.guild_id, cg.channel_id, l.user_id
               )
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
         ) THEN
             RAISE EXCEPTION USING
                 ERRCODE = '23514',
                 CONSTRAINT = 'e2ee_channel_permission_loss_requires_reconciliation',
                 MESSAGE = 'encrypted channel permission loss requires signed MLS Remove proposals';
         END IF;
         RETURN NULL;
     END;
     $$ LANGUAGE plpgsql";

const CREATE_CHANNEL_AUTHORIZATION_GUARD_TRIGGERS_SQL: &str = "DO $$
    BEGIN
        IF NOT EXISTS (
            SELECT 1 FROM pg_trigger
            WHERE tgname = 'e2ee_channel_role_authorization_guard'
              AND tgrelid = 'guild_roles'::regclass
        ) THEN
            CREATE CONSTRAINT TRIGGER e2ee_channel_role_authorization_guard
            AFTER UPDATE OR DELETE ON guild_roles
            DEFERRABLE INITIALLY DEFERRED
            FOR EACH ROW
            EXECUTE FUNCTION filament_guard_e2ee_channel_authorization();
        END IF;

        IF NOT EXISTS (
            SELECT 1 FROM pg_trigger
            WHERE tgname = 'e2ee_channel_role_member_authorization_guard'
              AND tgrelid = 'guild_role_members'::regclass
        ) THEN
            CREATE CONSTRAINT TRIGGER e2ee_channel_role_member_authorization_guard
            AFTER INSERT OR UPDATE OR DELETE ON guild_role_members
            DEFERRABLE INITIALLY DEFERRED
            FOR EACH ROW
            EXECUTE FUNCTION filament_guard_e2ee_channel_authorization();
        END IF;

        IF NOT EXISTS (
            SELECT 1 FROM pg_trigger
            WHERE tgname = 'e2ee_channel_legacy_member_role_authorization_guard'
              AND tgrelid = 'guild_members'::regclass
        ) THEN
            CREATE CONSTRAINT TRIGGER e2ee_channel_legacy_member_role_authorization_guard
            AFTER UPDATE ON guild_members
            DEFERRABLE INITIALLY DEFERRED
            FOR EACH ROW
            EXECUTE FUNCTION filament_guard_e2ee_channel_authorization();
        END IF;

        IF NOT EXISTS (
            SELECT 1 FROM pg_trigger
            WHERE tgname = 'e2ee_channel_override_authorization_guard'
              AND tgrelid = 'channel_permission_overrides'::regclass
        ) THEN
            CREATE CONSTRAINT TRIGGER e2ee_channel_override_authorization_guard
            AFTER INSERT OR UPDATE OR DELETE ON channel_permission_overrides
            DEFERRABLE INITIALLY DEFERRED
            FOR EACH ROW
            EXECUTE FUNCTION filament_guard_e2ee_channel_authorization();
        END IF;

        IF NOT EXISTS (
            SELECT 1 FROM pg_trigger
            WHERE tgname = 'e2ee_channel_legacy_override_authorization_guard'
              AND tgrelid = 'channel_role_overrides'::regclass
        ) THEN
            CREATE CONSTRAINT TRIGGER e2ee_channel_legacy_override_authorization_guard
            AFTER INSERT OR UPDATE OR DELETE ON channel_role_overrides
            DEFERRABLE INITIALLY DEFERRED
            FOR EACH ROW
            EXECUTE FUNCTION filament_guard_e2ee_channel_authorization();
        END IF;
    END
    $$";

/// Keep structural channel authorization and MLS leaf membership synchronized.
///
/// Constraint triggers are deferred so application transactions can first
/// change permissions and then atomically queue the exact signed removals.
pub(crate) async fn apply_e2ee_channel_authorization_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    for statement in [
        CREATE_CHANNEL_POST_AUTHORIZATION_FUNCTION_SQL,
        CREATE_CHANNEL_AUTHORIZATION_GUARD_FUNCTION_SQL,
        CREATE_CHANNEL_AUTHORIZATION_GUARD_TRIGGERS_SQL,
    ] {
        sqlx::query(statement).execute(&mut **tx).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorization_uses_permission_layers_and_exact_member_binding() {
        assert!(CREATE_CHANNEL_POST_AUTHORIZATION_FUNCTION_SQL.contains("is_member"));
        assert!(CREATE_CHANNEL_POST_AUTHORIZATION_FUNCTION_SQL.contains("is_owner"));
        assert!(CREATE_CHANNEL_POST_AUTHORIZATION_FUNCTION_SQL.contains("effective_role_members"));
        assert!(CREATE_CHANNEL_POST_AUTHORIZATION_FUNCTION_SQL.contains("base_allowed"));
        assert!(CREATE_CHANNEL_POST_AUTHORIZATION_FUNCTION_SQL.contains("everyone_deny"));
        assert!(CREATE_CHANNEL_POST_AUTHORIZATION_FUNCTION_SQL.contains("role_deny"));
        assert!(CREATE_CHANNEL_POST_AUTHORIZATION_FUNCTION_SQL.contains("legacy_role_deny"));
        assert!(CREATE_CHANNEL_POST_AUTHORIZATION_FUNCTION_SQL.contains("member_deny"));
        assert!(CREATE_CHANNEL_POST_AUTHORIZATION_FUNCTION_SQL.contains("& 256"));
    }

    #[test]
    fn permission_loss_requires_pending_signed_remove_for_every_leaf() {
        assert!(CREATE_CHANNEL_AUTHORIZATION_GUARD_FUNCTION_SQL
            .contains("e2ee_channel_permission_loss_requires_reconciliation"));
        assert!(CREATE_CHANNEL_AUTHORIZATION_GUARD_FUNCTION_SQL
            .contains("r.target_device_id = l.device_id"));
        assert!(
            CREATE_CHANNEL_AUTHORIZATION_GUARD_FUNCTION_SQL.contains("p.external_sender_index = 0")
        );
        assert!(CREATE_CHANNEL_AUTHORIZATION_GUARD_TRIGGERS_SQL
            .contains("DEFERRABLE INITIALLY DEFERRED"));
        assert!(CREATE_CHANNEL_AUTHORIZATION_GUARD_TRIGGERS_SQL
            .contains("channel_permission_overrides"));
        assert!(CREATE_CHANNEL_AUTHORIZATION_GUARD_TRIGGERS_SQL.contains("channel_role_overrides"));
        assert!(CREATE_CHANNEL_AUTHORIZATION_GUARD_TRIGGERS_SQL
            .contains("AFTER INSERT OR UPDATE OR DELETE ON guild_role_members"));
        assert!(CREATE_CHANNEL_AUTHORIZATION_GUARD_TRIGGERS_SQL
            .contains("AFTER UPDATE ON guild_members"));
    }
}
