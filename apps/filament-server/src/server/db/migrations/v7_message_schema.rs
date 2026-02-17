use sqlx::{Postgres, Transaction};

const CREATE_MESSAGES_TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS messages (
                    message_id TEXT PRIMARY KEY,
                    guild_id TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
                    channel_id TEXT NOT NULL REFERENCES channels(channel_id) ON DELETE CASCADE,
                    author_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    content TEXT NOT NULL,
                    created_at_unix BIGINT NOT NULL
                )";
const CREATE_MESSAGE_REACTIONS_TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS message_reactions (
                    guild_id TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
                    channel_id TEXT NOT NULL REFERENCES channels(channel_id) ON DELETE CASCADE,
                    message_id TEXT NOT NULL REFERENCES messages(message_id) ON DELETE CASCADE,
                    emoji TEXT NOT NULL,
                    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    created_at_unix BIGINT NOT NULL,
                    PRIMARY KEY(guild_id, channel_id, message_id, emoji, user_id)
                )";
const CREATE_MESSAGES_CHANNEL_MESSAGE_ID_INDEX_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_messages_channel_message_id
                    ON messages(channel_id, message_id DESC)";

pub(crate) async fn apply_message_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    sqlx::query(CREATE_MESSAGES_TABLE_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(CREATE_MESSAGE_REACTIONS_TABLE_SQL)
        .execute(&mut **tx)
        .await?;
    sqlx::query(CREATE_MESSAGES_CHANNEL_MESSAGE_ID_INDEX_SQL)
        .execute(&mut **tx)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        CREATE_MESSAGES_CHANNEL_MESSAGE_ID_INDEX_SQL, CREATE_MESSAGES_TABLE_SQL,
        CREATE_MESSAGE_REACTIONS_TABLE_SQL,
    };

    #[test]
    fn message_schema_statements_define_required_tables_and_indexes() {
        assert!(CREATE_MESSAGES_TABLE_SQL.contains("CREATE TABLE IF NOT EXISTS messages"));
        assert!(CREATE_MESSAGE_REACTIONS_TABLE_SQL
            .contains("CREATE TABLE IF NOT EXISTS message_reactions"));
        assert!(CREATE_MESSAGES_CHANNEL_MESSAGE_ID_INDEX_SQL
            .contains("idx_messages_channel_message_id"));
    }
}
