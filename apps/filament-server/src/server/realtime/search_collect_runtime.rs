use crate::server::{
    core::{AppState, IndexedMessage},
    errors::AuthFailure,
};

use super::search_collect_all::collect_all_indexed_messages_in_memory;
use super::search_collect_db::{
    collect_all_indexed_messages_rows, collect_indexed_messages_for_guild_rows,
    enforce_guild_collect_doc_cap, guild_collect_fetch_limit,
};
use super::search_collect_guild::collect_indexed_messages_for_guild_in_memory;

type IndexedMessageRow = (String, String, String, String, String, i64);

fn map_collect_all_rows(rows: Vec<IndexedMessageRow>) -> Vec<IndexedMessage> {
    collect_all_indexed_messages_rows(rows)
}

fn map_collect_guild_rows(
    rows: Vec<IndexedMessageRow>,
    max_docs: usize,
) -> Result<Vec<IndexedMessage>, AuthFailure> {
    enforce_guild_collect_doc_cap(rows.len(), max_docs)?;
    Ok(collect_indexed_messages_for_guild_rows(rows))
}

pub(crate) async fn collect_all_indexed_messages_runtime(
    state: &AppState,
) -> Result<Vec<IndexedMessage>, AuthFailure> {
    if let Some(pool) = &state.db_pool {
        let rows = sqlx::query_as::<_, IndexedMessageRow>(
            "SELECT message_id, guild_id, channel_id, author_id, content, created_at_unix
             FROM messages",
        )
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        return Ok(map_collect_all_rows(rows));
    }

    let guilds = state.guilds.read().await;
    Ok(collect_all_indexed_messages_in_memory(&guilds))
}

pub(crate) async fn collect_indexed_messages_for_guild_runtime(
    state: &AppState,
    guild_id: &str,
    max_docs: usize,
) -> Result<Vec<IndexedMessage>, AuthFailure> {
    if let Some(pool) = &state.db_pool {
        let limit = guild_collect_fetch_limit(max_docs)?;
        let rows = sqlx::query_as::<_, IndexedMessageRow>(
            "SELECT message_id, guild_id, channel_id, author_id, content, created_at_unix
             FROM messages
             WHERE guild_id = $1
             ORDER BY created_at_unix DESC
             LIMIT $2",
        )
        .bind(guild_id)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        return map_collect_guild_rows(rows, max_docs);
    }

    let guilds = state.guilds.read().await;
    collect_indexed_messages_for_guild_in_memory(&guilds, guild_id, max_docs)
}

#[cfg(test)]
mod tests {
    use super::{map_collect_all_rows, map_collect_guild_rows};
    use crate::server::errors::AuthFailure;

    #[test]
    fn map_collect_all_rows_maps_messages_in_order() {
        let docs = map_collect_all_rows(vec![
            (
                String::from("m1"),
                String::from("g1"),
                String::from("c1"),
                String::from("u1"),
                String::from("first"),
                10,
            ),
            (
                String::from("m2"),
                String::from("g1"),
                String::from("c2"),
                String::from("u2"),
                String::from("second"),
                11,
            ),
        ]);

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].message_id, "m1");
        assert_eq!(docs[1].message_id, "m2");
    }

    #[test]
    fn map_collect_guild_rows_fails_closed_when_rows_exceed_cap() {
        let result = map_collect_guild_rows(
            vec![
                (
                    String::from("m1"),
                    String::from("g1"),
                    String::from("c1"),
                    String::from("u1"),
                    String::from("first"),
                    10,
                ),
                (
                    String::from("m2"),
                    String::from("g1"),
                    String::from("c1"),
                    String::from("u1"),
                    String::from("second"),
                    11,
                ),
            ],
            1,
        );

        assert!(matches!(result, Err(AuthFailure::InvalidRequest)));
    }
}