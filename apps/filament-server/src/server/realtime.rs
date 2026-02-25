mod connection_disconnect_followups;
mod connection_registry;
mod connection_runtime;
mod connection_subscriptions;
mod emit_metrics;
mod hydration_db;
mod hydration_in_memory;
mod hydration_in_memory_attachments;
mod hydration_merge;
mod hydration_runtime;
mod ingress_message_create;
mod ingress_subscribe;
pub mod livekit_sync;
mod message_create_response;
mod message_emit;
mod message_record;
mod message_store_in_memory;
mod presence_disconnect_events;
mod presence_subscribe_events;
mod presence_sync_dispatch;
mod ready_enqueue;
mod search_apply_batch;
mod search_batch_drain;
mod search_bootstrap;
mod search_collect_db;
mod search_collect_runtime;
mod search_enqueue;
mod search_index_lookup;
mod search_query_input;
mod search_query_run;
mod search_reconciliation_plan;
mod search_runtime;
mod subscribe_ack;
mod subscription_insert;
mod voice_cleanup_dispatch;
mod voice_cleanup_registry;
mod voice_registration_events;
mod voice_subscribe_sync;
mod voice_sync_dispatch;
use std::{
    collections::{HashSet, VecDeque},
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use axum::{
    extract::{
        connect_info::ConnectInfo,
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade},
        Extension, Query, State,
    },
    http::HeaderMap,
    response::IntoResponse,
};
use filament_core::Permission;
use filament_protocol::parse_envelope;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, watch};
use ulid::Ulid;
use uuid::Uuid;

mod connection_control;
mod fanout_channel;
mod fanout_dispatch;
mod fanout_guild;
mod fanout_user;
mod fanout_user_targets;
mod hydration_order;
mod ingress_command;
mod ingress_message;
mod ingress_rate_limit;
mod message_attachment_bind;
mod message_prepare;
mod presence_disconnect;
mod presence_subscribe;
mod search_apply;
mod search_blocking;
mod search_collect_all;
mod search_collect_guild;
mod search_collect_index_ids;
mod search_indexed_message;
mod search_query_exec;
mod search_reconcile;
mod search_schema;
mod search_validation;
mod voice_cleanup_events;
mod voice_presence;
mod voice_registration;
mod voice_registry;

pub(crate) use connection_runtime::{
    add_subscription, broadcast_channel_event, broadcast_guild_event, broadcast_user_event,
    handle_presence_subscribe, handle_voice_subscribe, register_voice_participant_from_token,
    remove_connection, remove_voice_participant_for_channel,
    update_voice_participant_audio_state_for_channel,
};
use hydration_db::collect_hydrated_messages_db;
use hydration_in_memory::collect_hydrated_messages_in_memory;
use hydration_in_memory_attachments::apply_hydration_attachments;
use hydration_merge::merge_hydration_maps;
pub(crate) use hydration_order::collect_hydrated_in_request_order;
use ingress_command::{
    classify_ingress_command_parse_error, parse_gateway_ingress_command, GatewayAttachmentIds,
    GatewayIngressCommand, GatewayMessageContent, IngressCommandParseClassification,
};
use ingress_message::{decode_gateway_ingress_message, GatewayIngressMessageDecode};
use ingress_message_create::execute_message_create_command;
use ingress_rate_limit::allow_gateway_ingress;
use ingress_subscribe::execute_subscribe_command;
use message_attachment_bind::bind_message_attachments_in_memory;
use message_create_response::build_db_created_message_response;
use message_emit::emit_message_create_and_index;
use message_prepare::{prepare_message_body, prepare_prevalidated_message_body};
use message_record::{build_in_memory_message_record, build_message_response_from_record};
use message_store_in_memory::append_message_record;
use ready_enqueue::{ready_drop_metric_reason, ready_error_reason, try_enqueue_ready_event};
use search_blocking::run_search_blocking_with_timeout;
use search_collect_index_ids::collect_index_message_ids_for_guild as collect_index_message_ids_for_guild_from_index;
pub(crate) use search_index_lookup::collect_index_message_ids_for_guild;
use search_query_exec::run_search_query_against_index;
pub(crate) use search_query_run::run_search_query;
pub(crate) use search_reconcile::compute_reconciliation;
pub(crate) use search_reconciliation_plan::plan_search_reconciliation;
pub(crate) use search_runtime::{
    collect_all_indexed_messages, collect_indexed_messages_for_guild, enqueue_search_operation,
    ensure_search_bootstrapped, hydrate_messages_by_id, indexed_message_from_response,
    init_search_service, validate_search_query,
};

#[allow(dead_code)]
pub(crate) fn build_search_schema() -> (tantivy::schema::Schema, super::core::SearchFields) {
    search_runtime::build_search_schema()
}

use super::{
    auth::{authenticate_with_token, bearer_token, extract_client_ip, now_unix, ClientIp},
    core::{AppState, AuthContext, ConnectionControl, ConnectionPresence},
    domain::{
        attachments_for_message_in_memory, bind_message_attachments_db,
        channel_permission_snapshot, fetch_attachments_for_message_db, parse_attachment_ids,
        reaction_summaries_from_users,
    },
    errors::AuthFailure,
    gateway_events::{self},
    metrics::{
        record_gateway_event_dropped, record_gateway_event_emitted,
        record_gateway_event_parse_rejected, record_gateway_event_serialize_error,
        record_gateway_event_unknown_received, record_ws_disconnect,
    },
    types::{GatewayAuthQuery, MessageResponse},
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
        .realtime_registry
        .connection_senders()
        .write()
        .await
        .insert(connection_id, outbound_tx.clone());
    let (control_tx, mut control_rx) = watch::channel(ConnectionControl::Open);
    state
        .realtime_registry
        .connection_controls()
        .write()
        .await
        .insert(connection_id, control_tx);
    state
        .realtime_registry
        .connection_presence()
        .write()
        .await
        .insert(
            connection_id,
            ConnectionPresence {
                user_id: auth.user_id,
                guild_ids: HashSet::new(),
            },
        );
    state
        .realtime_registry
        .user_connections()
        .write()
        .await
        .entry(auth.user_id)
        .or_default()
        .insert(connection_id);

    let ready_event = match gateway_events::try_ready(auth.user_id) {
        Ok(event) => event,
        Err(error) => {
            tracing::error!(
                event = "gateway.ready.serialize_failed",
                connection_id = %connection_id,
                user_id = %auth.user_id,
                error = %error
            );
            record_gateway_event_serialize_error("connection", gateway_events::READY_EVENT);
            record_ws_disconnect("ready_serialize_error");
            remove_connection(&state, connection_id).await;
            return;
        }
    };
    let enqueue_result = try_enqueue_ready_event(
        &outbound_tx,
        ready_event.payload,
        state.runtime.max_gateway_event_bytes,
    );
    if let Some(reason) = ready_drop_metric_reason(&enqueue_result) {
        record_gateway_event_dropped("connection", ready_event.event_type, reason);
    }
    if let Some(reason) = ready_error_reason(&enqueue_result) {
        tracing::warn!(
            event = "gateway.ready.enqueue_rejected",
            connection_id = %connection_id,
            user_id = %auth.user_id,
            reject_reason = reason
        );
        record_ws_disconnect(reason);
        remove_connection(&state, connection_id).await;
        return;
    }
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

        let payload: Vec<u8> =
            match decode_gateway_ingress_message(message, state.runtime.max_gateway_event_bytes) {
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
                match classify_ingress_command_parse_error(&error) {
                    IngressCommandParseClassification::ParseRejected(reason) => {
                        record_gateway_event_parse_rejected("ingress", reason);
                    }
                    IngressCommandParseClassification::UnknownEventType(event_type) => {
                        record_gateway_event_unknown_received("ingress", event_type);
                    }
                }
                disconnect_reason = error.disconnect_reason();
                break;
            }
        };

        match command {
            GatewayIngressCommand::Subscribe(subscribe) => {
                if let Err(reason) = execute_subscribe_command(
                    &state,
                    connection_id,
                    auth.user_id,
                    client_ip,
                    subscribe,
                    &outbound_tx,
                )
                .await
                {
                    disconnect_reason = reason;
                    break;
                }
            }
            GatewayIngressCommand::MessageCreate(request) => {
                if let Err(reason) =
                    execute_message_create_command(&state, &auth, client_ip, request).await
                {
                    disconnect_reason = reason;
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
    create_message_internal_prepared(
        state,
        auth,
        guild_id,
        channel_id,
        prepared.content,
        prepared.markdown_tokens,
        attachment_ids,
    )
    .await
}

pub(crate) async fn create_message_internal_from_ingress_validated(
    state: &AppState,
    auth: &AuthContext,
    guild_id: &str,
    channel_id: &str,
    content: GatewayMessageContent,
    attachment_ids: GatewayAttachmentIds,
) -> Result<MessageResponse, AuthFailure> {
    let attachment_ids = attachment_ids.into_vec();
    let prepared = prepare_prevalidated_message_body(content.into_string());
    create_message_internal_prepared(
        state,
        auth,
        guild_id,
        channel_id,
        prepared.content,
        prepared.markdown_tokens,
        attachment_ids,
    )
    .await
}

async fn create_message_internal_prepared(
    state: &AppState,
    auth: &AuthContext,
    guild_id: &str,
    channel_id: &str,
    content: String,
    markdown_tokens: Vec<filament_core::MarkdownToken>,
    attachment_ids: Vec<String>,
) -> Result<MessageResponse, AuthFailure> {
    let (_, permissions) =
        channel_permission_snapshot(state, auth.user_id, guild_id, channel_id).await?;
    if !permissions.contains(Permission::CreateMessage) {
        return Err(AuthFailure::Forbidden);
    }

    if let Some(pool) = &state.db_pool {
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

        emit_message_create_and_index(state, guild_id, channel_id, &response).await?;
        return Ok(response);
    }

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
    {
        let mut guilds = state.membership_store.guilds().write().await;
        append_message_record(&mut guilds, guild_id, channel_id, record.clone())?;
    }

    let attachments = attachments_for_message_in_memory(state, &record.attachment_ids).await?;
    let response = build_message_response_from_record(
        &record,
        guild_id,
        channel_id,
        attachments,
        reaction_summaries_from_users(&record.reactions),
    );

    emit_message_create_and_index(state, guild_id, channel_id, &response).await?;

    Ok(response)
}
