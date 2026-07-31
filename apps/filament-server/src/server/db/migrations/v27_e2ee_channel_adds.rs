use sqlx::{Postgres, Transaction};

const CREATE_CHANNEL_LEAF_VALIDATION_FUNCTION_SQL: &str =
    "CREATE OR REPLACE FUNCTION filament_validate_e2ee_channel_leaf()
     RETURNS TRIGGER AS $$
     BEGIN
         IF EXISTS (
             SELECT 1
             FROM e2ee_channel_groups cg
             WHERE cg.group_id = NEW.group_id
               AND (
                   NOT filament_e2ee_channel_user_can_post(
                       cg.guild_id, cg.channel_id, NEW.user_id
                   )
                   OR NOT EXISTS (
                       SELECT 1
                       FROM e2ee_device_certificates d
                       WHERE d.device_id = NEW.device_id
                         AND d.user_id = NEW.user_id
                         AND d.tombstoned_at_unix IS NULL
                   )
               )
         ) THEN
             RAISE EXCEPTION USING
                 ERRCODE = '23514',
                 CONSTRAINT = 'e2ee_channel_leaf_requires_authorized_device',
                 MESSAGE = 'encrypted channel leaves require an authorized active owned device';
         END IF;
         RETURN NEW;
     END;
     $$ LANGUAGE plpgsql";

const CREATE_CHANNEL_LEAF_VALIDATION_TRIGGER_SQL: &str = "DO $$
    BEGIN
        IF NOT EXISTS (
            SELECT 1 FROM pg_trigger
            WHERE tgname = 'e2ee_channel_leaf_validate'
              AND tgrelid = 'e2ee_group_leaves'::regclass
        ) THEN
            CREATE TRIGGER e2ee_channel_leaf_validate
            BEFORE INSERT OR UPDATE ON e2ee_group_leaves
            FOR EACH ROW
            EXECUTE FUNCTION filament_validate_e2ee_channel_leaf();
        END IF;
    END
    $$";

const CREATE_CHANNEL_INITIAL_LEAVES_VALIDATION_FUNCTION_SQL: &str =
    "CREATE OR REPLACE FUNCTION filament_validate_e2ee_channel_initial_leaves()
     RETURNS TRIGGER AS $$
     BEGIN
         IF EXISTS (
             SELECT 1
             FROM e2ee_group_leaves l
             WHERE l.group_id = NEW.group_id
               AND (
                   NOT filament_e2ee_channel_user_can_post(
                       NEW.guild_id, NEW.channel_id, l.user_id
                   )
                   OR NOT EXISTS (
                       SELECT 1
                       FROM e2ee_device_certificates d
                       WHERE d.device_id = l.device_id
                         AND d.user_id = l.user_id
                         AND d.tombstoned_at_unix IS NULL
                   )
               )
         ) THEN
             RAISE EXCEPTION USING
                 ERRCODE = '23514',
                 CONSTRAINT = 'e2ee_channel_initial_leaves_require_authorized_devices',
                 MESSAGE = 'initial encrypted channel leaves require authorized active owned devices';
         END IF;
         RETURN NEW;
     END;
     $$ LANGUAGE plpgsql";

const CREATE_CHANNEL_INITIAL_LEAVES_VALIDATION_TRIGGER_SQL: &str = "DO $$
    BEGIN
        IF NOT EXISTS (
            SELECT 1 FROM pg_trigger
            WHERE tgname = 'e2ee_channel_initial_leaves_validate'
              AND tgrelid = 'e2ee_channel_groups'::regclass
        ) THEN
            CREATE TRIGGER e2ee_channel_initial_leaves_validate
            BEFORE INSERT ON e2ee_channel_groups
            FOR EACH ROW
            EXECUTE FUNCTION filament_validate_e2ee_channel_initial_leaves();
        END IF;
    END
    $$";

/// Require every encrypted-channel MLS Add to preserve the exact workspace
/// authorization and certified-device boundary.
///
/// The leaf trigger protects steady-state Adds. The mapping trigger also
/// validates leaves staged before the immutable channel/group binding during
/// atomic provisioning.
pub(crate) async fn apply_e2ee_channel_add_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    for statement in [
        CREATE_CHANNEL_LEAF_VALIDATION_FUNCTION_SQL,
        CREATE_CHANNEL_LEAF_VALIDATION_TRIGGER_SQL,
        CREATE_CHANNEL_INITIAL_LEAVES_VALIDATION_FUNCTION_SQL,
        CREATE_CHANNEL_INITIAL_LEAVES_VALIDATION_TRIGGER_SQL,
    ] {
        sqlx::query(statement).execute(&mut **tx).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_leaf_adds_require_authorization_and_exact_active_device_ownership() {
        assert!(CREATE_CHANNEL_LEAF_VALIDATION_FUNCTION_SQL
            .contains("filament_e2ee_channel_user_can_post"));
        assert!(CREATE_CHANNEL_LEAF_VALIDATION_FUNCTION_SQL.contains("d.user_id = NEW.user_id"));
        assert!(
            CREATE_CHANNEL_LEAF_VALIDATION_FUNCTION_SQL.contains("d.tombstoned_at_unix IS NULL")
        );
        assert!(CREATE_CHANNEL_LEAF_VALIDATION_FUNCTION_SQL
            .contains("e2ee_channel_leaf_requires_authorized_device"));
        assert!(CREATE_CHANNEL_LEAF_VALIDATION_TRIGGER_SQL
            .contains("BEFORE INSERT OR UPDATE ON e2ee_group_leaves"));
    }

    #[test]
    fn atomic_bootstrap_validates_leaves_staged_before_channel_binding() {
        assert!(CREATE_CHANNEL_INITIAL_LEAVES_VALIDATION_FUNCTION_SQL
            .contains("l.group_id = NEW.group_id"));
        assert!(
            CREATE_CHANNEL_INITIAL_LEAVES_VALIDATION_FUNCTION_SQL.contains("d.user_id = l.user_id")
        );
        assert!(CREATE_CHANNEL_INITIAL_LEAVES_VALIDATION_FUNCTION_SQL
            .contains("e2ee_channel_initial_leaves_require_authorized_devices"));
        assert!(CREATE_CHANNEL_INITIAL_LEAVES_VALIDATION_TRIGGER_SQL
            .contains("BEFORE INSERT ON e2ee_channel_groups"));
    }
}
