use crate::server::core::IndexedMessage;

type IndexedMessageRow = (String, String, String, String, String, i64);

fn indexed_messages_from_rows(rows: Vec<IndexedMessageRow>) -> Vec<IndexedMessage> {
    rows.into_iter()
        .map(
            |(message_id, guild_id, channel_id, author_id, content, created_at_unix)| {
                IndexedMessage {
                    message_id,
                    guild_id,
                    channel_id,
                    author_id,
                    content,
                    created_at_unix,
                }
            },
        )
        .collect()
}

pub(crate) fn collect_all_indexed_messages_rows(
    rows: Vec<IndexedMessageRow>,
) -> Vec<IndexedMessage> {
    indexed_messages_from_rows(rows)
}

pub(crate) fn collect_indexed_messages_for_guild_rows(
    rows: Vec<IndexedMessageRow>,
) -> Vec<IndexedMessage> {
    indexed_messages_from_rows(rows)
}

#[cfg(test)]
mod tests {
    use super::{collect_all_indexed_messages_rows, collect_indexed_messages_for_guild_rows};

    #[test]
    fn collect_all_indexed_messages_rows_maps_all_fields() {
        let docs = collect_all_indexed_messages_rows(vec![
            (
                String::from("m1"),
                String::from("g1"),
                String::from("c1"),
                String::from("u1"),
                String::from("hello"),
                7,
            ),
            (
                String::from("m2"),
                String::from("g2"),
                String::from("c2"),
                String::from("u2"),
                String::from("world"),
                8,
            ),
        ]);

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].message_id, "m1");
        assert_eq!(docs[0].guild_id, "g1");
        assert_eq!(docs[0].channel_id, "c1");
        assert_eq!(docs[0].author_id, "u1");
        assert_eq!(docs[0].content, "hello");
        assert_eq!(docs[0].created_at_unix, 7);
    }

    #[test]
    fn collect_indexed_messages_for_guild_rows_preserves_row_order() {
        let docs = collect_indexed_messages_for_guild_rows(vec![
            (
                String::from("newest"),
                String::from("g1"),
                String::from("c1"),
                String::from("u1"),
                String::from("new"),
                11,
            ),
            (
                String::from("older"),
                String::from("g1"),
                String::from("c1"),
                String::from("u1"),
                String::from("old"),
                10,
            ),
        ]);

        let ids: Vec<&str> = docs.iter().map(|doc| doc.message_id.as_str()).collect();
        assert_eq!(ids, vec!["newest", "older"]);
    }
}
