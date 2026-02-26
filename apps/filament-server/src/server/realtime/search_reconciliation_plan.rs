use std::collections::HashSet;

use tantivy::{
    collector::{Count, TopDocs},
    query::TermQuery,
    schema::{IndexRecordOption, Value},
    TantivyDocument, Term,
};

use crate::server::{
    core::{AppState, IndexedMessage, SearchIndexState},
    errors::AuthFailure,
};

use super::{
    collect_indexed_messages_for_guild, search_query_run::run_search_blocking_with_timeout,
};

pub(crate) fn build_search_reconciliation_plan(
    source_docs: Vec<IndexedMessage>,
    index_ids: std::collections::HashSet<String>,
) -> (Vec<IndexedMessage>, Vec<String>) {
    compute_reconciliation(source_docs, index_ids)
}

pub(crate) fn compute_reconciliation(
    source_docs: Vec<IndexedMessage>,
    index_ids: HashSet<String>,
) -> (Vec<IndexedMessage>, Vec<String>) {
    let source_ids: HashSet<String> = source_docs
        .iter()
        .map(|doc| doc.message_id.clone())
        .collect();
    let mut upserts: Vec<IndexedMessage> = source_docs
        .into_iter()
        .filter(|doc| !index_ids.contains(&doc.message_id))
        .collect();
    let mut delete_message_ids: Vec<String> = index_ids
        .into_iter()
        .filter(|message_id| !source_ids.contains(message_id))
        .collect();
    upserts.sort_by(|a, b| a.message_id.cmp(&b.message_id));
    delete_message_ids.sort_unstable();
    (upserts, delete_message_ids)
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

pub(crate) fn collect_index_message_ids_for_guild_from_index(
    search_state: &SearchIndexState,
    guild_id: &str,
    max_docs: usize,
) -> Result<HashSet<String>, AuthFailure> {
    let searcher = search_state.reader.searcher();
    let guild_query = TermQuery::new(
        Term::from_field_text(search_state.fields.guild_id, guild_id),
        IndexRecordOption::Basic,
    );
    let count = searcher
        .search(&guild_query, &Count)
        .map_err(|_| AuthFailure::Internal)?;
    if count > max_docs {
        return Err(AuthFailure::InvalidRequest);
    }
    if count == 0 {
        return Ok(HashSet::new());
    }

    let top_docs = searcher
        .search(&guild_query, &TopDocs::with_limit(count))
        .map_err(|_| AuthFailure::Internal)?;
    let mut message_ids = HashSet::with_capacity(top_docs.len());
    for (_score, address) in top_docs {
        let Ok(doc) = searcher.doc::<TantivyDocument>(address) else {
            continue;
        };
        let Some(value) = doc.get_first(search_state.fields.message_id) else {
            continue;
        };
        let Some(message_id) = value.as_str() else {
            continue;
        };
        message_ids.insert(message_id.to_owned());
    }
    Ok(message_ids)
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
    use tantivy::TantivyDocument;

    use super::{
        build_search_index_lookup_input, build_search_reconciliation_plan,
        collect_index_message_ids_for_guild_from_index, compute_reconciliation,
        SearchIndexLookupInput,
    };
    use crate::server::{
        core::{IndexedMessage, SearchIndexState},
        realtime::build_search_schema,
    };
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
    fn compute_reconciliation_returns_sorted_upserts_and_deletes() {
        let source_docs = vec![doc("m3"), doc("m1"), doc("m2")];
        let index_ids = HashSet::from([String::from("m2"), String::from("m4")]);

        let (upserts, deletes) = compute_reconciliation(source_docs, index_ids);

        let upsert_ids: Vec<String> = upserts.into_iter().map(|entry| entry.message_id).collect();
        assert_eq!(upsert_ids, vec![String::from("m1"), String::from("m3")]);
        assert_eq!(deletes, vec![String::from("m4")]);
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

    fn search_state_with_docs() -> SearchIndexState {
        let (schema, fields) = build_search_schema();
        let index = tantivy::Index::create_in_ram(schema);
        let reader = index.reader().expect("reader should initialize");

        let mut writer = index.writer(50_000_000).expect("writer should initialize");
        let mut guild1_doc1 = TantivyDocument::default();
        guild1_doc1.add_text(fields.message_id, "m1");
        guild1_doc1.add_text(fields.guild_id, "g1");
        guild1_doc1.add_text(fields.channel_id, "c1");
        guild1_doc1.add_text(fields.author_id, "u1");
        guild1_doc1.add_i64(fields.created_at_unix, 1);
        guild1_doc1.add_text(fields.content, "hello");
        let _ = writer.add_document(guild1_doc1);

        let mut guild1_doc2 = TantivyDocument::default();
        guild1_doc2.add_text(fields.message_id, "m2");
        guild1_doc2.add_text(fields.guild_id, "g1");
        guild1_doc2.add_text(fields.channel_id, "c1");
        guild1_doc2.add_text(fields.author_id, "u2");
        guild1_doc2.add_i64(fields.created_at_unix, 2);
        guild1_doc2.add_text(fields.content, "world");
        let _ = writer.add_document(guild1_doc2);

        let mut guild2_doc = TantivyDocument::default();
        guild2_doc.add_text(fields.message_id, "m3");
        guild2_doc.add_text(fields.guild_id, "g2");
        guild2_doc.add_text(fields.channel_id, "c2");
        guild2_doc.add_text(fields.author_id, "u3");
        guild2_doc.add_i64(fields.created_at_unix, 3);
        guild2_doc.add_text(fields.content, "other");
        let _ = writer.add_document(guild2_doc);

        let mut missing_id_doc = TantivyDocument::default();
        missing_id_doc.add_text(fields.guild_id, "g1");
        missing_id_doc.add_text(fields.channel_id, "c1");
        missing_id_doc.add_text(fields.author_id, "u4");
        missing_id_doc.add_i64(fields.created_at_unix, 4);
        missing_id_doc.add_text(fields.content, "missing-id");
        let _ = writer.add_document(missing_id_doc);

        writer.commit().expect("commit should succeed");
        reader.reload().expect("reader reload should succeed");

        SearchIndexState {
            index,
            reader,
            fields,
        }
    }

    #[test]
    fn collect_index_ids_returns_only_matching_guild_message_ids() {
        let search = search_state_with_docs();

        let ids = collect_index_message_ids_for_guild_from_index(&search, "g1", 10)
            .expect("guild ids should be collected");

        assert_eq!(ids.len(), 2);
        assert!(ids.contains("m1"));
        assert!(ids.contains("m2"));
        assert!(!ids.contains("m3"));
    }

    #[test]
    fn collect_index_ids_rejects_when_count_exceeds_cap() {
        let search = search_state_with_docs();

        let result = collect_index_message_ids_for_guild_from_index(&search, "g1", 1);

        assert!(matches!(
            result,
            Err(crate::server::errors::AuthFailure::InvalidRequest)
        ));
    }
}
