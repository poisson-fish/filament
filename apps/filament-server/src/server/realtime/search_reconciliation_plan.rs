use crate::server::{
    core::{AppState, IndexedMessage},
    errors::AuthFailure,
};

use super::{collect_indexed_messages_for_guild, compute_reconciliation};

pub(crate) fn build_search_reconciliation_plan(
    source_docs: Vec<IndexedMessage>,
    index_ids: std::collections::HashSet<String>,
) -> (Vec<IndexedMessage>, Vec<String>) {
    compute_reconciliation(source_docs, index_ids)
}

pub(crate) async fn plan_search_reconciliation(
    state: &AppState,
    guild_id: &str,
    max_docs: usize,
) -> Result<(Vec<IndexedMessage>, Vec<String>), AuthFailure> {
    let source_docs = collect_indexed_messages_for_guild(state, guild_id, max_docs).await?;
    let index_ids = super::collect_index_message_ids_for_guild(state, guild_id, max_docs).await?;
    Ok(build_search_reconciliation_plan(source_docs, index_ids))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::build_search_reconciliation_plan;
    use crate::server::core::IndexedMessage;

    fn doc(id: &str) -> IndexedMessage {
        IndexedMessage {
            message_id: id.to_owned(),
            guild_id: String::from("g1"),
            channel_id: String::from("c1"),
            author_id: String::from("u1"),
            created_at_unix: 1,
            content: format!("content-{id}"),
        }
    }

    #[test]
    fn build_search_reconciliation_plan_returns_sorted_upserts_and_deletes() {
        let source_docs = vec![doc("m3"), doc("m1"), doc("m2")];
        let index_ids = HashSet::from([String::from("m2"), String::from("m4")]);

        let (upserts, deletes) = build_search_reconciliation_plan(source_docs, index_ids);

        let upsert_ids: Vec<String> = upserts.into_iter().map(|entry| entry.message_id).collect();
        assert_eq!(upsert_ids, vec![String::from("m1"), String::from("m3")]);
        assert_eq!(deletes, vec![String::from("m4")]);
    }

    #[test]
    fn build_search_reconciliation_plan_is_empty_when_sets_match() {
        let source_docs = vec![doc("m1"), doc("m2")];
        let index_ids = HashSet::from([String::from("m1"), String::from("m2")]);

        let (upserts, deletes) = build_search_reconciliation_plan(source_docs, index_ids);

        assert!(upserts.is_empty());
        assert!(deletes.is_empty());
    }
}
