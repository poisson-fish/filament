use std::collections::HashSet;

use tantivy::{
    collector::{Count, TopDocs},
    query::TermQuery,
    schema::{IndexRecordOption, Value},
    TantivyDocument, Term,
};

use crate::server::{core::SearchIndexState, errors::AuthFailure};

pub(crate) fn collect_index_message_ids_for_guild(
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

#[cfg(test)]
mod tests {
    use tantivy::TantivyDocument;

    use super::collect_index_message_ids_for_guild;
    use crate::server::{
        core::SearchIndexState,
        errors::AuthFailure,
        realtime::build_search_schema,
    };

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

        let ids = collect_index_message_ids_for_guild(&search, "g1", 10)
            .expect("guild ids should be collected");

        assert_eq!(ids.len(), 2);
        assert!(ids.contains("m1"));
        assert!(ids.contains("m2"));
        assert!(!ids.contains("m3"));
    }

    #[test]
    fn collect_index_ids_rejects_when_count_exceeds_cap() {
        let search = search_state_with_docs();

        let result = collect_index_message_ids_for_guild(&search, "g1", 1);

        assert!(matches!(result, Err(AuthFailure::InvalidRequest)));
    }
}