use std::{collections::HashMap, sync::Arc};

use anyhow::anyhow;
use tantivy::schema::{NumericOptions, Schema, TextFieldIndexing, TextOptions, STORED, STRING};
use tokio::sync::mpsc;

use crate::server::{
    core::{
        AppState, GuildRecord, IndexedMessage, SearchCommand, SearchFields, SearchIndexState,
        SearchOperation, SearchService, DEFAULT_SEARCH_RESULT_LIMIT, MAX_SEARCH_FUZZY,
        MAX_SEARCH_TERMS, MAX_SEARCH_WILDCARDS, SEARCH_INDEX_QUEUE_CAPACITY,
    },
    errors::AuthFailure,
    types::{MessageResponse, SearchQuery},
};

use super::{
    hydration_runtime::hydrate_messages_by_id_runtime,
    search_apply::apply_search_operation as apply_search_operation_impl,
    search_apply_batch::apply_search_batch_with_ack,
    search_collect_db::{
        collect_all_indexed_messages_rows, collect_indexed_messages_for_guild_rows,
        enforce_guild_collect_doc_cap, guild_collect_fetch_limit,
    },
    search_enqueue::enqueue_search_command,
};

const SEARCH_WORKER_BATCH_LIMIT: usize = 128;
type IndexedMessageRow = (String, String, String, String, String, i64);

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
    hydrate_messages_by_id_runtime(state, guild_id, channel_id, message_ids).await
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use filament_core::{ChannelKind, Role, UserId};
    use tantivy::schema::Type;
    use tokio::sync::mpsc;

    use super::{
        build_search_rebuild_operation, build_search_schema,
        collect_all_indexed_messages_in_memory, collect_indexed_messages_for_guild_in_memory,
        drain_search_batch, effective_search_limit, indexed_message_from_response,
        map_collect_all_rows, map_collect_guild_rows, normalize_search_query,
        validate_search_query_limits, validate_search_query_with_limits,
    };
    use crate::server::{
        core::{
            ChannelRecord, GuildRecord, GuildVisibility, IndexedMessage, MessageRecord,
            SearchCommand, SearchOperation,
        },
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
}
