use tantivy::{TantivyDocument, Term};

use crate::server::core::{IndexedMessage, SearchIndexState, SearchOperation};

pub(crate) fn apply_search_operation(
    search: &SearchIndexState,
    writer: &mut tantivy::IndexWriter,
    op: SearchOperation,
) {
    match op {
        SearchOperation::Upsert(doc) => {
            upsert_doc(search, writer, doc);
        }
        SearchOperation::Delete { message_id } => {
            writer.delete_term(Term::from_field_text(search.fields.message_id, &message_id));
        }
        SearchOperation::Rebuild { docs } => {
            let _ = writer.delete_all_documents();
            for doc in docs {
                upsert_doc(search, writer, doc);
            }
        }
        SearchOperation::Reconcile {
            upserts,
            delete_message_ids,
        } => {
            for message_id in delete_message_ids {
                writer.delete_term(Term::from_field_text(search.fields.message_id, &message_id));
            }
            for doc in upserts {
                upsert_doc(search, writer, doc);
            }
        }
    }
}

fn upsert_doc(search: &SearchIndexState, writer: &mut tantivy::IndexWriter, doc: IndexedMessage) {
    writer.delete_term(Term::from_field_text(
        search.fields.message_id,
        &doc.message_id,
    ));
    let mut tantivy_doc = TantivyDocument::default();
    tantivy_doc.add_text(search.fields.message_id, doc.message_id);
    tantivy_doc.add_text(search.fields.guild_id, doc.guild_id);
    tantivy_doc.add_text(search.fields.channel_id, doc.channel_id);
    tantivy_doc.add_text(search.fields.author_id, doc.author_id);
    tantivy_doc.add_i64(search.fields.created_at_unix, doc.created_at_unix);
    tantivy_doc.add_text(search.fields.content, doc.content);
    let _ = writer.add_document(tantivy_doc);
}

#[cfg(test)]
mod tests {
    use tantivy::{
        collector::{Count, TopDocs},
        query::{AllQuery, TermQuery},
        schema::{IndexRecordOption, Value},
        TantivyDocument, Term,
    };

    use super::apply_search_operation;
    use crate::server::{
        core::{IndexedMessage, SearchIndexState, SearchOperation},
        realtime::build_search_schema,
    };

    fn test_search_state() -> SearchIndexState {
        let (schema, fields) = build_search_schema();
        let index = tantivy::Index::create_in_ram(schema);
        let reader = index.reader().expect("reader should initialize");
        SearchIndexState {
            index,
            reader,
            fields,
        }
    }

    fn commit_and_reload(search: &SearchIndexState, mut writer: tantivy::IndexWriter) {
        writer.commit().expect("commit should succeed");
        search
            .reader
            .reload()
            .expect("reader reload should succeed");
    }

    fn upsert(
        search: &SearchIndexState,
        message_id: &str,
        guild_id: &str,
        channel_id: &str,
        content: &str,
        created_at_unix: i64,
    ) {
        let mut writer = search
            .index
            .writer(50_000_000)
            .expect("writer should initialize");
        apply_search_operation(
            search,
            &mut writer,
            SearchOperation::Upsert(IndexedMessage {
                message_id: String::from(message_id),
                guild_id: String::from(guild_id),
                channel_id: String::from(channel_id),
                author_id: String::from("u1"),
                content: String::from(content),
                created_at_unix,
            }),
        );
        commit_and_reload(search, writer);
    }

    fn contains_message_with_content(
        search: &SearchIndexState,
        message_id: &str,
        expected_content: &str,
    ) -> bool {
        let searcher = search.reader.searcher();
        let query = TermQuery::new(
            Term::from_field_text(search.fields.message_id, message_id),
            IndexRecordOption::Basic,
        );
        let Ok(top_docs) = searcher.search(&query, &TopDocs::with_limit(1)) else {
            return false;
        };
        let Some((_score, address)) = top_docs.into_iter().next() else {
            return false;
        };
        let Ok(doc) = searcher.doc::<TantivyDocument>(address) else {
            return false;
        };
        let Some(value) = doc.get_first(search.fields.content) else {
            return false;
        };
        value.as_str() == Some(expected_content)
    }

    fn total_doc_count(search: &SearchIndexState) -> usize {
        let searcher = search.reader.searcher();
        searcher
            .search(&AllQuery, &Count)
            .expect("count should succeed")
    }

    #[test]
    fn upsert_replaces_existing_message_by_id() {
        let search = test_search_state();
        upsert(&search, "m1", "g1", "c1", "hello", 1);
        upsert(&search, "m1", "g1", "c1", "updated", 2);

        assert_eq!(total_doc_count(&search), 1);
        assert!(contains_message_with_content(&search, "m1", "updated"));
    }

    #[test]
    fn delete_removes_message_from_index() {
        let search = test_search_state();
        upsert(&search, "m1", "g1", "c1", "hello", 1);

        let mut writer = search
            .index
            .writer(50_000_000)
            .expect("writer should initialize");
        apply_search_operation(
            &search,
            &mut writer,
            SearchOperation::Delete {
                message_id: String::from("m1"),
            },
        );
        commit_and_reload(&search, writer);

        assert_eq!(total_doc_count(&search), 0);
        assert!(!contains_message_with_content(&search, "m1", "hello"));
    }

    #[test]
    fn reconcile_deletes_removed_and_upserts_new_docs() {
        let search = test_search_state();
        upsert(&search, "m1", "g1", "c1", "old", 1);

        let mut writer = search
            .index
            .writer(50_000_000)
            .expect("writer should initialize");
        apply_search_operation(
            &search,
            &mut writer,
            SearchOperation::Reconcile {
                upserts: vec![IndexedMessage {
                    message_id: String::from("m2"),
                    guild_id: String::from("g1"),
                    channel_id: String::from("c1"),
                    author_id: String::from("u2"),
                    content: String::from("new"),
                    created_at_unix: 2,
                }],
                delete_message_ids: vec![String::from("m1")],
            },
        );
        commit_and_reload(&search, writer);

        assert_eq!(total_doc_count(&search), 1);
        assert!(!contains_message_with_content(&search, "m1", "old"));
        assert!(contains_message_with_content(&search, "m2", "new"));
    }
}
