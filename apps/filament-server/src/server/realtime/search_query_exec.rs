use tantivy::{
    collector::TopDocs,
    query::{BooleanQuery, Occur, QueryParser, TermQuery},
    schema::{IndexRecordOption, Value},
    TantivyDocument, Term,
};

use crate::server::{core::SearchIndexState, errors::AuthFailure};

pub(crate) fn run_search_query_against_index(
    search_state: &SearchIndexState,
    guild_id: &str,
    channel_id: Option<&str>,
    raw_query: &str,
    limit: usize,
) -> Result<Vec<String>, AuthFailure> {
    let searcher = search_state.reader.searcher();
    let parser = QueryParser::for_index(&search_state.index, vec![search_state.fields.content]);
    let parsed = parser
        .parse_query(raw_query)
        .map_err(|_| AuthFailure::InvalidRequest)?;
    let mut clauses = vec![
        (
            Occur::Must,
            Box::new(TermQuery::new(
                Term::from_field_text(search_state.fields.guild_id, guild_id),
                IndexRecordOption::Basic,
            )) as Box<dyn tantivy::query::Query>,
        ),
        (Occur::Must, parsed),
    ];
    if let Some(channel_id) = channel_id {
        clauses.push((
            Occur::Must,
            Box::new(TermQuery::new(
                Term::from_field_text(search_state.fields.channel_id, channel_id),
                IndexRecordOption::Basic,
            )) as Box<dyn tantivy::query::Query>,
        ));
    }

    let boolean_query = BooleanQuery::from(clauses);
    let top_docs = searcher
        .search(&boolean_query, &TopDocs::with_limit(limit))
        .map_err(|_| AuthFailure::Internal)?;

    let mut message_ids = Vec::with_capacity(top_docs.len());
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
        message_ids.push(message_id.to_owned());
    }
    Ok(message_ids)
}

#[cfg(test)]
mod tests {
    use tantivy::TantivyDocument;

    use super::run_search_query_against_index;
    use crate::server::{core::SearchIndexState, realtime::build_search_schema};

    fn search_state_with_docs() -> SearchIndexState {
        let (schema, fields) = build_search_schema();
        let index = tantivy::Index::create_in_ram(schema);
        let reader = index.reader().expect("reader should initialize");

        let mut writer = index.writer(50_000_000).expect("writer should initialize");

        let mut g1c1 = TantivyDocument::default();
        g1c1.add_text(fields.message_id, "m1");
        g1c1.add_text(fields.guild_id, "g1");
        g1c1.add_text(fields.channel_id, "c1");
        g1c1.add_text(fields.author_id, "u1");
        g1c1.add_i64(fields.created_at_unix, 1);
        g1c1.add_text(fields.content, "rust gateway");
        let _ = writer.add_document(g1c1);

        let mut g1c2 = TantivyDocument::default();
        g1c2.add_text(fields.message_id, "m2");
        g1c2.add_text(fields.guild_id, "g1");
        g1c2.add_text(fields.channel_id, "c2");
        g1c2.add_text(fields.author_id, "u2");
        g1c2.add_i64(fields.created_at_unix, 2);
        g1c2.add_text(fields.content, "rust search");
        let _ = writer.add_document(g1c2);

        let mut g2c1 = TantivyDocument::default();
        g2c1.add_text(fields.message_id, "m3");
        g2c1.add_text(fields.guild_id, "g2");
        g2c1.add_text(fields.channel_id, "c1");
        g2c1.add_text(fields.author_id, "u3");
        g2c1.add_i64(fields.created_at_unix, 3);
        g2c1.add_text(fields.content, "rust elsewhere");
        let _ = writer.add_document(g2c1);

        writer.commit().expect("commit should succeed");
        reader.reload().expect("reader reload should succeed");

        SearchIndexState {
            index,
            reader,
            fields,
        }
    }

    #[test]
    fn run_search_query_filters_to_guild() {
        let search = search_state_with_docs();

        let ids = run_search_query_against_index(&search, "g1", None, "rust", 10)
            .expect("query should succeed");

        assert_eq!(ids.len(), 2);
        assert!(ids.iter().any(|id| id == "m1"));
        assert!(ids.iter().any(|id| id == "m2"));
        assert!(!ids.iter().any(|id| id == "m3"));
    }

    #[test]
    fn run_search_query_filters_to_channel_when_provided() {
        let search = search_state_with_docs();

        let ids = run_search_query_against_index(&search, "g1", Some("c2"), "rust", 10)
            .expect("query should succeed");

        assert_eq!(ids, vec![String::from("m2")]);
    }
}
