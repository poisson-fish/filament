mod voice_registration_events;
mod hydration_in_memory;
mod connection_subscriptions;
mod subscription_insert;
mod connection_registry;
mod hydration_in_memory_attachments;
mod search_batch_drain;
mod search_enqueue;
mod search_apply_batch;
mod subscribe_ack;
mod hydration_merge;
mod message_record;
mod presence_disconnect_events;
mod search_collect_db;
mod hydration_db;
mod message_create_response;
use std::{
    collections::{HashSet, VecDeque},
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
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
use filament_core::{Permission, UserId};
use filament_protocol::parse_envelope;
use futures_util::{SinkExt, StreamExt};
use tantivy::{
    schema::Schema,
};
use tokio::sync::{mpsc, watch};
use ulid::Ulid;
use uuid::Uuid;

mod ingress_rate_limit;
mod ingress_message;
mod fanout_dispatch;
mod fanout_guild;
mod fanout_user;
mod presence_disconnect;
mod presence_subscribe;
mod voice_presence;
mod voice_registry;
mod search_validation;
mod connection_control;
mod voice_registration;
mod search_reconcile;
mod hydration_order;
mod search_collect_all;
mod search_collect_guild;
mod search_indexed_message;
mod search_collect_index_ids;
mod search_query_exec;
mod ingress_command;
mod voice_cleanup_events;
mod fanout_user_targets;
mod search_schema;
mod search_apply;
mod message_prepare;
mod message_attachment_bind;
mod search_blocking;

use fanout_dispatch::dispatch_gateway_payload;
use fanout_guild::dispatch_guild_payload;
use fanout_user::dispatch_user_payload;
use ingress_message::{decode_gateway_ingress_message, GatewayIngressMessageDecode};
use ingress_rate_limit::allow_gateway_ingress;
use presence_disconnect::compute_disconnect_presence_outcome;
use presence_subscribe::{
    apply_presence_subscribe, build_presence_online_update,
    try_enqueue_presence_sync_event, PresenceSyncEnqueueResult,
};
use voice_presence::{
    collect_voice_snapshots, try_enqueue_voice_sync_event, voice_channel_key,
    OutboundEnqueueResult,
};
use voice_registration::apply_voice_registration_transition;
use voice_registry::{
    remove_user_voice_participant_removals, take_expired_voice_participant_removals,
};
use search_validation::validate_search_query_limits;
use connection_control::signal_slow_connections_close;
use search_reconcile::compute_reconciliation;
use hydration_order::collect_hydrated_in_request_order;
use ingress_command::{parse_gateway_ingress_command, GatewayIngressCommand};
use search_collect_all::collect_all_indexed_messages_in_memory;
use search_collect_guild::collect_indexed_messages_for_guild_in_memory;
use search_collect_index_ids::collect_index_message_ids_for_guild as collect_index_message_ids_for_guild_from_index;
use search_query_exec::run_search_query_against_index;
use fanout_user_targets::connection_ids_for_user;
use voice_cleanup_events::plan_voice_removal_broadcasts;
use search_schema::build_search_schema as build_search_schema_impl;
use search_apply::apply_search_operation as apply_search_operation_impl;
use message_prepare::prepare_message_body;
use message_attachment_bind::bind_message_attachments_in_memory;
use search_blocking::run_search_blocking_with_timeout;
use voice_registration_events::plan_voice_registration_events;
use hydration_in_memory::collect_hydrated_messages_in_memory;
use connection_subscriptions::remove_connection_from_subscriptions;
use subscription_insert::insert_connection_subscription;
use connection_registry::remove_connection_state;
use hydration_in_memory_attachments::apply_hydration_attachments;
use search_batch_drain::drain_search_batch;
use search_enqueue::enqueue_search_command;
use search_apply_batch::apply_search_batch_with_ack;
use subscribe_ack::{try_enqueue_subscribed_event, SubscribeAckEnqueueResult};
use hydration_merge::merge_hydration_maps;
use message_record::{build_in_memory_message_record, build_message_response_from_record};
use presence_disconnect_events::build_offline_presence_updates;
use search_collect_db::{
    collect_all_indexed_messages_rows, collect_indexed_messages_for_guild_rows,
};
use hydration_db::collect_hydrated_messages_db;
use message_create_response::build_db_created_message_response;

use super::{
    auth::{
        authenticate_with_token, bearer_token, channel_key, extract_client_ip, now_unix,
        ClientIp,
    },
    core::{
        AppState, AuthContext, ConnectionControl, ConnectionPresence, IndexedMessage,
        SearchCommand, SearchFields, SearchIndexState, SearchOperation,
        SearchService, VoiceStreamKind, DEFAULT_SEARCH_RESULT_LIMIT,
        MAX_TRACKED_VOICE_CHANNELS, MAX_TRACKED_VOICE_PARTICIPANTS_PER_CHANNEL,
        SEARCH_INDEX_QUEUE_CAPACITY,
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
    types::{GatewayAuthQuery, MessageResponse, SearchQuery},
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

        let payload: Vec<u8> = match decode_gateway_ingress_message(
            message,
            state.runtime.max_gateway_event_bytes,
        ) {
            GatewayIngressMessageDecode::Payload(payload) => payload,
            GatewayIngressMessageDecode::Continue => continue,
            GatewayIngressMessageDecode::Disconnect(reason) => {
                disconnect_reason = reason;
                break;
            }
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

        let command = match parse_gateway_ingress_command(envelope) {
            Ok(command) => command,
            Err(error) => {
                match &error {
                    ingress_command::GatewayIngressCommandParseError::InvalidSubscribePayload => {
                        record_gateway_event_parse_rejected(
                            "ingress",
                            "invalid_subscribe_payload",
                        );
                    }
                    ingress_command::GatewayIngressCommandParseError::InvalidMessageCreatePayload => {
                        record_gateway_event_parse_rejected(
                            "ingress",
                            "invalid_message_create_payload",
                        );
                    }
                    ingress_command::GatewayIngressCommandParseError::UnknownEventType(event_type) => {
                        record_gateway_event_unknown_received("ingress", event_type);
                    }
                }
                disconnect_reason = error.disconnect_reason();
                break;
            }
        };

        match command {
            GatewayIngressCommand::Subscribe(subscribe) => {
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
                match try_enqueue_subscribed_event(&outbound_tx, subscribed_event.payload) {
                    SubscribeAckEnqueueResult::Enqueued => {
                        record_gateway_event_emitted("connection", subscribed_event.event_type);
                    }
                    SubscribeAckEnqueueResult::Rejected => {
                        record_gateway_event_dropped(
                            "connection",
                            subscribed_event.event_type,
                            "full_queue",
                        );
                        disconnect_reason = "outbound_queue_full";
                        break;
                    }
                }
                handle_voice_subscribe(
                    &state,
                    &subscribe.guild_id,
                    &subscribe.channel_id,
                    &outbound_tx,
                )
                .await;
            }
            GatewayIngressCommand::MessageCreate(request) => {
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
    let prepared = prepare_message_body(content, !attachment_ids.is_empty())?;
    let content = prepared.content;
    let markdown_tokens = prepared.markdown_tokens;
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

        let response = build_db_created_message_response(
            message_id,
            guild_id,
            channel_id,
            auth.user_id,
            content,
            markdown_tokens,
            attachments,
            created_at_unix,
        );

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
    let created_at_unix = now_unix();
    let record = build_in_memory_message_record(
        message_id.clone(),
        auth.user_id,
        content,
        markdown_tokens.clone(),
        attachment_ids.clone(),
        created_at_unix,
    );
    if !attachment_ids.is_empty() {
        let mut attachments = state.attachments.write().await;
        bind_message_attachments_in_memory(
            &mut attachments,
            &attachment_ids,
            &message_id,
            guild_id,
            channel_id,
            auth.user_id,
        )?;
    }
    channel.messages.push(record.clone());
    drop(guilds);

    let attachments = attachments_for_message_in_memory(state, &record.attachment_ids).await?;
    let response = build_message_response_from_record(
        &record,
        guild_id,
        channel_id,
        attachments,
        reaction_summaries_from_users(&record.reactions),
    );

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
                let batch = drain_search_batch(command, &mut rx, 128);
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
    search_indexed_message::indexed_message_from_response(message)
}

pub(crate) fn validate_search_query(
    state: &AppState,
    query: &SearchQuery,
) -> Result<(), AuthFailure> {
    let raw = query.q.trim();
    let limit = query.limit.unwrap_or(DEFAULT_SEARCH_RESULT_LIMIT);
    validate_search_query_limits(
        raw,
        limit,
        state.runtime.search_query_max_chars,
        state.runtime.search_result_limit_max,
    )
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
    enqueue_search_command(&state.search.tx, op, wait_for_apply).await
}

pub(crate) async fn collect_all_indexed_messages(
    state: &AppState,
) -> Result<Vec<IndexedMessage>, AuthFailure> {
    if let Some(pool) = &state.db_pool {
        let rows = sqlx::query_as::<_, (String, String, String, String, String, i64)>(
            "SELECT message_id, guild_id, channel_id, author_id, content, created_at_unix
             FROM messages",
        )
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        return Ok(collect_all_indexed_messages_rows(rows));
    }

    let guilds = state.guilds.read().await;
    Ok(collect_all_indexed_messages_in_memory(&guilds))
}

pub(crate) async fn collect_indexed_messages_for_guild(
    state: &AppState,
    guild_id: &str,
    max_docs: usize,
) -> Result<Vec<IndexedMessage>, AuthFailure> {
    if let Some(pool) = &state.db_pool {
        let limit =
            i64::try_from(max_docs.saturating_add(1)).map_err(|_| AuthFailure::InvalidRequest)?;
        let rows = sqlx::query_as::<_, (String, String, String, String, String, i64)>(
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
        return Ok(collect_indexed_messages_for_guild_rows(rows));
    }

    let guilds = state.guilds.read().await;
    collect_indexed_messages_for_guild_in_memory(&guilds, guild_id, max_docs)
}

pub(crate) async fn collect_index_message_ids_for_guild(
    state: &AppState,
    guild_id: &str,
    max_docs: usize,
) -> Result<HashSet<String>, AuthFailure> {
    let guild = guild_id.to_owned();
    let search_state = state.search.state.clone();
    let timeout = state.runtime.search_query_timeout;

    run_search_blocking_with_timeout(timeout, move || {
            collect_index_message_ids_for_guild_from_index(&search_state, &guild, max_docs)
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
    let (upserts, delete_message_ids) = compute_reconciliation(source_docs, index_ids);
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

    run_search_blocking_with_timeout(timeout, move || {
            run_search_query_against_index(
                &search_state,
                &guild,
                channel.as_deref(),
                &query,
                limit,
            )
    })
    .await
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
        let mut by_id = collect_hydrated_messages_db(pool, guild_id, channel_id, message_ids)
            .await?;

        let message_ids_ordered: Vec<String> = message_ids.to_vec();
        let attachment_map =
            attachment_map_for_messages_db(pool, guild_id, channel_id, &message_ids_ordered)
                .await?;
        let reaction_map =
            reaction_map_for_messages_db(pool, guild_id, channel_id, &message_ids_ordered).await?;
        merge_hydration_maps(&mut by_id, &attachment_map, &reaction_map);

        let hydrated = collect_hydrated_in_request_order(by_id, message_ids);
        return Ok(hydrated);
    }

    let guilds = state.guilds.read().await;
    let guild = guilds.get(guild_id).ok_or(AuthFailure::NotFound)?;
    let mut by_id = collect_hydrated_messages_in_memory(guild, guild_id, channel_id)?;

    let attachment_map =
        attachment_map_for_messages_in_memory(state, guild_id, channel_id, message_ids).await;
    apply_hydration_attachments(&mut by_id, &attachment_map);

    let hydrated = collect_hydrated_in_request_order(by_id, message_ids);
    Ok(hydrated)
}

async fn close_slow_connections(state: &AppState, slow_connections: Vec<Uuid>) {
    if slow_connections.is_empty() {
        return;
    }

    let controls = state.connection_controls.read().await;
    signal_slow_connections_close(&controls, slow_connections);
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
    let mut subscriptions = state.subscriptions.write().await;
    let delivered = dispatch_guild_payload(
        &mut subscriptions,
        guild_id,
        &event.payload,
        event.event_type,
        &mut slow_connections,
    );
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
    let connection_ids = {
        let presence = state.connection_presence.read().await;
        connection_ids_for_user(&presence, user_id)
    };
    if connection_ids.is_empty() {
        return;
    }

    let mut slow_connections = Vec::new();
    let mut senders = state.connection_senders.write().await;
    let delivered = dispatch_user_payload(
        &mut senders,
        &connection_ids,
        &event.payload,
        event.event_type,
        &mut slow_connections,
    );
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

async fn prune_expired_voice_participants(state: &AppState, now_unix: i64) {
    let removed = {
        let mut voice = state.voice_participants.write().await;
        take_expired_voice_participant_removals(&mut voice, now_unix)
    };

    for (channel_subscription_key, event) in plan_voice_removal_broadcasts(removed, now_unix) {
        broadcast_channel_event(state, &channel_subscription_key, &event).await;
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
    let transition = {
        let mut channels = state.voice_participants.write().await;
        apply_voice_registration_transition(
            &mut channels,
            &key,
            user_id,
            identity,
            publish_streams,
            expires_at_unix,
            now,
            MAX_TRACKED_VOICE_CHANNELS,
            MAX_TRACKED_VOICE_PARTICIPANTS_PER_CHANNEL,
        )?
    };
    for (subscription_key, event) in plan_voice_registration_events(
        transition,
        guild_id,
        channel_id,
        user_id,
        identity,
        now,
    ) {
        broadcast_channel_event(state, &subscription_key, &event).await;
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
        collect_voice_snapshots(&voice, &key)
    };

    let sync_event =
        gateway_events::voice_participant_sync(guild_id, channel_id, participants, now_unix());
    match try_enqueue_voice_sync_event(outbound_tx, sync_event.payload) {
        OutboundEnqueueResult::Enqueued => {
            record_gateway_event_emitted("connection", sync_event.event_type);
            record_voice_sync_repair("subscribe");
        }
        OutboundEnqueueResult::Closed => {
            record_gateway_event_dropped("connection", sync_event.event_type, "closed");
        }
        OutboundEnqueueResult::Full => {
            record_gateway_event_dropped("connection", sync_event.event_type, "full_queue");
        }
    }
}

async fn remove_disconnected_user_voice_participants(
    state: &AppState,
    user_id: UserId,
    disconnected_at_unix: i64,
) {
    let removed = {
        let mut voice = state.voice_participants.write().await;
        remove_user_voice_participant_removals(&mut voice, user_id)
    };

    for (channel_subscription_key, event) in
        plan_voice_removal_broadcasts(removed, disconnected_at_unix)
    {
        broadcast_channel_event(state, &channel_subscription_key, &event).await;
    }
}

pub(crate) async fn handle_presence_subscribe(
    state: &AppState,
    connection_id: Uuid,
    user_id: UserId,
    guild_id: &str,
    outbound_tx: &mpsc::Sender<String>,
) {
    let result = {
        let mut presence = state.connection_presence.write().await;
        apply_presence_subscribe(&mut presence, connection_id, user_id, guild_id)
    };
    let Some(result) = result else {
        return;
    };

    let snapshot_event = gateway_events::presence_sync(guild_id, result.snapshot_user_ids);
    match try_enqueue_presence_sync_event(outbound_tx, snapshot_event.payload) {
        PresenceSyncEnqueueResult::Enqueued => {
            record_gateway_event_emitted("connection", snapshot_event.event_type)
        }
        PresenceSyncEnqueueResult::Closed => {
            record_gateway_event_dropped("connection", snapshot_event.event_type, "closed");
        }
        PresenceSyncEnqueueResult::Full => {
            record_gateway_event_dropped("connection", snapshot_event.event_type, "full_queue");
        }
    }

    if let Some(update) = build_presence_online_update(guild_id, user_id, result.became_online) {
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
    insert_connection_subscription(&mut subscriptions, connection_id, key, outbound_tx);
}

pub(crate) async fn remove_connection(state: &AppState, connection_id: Uuid) {
    let removed_presence = {
        let mut presence = state.connection_presence.write().await;
        let mut controls = state.connection_controls.write().await;
        let mut senders = state.connection_senders.write().await;
        remove_connection_state(
            &mut presence,
            &mut controls,
            &mut senders,
            connection_id,
        )
    };

    let mut subscriptions = state.subscriptions.write().await;
    remove_connection_from_subscriptions(&mut subscriptions, connection_id);
    drop(subscriptions);

    let Some(removed_presence) = removed_presence else {
        return;
    };
    let outcome = {
        let remaining = state.connection_presence.read().await;
        compute_disconnect_presence_outcome(&remaining, &removed_presence)
    };

    if !outcome.user_has_other_connections {
        remove_disconnected_user_voice_participants(state, removed_presence.user_id, now_unix())
            .await;
    }

    for (guild_id, update) in
        build_offline_presence_updates(outcome.offline_guilds, removed_presence.user_id)
    {
        broadcast_guild_event(state, &guild_id, &update).await;
    }
}
