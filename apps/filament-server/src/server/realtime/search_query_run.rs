use std::time::Duration;

use tantivy::{
    collector::TopDocs,
    query::{BooleanQuery, Occur, QueryParser, TermQuery},
    schema::{IndexRecordOption, Value},
    TantivyDocument, Term,
};

use crate::server::{core::AppState, errors::AuthFailure};

use super::search_runtime;

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchQueryRunInput {
    guild_id: String,
    channel_id: Option<String>,
    query: String,
    limit: usize,
}

fn build_search_query_run_input(
    guild_id: &str,
    channel_id: Option<&str>,
    raw_query: &str,
    limit: usize,
) -> SearchQueryRunInput {
    SearchQueryRunInput {
        guild_id: guild_id.to_owned(),
        channel_id: channel_id.map(ToOwned::to_owned),
        query: search_runtime::normalize_search_query(raw_query),
        limit,
    }
}

pub(crate) async fn run_search_blocking_with_timeout<T, F>(
    timeout: Duration,
    task: F,
) -> Result<T, AuthFailure>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, AuthFailure> + Send + 'static,
{
    tokio::time::timeout(timeout, async move {
        tokio::task::spawn_blocking(task)
            .await
            .map_err(|_| AuthFailure::Internal)?
    })
    .await
    .map_err(|_| AuthFailure::InvalidRequest)?
}

pub(crate) fn run_search_query_against_index(
    search_state: &crate::server::core::SearchIndexState,
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

pub(crate) async fn run_search_query(
    state: &AppState,
    guild_id: &str,
    channel_id: Option<&str>,
    raw_query: &str,
    limit: usize,
) -> Result<Vec<String>, AuthFailure> {
    let input = build_search_query_run_input(guild_id, channel_id, raw_query, limit);
    let search_state = state.search.state.clone();
    let timeout = state.runtime.search_query_timeout;

    run_search_blocking_with_timeout(timeout, move || {
        run_search_query_against_index(
            &search_state,
            &input.guild_id,
            input.channel_id.as_deref(),
            &input.query,
            input.limit,
        )
    })
    .await
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use tantivy::TantivyDocument;

    use crate::server::errors::AuthFailure;
    use crate::server::{core::SearchIndexState, realtime::build_search_schema};

    use super::{
        build_search_query_run_input, run_search_blocking_with_timeout,
        run_search_query_against_index, SearchQueryRunInput,
    };

    #[test]
    fn build_search_query_run_input_trims_and_copies_values() {
        let input =
            build_search_query_run_input("guild-1", Some("channel-9"), "  hello world  ", 17);

        assert_eq!(
            input,
            SearchQueryRunInput {
                guild_id: String::from("guild-1"),
                channel_id: Some(String::from("channel-9")),
                query: String::from("hello world"),
                limit: 17,
            }
        );
    }

    #[test]
    fn build_search_query_run_input_handles_global_channel_scope() {
        let input = build_search_query_run_input("guild-2", None, "query", 5);

        assert_eq!(input.channel_id, None);
        assert_eq!(input.guild_id, "guild-2");
        assert_eq!(input.query, "query");
        assert_eq!(input.limit, 5);
    }

    #[tokio::test]
    async fn returns_task_result_before_timeout() {
        let result = run_search_blocking_with_timeout(Duration::from_millis(100), || Ok(42_i32))
            .await
            .expect("task should complete");

        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn fails_closed_when_timeout_expires() {
        let result = run_search_blocking_with_timeout(Duration::from_millis(20), || {
            thread::sleep(Duration::from_millis(80));
            Ok(1_i32)
        })
        .await;

        assert!(matches!(result, Err(AuthFailure::InvalidRequest)));
    }

    #[tokio::test]
    async fn maps_task_panic_to_internal_error() {
        let result: Result<i32, AuthFailure> =
            run_search_blocking_with_timeout(Duration::from_millis(100), || {
                panic!("simulated panic")
            })
            .await;

        assert!(matches!(result, Err(AuthFailure::Internal)));
    }

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
