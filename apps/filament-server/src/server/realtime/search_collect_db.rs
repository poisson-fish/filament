use crate::server::core::IndexedMessage;
use crate::server::errors::AuthFailure;

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

pub(crate) fn guild_collect_fetch_limit(max_docs: usize) -> Result<i64, AuthFailure> {
    i64::try_from(max_docs.saturating_add(1)).map_err(|_| AuthFailure::InvalidRequest)
}

pub(crate) fn enforce_guild_collect_doc_cap(
    row_count: usize,
    max_docs: usize,
) -> Result<(), AuthFailure> {
    if row_count > max_docs {
        return Err(AuthFailure::InvalidRequest);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        collect_all_indexed_messages_rows, collect_indexed_messages_for_guild_rows,
        enforce_guild_collect_doc_cap, guild_collect_fetch_limit,
    };
    use crate::server::errors::AuthFailure;

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

    #[test]
    fn guild_collect_fetch_limit_adds_one_for_fail_closed_cap_check() {
        let limit = guild_collect_fetch_limit(50).expect("valid max docs should convert");
        assert_eq!(limit, 51);
    }

    #[test]
    fn enforce_guild_collect_doc_cap_rejects_over_cap_results() {
        assert!(enforce_guild_collect_doc_cap(2, 2).is_ok());
        let error =
            enforce_guild_collect_doc_cap(3, 2).expect_err("over-cap row count should fail closed");
        assert!(matches!(error, AuthFailure::InvalidRequest));
    }
}
