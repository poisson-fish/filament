use std::collections::HashMap;

use filament_core::tokenize_markdown;

use crate::server::{errors::AuthFailure, types::MessageResponse};

type HydratedMessageRow = (String, String, String, String, String, i64);

fn map_hydrated_rows(rows: Vec<HydratedMessageRow>) -> HashMap<String, MessageResponse> {
    let mut by_id = HashMap::with_capacity(rows.len());
    for (message_id, guild_id, channel_id, author_id, content, created_at_unix) in rows {
        by_id.insert(
            message_id.clone(),
            MessageResponse {
                message_id,
                guild_id,
                channel_id,
                author_id,
                markdown_tokens: tokenize_markdown(&content),
                content,
                attachments: Vec::new(),
                reactions: Vec::new(),
                created_at_unix,
            },
        );
    }
    by_id
}

pub(crate) async fn collect_hydrated_messages_db(
    pool: &sqlx::PgPool,
    guild_id: &str,
    channel_id: Option<&str>,
    message_ids: &[String],
) -> Result<HashMap<String, MessageResponse>, AuthFailure> {
    let rows = if let Some(channel_id) = channel_id {
        sqlx::query_as::<_, HydratedMessageRow>(
            "SELECT message_id, guild_id, channel_id, author_id, content, created_at_unix
             FROM messages
             WHERE guild_id = $1 AND channel_id = $2 AND message_id = ANY($3::text[])",
        )
        .bind(guild_id)
        .bind(channel_id)
        .bind(message_ids)
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?
    } else {
        sqlx::query_as::<_, HydratedMessageRow>(
            "SELECT message_id, guild_id, channel_id, author_id, content, created_at_unix
             FROM messages
             WHERE guild_id = $1 AND message_id = ANY($2::text[])",
        )
        .bind(guild_id)
        .bind(message_ids)
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?
    };

    Ok(map_hydrated_rows(rows))
}

#[cfg(test)]
mod tests {
    use super::map_hydrated_rows;

    #[test]
    fn map_hydrated_rows_maps_fields_and_tokenizes_content() {
        let by_id = map_hydrated_rows(vec![(
            String::from("m1"),
            String::from("g1"),
            String::from("c1"),
            String::from("u1"),
            String::from("hello **bold**"),
            12,
        )]);

        let message = by_id.get("m1").expect("mapped message should be present");
        assert_eq!(message.guild_id, "g1");
        assert_eq!(message.channel_id, "c1");
        assert_eq!(message.author_id, "u1");
        assert_eq!(message.content, "hello **bold**");
        assert!(!message.markdown_tokens.is_empty());
        assert!(message.attachments.is_empty());
        assert!(message.reactions.is_empty());
        assert_eq!(message.created_at_unix, 12);
    }

    #[test]
    fn map_hydrated_rows_overwrites_duplicate_message_ids_with_last_row() {
        let by_id = map_hydrated_rows(vec![
            (
                String::from("m1"),
                String::from("g1"),
                String::from("c1"),
                String::from("u1"),
                String::from("old"),
                10,
            ),
            (
                String::from("m1"),
                String::from("g1"),
                String::from("c1"),
                String::from("u1"),
                String::from("new"),
                11,
            ),
        ]);

        let message = by_id.get("m1").expect("mapped message should be present");
        assert_eq!(message.content, "new");
        assert_eq!(message.created_at_unix, 11);
    }
}
