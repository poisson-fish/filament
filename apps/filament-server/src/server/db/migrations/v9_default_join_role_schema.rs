use sqlx::{Postgres, Transaction};

const ADD_DEFAULT_JOIN_ROLE_COLUMN_SQL: &str = "ALTER TABLE guilds
                 ADD COLUMN IF NOT EXISTS default_join_role_id TEXT";
const BACKFILL_DEFAULT_JOIN_ROLE_SQL: &str = "UPDATE guilds AS g
                 SET default_join_role_id = gr.role_id
                 FROM guild_roles AS gr
                 WHERE g.default_join_role_id IS NULL
                   AND gr.guild_id = g.guild_id
                   AND gr.system_key = $1";

pub(crate) async fn apply_default_join_role_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    sqlx::query(ADD_DEFAULT_JOIN_ROLE_COLUMN_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(BACKFILL_DEFAULT_JOIN_ROLE_SQL)
        .bind("member")
        .execute(&mut **tx)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ADD_DEFAULT_JOIN_ROLE_COLUMN_SQL, BACKFILL_DEFAULT_JOIN_ROLE_SQL};

    #[test]
    fn default_join_role_schema_statements_cover_column_and_backfill() {
        assert!(ADD_DEFAULT_JOIN_ROLE_COLUMN_SQL
            .contains("ADD COLUMN IF NOT EXISTS default_join_role_id"));
        assert!(BACKFILL_DEFAULT_JOIN_ROLE_SQL.contains("SET default_join_role_id = gr.role_id"));
        assert!(BACKFILL_DEFAULT_JOIN_ROLE_SQL.contains("gr.system_key = $1"));
    }
}
