mod connection_disconnect_followups;
mod connection_runtime;
mod hydration_runtime;
pub mod livekit_sync;
mod message_record;
mod search_query_run;
mod search_reconciliation_plan;
mod search_runtime;
mod voice_cleanup_dispatch;
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

mod fanout_dispatch;
mod ingress_command;
mod presence_subscribe;
mod voice_registration;
mod voice_registry;

pub(crate) use connection_runtime::{
    add_subscription, broadcast_channel_event, broadcast_guild_event, broadcast_user_event,
    handle_presence_subscribe, handle_voice_subscribe, register_voice_participant_from_token,
    remove_connection, remove_voice_participant_for_channel,
    update_voice_participant_audio_state_for_channel,
};
use ingress_command::{
    allow_gateway_ingress, classify_ingress_command_parse_error, decode_gateway_ingress_message,
    execute_message_create_command, execute_subscribe_command, parse_gateway_ingress_command,
    GatewayAttachmentIds, GatewayIngressCommand, GatewayIngressMessageDecode,
    GatewayMessageContent, IngressCommandParseClassification,
};
use message_record::{
    append_message_record, bind_message_attachments_in_memory, build_db_created_message_response,
    build_in_memory_message_record, build_message_response_from_record,
};
pub(crate) use search_query_run::run_search_query;
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
    auth::{
        authenticate_with_token, bearer_token, channel_key, extract_client_ip, now_unix,
        validate_message_content, ClientIp,
    },
    core::{AppState, AuthContext, ConnectionControl, ConnectionPresence, SearchOperation},
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

enum ReadyEnqueueResult {
    Enqueued,
    Closed,
    Full,
    Oversized,
}

struct PreparedMessageBody {
    content: String,
    markdown_tokens: Vec<filament_core::MarkdownToken>,
}

fn prepare_message_body(
    content: String,
    has_attachments: bool,
) -> Result<PreparedMessageBody, AuthFailure> {
    if content.is_empty() {
        if !has_attachments {
            return Err(AuthFailure::InvalidRequest);
        }
        return Ok(PreparedMessageBody {
            content,
            markdown_tokens: Vec::new(),
        });
    }

    validate_message_content(&content)?;
    Ok(PreparedMessageBody {
        markdown_tokens: filament_core::tokenize_markdown(&content),
        content,
    })
}

fn prepare_prevalidated_message_body(content: String) -> PreparedMessageBody {
    if content.is_empty() {
        return PreparedMessageBody {
            content,
            markdown_tokens: Vec::new(),
        };
    }

    PreparedMessageBody {
        markdown_tokens: filament_core::tokenize_markdown(&content),
        content,
    }
}

fn try_enqueue_ready_event(
    outbound_tx: &mpsc::Sender<String>,
    payload: String,
    max_gateway_event_bytes: usize,
) -> ReadyEnqueueResult {
    if payload.len() > max_gateway_event_bytes {
        return ReadyEnqueueResult::Oversized;
    }

    match outbound_tx.try_send(payload) {
        Ok(()) => ReadyEnqueueResult::Enqueued,
        Err(mpsc::error::TrySendError::Closed(_)) => ReadyEnqueueResult::Closed,
        Err(mpsc::error::TrySendError::Full(_)) => ReadyEnqueueResult::Full,
    }
}

fn ready_error_reason(result: &ReadyEnqueueResult) -> Option<&'static str> {
    match result {
        ReadyEnqueueResult::Enqueued => None,
        ReadyEnqueueResult::Full => Some("outbound_queue_full"),
        ReadyEnqueueResult::Closed => Some("outbound_queue_closed"),
        ReadyEnqueueResult::Oversized => Some("outbound_payload_too_large"),
    }
}

fn ready_drop_metric_reason(result: &ReadyEnqueueResult) -> Option<&'static str> {
    match result {
        ReadyEnqueueResult::Enqueued => None,
        ReadyEnqueueResult::Full => Some("full_queue"),
        ReadyEnqueueResult::Closed => Some("closed"),
        ReadyEnqueueResult::Oversized => Some("oversized_outbound"),
    }
}

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

fn message_upsert_operation(response: &MessageResponse) -> SearchOperation {
    SearchOperation::Upsert(indexed_message_from_response(response))
}

async fn emit_message_create_and_index(
    state: &AppState,
    guild_id: &str,
    channel_id: &str,
    response: &MessageResponse,
) -> Result<(), AuthFailure> {
    if let Ok(event) = gateway_events::try_message_create(response) {
        broadcast_channel_event(state, &channel_key(guild_id, channel_id), &event).await;
    } else {
        record_gateway_event_serialize_error("channel", gateway_events::MESSAGE_CREATE_EVENT);
        tracing::warn!(
            guild_id,
            channel_id,
            "dropped message_create outbound event because serialization failed"
        );
    }
    enqueue_search_operation(state, message_upsert_operation(response), true).await
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

#[cfg(test)]
mod tests {
    use filament_core::MarkdownToken;
    use tokio::sync::mpsc;

    use super::{
        message_upsert_operation, ready_drop_metric_reason, ready_error_reason,
        try_enqueue_ready_event, ReadyEnqueueResult,
    };
    use crate::server::{core::SearchOperation, types::MessageResponse};

    #[test]
    fn ready_enqueue_returns_enqueued_when_sender_has_capacity() {
        let (tx, _rx) = mpsc::channel::<String>(1);

        let result = try_enqueue_ready_event(&tx, String::from("payload"), 1024);

        assert!(matches!(result, ReadyEnqueueResult::Enqueued));
    }

    #[test]
    fn ready_enqueue_returns_full_when_sender_is_full() {
        let (tx, _rx) = mpsc::channel::<String>(1);
        tx.try_send(String::from("first"))
            .expect("first send should fill queue");

        let result = try_enqueue_ready_event(&tx, String::from("second"), 1024);

        assert!(matches!(result, ReadyEnqueueResult::Full));
    }

    #[test]
    fn ready_enqueue_returns_closed_when_sender_is_closed() {
        let (tx, rx) = mpsc::channel::<String>(1);
        drop(rx);

        let result = try_enqueue_ready_event(&tx, String::from("payload"), 1024);

        assert!(matches!(result, ReadyEnqueueResult::Closed));
    }

    #[test]
    fn ready_enqueue_returns_oversized_when_payload_exceeds_limit() {
        let (tx, _rx) = mpsc::channel::<String>(1);

        let result = try_enqueue_ready_event(&tx, String::from("payload"), 3);

        assert!(matches!(result, ReadyEnqueueResult::Oversized));
    }

    #[test]
    fn ready_error_reason_maps_all_rejections() {
        assert_eq!(ready_error_reason(&ReadyEnqueueResult::Enqueued), None);
        assert_eq!(
            ready_error_reason(&ReadyEnqueueResult::Full),
            Some("outbound_queue_full")
        );
        assert_eq!(
            ready_error_reason(&ReadyEnqueueResult::Closed),
            Some("outbound_queue_closed")
        );
        assert_eq!(
            ready_error_reason(&ReadyEnqueueResult::Oversized),
            Some("outbound_payload_too_large")
        );
    }

    #[test]
    fn ready_drop_metric_reason_maps_all_rejections() {
        assert_eq!(
            ready_drop_metric_reason(&ReadyEnqueueResult::Enqueued),
            None
        );
        assert_eq!(
            ready_drop_metric_reason(&ReadyEnqueueResult::Full),
            Some("full_queue")
        );
        assert_eq!(
            ready_drop_metric_reason(&ReadyEnqueueResult::Closed),
            Some("closed")
        );
        assert_eq!(
            ready_drop_metric_reason(&ReadyEnqueueResult::Oversized),
            Some("oversized_outbound")
        );
    }

    #[test]
    fn message_upsert_operation_maps_response_fields() {
        let response = MessageResponse {
            message_id: String::from("m1"),
            guild_id: String::from("g1"),
            channel_id: String::from("c1"),
            author_id: String::from("u1"),
            content: String::from("hello"),
            markdown_tokens: vec![MarkdownToken::Text {
                text: String::from("hello"),
            }],
            attachments: Vec::new(),
            reactions: Vec::new(),
            created_at_unix: 42,
        };

        let op = message_upsert_operation(&response);
        let SearchOperation::Upsert(doc) = op else {
            panic!("expected upsert operation");
        };

        assert_eq!(doc.message_id, "m1");
        assert_eq!(doc.guild_id, "g1");
        assert_eq!(doc.channel_id, "c1");
        assert_eq!(doc.author_id, "u1");
        assert_eq!(doc.content, "hello");
        assert_eq!(doc.created_at_unix, 42);
    }

    #[test]
    fn prepare_message_body_rejects_empty_content_without_attachments() {
        let result = super::prepare_message_body(String::new(), false);
        assert!(matches!(
            result,
            Err(crate::server::errors::AuthFailure::InvalidRequest)
        ));
    }

    #[test]
    fn prepare_message_body_accepts_empty_content_with_attachments() {
        let prepared = super::prepare_message_body(String::new(), true)
            .expect("empty message with attachments should be accepted");

        assert!(prepared.content.is_empty());
        assert!(prepared.markdown_tokens.is_empty());
    }

    #[test]
    fn prepare_message_body_tokenizes_non_empty_content() {
        let prepared = super::prepare_message_body(String::from("hello **world**"), false)
            .expect("valid message should be accepted");

        assert_eq!(prepared.content, "hello **world**");
        assert!(!prepared.markdown_tokens.is_empty());
    }

    #[test]
    fn prepare_message_body_rejects_oversized_content() {
        let oversized = "a".repeat(2001);
        let result = super::prepare_message_body(oversized, false);

        assert!(matches!(
            result,
            Err(crate::server::errors::AuthFailure::InvalidRequest)
        ));
    }

    #[test]
    fn prepare_prevalidated_message_body_preserves_empty_content_without_tokens() {
        let prepared = super::prepare_prevalidated_message_body(String::new());

        assert!(prepared.content.is_empty());
        assert!(prepared.markdown_tokens.is_empty());
    }

    #[test]
    fn prepare_prevalidated_message_body_tokenizes_non_empty_content() {
        let prepared = super::prepare_prevalidated_message_body(String::from("hello **world**"));

        assert_eq!(prepared.content, "hello **world**");
        assert!(!prepared.markdown_tokens.is_empty());
    }
}
