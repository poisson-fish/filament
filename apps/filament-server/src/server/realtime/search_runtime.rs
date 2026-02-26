use std::sync::Arc;

use anyhow::anyhow;
use tantivy::schema::Schema;
use tokio::sync::mpsc;

use crate::server::{
    core::{
        AppState, IndexedMessage, SearchCommand, SearchFields, SearchIndexState, SearchOperation,
        SearchService, DEFAULT_SEARCH_RESULT_LIMIT, SEARCH_INDEX_QUEUE_CAPACITY,
    },
    errors::AuthFailure,
    types::{MessageResponse, SearchQuery},
};

use super::{
    hydration_runtime::hydrate_messages_by_id_runtime,
    search_apply::apply_search_operation as apply_search_operation_impl,
    search_apply_batch::apply_search_batch_with_ack,
    search_batch_drain::drain_search_batch,
    search_collect_runtime::{
        collect_all_indexed_messages_runtime, collect_indexed_messages_for_guild_runtime,
    },
    search_enqueue::enqueue_search_command,
    search_query_input::validate_search_query_request,
    search_schema::build_search_schema as build_search_schema_impl,
};

const SEARCH_WORKER_BATCH_LIMIT: usize = 128;

pub(crate) fn build_search_schema() -> (Schema, SearchFields) {
    build_search_schema_impl()
}

pub(crate) fn init_search_service() -> anyhow::Result<SearchService> {
    let (schema, fields) = build_search_schema();
    let index = tantivy::Index::create_in_ram(schema);
    let reader = index
        .reader()
        .map_err(|e| anyhow!("search reader init failed: {e}"))?;
    let state = Arc::new(SearchIndexState {
        index,
        reader,
        fields,
    });
    let (tx, mut rx) = mpsc::channel::<SearchCommand>(SEARCH_INDEX_QUEUE_CAPACITY);
    let worker_state = state.clone();
    std::thread::Builder::new()
        .name(String::from("filament-search-index"))
        .spawn(move || {
            while let Some(command) = rx.blocking_recv() {
                let batch = drain_search_batch(command, &mut rx, SEARCH_WORKER_BATCH_LIMIT);
                let batch_result = apply_search_batch(&worker_state, batch);
                if let Err(error) = batch_result {
                    tracing::error!(event = "search.index.batch", error = %error);
                }
            }
        })
        .map_err(|e| anyhow!("search worker spawn failed: {e}"))?;
    Ok(SearchService { tx, state })
}

pub(crate) fn apply_search_batch(
    search: &Arc<SearchIndexState>,
    mut batch: Vec<SearchCommand>,
) -> anyhow::Result<()> {
    apply_search_batch_with_ack(search, &mut batch, apply_search_operation)
}

pub(crate) fn apply_search_operation(
    search: &SearchIndexState,
    writer: &mut tantivy::IndexWriter,
    op: SearchOperation,
) {
    apply_search_operation_impl(search, writer, op);
}

pub(crate) fn indexed_message_from_response(message: &MessageResponse) -> IndexedMessage {
    IndexedMessage {
        message_id: message.message_id.clone(),
        guild_id: message.guild_id.clone(),
        channel_id: message.channel_id.clone(),
        author_id: message.author_id.clone(),
        created_at_unix: message.created_at_unix,
        content: message.content.clone(),
    }
}

pub(crate) fn validate_search_query(
    state: &AppState,
    query: &SearchQuery,
) -> Result<(), AuthFailure> {
    validate_search_query_with_limits(
        query,
        DEFAULT_SEARCH_RESULT_LIMIT,
        state.runtime.search_query_max_chars,
        state.runtime.search_result_limit_max,
    )
}

fn build_search_rebuild_operation(docs: Vec<IndexedMessage>) -> SearchOperation {
    SearchOperation::Rebuild { docs }
}

fn validate_search_query_with_limits(
    query: &SearchQuery,
    default_limit: usize,
    max_chars: usize,
    max_limit: usize,
) -> Result<(), AuthFailure> {
    validate_search_query_request(query, default_limit, max_chars, max_limit)
}

pub(crate) async fn ensure_search_bootstrapped(state: &AppState) -> Result<(), AuthFailure> {
    state
        .search_bootstrapped
        .get_or_try_init(|| async move {
            let docs = collect_all_indexed_messages(state).await?;
            let rebuild = build_search_rebuild_operation(docs);
            enqueue_search_operation(state, rebuild, true).await?;
            Ok(())
        })
        .await?;
    Ok(())
}

pub(crate) async fn enqueue_search_operation(
    state: &AppState,
    op: SearchOperation,
    wait_for_apply: bool,
) -> Result<(), AuthFailure> {
    enqueue_search_command(&state.search.tx, op, wait_for_apply).await
}

pub(crate) async fn collect_all_indexed_messages(
    state: &AppState,
) -> Result<Vec<IndexedMessage>, AuthFailure> {
    collect_all_indexed_messages_runtime(state).await
}

pub(crate) async fn collect_indexed_messages_for_guild(
    state: &AppState,
    guild_id: &str,
    max_docs: usize,
) -> Result<Vec<IndexedMessage>, AuthFailure> {
    collect_indexed_messages_for_guild_runtime(state, guild_id, max_docs).await
}

pub(crate) async fn hydrate_messages_by_id(
    state: &AppState,
    guild_id: &str,
    channel_id: Option<&str>,
    message_ids: &[String],
) -> Result<Vec<MessageResponse>, AuthFailure> {
    hydrate_messages_by_id_runtime(state, guild_id, channel_id, message_ids).await
}

#[cfg(test)]
mod tests {
    use super::{
        build_search_rebuild_operation, indexed_message_from_response,
        validate_search_query_with_limits,
    };
    use crate::server::{
        core::{IndexedMessage, SearchOperation},
        errors::AuthFailure,
        types::{MessageResponse, SearchQuery},
    };

    fn sample_doc(id: &str) -> IndexedMessage {
        IndexedMessage {
            message_id: id.to_owned(),
            guild_id: String::from("g1"),
            channel_id: String::from("c1"),
            author_id: String::from("u1"),
            created_at_unix: 1,
            content: String::from("hello"),
        }
    }

    #[test]
    fn validate_search_query_with_limits_rejects_blank_query() {
        let query = SearchQuery {
            q: String::from("  "),
            limit: Some(5),
            channel_id: None,
        };

        let result = validate_search_query_with_limits(&query, 20, 256, 50);

        assert!(matches!(result, Err(AuthFailure::InvalidRequest)));
    }

    #[test]
    fn validate_search_query_with_limits_accepts_default_limit_when_missing() {
        let query = SearchQuery {
            q: String::from("hello"),
            limit: None,
            channel_id: Some(String::from("c1")),
        };

        let result = validate_search_query_with_limits(&query, 20, 256, 50);

        assert!(result.is_ok());
    }

    #[test]
    fn build_search_rebuild_operation_wraps_docs() {
        let op = build_search_rebuild_operation(vec![sample_doc("m1"), sample_doc("m2")]);

        match op {
            SearchOperation::Rebuild { docs } => {
                assert_eq!(docs.len(), 2);
                assert_eq!(docs[0].message_id, "m1");
                assert_eq!(docs[1].message_id, "m2");
            }
            _ => panic!("expected rebuild operation"),
        }
    }

    #[test]
    fn build_search_rebuild_operation_supports_empty_docs() {
        let op = build_search_rebuild_operation(Vec::new());

        match op {
            SearchOperation::Rebuild { docs } => assert!(docs.is_empty()),
            _ => panic!("expected rebuild operation"),
        }
    }

    #[test]
    fn indexed_message_from_response_maps_all_fields() {
        let response = MessageResponse {
            message_id: String::from("m1"),
            guild_id: String::from("g1"),
            channel_id: String::from("c1"),
            author_id: String::from("u1"),
            content: String::from("hello"),
            markdown_tokens: Vec::new(),
            attachments: Vec::new(),
            reactions: Vec::new(),
            created_at_unix: 42,
        };

        let indexed = indexed_message_from_response(&response);

        assert_eq!(indexed.message_id, "m1");
        assert_eq!(indexed.guild_id, "g1");
        assert_eq!(indexed.channel_id, "c1");
        assert_eq!(indexed.author_id, "u1");
        assert_eq!(indexed.content, "hello");
        assert_eq!(indexed.created_at_unix, 42);
    }
}
