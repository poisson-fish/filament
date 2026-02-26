use std::{collections::HashMap, sync::Arc, time::Instant};

use anyhow::anyhow;
use tantivy::{
    schema::{NumericOptions, Schema, TextFieldIndexing, TextOptions, STORED, STRING},
    TantivyDocument, Term,
};
use tokio::sync::{mpsc, oneshot};

use crate::server::{
    core::{
        AppState, GuildRecord, IndexedMessage, SearchCommand, SearchFields, SearchIndexState,
        SearchOperation, SearchService, DEFAULT_SEARCH_RESULT_LIMIT, MAX_SEARCH_FUZZY,
        MAX_SEARCH_TERMS, MAX_SEARCH_WILDCARDS, SEARCH_INDEX_QUEUE_CAPACITY,
    },
    domain::{
        attachment_map_for_messages_db, attachment_map_for_messages_in_memory,
        reaction_map_for_messages_db,
    },
    errors::AuthFailure,
    types::{MessageResponse, SearchQuery},
};

use super::hydration_runtime::{
    apply_hydration_attachments, collect_hydrated_in_request_order, collect_hydrated_messages_db,
    collect_hydrated_messages_in_memory, merge_hydration_maps,
};

const SEARCH_WORKER_BATCH_LIMIT: usize = 128;
type IndexedMessageRow = (String, String, String, String, String, i64);

pub(crate) async fn enqueue_search_command(
    tx: &mpsc::Sender<SearchCommand>,
    op: SearchOperation,
    wait_for_apply: bool,
) -> Result<(), AuthFailure> {
    let timing_enabled = std::env::var_os("FILAMENT_DEBUG_REQUEST_TIMINGS").is_some();
    let total_start = Instant::now();
    if wait_for_apply {
        let send_start = Instant::now();
        let (ack_tx, ack_rx) = oneshot::channel();
        tx.send(SearchCommand {
            op,
            ack: Some(ack_tx),
        })
        .await
        .map_err(|_| AuthFailure::Internal)?;
        let send_ms = send_start.elapsed().as_millis();
        let ack_start = Instant::now();
        let result = ack_rx.await.map_err(|_| AuthFailure::Internal)?;
        if timing_enabled {
            tracing::info!(
                event = "debug.search.enqueue_search_command.timing",
                wait_for_apply,
                send_ms,
                ack_wait_ms = ack_start.elapsed().as_millis(),
                total_ms = total_start.elapsed().as_millis()
            );
        }
        result
    } else {
        let result = tx
            .send(SearchCommand { op, ack: None })
            .await
            .map_err(|_| AuthFailure::Internal);
        if timing_enabled {
            tracing::info!(
                event = "debug.search.enqueue_search_command.timing",
                wait_for_apply,
                total_ms = total_start.elapsed().as_millis()
            );
        }
        result
    }
}

pub(crate) fn drain_search_batch(
    first: SearchCommand,
    rx: &mut mpsc::Receiver<SearchCommand>,
    max_batch: usize,
) -> Vec<SearchCommand> {
    let max_batch = max_batch.max(1);
    let mut batch = vec![first];
    while batch.len() < max_batch {
        let Ok(next) = rx.try_recv() else {
            break;
        };
        batch.push(next);
    }
    batch
}

pub(crate) fn build_search_schema() -> (Schema, SearchFields) {
    let mut schema_builder = Schema::builder();
    let message_id = schema_builder.add_text_field("message_id", STRING | STORED);
    let guild_id = schema_builder.add_text_field("guild_id", STRING | STORED);
    let channel_id = schema_builder.add_text_field("channel_id", STRING | STORED);
    let author_id = schema_builder.add_text_field("author_id", STRING | STORED);
    let created_at_unix =
        schema_builder.add_i64_field("created_at_unix", NumericOptions::default().set_stored());
    let content_options = TextOptions::default()
        .set_stored()
        .set_indexing_options(TextFieldIndexing::default().set_tokenizer("default"));
    let content = schema_builder.add_text_field("content", content_options);
    let schema = schema_builder.build();

    (
        schema,
        SearchFields {
            message_id,
            guild_id,
            channel_id,
            author_id,
            created_at_unix,
            content,
        },
    )
}

fn indexed_messages_from_rows(rows: Vec<IndexedMessageRow>) -> Vec<IndexedMessage> {
    rows.into_iter()
        .map(
            |(message_id, guild_id, channel_id, author_id, content, created_at_unix)| {
                IndexedMessage {
                    message_id,
                    guild_id,
                    channel_id,
                    author_id,
                    created_at_unix,
                    content,
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

fn apply_search_batch_with_ack<F>(
    search: &Arc<SearchIndexState>,
    batch: &mut Vec<SearchCommand>,
    apply_op: F,
) -> anyhow::Result<()>
where
    F: Fn(&SearchIndexState, &mut tantivy::IndexWriter, SearchOperation),
{
    let mut ops = Vec::with_capacity(batch.len());
    let mut pending_acks = Vec::new();
    for command in batch.drain(..) {
        if let Some(ack) = command.ack {
            pending_acks.push(ack);
        }
        ops.push(command.op);
    }

    let apply_result = (|| -> anyhow::Result<()> {
        let mut writer = search.index.writer(50_000_000)?;
        for op in ops {
            apply_op(search, &mut writer, op);
        }
        writer.commit()?;
        search.reader.reload()?;
        Ok(())
    })();

    match apply_result {
        Ok(()) => {
            for ack in pending_acks {
                let _ = ack.send(Ok(()));
            }
            Ok(())
        }
        Err(error) => {
            for ack in pending_acks {
                let _ = ack.send(Err(AuthFailure::Internal));
            }
            Err(error)
        }
    }
}

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

pub(crate) fn validate_search_query_limits(
    raw_query: &str,
    limit: usize,
    max_query_chars: usize,
    max_result_limit: usize,
) -> Result<(), AuthFailure> {
    if raw_query.is_empty() || raw_query.len() > max_query_chars {
        return Err(AuthFailure::InvalidRequest);
    }
    if limit == 0 || limit > max_result_limit {
        return Err(AuthFailure::InvalidRequest);
    }
    if raw_query.split_whitespace().count() > MAX_SEARCH_TERMS {
        return Err(AuthFailure::InvalidRequest);
    }
    let wildcard_count = raw_query.matches('*').count() + raw_query.matches('?').count();
    if wildcard_count > MAX_SEARCH_WILDCARDS {
        return Err(AuthFailure::InvalidRequest);
    }
    if raw_query.matches('~').count() > MAX_SEARCH_FUZZY {
        return Err(AuthFailure::InvalidRequest);
    }
    if raw_query.contains(':') {
        return Err(AuthFailure::InvalidRequest);
    }
    Ok(())
}

fn build_search_rebuild_operation(docs: Vec<IndexedMessage>) -> SearchOperation {
    SearchOperation::Rebuild { docs }
}

fn map_collect_all_rows(rows: Vec<IndexedMessageRow>) -> Vec<IndexedMessage> {
    collect_all_indexed_messages_rows(rows)
}

fn collect_all_indexed_messages_in_memory(
    guilds: &HashMap<String, GuildRecord>,
) -> Vec<IndexedMessage> {
    let mut docs = Vec::new();
    for (guild_id, guild) in guilds {
        for (channel_id, channel) in &guild.channels {
            for message in &channel.messages {
                docs.push(IndexedMessage {
                    message_id: message.id.clone(),
                    guild_id: guild_id.clone(),
                    channel_id: channel_id.clone(),
                    author_id: message.author_id.to_string(),
                    content: message.content.clone(),
                    created_at_unix: message.created_at_unix,
                });
            }
        }
    }
    docs
}

fn map_collect_guild_rows(
    rows: Vec<IndexedMessageRow>,
    max_docs: usize,
) -> Result<Vec<IndexedMessage>, AuthFailure> {
    enforce_guild_collect_doc_cap(rows.len(), max_docs)?;
    Ok(collect_indexed_messages_for_guild_rows(rows))
}

fn collect_indexed_messages_for_guild_in_memory(
    guilds: &HashMap<String, GuildRecord>,
    guild_id: &str,
    max_docs: usize,
) -> Result<Vec<IndexedMessage>, AuthFailure> {
    let Some(guild) = guilds.get(guild_id) else {
        return Err(AuthFailure::NotFound);
    };

    let mut docs = Vec::new();
    for (channel_id, channel) in &guild.channels {
        for message in &channel.messages {
            if docs.len() >= max_docs {
                return Err(AuthFailure::InvalidRequest);
            }
            docs.push(IndexedMessage {
                message_id: message.id.clone(),
                guild_id: guild_id.to_owned(),
                channel_id: channel_id.clone(),
                author_id: message.author_id.to_string(),
                content: message.content.clone(),
                created_at_unix: message.created_at_unix,
            });
        }
    }
    Ok(docs)
}

fn validate_search_query_with_limits(
    query: &SearchQuery,
    default_limit: usize,
    max_chars: usize,
    max_limit: usize,
) -> Result<(), AuthFailure> {
    let raw = normalize_search_query(&query.q);
    let limit = effective_search_limit(query.limit, default_limit);
    validate_search_query_limits(&raw, limit, max_chars, max_limit)
}

pub(super) fn normalize_search_query(raw_query: &str) -> String {
    raw_query.trim().to_owned()
}

fn effective_search_limit(requested_limit: Option<usize>, default_limit: usize) -> usize {
    requested_limit.unwrap_or(default_limit)
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
    if let Some(pool) = &state.db_pool {
        let rows = sqlx::query_as::<_, IndexedMessageRow>(
            "SELECT message_id, guild_id, channel_id, author_id, content, created_at_unix
             FROM messages",
        )
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        return Ok(map_collect_all_rows(rows));
    }

    let guilds = state.membership_store.guilds().read().await;
    Ok(collect_all_indexed_messages_in_memory(&guilds))
}

pub(crate) async fn collect_indexed_messages_for_guild(
    state: &AppState,
    guild_id: &str,
    max_docs: usize,
) -> Result<Vec<IndexedMessage>, AuthFailure> {
    if let Some(pool) = &state.db_pool {
        let limit = guild_collect_fetch_limit(max_docs)?;
        let rows = sqlx::query_as::<_, IndexedMessageRow>(
            "SELECT message_id, guild_id, channel_id, author_id, content, created_at_unix
             FROM messages
             WHERE guild_id = $1
             ORDER BY created_at_unix DESC
             LIMIT $2",
        )
        .bind(guild_id)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        return map_collect_guild_rows(rows, max_docs);
    }

    let guilds = state.membership_store.guilds().read().await;
    collect_indexed_messages_for_guild_in_memory(&guilds, guild_id, max_docs)
}

pub(crate) async fn hydrate_messages_by_id(
    state: &AppState,
    guild_id: &str,
    channel_id: Option<&str>,
    message_ids: &[String],
) -> Result<Vec<MessageResponse>, AuthFailure> {
    if message_ids.is_empty() {
        return Ok(Vec::new());
    }

    if let Some(pool) = &state.db_pool {
        let mut by_id =
            collect_hydrated_messages_db(pool, guild_id, channel_id, message_ids).await?;
        let message_ids_ordered: Vec<String> = message_ids.to_vec();
        let attachment_map =
            attachment_map_for_messages_db(pool, guild_id, channel_id, &message_ids_ordered)
                .await?;
        let reaction_map =
            reaction_map_for_messages_db(pool, guild_id, channel_id, &message_ids_ordered, None)
                .await?;
        merge_hydration_maps(&mut by_id, &attachment_map, &reaction_map);
        return Ok(collect_hydrated_in_request_order(by_id, message_ids));
    }

    let guilds = state.membership_store.guilds().read().await;
    let guild = guilds.get(guild_id).ok_or(AuthFailure::NotFound)?;
    let mut by_id = collect_hydrated_messages_in_memory(guild, guild_id, channel_id)?;
    let attachment_map =
        attachment_map_for_messages_in_memory(state, guild_id, channel_id, message_ids).await;
    apply_hydration_attachments(&mut by_id, &attachment_map);
    Ok(collect_hydrated_in_request_order(by_id, message_ids))
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use filament_core::{ChannelKind, Role, UserId};
    use std::sync::Arc;
    use tantivy::{
        collector::{Count, TopDocs},
        query::{AllQuery, TermQuery},
        schema::{IndexRecordOption, Type, Value},
        TantivyDocument, Term,
    };

    use tokio::sync::{mpsc, oneshot};

    use super::{
        apply_search_batch_with_ack, apply_search_operation, build_search_rebuild_operation,
        build_search_schema, collect_all_indexed_messages_in_memory,
        collect_all_indexed_messages_rows, collect_indexed_messages_for_guild_in_memory,
        collect_indexed_messages_for_guild_rows, drain_search_batch, effective_search_limit,
        enforce_guild_collect_doc_cap, enqueue_search_command, guild_collect_fetch_limit,
        indexed_message_from_response, map_collect_all_rows, map_collect_guild_rows,
        normalize_search_query, validate_search_query_limits, validate_search_query_with_limits,
    };
    use crate::server::{
        core::{
            ChannelRecord, GuildRecord, GuildVisibility, IndexedMessage, MessageRecord,
            SearchCommand, SearchOperation,
        },
        errors::AuthFailure,
        types::{MessageResponse, SearchQuery},
    };

    fn search_state() -> Arc<crate::server::core::SearchIndexState> {
        let (schema, fields) = build_search_schema();
        let index = tantivy::Index::create_in_ram(schema);
        let reader = index.reader().expect("reader should initialize");
        Arc::new(crate::server::core::SearchIndexState {
            index,
            reader,
            fields,
        })
    }

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

    fn command(message_id: &str) -> SearchCommand {
        SearchCommand {
            op: SearchOperation::Delete {
                message_id: message_id.to_owned(),
            },
            ack: None,
        }
    }

    fn message_id(command: &SearchCommand) -> Option<&str> {
        match &command.op {
            SearchOperation::Delete { message_id } => Some(message_id.as_str()),
            _ => None,
        }
    }

    fn guild_with_messages(guild_id: &str, message_ids: &[&str]) -> HashMap<String, GuildRecord> {
        let author = UserId::new();
        let messages = message_ids
            .iter()
            .map(|message_id| MessageRecord {
                id: (*message_id).to_owned(),
                author_id: author,
                content: format!("message-{message_id}"),
                markdown_tokens: Vec::new(),
                attachment_ids: Vec::new(),
                created_at_unix: 1,
                reactions: HashMap::new(),
            })
            .collect();

        HashMap::from([(
            guild_id.to_owned(),
            GuildRecord {
                name: String::from("Guild"),
                visibility: GuildVisibility::Private,
                created_by_user_id: author,
                default_join_role_id: None,
                members: HashMap::from([(author, Role::Owner)]),
                banned_members: HashSet::new(),
                channels: HashMap::from([(
                    String::from("c1"),
                    ChannelRecord {
                        name: String::from("general"),
                        kind: ChannelKind::Text,
                        messages,
                        role_overrides: HashMap::new(),
                    },
                )]),
            },
        )])
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
    fn validate_search_query_limits_rejects_invalid_limit_values() {
        assert!(validate_search_query_limits("ok", 0, 256, 50).is_err());
        assert!(validate_search_query_limits("ok", 51, 256, 50).is_err());
    }

    #[test]
    fn validate_search_query_limits_rejects_abusive_patterns() {
        assert!(validate_search_query_limits(
            "a b c d e f g h i j k l m n o p q r s t u",
            20,
            256,
            50
        )
        .is_err());
        assert!(validate_search_query_limits("a*b?c*d?e*f", 20, 256, 50).is_err());
        assert!(validate_search_query_limits("a~b~c~", 20, 256, 50).is_err());
        assert!(validate_search_query_limits("author:alice", 20, 256, 50).is_err());
    }

    #[test]
    fn normalize_search_query_trims_surrounding_whitespace() {
        assert_eq!(normalize_search_query("  hello world  \n"), "hello world");
    }

    #[test]
    fn normalize_search_query_preserves_internal_whitespace() {
        assert_eq!(normalize_search_query("  hello   world  "), "hello   world");
    }

    #[test]
    fn effective_search_limit_uses_default_when_missing() {
        assert_eq!(effective_search_limit(None, 25), 25);
    }

    #[test]
    fn effective_search_limit_uses_requested_when_present() {
        assert_eq!(effective_search_limit(Some(10), 25), 10);
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

    #[test]
    fn build_search_schema_registers_expected_fields() {
        let (schema, fields) = build_search_schema();

        let message_field_name = schema.get_field_name(fields.message_id);
        let guild_field_name = schema.get_field_name(fields.guild_id);
        let channel_field_name = schema.get_field_name(fields.channel_id);
        let author_field_name = schema.get_field_name(fields.author_id);
        let content_field_name = schema.get_field_name(fields.content);

        assert_eq!(message_field_name, "message_id");
        assert_eq!(guild_field_name, "guild_id");
        assert_eq!(channel_field_name, "channel_id");
        assert_eq!(author_field_name, "author_id");
        assert_eq!(content_field_name, "content");
    }

    #[test]
    fn build_search_schema_marks_created_at_as_i64() {
        let (schema, fields) = build_search_schema();

        let entry = schema.get_field_entry(fields.created_at_unix);

        assert_eq!(entry.field_type().value_type(), Type::I64);
    }

    #[test]
    fn map_collect_all_rows_maps_messages_in_order() {
        let docs = map_collect_all_rows(vec![
            (
                String::from("m1"),
                String::from("g1"),
                String::from("c1"),
                String::from("u1"),
                String::from("first"),
                10,
            ),
            (
                String::from("m2"),
                String::from("g1"),
                String::from("c2"),
                String::from("u2"),
                String::from("second"),
                11,
            ),
        ]);

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].message_id, "m1");
        assert_eq!(docs[1].message_id, "m2");
    }

    #[test]
    fn map_collect_guild_rows_fails_closed_when_rows_exceed_cap() {
        let result = map_collect_guild_rows(
            vec![
                (
                    String::from("m1"),
                    String::from("g1"),
                    String::from("c1"),
                    String::from("u1"),
                    String::from("first"),
                    10,
                ),
                (
                    String::from("m2"),
                    String::from("g1"),
                    String::from("c1"),
                    String::from("u1"),
                    String::from("second"),
                    11,
                ),
            ],
            1,
        );

        assert!(matches!(result, Err(AuthFailure::InvalidRequest)));
    }

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

    #[test]
    fn drain_search_batch_drains_up_to_max_batch_size() {
        let (tx, mut rx) = mpsc::channel::<SearchCommand>(8);
        tx.try_send(command("m2"))
            .expect("second command should queue");
        tx.try_send(command("m3"))
            .expect("third command should queue");

        let batch = drain_search_batch(command("m1"), &mut rx, 2);

        assert_eq!(batch.len(), 2);
        assert_eq!(message_id(&batch[0]), Some("m1"));
        assert_eq!(message_id(&batch[1]), Some("m2"));
        assert_eq!(rx.try_recv().ok().as_ref().and_then(message_id), Some("m3"));
    }

    #[test]
    fn drain_search_batch_defaults_to_single_item_when_max_batch_is_zero() {
        let (tx, mut rx) = mpsc::channel::<SearchCommand>(4);
        tx.try_send(command("m2"))
            .expect("second command should queue");

        let batch = drain_search_batch(command("m1"), &mut rx, 0);

        assert_eq!(batch.len(), 1);
        assert_eq!(message_id(&batch[0]), Some("m1"));
        assert_eq!(rx.try_recv().ok().as_ref().and_then(message_id), Some("m2"));
    }

    #[test]
    fn collect_all_indexed_messages_returns_documents_for_all_channels() {
        let author = UserId::new();
        let guild_id = String::from("g1");
        let mut guilds = HashMap::new();
        guilds.insert(
            guild_id.clone(),
            GuildRecord {
                name: String::from("Guild"),
                visibility: GuildVisibility::Private,
                created_by_user_id: author,
                default_join_role_id: None,
                members: HashMap::from([(author, Role::Owner)]),
                banned_members: HashSet::new(),
                channels: HashMap::from([
                    (
                        String::from("c1"),
                        ChannelRecord {
                            name: String::from("general"),
                            kind: ChannelKind::Text,
                            messages: vec![MessageRecord {
                                id: String::from("m1"),
                                author_id: author,
                                content: String::from("hello"),
                                markdown_tokens: Vec::new(),
                                attachment_ids: Vec::new(),
                                created_at_unix: 10,
                                reactions: HashMap::new(),
                            }],
                            role_overrides: HashMap::new(),
                        },
                    ),
                    (
                        String::from("c2"),
                        ChannelRecord {
                            name: String::from("random"),
                            kind: ChannelKind::Text,
                            messages: vec![MessageRecord {
                                id: String::from("m2"),
                                author_id: author,
                                content: String::from("world"),
                                markdown_tokens: Vec::new(),
                                attachment_ids: Vec::new(),
                                created_at_unix: 11,
                                reactions: HashMap::new(),
                            }],
                            role_overrides: HashMap::new(),
                        },
                    ),
                ]),
            },
        );

        let docs = collect_all_indexed_messages_in_memory(&guilds);

        assert_eq!(docs.len(), 2);
        assert!(docs.iter().any(|doc| {
            doc.message_id == "m1"
                && doc.guild_id == "g1"
                && doc.channel_id == "c1"
                && doc.content == "hello"
        }));
        assert!(docs.iter().any(|doc| {
            doc.message_id == "m2"
                && doc.guild_id == "g1"
                && doc.channel_id == "c2"
                && doc.content == "world"
        }));
    }

    #[test]
    fn collect_indexed_messages_for_guild_returns_not_found_for_missing_guild() {
        let guilds = guild_with_messages("g1", &["m1"]);

        let result = collect_indexed_messages_for_guild_in_memory(&guilds, "missing", 10);

        assert!(matches!(result, Err(AuthFailure::NotFound)));
    }

    #[test]
    fn collect_indexed_messages_for_guild_rejects_when_cap_is_exceeded() {
        let guilds = guild_with_messages("g1", &["m1", "m2"]);

        let result = collect_indexed_messages_for_guild_in_memory(&guilds, "g1", 1);

        assert!(matches!(result, Err(AuthFailure::InvalidRequest)));
    }

    #[test]
    fn collect_indexed_messages_for_guild_returns_all_messages_when_within_cap() {
        let guilds = guild_with_messages("g1", &["m1", "m2"]);

        let docs = collect_indexed_messages_for_guild_in_memory(&guilds, "g1", 2)
            .expect("documents should be collected");

        assert_eq!(docs.len(), 2);
        assert!(docs.iter().any(|doc| doc.message_id == "m1"));
        assert!(docs.iter().any(|doc| doc.message_id == "m2"));
    }

    #[test]
    fn apply_search_batch_with_ack_sends_success_ack_when_batch_applies() {
        let search = search_state();
        let (ack_tx, ack_rx) = oneshot::channel();
        let mut batch = vec![SearchCommand {
            op: SearchOperation::Delete {
                message_id: String::from("m1"),
            },
            ack: Some(ack_tx),
        }];

        let result = apply_search_batch_with_ack(&search, &mut batch, |_search, _writer, _op| {});

        assert!(result.is_ok());
        assert!(batch.is_empty());
        assert!(matches!(ack_rx.blocking_recv(), Ok(Ok(()))));
    }

    #[test]
    fn apply_search_batch_with_ack_sends_internal_ack_when_batch_apply_fails() {
        let search = search_state();
        let writer_guard: tantivy::IndexWriter<tantivy::schema::TantivyDocument> = search
            .index
            .writer(50_000_000)
            .expect("lock writer for failure path");
        let (ack_tx, ack_rx) = oneshot::channel();
        let mut batch = vec![SearchCommand {
            op: SearchOperation::Delete {
                message_id: String::from("m2"),
            },
            ack: Some(ack_tx),
        }];

        let result = apply_search_batch_with_ack(&search, &mut batch, |_search, _writer, _op| {});

        assert!(result.is_err());
        assert!(batch.is_empty());
        assert!(matches!(
            ack_rx.blocking_recv(),
            Ok(Err(AuthFailure::Internal))
        ));
        drop(writer_guard);
    }

    #[tokio::test]
    async fn enqueue_search_command_sends_without_ack_when_wait_is_false() {
        let (tx, mut rx) =
            mpsc::channel::<SearchCommand>(crate::server::core::SEARCH_INDEX_QUEUE_CAPACITY);

        enqueue_search_command(
            &tx,
            SearchOperation::Delete {
                message_id: String::from("m1"),
            },
            false,
        )
        .await
        .expect("enqueue should succeed");

        let command = rx.recv().await.expect("command should be queued");
        assert!(command.ack.is_none());
        match command.op {
            SearchOperation::Delete { message_id } => assert_eq!(message_id, "m1"),
            _ => panic!("expected delete operation"),
        }
    }

    #[tokio::test]
    async fn enqueue_search_command_waits_for_ack_when_wait_is_true() {
        let (tx, mut rx) =
            mpsc::channel::<SearchCommand>(crate::server::core::SEARCH_INDEX_QUEUE_CAPACITY);
        let receive_task = tokio::spawn(async move {
            let command = rx.recv().await.expect("command should be queued");
            let ack = command.ack.expect("ack channel should be present");
            ack.send(Ok(())).expect("ack should be delivered");
        });

        enqueue_search_command(
            &tx,
            SearchOperation::Delete {
                message_id: String::from("m2"),
            },
            true,
        )
        .await
        .expect("enqueue should succeed with ack");

        receive_task.await.expect("receiver task should join");
    }

    #[tokio::test]
    async fn enqueue_search_command_returns_internal_when_ack_channel_closes_without_response() {
        let (tx, mut rx) =
            mpsc::channel::<SearchCommand>(crate::server::core::SEARCH_INDEX_QUEUE_CAPACITY);
        let receive_task = tokio::spawn(async move {
            let _command = rx.recv().await.expect("command should be queued");
        });

        let result = enqueue_search_command(
            &tx,
            SearchOperation::Delete {
                message_id: String::from("m3"),
            },
            true,
        )
        .await;

        assert!(matches!(result, Err(AuthFailure::Internal)));
        receive_task.await.expect("receiver task should join");
    }

    #[tokio::test]
    async fn enqueue_search_command_returns_internal_when_sender_channel_is_closed() {
        let (tx, rx) =
            mpsc::channel::<SearchCommand>(crate::server::core::SEARCH_INDEX_QUEUE_CAPACITY);
        drop(rx);

        let result = enqueue_search_command(
            &tx,
            SearchOperation::Delete {
                message_id: String::from("m4"),
            },
            false,
        )
        .await;

        assert!(matches!(result, Err(AuthFailure::Internal)));
    }

    fn contains_message_with_content(
        search: &crate::server::core::SearchIndexState,
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

    fn total_doc_count(search: &crate::server::core::SearchIndexState) -> usize {
        let searcher = search.reader.searcher();
        searcher
            .search(&AllQuery, &Count)
            .expect("count should succeed")
    }

    fn apply_upsert(
        search: &crate::server::core::SearchIndexState,
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
        writer.commit().expect("commit should succeed");
        search
            .reader
            .reload()
            .expect("reader reload should succeed");
    }

    #[test]
    fn apply_search_operation_upsert_replaces_existing_message_by_id() {
        let search = search_state();
        apply_upsert(&search, "m1", "g1", "c1", "hello", 1);
        apply_upsert(&search, "m1", "g1", "c1", "updated", 2);

        assert_eq!(total_doc_count(&search), 1);
        assert!(contains_message_with_content(&search, "m1", "updated"));
    }

    #[test]
    fn apply_search_operation_delete_removes_message_from_index() {
        let search = search_state();
        apply_upsert(&search, "m1", "g1", "c1", "hello", 1);

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
        writer.commit().expect("commit should succeed");
        search
            .reader
            .reload()
            .expect("reader reload should succeed");

        assert_eq!(total_doc_count(&search), 0);
        assert!(!contains_message_with_content(&search, "m1", "hello"));
    }

    #[test]
    fn apply_search_operation_reconcile_deletes_removed_and_upserts_new_docs() {
        let search = search_state();
        apply_upsert(&search, "m1", "g1", "c1", "old", 1);

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
        writer.commit().expect("commit should succeed");
        search
            .reader
            .reload()
            .expect("reader reload should succeed");

        assert_eq!(total_doc_count(&search), 1);
        assert!(!contains_message_with_content(&search, "m1", "old"));
        assert!(contains_message_with_content(&search, "m2", "new"));
    }
}
