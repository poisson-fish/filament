use std::{
    collections::{HashMap, HashSet, VecDeque},
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use anyhow::anyhow;
use axum::{
    extract::{
        connect_info::ConnectInfo,
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade},
        Extension, Query, State,
    },
    http::HeaderMap,
    response::IntoResponse,
};
use filament_core::{tokenize_markdown, Permission, UserId};
use filament_protocol::parse_envelope;
use futures_util::{SinkExt, StreamExt};
use sqlx::Row;
use tantivy::{
    collector::{Count, TopDocs},
    query::{BooleanQuery, Occur, QueryParser, TermQuery},
    schema::{
        IndexRecordOption, NumericOptions, Schema, TextFieldIndexing, TextOptions, Value, STORED,
        STRING,
    },
    TantivyDocument, Term,
};
use tokio::sync::{mpsc, oneshot, watch};
use ulid::Ulid;
use uuid::Uuid;

use super::{
    auth::{
        authenticate_with_token, bearer_token, channel_key, extract_client_ip, now_unix,
        validate_message_content, ClientIp,
    },
    core::{
        AppState, AuthContext, ConnectionControl, ConnectionPresence, IndexedMessage,
        MessageRecord, SearchCommand, SearchFields, SearchIndexState, SearchOperation,
        SearchService, VoiceParticipant, VoiceStreamKind, DEFAULT_SEARCH_RESULT_LIMIT,
        MAX_SEARCH_FUZZY, MAX_SEARCH_TERMS, MAX_SEARCH_WILDCARDS, MAX_TRACKED_VOICE_CHANNELS,
        MAX_TRACKED_VOICE_PARTICIPANTS_PER_CHANNEL, SEARCH_INDEX_QUEUE_CAPACITY,
    },
    db::ensure_db_schema,
    domain::{
        attachment_map_for_messages_db, attachment_map_for_messages_in_memory,
        attachments_for_message_in_memory, bind_message_attachments_db,
        channel_permission_snapshot, enforce_guild_ip_ban_for_request,
        fetch_attachments_for_message_db, parse_attachment_ids, reaction_map_for_messages_db,
        reaction_summaries_from_users, user_can_write_channel,
    },
    errors::AuthFailure,
    gateway_events::{self, GatewayEvent},
    metrics::{
        record_gateway_event_dropped, record_gateway_event_emitted,
        record_gateway_event_parse_rejected, record_gateway_event_unknown_received,
        record_voice_sync_repair, record_ws_disconnect,
    },
    types::{
        GatewayAuthQuery, GatewayMessageCreate, GatewaySubscribe, MessageResponse, SearchQuery,
    },
};

pub(crate) async fn gateway_ws(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
    Query(query): Query<GatewayAuthQuery>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
) -> Result<impl IntoResponse, AuthFailure> {
    let token = query
        .access_token
        .or_else(|| bearer_token(&headers).map(ToOwned::to_owned))
        .ok_or(AuthFailure::Unauthorized)?;
    let auth = authenticate_with_token(&state, &token).await?;
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );

    Ok(ws.on_upgrade(move |socket| async move {
        handle_gateway_connection(state, socket, auth, client_ip).await;
    }))
}

#[allow(clippy::too_many_lines)]
pub(crate) async fn handle_gateway_connection(
    state: AppState,
    socket: WebSocket,
    auth: AuthContext,
    client_ip: ClientIp,
) {
    let connection_id = Uuid::new_v4();
    let (mut sink, mut stream) = socket.split();
    let slow_consumer_disconnect = Arc::new(AtomicBool::new(false));

    let (outbound_tx, mut outbound_rx) =
        mpsc::channel::<String>(state.runtime.gateway_outbound_queue);
    state
        .connection_senders
        .write()
        .await
        .insert(connection_id, outbound_tx.clone());
    let (control_tx, mut control_rx) = watch::channel(ConnectionControl::Open);
    state
        .connection_controls
        .write()
        .await
        .insert(connection_id, control_tx);
    state.connection_presence.write().await.insert(
        connection_id,
        ConnectionPresence {
            user_id: auth.user_id,
            guild_ids: HashSet::new(),
        },
    );

    let ready_event = gateway_events::ready(auth.user_id);
    let _ = outbound_tx.send(ready_event.payload).await;
    record_gateway_event_emitted("connection", ready_event.event_type);

    let slow_consumer_disconnect_send = Arc::clone(&slow_consumer_disconnect);
    let send_task = tokio::spawn(async move {
        let mut ping_interval = tokio::time::interval(Duration::from_secs(30));
        ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = ping_interval.tick() => {
                    if sink.send(Message::Ping(vec![].into())).await.is_err() {
                        break;
                    }
                }
                control_change = control_rx.changed() => {
                    if control_change.is_ok() && *control_rx.borrow() == ConnectionControl::Close {
                        slow_consumer_disconnect_send.store(true, Ordering::Relaxed);
                        record_ws_disconnect("slow_consumer");
                        let _ = sink
                            .send(Message::Close(Some(CloseFrame {
                                code: 1008,
                                reason: "slow_consumer".into(),
                            })))
                            .await;
                        break;
                    }
                }
                maybe_payload = outbound_rx.recv() => {
                    match maybe_payload {
                        Some(payload) => {
                            if sink.send(Message::Text(payload.into())).await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
            }
        }
    });

    let mut ingress = VecDeque::new();
    let mut disconnect_reason = "connection_closed";
    while let Some(incoming) = stream.next().await {
        let Ok(message) = incoming else {
            disconnect_reason = "socket_error";
            break;
        };

        let payload: Vec<u8> = match message {
            Message::Text(text) => {
                if text.len() > state.runtime.max_gateway_event_bytes {
                    disconnect_reason = "event_too_large";
                    break;
                }
                text.as_bytes().to_vec()
            }
            Message::Binary(bytes) => {
                if bytes.len() > state.runtime.max_gateway_event_bytes {
                    disconnect_reason = "event_too_large";
                    break;
                }
                bytes.to_vec()
            }
            Message::Close(_) => {
                disconnect_reason = "client_close";
                break;
            }
            Message::Ping(_) | Message::Pong(_) => continue,
        };

        if !allow_gateway_ingress(
            &mut ingress,
            state.runtime.gateway_ingress_events_per_window,
            state.runtime.gateway_ingress_window,
        ) {
            disconnect_reason = "ingress_rate_limited";
            break;
        }

        let Ok(envelope) = parse_envelope(&payload) else {
            record_gateway_event_parse_rejected("ingress", "invalid_envelope");
            disconnect_reason = "invalid_envelope";
            break;
        };

        match envelope.t.as_str() {
            "subscribe" => {
                let Ok(subscribe) = serde_json::from_value::<GatewaySubscribe>(envelope.d) else {
                    record_gateway_event_parse_rejected("ingress", "invalid_subscribe_payload");
                    disconnect_reason = "invalid_subscribe_payload";
                    break;
                };
                if enforce_guild_ip_ban_for_request(
                    &state,
                    &subscribe.guild_id,
                    auth.user_id,
                    client_ip,
                    "gateway.subscribe",
                )
                .await
                .is_err()
                {
                    disconnect_reason = "ip_banned";
                    break;
                }
                if !user_can_write_channel(
                    &state,
                    auth.user_id,
                    &subscribe.guild_id,
                    &subscribe.channel_id,
                )
                .await
                {
                    disconnect_reason = "forbidden_channel";
                    break;
                }

                add_subscription(
                    &state,
                    connection_id,
                    channel_key(&subscribe.guild_id, &subscribe.channel_id),
                    outbound_tx.clone(),
                )
                .await;
                handle_presence_subscribe(
                    &state,
                    connection_id,
                    auth.user_id,
                    &subscribe.guild_id,
                    &outbound_tx,
                )
                .await;

                let subscribed_event =
                    gateway_events::subscribed(&subscribe.guild_id, &subscribe.channel_id);
                if outbound_tx.try_send(subscribed_event.payload).is_err() {
                    record_gateway_event_dropped(
                        "connection",
                        subscribed_event.event_type,
                        "full_queue",
                    );
                    disconnect_reason = "outbound_queue_full";
                    break;
                }
                record_gateway_event_emitted("connection", subscribed_event.event_type);
                handle_voice_subscribe(
                    &state,
                    &subscribe.guild_id,
                    &subscribe.channel_id,
                    &outbound_tx,
                )
                .await;
            }
            "message_create" => {
                let Ok(request) = serde_json::from_value::<GatewayMessageCreate>(envelope.d) else {
                    record_gateway_event_parse_rejected(
                        "ingress",
                        "invalid_message_create_payload",
                    );
                    disconnect_reason = "invalid_message_create_payload";
                    break;
                };
                if enforce_guild_ip_ban_for_request(
                    &state,
                    &request.guild_id,
                    auth.user_id,
                    client_ip,
                    "gateway.message_create",
                )
                .await
                .is_err()
                {
                    disconnect_reason = "ip_banned";
                    break;
                }
                if create_message_internal(
                    &state,
                    &auth,
                    &request.guild_id,
                    &request.channel_id,
                    request.content,
                    request.attachment_ids.unwrap_or_default(),
                )
                .await
                .is_err()
                {
                    disconnect_reason = "message_rejected";
                    break;
                }
            }
            _ => {
                record_gateway_event_unknown_received("ingress", envelope.t.as_str());
                disconnect_reason = "unknown_event";
                break;
            }
        }
    }

    if !slow_consumer_disconnect.load(Ordering::Relaxed) {
        record_ws_disconnect(disconnect_reason);
    }
    remove_connection(&state, connection_id).await;
    send_task.abort();
}

#[allow(clippy::too_many_lines)]
pub(crate) async fn create_message_internal(
    state: &AppState,
    auth: &AuthContext,
    guild_id: &str,
    channel_id: &str,
    content: String,
    attachment_ids: Vec<String>,
) -> Result<MessageResponse, AuthFailure> {
    let attachment_ids = parse_attachment_ids(attachment_ids)?;
    if content.is_empty() {
        if attachment_ids.is_empty() {
            return Err(AuthFailure::InvalidRequest);
        }
    } else {
        validate_message_content(&content)?;
    }
    let markdown_tokens = if content.is_empty() {
        Vec::new()
    } else {
        tokenize_markdown(&content)
    };
    let (_, permissions) =
        channel_permission_snapshot(state, auth.user_id, guild_id, channel_id).await?;
    if !permissions.contains(Permission::CreateMessage) {
        return Err(AuthFailure::Forbidden);
    }

    if let Some(pool) = &state.db_pool {
        ensure_db_schema(state).await?;
        let message_id = Ulid::new().to_string();
        let created_at_unix = now_unix();
        let mut tx = pool.begin().await.map_err(|_| AuthFailure::Internal)?;
        sqlx::query(
            "INSERT INTO messages (message_id, guild_id, channel_id, author_id, content, created_at_unix)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(&message_id)
        .bind(guild_id)
        .bind(channel_id)
        .bind(auth.user_id.to_string())
        .bind(&content)
        .bind(created_at_unix)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            if matches!(e, sqlx::Error::Database(_)) {
                AuthFailure::NotFound
            } else {
                AuthFailure::Internal
            }
        })?;

        bind_message_attachments_db(
            &mut tx,
            &attachment_ids,
            &message_id,
            guild_id,
            channel_id,
            auth.user_id,
        )
        .await?;
        let attachments =
            fetch_attachments_for_message_db(&mut tx, guild_id, channel_id, &message_id).await?;
        tx.commit().await.map_err(|_| AuthFailure::Internal)?;

        let response = MessageResponse {
            message_id,
            guild_id: guild_id.to_owned(),
            channel_id: channel_id.to_owned(),
            author_id: auth.user_id.to_string(),
            content,
            markdown_tokens,
            attachments,
            reactions: Vec::new(),
            created_at_unix,
        };

        let event = gateway_events::message_create(&response);
        broadcast_channel_event(state, &channel_key(guild_id, channel_id), &event).await;
        enqueue_search_operation(
            state,
            SearchOperation::Upsert(indexed_message_from_response(&response)),
            true,
        )
        .await?;
        return Ok(response);
    }

    let mut guilds = state.guilds.write().await;
    let guild = guilds.get_mut(guild_id).ok_or(AuthFailure::NotFound)?;
    let channel = guild
        .channels
        .get_mut(channel_id)
        .ok_or(AuthFailure::NotFound)?;

    let message_id = Ulid::new().to_string();
    let record = MessageRecord {
        id: message_id.clone(),
        author_id: auth.user_id,
        content,
        markdown_tokens: markdown_tokens.clone(),
        attachment_ids: attachment_ids.clone(),
        created_at_unix: now_unix(),
        reactions: HashMap::new(),
    };
    if !attachment_ids.is_empty() {
        let mut attachments = state.attachments.write().await;
        for attachment_id in &attachment_ids {
            let Some(attachment) = attachments.get_mut(attachment_id) else {
                return Err(AuthFailure::InvalidRequest);
            };
            if attachment.guild_id != guild_id
                || attachment.channel_id != channel_id
                || attachment.owner_id != auth.user_id
                || attachment.message_id.is_some()
            {
                return Err(AuthFailure::InvalidRequest);
            }
            attachment.message_id = Some(message_id.clone());
        }
    }
    channel.messages.push(record.clone());
    drop(guilds);

    let attachments = attachments_for_message_in_memory(state, &record.attachment_ids).await?;
    let response = MessageResponse {
        message_id,
        guild_id: guild_id.to_owned(),
        channel_id: channel_id.to_owned(),
        author_id: auth.user_id.to_string(),
        content: record.content,
        markdown_tokens: record.markdown_tokens,
        attachments,
        reactions: reaction_summaries_from_users(&record.reactions),
        created_at_unix: record.created_at_unix,
    };

    let event = gateway_events::message_create(&response);
    broadcast_channel_event(state, &channel_key(guild_id, channel_id), &event).await;
    enqueue_search_operation(
        state,
        SearchOperation::Upsert(indexed_message_from_response(&response)),
        true,
    )
    .await?;

    Ok(response)
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
                let mut batch = vec![command];
                while batch.len() < 128 {
                    let Ok(next) = rx.try_recv() else {
                        break;
                    };
                    batch.push(next);
                }
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
            apply_search_operation(search, &mut writer, op);
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
    fn upsert_doc(
        search: &SearchIndexState,
        writer: &mut tantivy::IndexWriter,
        doc: IndexedMessage,
    ) {
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
    let raw = query.q.trim();
    if raw.is_empty() || raw.len() > state.runtime.search_query_max_chars {
        return Err(AuthFailure::InvalidRequest);
    }
    let limit = query.limit.unwrap_or(DEFAULT_SEARCH_RESULT_LIMIT);
    if limit == 0 || limit > state.runtime.search_result_limit_max {
        return Err(AuthFailure::InvalidRequest);
    }
    if raw.split_whitespace().count() > MAX_SEARCH_TERMS {
        return Err(AuthFailure::InvalidRequest);
    }
    let wildcard_count = raw.matches('*').count() + raw.matches('?').count();
    if wildcard_count > MAX_SEARCH_WILDCARDS {
        return Err(AuthFailure::InvalidRequest);
    }
    if raw.matches('~').count() > MAX_SEARCH_FUZZY {
        return Err(AuthFailure::InvalidRequest);
    }
    if raw.contains(':') {
        return Err(AuthFailure::InvalidRequest);
    }
    Ok(())
}

pub(crate) async fn ensure_search_bootstrapped(state: &AppState) -> Result<(), AuthFailure> {
    state
        .search_bootstrapped
        .get_or_try_init(|| async move {
            let docs = collect_all_indexed_messages(state).await?;
            enqueue_search_operation(state, SearchOperation::Rebuild { docs }, true).await?;
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
    if wait_for_apply {
        let (ack_tx, ack_rx) = oneshot::channel();
        state
            .search
            .tx
            .send(SearchCommand {
                op,
                ack: Some(ack_tx),
            })
            .await
            .map_err(|_| AuthFailure::Internal)?;
        ack_rx.await.map_err(|_| AuthFailure::Internal)?
    } else {
        state
            .search
            .tx
            .send(SearchCommand { op, ack: None })
            .await
            .map_err(|_| AuthFailure::Internal)
    }
}

pub(crate) async fn collect_all_indexed_messages(
    state: &AppState,
) -> Result<Vec<IndexedMessage>, AuthFailure> {
    if let Some(pool) = &state.db_pool {
        let rows = sqlx::query(
            "SELECT message_id, guild_id, channel_id, author_id, content, created_at_unix
             FROM messages",
        )
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        let mut docs = Vec::with_capacity(rows.len());
        for row in rows {
            docs.push(IndexedMessage {
                message_id: row
                    .try_get("message_id")
                    .map_err(|_| AuthFailure::Internal)?,
                guild_id: row.try_get("guild_id").map_err(|_| AuthFailure::Internal)?,
                channel_id: row
                    .try_get("channel_id")
                    .map_err(|_| AuthFailure::Internal)?,
                author_id: row
                    .try_get("author_id")
                    .map_err(|_| AuthFailure::Internal)?,
                content: row.try_get("content").map_err(|_| AuthFailure::Internal)?,
                created_at_unix: row
                    .try_get("created_at_unix")
                    .map_err(|_| AuthFailure::Internal)?,
            });
        }
        return Ok(docs);
    }

    let guilds = state.guilds.read().await;
    let mut docs = Vec::new();
    for (guild_id, guild) in &*guilds {
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
    Ok(docs)
}

pub(crate) async fn collect_indexed_messages_for_guild(
    state: &AppState,
    guild_id: &str,
    max_docs: usize,
) -> Result<Vec<IndexedMessage>, AuthFailure> {
    if let Some(pool) = &state.db_pool {
        let limit =
            i64::try_from(max_docs.saturating_add(1)).map_err(|_| AuthFailure::InvalidRequest)?;
        let rows = sqlx::query(
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
        if rows.len() > max_docs {
            return Err(AuthFailure::InvalidRequest);
        }
        let mut docs = Vec::with_capacity(rows.len());
        for row in rows {
            docs.push(IndexedMessage {
                message_id: row
                    .try_get("message_id")
                    .map_err(|_| AuthFailure::Internal)?,
                guild_id: row.try_get("guild_id").map_err(|_| AuthFailure::Internal)?,
                channel_id: row
                    .try_get("channel_id")
                    .map_err(|_| AuthFailure::Internal)?,
                author_id: row
                    .try_get("author_id")
                    .map_err(|_| AuthFailure::Internal)?,
                content: row.try_get("content").map_err(|_| AuthFailure::Internal)?,
                created_at_unix: row
                    .try_get("created_at_unix")
                    .map_err(|_| AuthFailure::Internal)?,
            });
        }
        return Ok(docs);
    }

    let guilds = state.guilds.read().await;
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

pub(crate) async fn collect_index_message_ids_for_guild(
    state: &AppState,
    guild_id: &str,
    max_docs: usize,
) -> Result<HashSet<String>, AuthFailure> {
    let guild = guild_id.to_owned();
    let search_state = state.search.state.clone();
    let timeout = state.runtime.search_query_timeout;

    tokio::time::timeout(timeout, async move {
        tokio::task::spawn_blocking(move || {
            let searcher = search_state.reader.searcher();
            let guild_query = TermQuery::new(
                Term::from_field_text(search_state.fields.guild_id, &guild),
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
            Ok::<HashSet<String>, AuthFailure>(message_ids)
        })
        .await
        .map_err(|_| AuthFailure::Internal)?
    })
    .await
    .map_err(|_| AuthFailure::InvalidRequest)?
}

pub(crate) async fn plan_search_reconciliation(
    state: &AppState,
    guild_id: &str,
    max_docs: usize,
) -> Result<(Vec<IndexedMessage>, Vec<String>), AuthFailure> {
    let source_docs = collect_indexed_messages_for_guild(state, guild_id, max_docs).await?;
    let index_ids = collect_index_message_ids_for_guild(state, guild_id, max_docs).await?;
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
    Ok((upserts, delete_message_ids))
}

pub(crate) async fn run_search_query(
    state: &AppState,
    guild_id: &str,
    channel_id: Option<&str>,
    raw_query: &str,
    limit: usize,
) -> Result<Vec<String>, AuthFailure> {
    let query = raw_query.trim().to_owned();
    let guild = guild_id.to_owned();
    let channel = channel_id.map(ToOwned::to_owned);
    let search_state = state.search.state.clone();
    let timeout = state.runtime.search_query_timeout;

    tokio::time::timeout(timeout, async move {
        tokio::task::spawn_blocking(move || {
            let searcher = search_state.reader.searcher();
            let parser =
                QueryParser::for_index(&search_state.index, vec![search_state.fields.content]);
            let parsed = parser
                .parse_query(&query)
                .map_err(|_| AuthFailure::InvalidRequest)?;
            let mut clauses = vec![
                (
                    Occur::Must,
                    Box::new(TermQuery::new(
                        Term::from_field_text(search_state.fields.guild_id, &guild),
                        IndexRecordOption::Basic,
                    )) as Box<dyn tantivy::query::Query>,
                ),
                (Occur::Must, parsed),
            ];
            if let Some(channel_id) = channel {
                clauses.push((
                    Occur::Must,
                    Box::new(TermQuery::new(
                        Term::from_field_text(search_state.fields.channel_id, &channel_id),
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
            Ok::<Vec<String>, AuthFailure>(message_ids)
        })
        .await
        .map_err(|_| AuthFailure::Internal)?
    })
    .await
    .map_err(|_| AuthFailure::InvalidRequest)?
}

#[allow(clippy::too_many_lines)]
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
        let rows = if let Some(channel_id) = channel_id {
            sqlx::query(
                "SELECT message_id, guild_id, channel_id, author_id, content, created_at_unix
                 FROM messages
                 WHERE guild_id = $1 AND channel_id = $2 AND message_id = ANY($3::text[])",
            )
            .bind(guild_id)
            .bind(channel_id)
            .bind(message_ids)
            .fetch_all(pool)
            .await
            .map_err(|_| AuthFailure::Internal)?
        } else {
            sqlx::query(
                "SELECT message_id, guild_id, channel_id, author_id, content, created_at_unix
                 FROM messages
                 WHERE guild_id = $1 AND message_id = ANY($2::text[])",
            )
            .bind(guild_id)
            .bind(message_ids)
            .fetch_all(pool)
            .await
            .map_err(|_| AuthFailure::Internal)?
        };

        let mut by_id = HashMap::with_capacity(rows.len());
        for row in rows {
            let message_id: String = row
                .try_get("message_id")
                .map_err(|_| AuthFailure::Internal)?;
            let guild_id: String = row.try_get("guild_id").map_err(|_| AuthFailure::Internal)?;
            let channel_id: String = row
                .try_get("channel_id")
                .map_err(|_| AuthFailure::Internal)?;
            let author_id: String = row
                .try_get("author_id")
                .map_err(|_| AuthFailure::Internal)?;
            let content: String = row.try_get("content").map_err(|_| AuthFailure::Internal)?;
            let created_at_unix: i64 = row
                .try_get("created_at_unix")
                .map_err(|_| AuthFailure::Internal)?;
            by_id.insert(
                message_id.clone(),
                MessageResponse {
                    message_id,
                    guild_id,
                    channel_id,
                    author_id,
                    markdown_tokens: tokenize_markdown(&content),
                    content,
                    attachments: Vec::new(),
                    reactions: Vec::new(),
                    created_at_unix,
                },
            );
        }

        let message_ids_ordered: Vec<String> = message_ids.to_vec();
        let attachment_map =
            attachment_map_for_messages_db(pool, guild_id, channel_id, &message_ids_ordered)
                .await?;
        let reaction_map =
            reaction_map_for_messages_db(pool, guild_id, channel_id, &message_ids_ordered).await?;
        for (id, message) in &mut by_id {
            message.attachments = attachment_map.get(id).cloned().unwrap_or_default();
            message.reactions = reaction_map.get(id).cloned().unwrap_or_default();
        }

        let mut hydrated = Vec::with_capacity(message_ids.len());
        for message_id in message_ids {
            if let Some(message) = by_id.remove(message_id) {
                hydrated.push(message);
            }
        }
        return Ok(hydrated);
    }

    let guilds = state.guilds.read().await;
    let guild = guilds.get(guild_id).ok_or(AuthFailure::NotFound)?;
    let mut by_id = HashMap::new();
    if let Some(channel_id) = channel_id {
        let channel = guild
            .channels
            .get(channel_id)
            .ok_or(AuthFailure::NotFound)?;
        for message in &channel.messages {
            by_id.insert(
                message.id.clone(),
                MessageResponse {
                    message_id: message.id.clone(),
                    guild_id: guild_id.to_owned(),
                    channel_id: channel_id.to_owned(),
                    author_id: message.author_id.to_string(),
                    content: message.content.clone(),
                    markdown_tokens: message.markdown_tokens.clone(),
                    attachments: Vec::new(),
                    reactions: reaction_summaries_from_users(&message.reactions),
                    created_at_unix: message.created_at_unix,
                },
            );
        }
    } else {
        for (channel_id, channel) in &guild.channels {
            for message in &channel.messages {
                by_id.insert(
                    message.id.clone(),
                    MessageResponse {
                        message_id: message.id.clone(),
                        guild_id: guild_id.to_owned(),
                        channel_id: channel_id.clone(),
                        author_id: message.author_id.to_string(),
                        content: message.content.clone(),
                        markdown_tokens: message.markdown_tokens.clone(),
                        attachments: Vec::new(),
                        reactions: reaction_summaries_from_users(&message.reactions),
                        created_at_unix: message.created_at_unix,
                    },
                );
            }
        }
    }

    let attachment_map =
        attachment_map_for_messages_in_memory(state, guild_id, channel_id, message_ids).await;
    for (id, message) in &mut by_id {
        message.attachments = attachment_map.get(id).cloned().unwrap_or_default();
    }

    let mut hydrated = Vec::with_capacity(message_ids.len());
    for message_id in message_ids {
        if let Some(message) = by_id.remove(message_id) {
            hydrated.push(message);
        }
    }
    Ok(hydrated)
}

fn dispatch_gateway_payload(
    listeners: &mut HashMap<Uuid, mpsc::Sender<String>>,
    payload: &str,
    event_type: &'static str,
    scope: &'static str,
    slow_connections: &mut Vec<Uuid>,
) -> usize {
    let mut delivered = 0usize;
    listeners.retain(
        |connection_id, sender| match sender.try_send(payload.to_owned()) {
            Ok(()) => {
                delivered += 1;
                true
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                record_gateway_event_dropped(scope, event_type, "closed");
                false
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                record_gateway_event_dropped(scope, event_type, "full_queue");
                slow_connections.push(*connection_id);
                false
            }
        },
    );
    delivered
}

async fn close_slow_connections(state: &AppState, slow_connections: Vec<Uuid>) {
    if slow_connections.is_empty() {
        return;
    }

    let controls = state.connection_controls.read().await;
    for connection_id in slow_connections {
        if let Some(control) = controls.get(&connection_id) {
            let _ = control.send(ConnectionControl::Close);
        }
    }
}

pub(crate) async fn broadcast_channel_event(state: &AppState, key: &str, event: &GatewayEvent) {
    let mut slow_connections = Vec::new();
    let mut delivered = 0usize;
    let mut subscriptions = state.subscriptions.write().await;
    if let Some(listeners) = subscriptions.get_mut(key) {
        delivered = dispatch_gateway_payload(
            listeners,
            &event.payload,
            event.event_type,
            "channel",
            &mut slow_connections,
        );
        if listeners.is_empty() {
            subscriptions.remove(key);
        }
    }
    drop(subscriptions);

    close_slow_connections(state, slow_connections).await;
    if delivered > 0 {
        tracing::debug!(
            event = "gateway.event.emit",
            scope = "channel",
            event_type = event.event_type,
            delivered
        );
        for _ in 0..delivered {
            record_gateway_event_emitted("channel", event.event_type);
        }
    }
}

pub(crate) async fn broadcast_guild_event(state: &AppState, guild_id: &str, event: &GatewayEvent) {
    let mut slow_connections = Vec::new();
    let mut seen_connections = HashSet::new();
    let mut delivered = 0usize;
    let mut subscriptions = state.subscriptions.write().await;
    for (key, listeners) in subscriptions.iter_mut() {
        if !key.starts_with(guild_id) || !key[guild_id.len()..].starts_with(':') {
            continue;
        }
        let mut stale_connections = Vec::new();
        for (connection_id, sender) in listeners.iter() {
            if !seen_connections.insert(*connection_id) {
                continue;
            }
            match sender.try_send(event.payload.clone()) {
                Ok(()) => delivered += 1,
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    record_gateway_event_dropped("guild", event.event_type, "closed");
                    stale_connections.push(*connection_id);
                }
                Err(mpsc::error::TrySendError::Full(_)) => {
                    record_gateway_event_dropped("guild", event.event_type, "full_queue");
                    slow_connections.push(*connection_id);
                    stale_connections.push(*connection_id);
                }
            }
        }
        for connection_id in stale_connections {
            listeners.remove(&connection_id);
        }
    }
    subscriptions.retain(|_, listeners| !listeners.is_empty());
    drop(subscriptions);

    close_slow_connections(state, slow_connections).await;
    if delivered > 0 {
        tracing::debug!(
            event = "gateway.event.emit",
            scope = "guild",
            event_type = event.event_type,
            delivered
        );
        for _ in 0..delivered {
            record_gateway_event_emitted("guild", event.event_type);
        }
    }
}

#[allow(dead_code)]
pub(crate) async fn broadcast_user_event(state: &AppState, user_id: UserId, event: &GatewayEvent) {
    let connection_ids: Vec<Uuid> = state
        .connection_presence
        .read()
        .await
        .iter()
        .filter_map(|(connection_id, presence)| {
            (presence.user_id == user_id).then_some(*connection_id)
        })
        .collect();
    if connection_ids.is_empty() {
        return;
    }

    let mut slow_connections = Vec::new();
    let mut delivered = 0usize;
    let mut senders = state.connection_senders.write().await;
    for connection_id in connection_ids {
        let Some(sender) = senders.get(&connection_id) else {
            continue;
        };
        match sender.try_send(event.payload.clone()) {
            Ok(()) => delivered += 1,
            Err(mpsc::error::TrySendError::Closed(_)) => {
                record_gateway_event_dropped("user", event.event_type, "closed");
                senders.remove(&connection_id);
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                record_gateway_event_dropped("user", event.event_type, "full_queue");
                slow_connections.push(connection_id);
                senders.remove(&connection_id);
            }
        }
    }
    drop(senders);

    close_slow_connections(state, slow_connections).await;
    if delivered > 0 {
        tracing::debug!(
            event = "gateway.event.emit",
            scope = "user",
            event_type = event.event_type,
            delivered
        );
        for _ in 0..delivered {
            record_gateway_event_emitted("user", event.event_type);
        }
    }
}

fn voice_channel_key(guild_id: &str, channel_id: &str) -> String {
    format!("{guild_id}:{channel_id}")
}

fn voice_snapshot_from_record(
    participant: &VoiceParticipant,
) -> gateway_events::VoiceParticipantSnapshot {
    gateway_events::VoiceParticipantSnapshot {
        user_id: participant.user_id,
        identity: participant.identity.clone(),
        joined_at_unix: participant.joined_at_unix,
        updated_at_unix: participant.updated_at_unix,
        is_muted: participant.is_muted,
        is_deafened: participant.is_deafened,
        is_speaking: participant.is_speaking,
        is_video_enabled: participant.is_video_enabled,
        is_screen_share_enabled: participant.is_screen_share_enabled,
    }
}

async fn prune_expired_voice_participants(state: &AppState, now_unix: i64) {
    let mut removed = Vec::new();
    {
        let mut voice = state.voice_participants.write().await;
        voice.retain(|channel_key, participants| {
            participants.retain(|_, participant| {
                if participant.expires_at_unix > now_unix {
                    return true;
                }
                removed.push((channel_key.clone(), participant.clone()));
                false
            });
            !participants.is_empty()
        });
    }

    for (key, participant) in removed {
        let Some((guild_id, channel_id)) = key.split_once(':') else {
            continue;
        };
        for stream in participant.published_streams {
            let event = gateway_events::voice_stream_unpublish(
                guild_id,
                channel_id,
                participant.user_id,
                &participant.identity,
                stream,
                now_unix,
            );
            broadcast_channel_event(state, &channel_key(guild_id, channel_id), &event).await;
        }
        let leave = gateway_events::voice_participant_leave(
            guild_id,
            channel_id,
            participant.user_id,
            &participant.identity,
            now_unix,
        );
        broadcast_channel_event(state, &channel_key(guild_id, channel_id), &leave).await;
    }
}

pub(crate) async fn register_voice_participant_from_token(
    state: &AppState,
    user_id: UserId,
    guild_id: &str,
    channel_id: &str,
    identity: &str,
    publish_streams: &[VoiceStreamKind],
    expires_at_unix: i64,
) -> Result<(), AuthFailure> {
    prune_expired_voice_participants(state, now_unix()).await;
    let now = now_unix();
    let key = voice_channel_key(guild_id, channel_id);
    let mut removed = Vec::new();
    let mut joined = None;
    let mut updated = None;
    let mut newly_published = Vec::new();
    let mut unpublished = Vec::new();

    {
        let mut channels = state.voice_participants.write().await;
        for (existing_key, participants) in channels.iter_mut() {
            if existing_key == &key {
                continue;
            }
            if let Some(existing) = participants.remove(&user_id) {
                removed.push((existing_key.clone(), existing));
            }
        }
        channels.retain(|_, participants| !participants.is_empty());
        if !channels.contains_key(&key) && channels.len() >= MAX_TRACKED_VOICE_CHANNELS {
            return Err(AuthFailure::RateLimited);
        }
        let channel_participants = channels.entry(key.clone()).or_default();
        if !channel_participants.contains_key(&user_id)
            && channel_participants.len() >= MAX_TRACKED_VOICE_PARTICIPANTS_PER_CHANNEL
        {
            return Err(AuthFailure::RateLimited);
        }

        let next_streams: HashSet<VoiceStreamKind> = publish_streams.iter().copied().collect();
        let next_video = next_streams.contains(&VoiceStreamKind::Camera);
        let next_screen = next_streams.contains(&VoiceStreamKind::ScreenShare);
        if let Some(existing) = channel_participants.get_mut(&user_id) {
            let prev_streams = existing.published_streams.clone();
            for stream in next_streams.difference(&prev_streams) {
                newly_published.push(*stream);
            }
            for stream in prev_streams.difference(&next_streams) {
                unpublished.push(*stream);
            }
            existing.identity = identity.to_owned();
            existing.updated_at_unix = now;
            existing.expires_at_unix = expires_at_unix;
            existing.is_video_enabled = next_video;
            existing.is_screen_share_enabled = next_screen;
            existing.published_streams = next_streams;
            updated = Some(existing.clone());
        } else {
            let participant = VoiceParticipant {
                user_id,
                identity: identity.to_owned(),
                joined_at_unix: now,
                updated_at_unix: now,
                expires_at_unix,
                is_muted: false,
                is_deafened: false,
                is_speaking: false,
                is_video_enabled: next_video,
                is_screen_share_enabled: next_screen,
                published_streams: next_streams.clone(),
            };
            joined = Some(participant.clone());
            newly_published.extend(next_streams);
            channel_participants.insert(user_id, participant);
        }
    }

    for (old_key, participant) in removed {
        let Some((old_guild_id, old_channel_id)) = old_key.split_once(':') else {
            continue;
        };
        for stream in participant.published_streams {
            let event = gateway_events::voice_stream_unpublish(
                old_guild_id,
                old_channel_id,
                participant.user_id,
                &participant.identity,
                stream,
                now,
            );
            broadcast_channel_event(state, &channel_key(old_guild_id, old_channel_id), &event)
                .await;
        }
        let leave = gateway_events::voice_participant_leave(
            old_guild_id,
            old_channel_id,
            participant.user_id,
            &participant.identity,
            now,
        );
        broadcast_channel_event(state, &channel_key(old_guild_id, old_channel_id), &leave).await;
    }

    if let Some(participant) = joined {
        let event = gateway_events::voice_participant_join(
            guild_id,
            channel_id,
            voice_snapshot_from_record(&participant),
        );
        broadcast_channel_event(state, &channel_key(guild_id, channel_id), &event).await;
    }
    if let Some(participant) = updated {
        let event = gateway_events::voice_participant_update(
            guild_id,
            channel_id,
            participant.user_id,
            &participant.identity,
            None,
            None,
            Some(participant.is_speaking),
            Some(participant.is_video_enabled),
            Some(participant.is_screen_share_enabled),
            participant.updated_at_unix,
        );
        broadcast_channel_event(state, &channel_key(guild_id, channel_id), &event).await;
    }
    for stream in unpublished {
        let event = gateway_events::voice_stream_unpublish(
            guild_id, channel_id, user_id, identity, stream, now,
        );
        broadcast_channel_event(state, &channel_key(guild_id, channel_id), &event).await;
    }
    for stream in newly_published {
        let event = gateway_events::voice_stream_publish(
            guild_id, channel_id, user_id, identity, stream, now,
        );
        broadcast_channel_event(state, &channel_key(guild_id, channel_id), &event).await;
    }

    Ok(())
}

pub(crate) async fn handle_voice_subscribe(
    state: &AppState,
    guild_id: &str,
    channel_id: &str,
    outbound_tx: &mpsc::Sender<String>,
) {
    prune_expired_voice_participants(state, now_unix()).await;
    let key = voice_channel_key(guild_id, channel_id);
    let participants = {
        let voice = state.voice_participants.read().await;
        let mut list = Vec::new();
        if let Some(channel_participants) = voice.get(&key) {
            list.extend(
                channel_participants
                    .values()
                    .map(voice_snapshot_from_record),
            );
        }
        list.sort_by(|a, b| {
            a.joined_at_unix
                .cmp(&b.joined_at_unix)
                .then(a.identity.cmp(&b.identity))
        });
        list
    };

    let sync_event =
        gateway_events::voice_participant_sync(guild_id, channel_id, participants, now_unix());
    match outbound_tx.try_send(sync_event.payload) {
        Ok(()) => {
            record_gateway_event_emitted("connection", sync_event.event_type);
            record_voice_sync_repair("subscribe");
        }
        Err(mpsc::error::TrySendError::Closed(_)) => {
            record_gateway_event_dropped("connection", sync_event.event_type, "closed");
        }
        Err(mpsc::error::TrySendError::Full(_)) => {
            record_gateway_event_dropped("connection", sync_event.event_type, "full_queue");
        }
    }
}

async fn remove_disconnected_user_voice_participants(
    state: &AppState,
    user_id: UserId,
    disconnected_at_unix: i64,
) {
    let mut removed = Vec::new();
    {
        let mut voice = state.voice_participants.write().await;
        voice.retain(|channel_key, participants| {
            if let Some(participant) = participants.remove(&user_id) {
                removed.push((channel_key.clone(), participant));
            }
            !participants.is_empty()
        });
    }

    for (channel_key_value, participant) in removed {
        let Some((guild_id, channel_id)) = channel_key_value.split_once(':') else {
            continue;
        };
        for stream in participant.published_streams {
            let unpublish = gateway_events::voice_stream_unpublish(
                guild_id,
                channel_id,
                participant.user_id,
                &participant.identity,
                stream,
                disconnected_at_unix,
            );
            broadcast_channel_event(state, &channel_key(guild_id, channel_id), &unpublish).await;
        }
        let leave = gateway_events::voice_participant_leave(
            guild_id,
            channel_id,
            participant.user_id,
            &participant.identity,
            disconnected_at_unix,
        );
        broadcast_channel_event(state, &channel_key(guild_id, channel_id), &leave).await;
    }
}

pub(crate) async fn handle_presence_subscribe(
    state: &AppState,
    connection_id: Uuid,
    user_id: UserId,
    guild_id: &str,
    outbound_tx: &mpsc::Sender<String>,
) {
    let (snapshot_user_ids, became_online) = {
        let mut presence = state.connection_presence.write().await;
        let guild = guild_id.to_owned();
        let Some(existing) = presence.get(&connection_id) else {
            return;
        };
        let already_subscribed = existing.guild_ids.contains(&guild);
        let was_online = presence
            .values()
            .any(|entry| entry.user_id == user_id && entry.guild_ids.contains(&guild));
        if let Some(connection) = presence.get_mut(&connection_id) {
            connection.guild_ids.insert(guild.clone());
        }
        let snapshot = presence
            .values()
            .filter(|entry| entry.guild_ids.contains(&guild))
            .map(|entry| entry.user_id.to_string())
            .collect::<HashSet<_>>();
        (snapshot, !was_online && !already_subscribed)
    };

    let snapshot_event = gateway_events::presence_sync(guild_id, snapshot_user_ids);
    match outbound_tx.try_send(snapshot_event.payload) {
        Ok(()) => record_gateway_event_emitted("connection", snapshot_event.event_type),
        Err(mpsc::error::TrySendError::Closed(_)) => {
            record_gateway_event_dropped("connection", snapshot_event.event_type, "closed");
        }
        Err(mpsc::error::TrySendError::Full(_)) => {
            record_gateway_event_dropped("connection", snapshot_event.event_type, "full_queue");
        }
    }

    if became_online {
        let update = gateway_events::presence_update(guild_id, user_id, "online");
        broadcast_guild_event(state, guild_id, &update).await;
    }
}

pub(crate) async fn add_subscription(
    state: &AppState,
    connection_id: Uuid,
    key: String,
    outbound_tx: mpsc::Sender<String>,
) {
    let mut subscriptions = state.subscriptions.write().await;
    subscriptions
        .entry(key)
        .or_default()
        .insert(connection_id, outbound_tx);
}

pub(crate) async fn remove_connection(state: &AppState, connection_id: Uuid) {
    let removed_presence = state
        .connection_presence
        .write()
        .await
        .remove(&connection_id);
    state
        .connection_controls
        .write()
        .await
        .remove(&connection_id);
    state
        .connection_senders
        .write()
        .await
        .remove(&connection_id);

    let mut subscriptions = state.subscriptions.write().await;
    subscriptions.retain(|_, listeners| {
        listeners.remove(&connection_id);
        !listeners.is_empty()
    });
    drop(subscriptions);

    let Some(removed_presence) = removed_presence else {
        return;
    };
    let remaining = state.connection_presence.read().await;
    let mut offline_guilds = Vec::new();
    let user_has_other_connections = remaining
        .values()
        .any(|entry| entry.user_id == removed_presence.user_id);
    for guild_id in &removed_presence.guild_ids {
        let still_online = remaining.values().any(|entry| {
            entry.user_id == removed_presence.user_id && entry.guild_ids.contains(guild_id)
        });
        if !still_online {
            offline_guilds.push(guild_id.clone());
        }
    }
    drop(remaining);

    if !user_has_other_connections {
        remove_disconnected_user_voice_participants(state, removed_presence.user_id, now_unix())
            .await;
    }

    for guild_id in offline_guilds {
        let update =
            gateway_events::presence_update(&guild_id, removed_presence.user_id, "offline");
        broadcast_guild_event(state, &guild_id, &update).await;
    }
}

pub(crate) fn allow_gateway_ingress(
    ingress: &mut VecDeque<Instant>,
    limit: u32,
    window: Duration,
) -> bool {
    let now = Instant::now();
    while ingress
        .front()
        .is_some_and(|oldest| now.duration_since(*oldest) > window)
    {
        let _ = ingress.pop_front();
    }

    if ingress.len() >= limit as usize {
        return false;
    }

    ingress.push_back(now);
    true
}
