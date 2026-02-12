use axum::{
    extract::{connect_info::ConnectInfo, Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use filament_core::{tokenize_markdown, Permission, UserId};
use object_store::{path::Path as ObjectPath, ObjectStore};
use sqlx::Row;
use std::net::SocketAddr;

use crate::server::{
    auth::{authenticate, channel_key, extract_client_ip, now_unix, validate_message_content},
    core::{AppState, SearchOperation, MAX_HISTORY_LIMIT},
    db::{ensure_db_schema, permission_list_from_set},
    domain::{
        attach_message_media, attach_message_reactions, attachment_map_for_messages_db,
        attachment_map_for_messages_in_memory, attachments_for_message_in_memory,
        channel_permission_snapshot, enforce_guild_ip_ban_for_request,
        reaction_map_for_messages_db, reaction_summaries_from_users, user_can_write_channel,
        validate_reaction_emoji, write_audit_log,
    },
    errors::AuthFailure,
    gateway_events,
    realtime::{
        broadcast_channel_event, create_message_internal, enqueue_search_operation,
        indexed_message_from_response,
    },
    types::{
        ChannelPath, ChannelPermissionsResponse, CreateMessageRequest, EditMessageRequest,
        HistoryQuery, MessageHistoryResponse, MessagePath, MessageResponse, ReactionPath,
        ReactionResponse,
    },
};

async fn broadcast_message_reaction_event(state: &AppState, path: &ReactionPath, count: usize) {
    let event = gateway_events::message_reaction(
        &path.guild_id,
        &path.channel_id,
        &path.message_id,
        &path.emoji,
        count,
    );
    broadcast_channel_event(
        state,
        &channel_key(&path.guild_id, &path.channel_id),
        &event,
    )
    .await;
}

async fn broadcast_message_update_event(state: &AppState, response: &MessageResponse) {
    let event = gateway_events::message_update(
        &response.guild_id,
        &response.channel_id,
        &response.message_id,
        &response.content,
        &response.markdown_tokens,
        now_unix(),
    );
    broadcast_channel_event(
        state,
        &channel_key(&response.guild_id, &response.channel_id),
        &event,
    )
    .await;
}

async fn broadcast_message_delete_event(state: &AppState, path: &MessagePath) {
    let event = gateway_events::message_delete(
        &path.guild_id,
        &path.channel_id,
        &path.message_id,
        now_unix(),
    );
    broadcast_channel_event(
        state,
        &channel_key(&path.guild_id, &path.channel_id),
        &event,
    )
    .await;
}

pub(crate) async fn create_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(path): Path<ChannelPath>,
    Json(payload): Json<CreateMessageRequest>,
) -> Result<Json<MessageResponse>, AuthFailure> {
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    enforce_guild_ip_ban_for_request(
        &state,
        &path.guild_id,
        auth.user_id,
        client_ip,
        "messages.create",
    )
    .await?;
    let response = create_message_internal(
        &state,
        &auth,
        &path.guild_id,
        &path.channel_id,
        payload.content,
        payload.attachment_ids.unwrap_or_default(),
    )
    .await?;
    Ok(Json(response))
}

pub(crate) async fn get_channel_permissions(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(path): Path<ChannelPath>,
) -> Result<Json<ChannelPermissionsResponse>, AuthFailure> {
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    enforce_guild_ip_ban_for_request(
        &state,
        &path.guild_id,
        auth.user_id,
        client_ip,
        "channels.permissions.self",
    )
    .await?;
    let (role, permissions) =
        channel_permission_snapshot(&state, auth.user_id, &path.guild_id, &path.channel_id).await?;
    if !permissions.contains(Permission::CreateMessage) {
        return Err(AuthFailure::Forbidden);
    }

    Ok(Json(ChannelPermissionsResponse {
        role,
        permissions: permission_list_from_set(permissions),
    }))
}

#[allow(clippy::too_many_lines)]
pub(crate) async fn get_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(path): Path<ChannelPath>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<MessageHistoryResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    enforce_guild_ip_ban_for_request(
        &state,
        &path.guild_id,
        auth.user_id,
        client_ip,
        "messages.list",
    )
    .await?;
    let limit = query.limit.unwrap_or(20);
    if limit == 0 || limit > MAX_HISTORY_LIMIT {
        return Err(AuthFailure::InvalidRequest);
    }
    let (_, permissions) =
        channel_permission_snapshot(&state, auth.user_id, &path.guild_id, &path.channel_id).await?;
    if !permissions.contains(Permission::CreateMessage) {
        return Err(AuthFailure::Forbidden);
    }

    if let Some(pool) = &state.db_pool {
        let limit_i64 = i64::try_from(limit).map_err(|_| AuthFailure::InvalidRequest)?;
        let rows = sqlx::query(
            "SELECT message_id, author_id, content, created_at_unix
             FROM messages
             WHERE guild_id = $1 AND channel_id = $2 AND ($3::text IS NULL OR message_id < $3)
             ORDER BY message_id DESC
             LIMIT $4",
        )
        .bind(&path.guild_id)
        .bind(&path.channel_id)
        .bind(query.before.clone())
        .bind(limit_i64)
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;

        let mut messages = Vec::with_capacity(rows.len());
        for row in rows {
            let message_id: String = row
                .try_get("message_id")
                .map_err(|_| AuthFailure::Internal)?;
            let author_id: String = row
                .try_get("author_id")
                .map_err(|_| AuthFailure::Internal)?;
            let content: String = row.try_get("content").map_err(|_| AuthFailure::Internal)?;
            let created_at_unix: i64 = row
                .try_get("created_at_unix")
                .map_err(|_| AuthFailure::Internal)?;
            messages.push(MessageResponse {
                message_id,
                guild_id: path.guild_id.clone(),
                channel_id: path.channel_id.clone(),
                author_id,
                content: content.clone(),
                markdown_tokens: tokenize_markdown(&content),
                attachments: Vec::new(),
                reactions: Vec::new(),
                created_at_unix,
            });
        }
        let message_ids: Vec<String> = messages
            .iter()
            .map(|message| message.message_id.clone())
            .collect();
        let attachment_map = attachment_map_for_messages_db(
            pool,
            &path.guild_id,
            Some(&path.channel_id),
            &message_ids,
        )
        .await?;
        let reaction_map = reaction_map_for_messages_db(
            pool,
            &path.guild_id,
            Some(&path.channel_id),
            &message_ids,
        )
        .await?;
        attach_message_media(&mut messages, &attachment_map);
        attach_message_reactions(&mut messages, &reaction_map);
        let next_before = messages.last().map(|message| message.message_id.clone());
        return Ok(Json(MessageHistoryResponse {
            messages,
            next_before,
        }));
    }

    let guilds = state.guilds.read().await;
    let guild = guilds.get(&path.guild_id).ok_or(AuthFailure::NotFound)?;
    let channel = guild
        .channels
        .get(&path.channel_id)
        .ok_or(AuthFailure::NotFound)?;

    let mut messages = Vec::with_capacity(limit);
    let mut collecting = query.before.is_none();

    for message in channel.messages.iter().rev() {
        if !collecting {
            if query.before.as_deref() == Some(message.id.as_str()) {
                collecting = true;
            }
            continue;
        }

        if messages.len() >= limit {
            break;
        }

        messages.push(MessageResponse {
            message_id: message.id.clone(),
            guild_id: path.guild_id.clone(),
            channel_id: path.channel_id.clone(),
            author_id: message.author_id.to_string(),
            content: message.content.clone(),
            markdown_tokens: message.markdown_tokens.clone(),
            attachments: Vec::new(),
            reactions: reaction_summaries_from_users(&message.reactions),
            created_at_unix: message.created_at_unix,
        });
    }

    let message_ids: Vec<String> = messages
        .iter()
        .map(|message| message.message_id.clone())
        .collect();
    let attachment_map = attachment_map_for_messages_in_memory(
        &state,
        &path.guild_id,
        Some(&path.channel_id),
        &message_ids,
    )
    .await;
    attach_message_media(&mut messages, &attachment_map);

    let next_before = messages.last().map(|message| message.message_id.clone());

    Ok(Json(MessageHistoryResponse {
        messages,
        next_before,
    }))
}

#[allow(clippy::too_many_lines)]
pub(crate) async fn edit_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(path): Path<MessagePath>,
    Json(payload): Json<EditMessageRequest>,
) -> Result<Json<MessageResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    enforce_guild_ip_ban_for_request(
        &state,
        &path.guild_id,
        auth.user_id,
        client_ip,
        "messages.edit",
    )
    .await?;
    validate_message_content(&payload.content)?;
    let markdown_tokens = tokenize_markdown(&payload.content);
    let (_, permissions) =
        channel_permission_snapshot(&state, auth.user_id, &path.guild_id, &path.channel_id).await?;

    if let Some(pool) = &state.db_pool {
        let row = sqlx::query(
            "SELECT m.author_id
             FROM messages m
             WHERE m.guild_id = $1 AND m.channel_id = $2 AND m.message_id = $3",
        )
        .bind(&path.guild_id)
        .bind(&path.channel_id)
        .bind(&path.message_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        let row = row.ok_or(AuthFailure::NotFound)?;
        let author_id: String = row
            .try_get("author_id")
            .map_err(|_| AuthFailure::Internal)?;
        if author_id != auth.user_id.to_string() && !permissions.contains(Permission::DeleteMessage)
        {
            return Err(AuthFailure::Forbidden);
        }

        sqlx::query(
            "UPDATE messages SET content = $4
             WHERE guild_id = $1 AND channel_id = $2 AND message_id = $3",
        )
        .bind(&path.guild_id)
        .bind(&path.channel_id)
        .bind(&path.message_id)
        .bind(&payload.content)
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;

        let attachment_map = attachment_map_for_messages_db(
            pool,
            &path.guild_id,
            Some(&path.channel_id),
            std::slice::from_ref(&path.message_id),
        )
        .await?;
        let reaction_map = reaction_map_for_messages_db(
            pool,
            &path.guild_id,
            Some(&path.channel_id),
            std::slice::from_ref(&path.message_id),
        )
        .await?;
        let response = MessageResponse {
            message_id: path.message_id.clone(),
            guild_id: path.guild_id.clone(),
            channel_id: path.channel_id.clone(),
            author_id: author_id.clone(),
            content: payload.content,
            markdown_tokens,
            attachments: attachment_map
                .get(&path.message_id)
                .cloned()
                .unwrap_or_default(),
            reactions: reaction_map
                .get(&path.message_id)
                .cloned()
                .unwrap_or_default(),
            created_at_unix: now_unix(),
        };
        if author_id != auth.user_id.to_string() {
            write_audit_log(
                &state,
                Some(path.guild_id.clone()),
                auth.user_id,
                Some(UserId::try_from(author_id).map_err(|_| AuthFailure::Internal)?),
                "message.edit.moderation",
                serde_json::json!({"message_id": path.message_id, "channel_id": path.channel_id}),
            )
            .await?;
        }
        enqueue_search_operation(
            &state,
            SearchOperation::Upsert(indexed_message_from_response(&response)),
            true,
        )
        .await?;
        broadcast_message_update_event(&state, &response).await;
        return Ok(Json(response));
    }

    let mut guilds = state.guilds.write().await;
    let guild = guilds
        .get_mut(&path.guild_id)
        .ok_or(AuthFailure::NotFound)?;
    let channel = guild
        .channels
        .get_mut(&path.channel_id)
        .ok_or(AuthFailure::NotFound)?;
    let message = channel
        .messages
        .iter_mut()
        .find(|message| message.id == path.message_id)
        .ok_or(AuthFailure::NotFound)?;
    if message.author_id != auth.user_id && !permissions.contains(Permission::DeleteMessage) {
        return Err(AuthFailure::Forbidden);
    }
    message.content.clone_from(&payload.content);
    message.markdown_tokens.clone_from(&markdown_tokens);

    let response = MessageResponse {
        message_id: message.id.clone(),
        guild_id: path.guild_id,
        channel_id: path.channel_id,
        author_id: message.author_id.to_string(),
        content: message.content.clone(),
        markdown_tokens,
        attachments: attachments_for_message_in_memory(&state, &message.attachment_ids).await?,
        reactions: reaction_summaries_from_users(&message.reactions),
        created_at_unix: message.created_at_unix,
    };
    enqueue_search_operation(
        &state,
        SearchOperation::Upsert(indexed_message_from_response(&response)),
        true,
    )
    .await?;
    broadcast_message_update_event(&state, &response).await;
    Ok(Json(response))
}

#[allow(clippy::too_many_lines)]
pub(crate) async fn delete_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(path): Path<MessagePath>,
) -> Result<StatusCode, AuthFailure> {
    ensure_db_schema(&state).await?;
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    enforce_guild_ip_ban_for_request(
        &state,
        &path.guild_id,
        auth.user_id,
        client_ip,
        "messages.delete",
    )
    .await?;
    let (_, permissions) =
        channel_permission_snapshot(&state, auth.user_id, &path.guild_id, &path.channel_id).await?;

    if let Some(pool) = &state.db_pool {
        let row = sqlx::query(
            "SELECT m.author_id
             FROM messages m
             WHERE m.guild_id = $1 AND m.channel_id = $2 AND m.message_id = $3",
        )
        .bind(&path.guild_id)
        .bind(&path.channel_id)
        .bind(&path.message_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        let row = row.ok_or(AuthFailure::NotFound)?;
        let author_id: String = row
            .try_get("author_id")
            .map_err(|_| AuthFailure::Internal)?;
        if author_id != auth.user_id.to_string() && !permissions.contains(Permission::DeleteMessage)
        {
            return Err(AuthFailure::Forbidden);
        }

        let linked_attachment_rows = sqlx::query(
            "SELECT attachment_id, object_key
             FROM attachments
             WHERE guild_id = $1 AND channel_id = $2 AND message_id = $3",
        )
        .bind(&path.guild_id)
        .bind(&path.channel_id)
        .bind(&path.message_id)
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;

        sqlx::query(
            "DELETE FROM messages
             WHERE guild_id = $1 AND channel_id = $2 AND message_id = $3",
        )
        .bind(&path.guild_id)
        .bind(&path.channel_id)
        .bind(&path.message_id)
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        if !linked_attachment_rows.is_empty() {
            sqlx::query(
                "DELETE FROM attachments
                 WHERE guild_id = $1 AND channel_id = $2 AND message_id = $3",
            )
            .bind(&path.guild_id)
            .bind(&path.channel_id)
            .bind(&path.message_id)
            .execute(pool)
            .await
            .map_err(|_| AuthFailure::Internal)?;
        }
        for row in linked_attachment_rows {
            let object_key: String = row
                .try_get("object_key")
                .map_err(|_| AuthFailure::Internal)?;
            let object_path = ObjectPath::from(object_key);
            let _ = state.attachment_store.delete(&object_path).await;
        }

        if author_id != auth.user_id.to_string() {
            write_audit_log(
                &state,
                Some(path.guild_id.clone()),
                auth.user_id,
                Some(UserId::try_from(author_id).map_err(|_| AuthFailure::Internal)?),
                "message.delete.moderation",
                serde_json::json!({"message_id": path.message_id, "channel_id": path.channel_id}),
            )
            .await?;
        }
        enqueue_search_operation(
            &state,
            SearchOperation::Delete {
                message_id: path.message_id.clone(),
            },
            true,
        )
        .await?;
        broadcast_message_delete_event(&state, &path).await;
        return Ok(StatusCode::NO_CONTENT);
    }

    let mut guilds = state.guilds.write().await;
    let guild = guilds
        .get_mut(&path.guild_id)
        .ok_or(AuthFailure::NotFound)?;
    let channel = guild
        .channels
        .get_mut(&path.channel_id)
        .ok_or(AuthFailure::NotFound)?;
    let Some(index) = channel
        .messages
        .iter()
        .position(|message| message.id == path.message_id)
    else {
        return Err(AuthFailure::NotFound);
    };
    let author_id = channel.messages[index].author_id;
    if author_id != auth.user_id && !permissions.contains(Permission::DeleteMessage) {
        return Err(AuthFailure::Forbidden);
    }
    let removed = channel.messages.remove(index);
    if !removed.attachment_ids.is_empty() {
        let mut attachments = state.attachments.write().await;
        let mut object_keys = Vec::new();
        for attachment_id in removed.attachment_ids {
            if let Some(record) = attachments.remove(&attachment_id) {
                object_keys.push(record.object_key);
            }
        }
        drop(attachments);
        for object_key in object_keys {
            let object_path = ObjectPath::from(object_key);
            let _ = state.attachment_store.delete(&object_path).await;
        }
    }
    enqueue_search_operation(
        &state,
        SearchOperation::Delete {
            message_id: path.message_id.clone(),
        },
        true,
    )
    .await?;
    broadcast_message_delete_event(&state, &path).await;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn add_reaction(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(path): Path<ReactionPath>,
) -> Result<Json<ReactionResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    enforce_guild_ip_ban_for_request(
        &state,
        &path.guild_id,
        auth.user_id,
        client_ip,
        "messages.reactions.add",
    )
    .await?;
    validate_reaction_emoji(&path.emoji)?;
    if !user_can_write_channel(&state, auth.user_id, &path.guild_id, &path.channel_id).await {
        return Err(AuthFailure::Forbidden);
    }

    if let Some(pool) = &state.db_pool {
        sqlx::query(
            "INSERT INTO message_reactions (guild_id, channel_id, message_id, emoji, user_id, created_at_unix)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (guild_id, channel_id, message_id, emoji, user_id) DO NOTHING",
        )
        .bind(&path.guild_id)
        .bind(&path.channel_id)
        .bind(&path.message_id)
        .bind(&path.emoji)
        .bind(auth.user_id.to_string())
        .bind(now_unix())
        .execute(pool)
        .await
        .map_err(|e| {
            if matches!(e, sqlx::Error::Database(_)) {
                AuthFailure::NotFound
            } else {
                AuthFailure::Internal
            }
        })?;

        let row = sqlx::query(
            "SELECT COUNT(*) AS count
             FROM message_reactions
             WHERE guild_id = $1 AND channel_id = $2 AND message_id = $3 AND emoji = $4",
        )
        .bind(&path.guild_id)
        .bind(&path.channel_id)
        .bind(&path.message_id)
        .bind(&path.emoji)
        .fetch_one(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        let count: i64 = row.try_get("count").map_err(|_| AuthFailure::Internal)?;
        let count = usize::try_from(count).map_err(|_| AuthFailure::Internal)?;
        let response = ReactionResponse {
            emoji: path.emoji.clone(),
            count,
        };
        broadcast_message_reaction_event(&state, &path, response.count).await;
        return Ok(Json(response));
    }

    let mut guilds = state.guilds.write().await;
    let guild = guilds
        .get_mut(&path.guild_id)
        .ok_or(AuthFailure::NotFound)?;
    let channel = guild
        .channels
        .get_mut(&path.channel_id)
        .ok_or(AuthFailure::NotFound)?;
    let message = channel
        .messages
        .iter_mut()
        .find(|message| message.id == path.message_id)
        .ok_or(AuthFailure::NotFound)?;
    let users = message.reactions.entry(path.emoji.clone()).or_default();
    users.insert(auth.user_id);
    let response = ReactionResponse {
        emoji: path.emoji.clone(),
        count: users.len(),
    };
    drop(guilds);
    broadcast_message_reaction_event(&state, &path, response.count).await;
    Ok(Json(response))
}

pub(crate) async fn remove_reaction(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(path): Path<ReactionPath>,
) -> Result<Json<ReactionResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    enforce_guild_ip_ban_for_request(
        &state,
        &path.guild_id,
        auth.user_id,
        client_ip,
        "messages.reactions.remove",
    )
    .await?;
    validate_reaction_emoji(&path.emoji)?;
    if !user_can_write_channel(&state, auth.user_id, &path.guild_id, &path.channel_id).await {
        return Err(AuthFailure::Forbidden);
    }

    if let Some(pool) = &state.db_pool {
        sqlx::query(
            "DELETE FROM message_reactions
             WHERE guild_id = $1 AND channel_id = $2 AND message_id = $3 AND emoji = $4 AND user_id = $5",
        )
        .bind(&path.guild_id)
        .bind(&path.channel_id)
        .bind(&path.message_id)
        .bind(&path.emoji)
        .bind(auth.user_id.to_string())
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;

        let row = sqlx::query(
            "SELECT COUNT(*) AS count
             FROM message_reactions
             WHERE guild_id = $1 AND channel_id = $2 AND message_id = $3 AND emoji = $4",
        )
        .bind(&path.guild_id)
        .bind(&path.channel_id)
        .bind(&path.message_id)
        .bind(&path.emoji)
        .fetch_one(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        let count: i64 = row.try_get("count").map_err(|_| AuthFailure::Internal)?;
        let count = usize::try_from(count).map_err(|_| AuthFailure::Internal)?;
        let response = ReactionResponse {
            emoji: path.emoji.clone(),
            count,
        };
        broadcast_message_reaction_event(&state, &path, response.count).await;
        return Ok(Json(response));
    }

    let mut guilds = state.guilds.write().await;
    let guild = guilds
        .get_mut(&path.guild_id)
        .ok_or(AuthFailure::NotFound)?;
    let channel = guild
        .channels
        .get_mut(&path.channel_id)
        .ok_or(AuthFailure::NotFound)?;
    let message = channel
        .messages
        .iter_mut()
        .find(|message| message.id == path.message_id)
        .ok_or(AuthFailure::NotFound)?;
    let count = if let Some(users) = message.reactions.get_mut(&path.emoji) {
        users.remove(&auth.user_id);
        if users.is_empty() {
            message.reactions.remove(&path.emoji);
            0
        } else {
            users.len()
        }
    } else {
        0
    };

    let response = ReactionResponse {
        emoji: path.emoji.clone(),
        count,
    };
    drop(guilds);
    broadcast_message_reaction_event(&state, &path, response.count).await;
    Ok(Json(response))
}
