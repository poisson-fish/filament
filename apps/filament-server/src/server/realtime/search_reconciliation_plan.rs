use std::collections::HashSet;

use crate::server::{
    core::{AppState, IndexedMessage},
    errors::AuthFailure,
};

use super::{
    collect_index_message_ids_for_guild_from_index, collect_indexed_messages_for_guild,
    compute_reconciliation, search_query_run::run_search_blocking_with_timeout,
};

pub(crate) fn build_search_reconciliation_plan(
    source_docs: Vec<IndexedMessage>,
    index_ids: std::collections::HashSet<String>,
) -> (Vec<IndexedMessage>, Vec<String>) {
    compute_reconciliation(source_docs, index_ids)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchIndexLookupInput {
    guild_id: String,
    max_docs: usize,
}

fn build_search_index_lookup_input(guild_id: &str, max_docs: usize) -> SearchIndexLookupInput {
    SearchIndexLookupInput {
        guild_id: guild_id.to_owned(),
        max_docs,
    }
}

async fn collect_index_message_ids_for_guild(
    state: &AppState,
    guild_id: &str,
    max_docs: usize,
) -> Result<HashSet<String>, AuthFailure> {
    let input = build_search_index_lookup_input(guild_id, max_docs);
    let search_state = state.search.state.clone();
    let timeout = state.runtime.search_query_timeout;

    run_search_blocking_with_timeout(timeout, move || {
        collect_index_message_ids_for_guild_from_index(
            &search_state,
            &input.guild_id,
            input.max_docs,
        )
    })
    .await
}

pub(crate) async fn plan_search_reconciliation(
    state: &AppState,
    guild_id: &str,
    max_docs: usize,
) -> Result<(Vec<IndexedMessage>, Vec<String>), AuthFailure> {
    let source_docs = collect_indexed_messages_for_guild(state, guild_id, max_docs).await?;
    let index_ids = collect_index_message_ids_for_guild(state, guild_id, max_docs).await?;
    Ok(build_search_reconciliation_plan(source_docs, index_ids))
}

#[cfg(test)]
mod tests {
    use super::{
        build_search_index_lookup_input, build_search_reconciliation_plan, SearchIndexLookupInput,
    };
    use crate::server::core::IndexedMessage;
    use std::collections::HashSet;

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

    #[test]
    fn build_search_index_lookup_input_copies_values() {
        let input = build_search_index_lookup_input("guild-1", 55);

        assert_eq!(
            input,
            SearchIndexLookupInput {
                guild_id: String::from("guild-1"),
                max_docs: 55,
            }
        );
    }

    #[test]
    fn build_search_index_lookup_input_preserves_empty_guild_id() {
        let input = build_search_index_lookup_input("", 1);

        assert_eq!(input.guild_id, "");
        assert_eq!(input.max_docs, 1);
    }
}
