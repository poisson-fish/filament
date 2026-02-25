use sqlx::{Postgres, Transaction};

const ADD_ROLE_COLOR_COLUMN_SQL: &str = "ALTER TABLE guild_roles
                 ADD COLUMN IF NOT EXISTS color_hex TEXT";
const DROP_ROLE_COLOR_CONSTRAINT_SQL: &str = "ALTER TABLE guild_roles
                 DROP CONSTRAINT IF EXISTS guild_roles_color_hex_format";
const ADD_ROLE_COLOR_CONSTRAINT_SQL: &str = "ALTER TABLE guild_roles
                 ADD CONSTRAINT guild_roles_color_hex_format
                 CHECK (color_hex IS NULL OR color_hex ~ '^#[0-9A-F]{6}$')";

pub(crate) async fn apply_role_color_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    sqlx::query(ADD_ROLE_COLOR_COLUMN_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(DROP_ROLE_COLOR_CONSTRAINT_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(ADD_ROLE_COLOR_CONSTRAINT_SQL)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ADD_ROLE_COLOR_COLUMN_SQL, ADD_ROLE_COLOR_CONSTRAINT_SQL, DROP_ROLE_COLOR_CONSTRAINT_SQL,
    };

    #[test]
    fn role_color_schema_statements_cover_column_and_constraint() {
        assert!(ADD_ROLE_COLOR_COLUMN_SQL.contains("ADD COLUMN IF NOT EXISTS color_hex"));
        assert!(DROP_ROLE_COLOR_CONSTRAINT_SQL.contains("DROP CONSTRAINT IF EXISTS"));
        assert!(ADD_ROLE_COLOR_CONSTRAINT_SQL.contains("guild_roles_color_hex_format"));
    }
}
