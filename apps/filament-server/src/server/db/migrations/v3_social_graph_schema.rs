use sqlx::{Postgres, Transaction};

const CREATE_FRIENDSHIPS_TABLE_SQL: &str =
    "CREATE TABLE IF NOT EXISTS friendships (
                    user_a_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    user_b_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    created_at_unix BIGINT NOT NULL,
                    CHECK (user_a_id < user_b_id),
                    PRIMARY KEY(user_a_id, user_b_id)
                )";
const CREATE_FRIENDSHIP_REQUESTS_TABLE_SQL: &str =
    "CREATE TABLE IF NOT EXISTS friendship_requests (
                    request_id TEXT PRIMARY KEY,
                    sender_user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    recipient_user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    created_at_unix BIGINT NOT NULL,
                    CHECK (sender_user_id <> recipient_user_id)
                )";
const CREATE_FRIENDSHIP_REQUESTS_SENDER_INDEX_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_friendship_requests_sender
                    ON friendship_requests(sender_user_id)";
const CREATE_FRIENDSHIP_REQUESTS_RECIPIENT_INDEX_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_friendship_requests_recipient
                    ON friendship_requests(recipient_user_id)";
const CREATE_GUILD_BANS_TABLE_SQL: &str =
    "CREATE TABLE IF NOT EXISTS guild_bans (
                    guild_id TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
                    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    banned_by_user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    created_at_unix BIGINT NOT NULL,
                    PRIMARY KEY(guild_id, user_id)
                )";

pub(crate) async fn apply_social_graph_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    sqlx::query(CREATE_FRIENDSHIPS_TABLE_SQL)
        .execute(&mut **tx)
        .await?;

    sqlx::query(CREATE_FRIENDSHIP_REQUESTS_TABLE_SQL)
        .execute(&mut **tx)
        .await?;

    sqlx::query(CREATE_FRIENDSHIP_REQUESTS_SENDER_INDEX_SQL)
        .execute(&mut **tx)
        .await?;

    sqlx::query(CREATE_FRIENDSHIP_REQUESTS_RECIPIENT_INDEX_SQL)
        .execute(&mut **tx)
        .await?;

    sqlx::query(CREATE_GUILD_BANS_TABLE_SQL)
        .execute(&mut **tx)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        CREATE_FRIENDSHIPS_TABLE_SQL, CREATE_FRIENDSHIP_REQUESTS_RECIPIENT_INDEX_SQL,
        CREATE_FRIENDSHIP_REQUESTS_SENDER_INDEX_SQL, CREATE_FRIENDSHIP_REQUESTS_TABLE_SQL,
        CREATE_GUILD_BANS_TABLE_SQL,
    };

    #[test]
    fn social_graph_schema_statements_define_required_tables_and_indexes() {
        assert!(CREATE_FRIENDSHIPS_TABLE_SQL.contains("CREATE TABLE IF NOT EXISTS friendships"));
        assert!(CREATE_FRIENDSHIPS_TABLE_SQL.contains("CHECK (user_a_id < user_b_id)"));
        assert!(
            CREATE_FRIENDSHIP_REQUESTS_TABLE_SQL
                .contains("CREATE TABLE IF NOT EXISTS friendship_requests")
        );
        assert!(
            CREATE_FRIENDSHIP_REQUESTS_TABLE_SQL
                .contains("CHECK (sender_user_id <> recipient_user_id)")
        );
        assert!(
            CREATE_FRIENDSHIP_REQUESTS_SENDER_INDEX_SQL.contains("idx_friendship_requests_sender")
        );
        assert!(
            CREATE_FRIENDSHIP_REQUESTS_RECIPIENT_INDEX_SQL
                .contains("idx_friendship_requests_recipient")
        );
        assert!(CREATE_GUILD_BANS_TABLE_SQL.contains("CREATE TABLE IF NOT EXISTS guild_bans"));
    }
}
