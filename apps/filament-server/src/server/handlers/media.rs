use axum::{
    body::Body,
    extract::{connect_info::ConnectInfo, Extension, Path, Query, State},
    http::{
        header::CONTENT_LENGTH, header::CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue,
        StatusCode,
    },
    response::Response,
    Json,
};
use futures_util::StreamExt;
use livekit_api::access_token::{AccessToken as LiveKitAccessToken, VideoGrants};
use object_store::{path::Path as ObjectPath, ObjectStore};
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use ulid::Ulid;

use filament_core::{has_permission, LiveKitIdentity, LiveKitRoomName, Permission};

use crate::server::{
    auth::{
        allowed_publish_sources, authenticate, dedup_publish_sources,
        enforce_media_publish_rate_limit, enforce_media_subscribe_cap,
        enforce_media_token_rate_limit, extract_client_ip, now_unix,
    },
    core::{AppState, AttachmentRecord, MAX_MIME_SNIFF_BYTES},
    db::ensure_db_schema,
    domain::{
        attachment_usage_for_user, channel_permission_snapshot, find_attachment,
        user_can_write_channel, user_role_in_guild, validate_attachment_filename, write_audit_log,
    },
    errors::AuthFailure,
    types::{
        AttachmentPath, AttachmentResponse, ChannelPath, MediaPublishSource, UploadAttachmentQuery,
        VoiceTokenRequest, VoiceTokenResponse,
    },
};

#[allow(clippy::too_many_lines)]
pub(crate) async fn upload_attachment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<ChannelPath>,
    Query(query): Query<UploadAttachmentQuery>,
    body: Body,
) -> Result<Json<AttachmentResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    if !user_can_write_channel(&state, auth.user_id, &path.guild_id, &path.channel_id).await {
        return Err(AuthFailure::Forbidden);
    }

    let declared_content_type = if let Some(content_type) = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
    {
        Some(
            content_type
                .parse::<mime::Mime>()
                .map_err(|_| AuthFailure::InvalidRequest)?,
        )
    } else {
        None
    };

    let filename =
        validate_attachment_filename(query.filename.unwrap_or_else(|| String::from("upload.bin")))?;
    let usage = attachment_usage_for_user(&state, auth.user_id).await?;
    let remaining_quota = state
        .runtime
        .user_attachment_quota_bytes
        .saturating_sub(usage);
    if remaining_quota == 0 {
        return Err(AuthFailure::QuotaExceeded);
    }

    let attachment_id = Ulid::new().to_string();
    let object_key = format!("attachments/{attachment_id}");
    let object_path = ObjectPath::from(object_key.clone());
    let mut upload = state
        .attachment_store
        .put_multipart(&object_path)
        .await
        .map_err(|_| AuthFailure::Internal)?;
    let mut stream = body.into_data_stream();
    let mut sniff_buffer = Vec::new();
    let mut hasher = Sha256::new();
    let mut total_size: u64 = 0;
    let max_attachment_bytes =
        u64::try_from(state.runtime.max_attachment_bytes).map_err(|_| AuthFailure::Internal)?;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| AuthFailure::InvalidRequest)?;
        if chunk.is_empty() {
            continue;
        }
        let chunk_len = u64::try_from(chunk.len()).map_err(|_| AuthFailure::InvalidRequest)?;
        total_size = total_size
            .checked_add(chunk_len)
            .ok_or(AuthFailure::PayloadTooLarge)?;
        if total_size > max_attachment_bytes {
            let _ = upload.abort().await;
            return Err(AuthFailure::PayloadTooLarge);
        }
        if total_size > remaining_quota {
            let _ = upload.abort().await;
            return Err(AuthFailure::QuotaExceeded);
        }

        if sniff_buffer.len() < MAX_MIME_SNIFF_BYTES {
            let remaining = MAX_MIME_SNIFF_BYTES - sniff_buffer.len();
            let copy_len = remaining.min(chunk.len());
            sniff_buffer.extend_from_slice(&chunk[..copy_len]);
        }
        hasher.update(chunk.as_ref());
        if upload.put_part(chunk.into()).await.is_err() {
            let _ = upload.abort().await;
            return Err(AuthFailure::Internal);
        }
    }

    if total_size == 0 {
        let _ = upload.abort().await;
        return Err(AuthFailure::InvalidRequest);
    }
    let Some(sniffed) = infer::get(&sniff_buffer) else {
        let _ = upload.abort().await;
        return Err(AuthFailure::InvalidRequest);
    };
    let sniffed_mime = sniffed.mime_type();
    if let Some(declared) = declared_content_type.as_ref() {
        if declared.essence_str() != sniffed_mime {
            let _ = upload.abort().await;
            return Err(AuthFailure::InvalidRequest);
        }
    }
    upload.complete().await.map_err(|_| AuthFailure::Internal)?;

    let sha256_hex = {
        let digest = hasher.finalize();
        let mut out = String::with_capacity(digest.len() * 2);
        for byte in digest {
            let _ = std::fmt::Write::write_fmt(&mut out, format_args!("{byte:02x}"));
        }
        out
    };

    if let Some(pool) = &state.db_pool {
        let persist_result = sqlx::query(
            "INSERT INTO attachments (attachment_id, guild_id, channel_id, owner_id, filename, mime_type, size_bytes, sha256_hex, object_key, created_at_unix)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(&attachment_id)
        .bind(&path.guild_id)
        .bind(&path.channel_id)
        .bind(auth.user_id.to_string())
        .bind(&filename)
        .bind(sniffed_mime)
        .bind(i64::try_from(total_size).map_err(|_| AuthFailure::InvalidRequest)?)
        .bind(&sha256_hex)
        .bind(&object_key)
        .bind(now_unix())
        .execute(pool)
        .await;
        if let Err(error) = persist_result {
            tracing::error!(
                event = "attachments.persist_failed",
                attachment_id = %attachment_id,
                guild_id = %path.guild_id,
                channel_id = %path.channel_id,
                user_id = %auth.user_id,
                error = %error
            );
            let _ = state.attachment_store.delete(&object_path).await;
            return Err(AuthFailure::Internal);
        }
    } else {
        state.attachments.write().await.insert(
            attachment_id.clone(),
            AttachmentRecord {
                attachment_id: attachment_id.clone(),
                guild_id: path.guild_id.clone(),
                channel_id: path.channel_id.clone(),
                owner_id: auth.user_id,
                filename: filename.clone(),
                mime_type: String::from(sniffed_mime),
                size_bytes: total_size,
                sha256_hex: sha256_hex.clone(),
                object_key: object_key.clone(),
                message_id: None,
            },
        );
    }

    Ok(Json(AttachmentResponse {
        attachment_id,
        guild_id: path.guild_id,
        channel_id: path.channel_id,
        owner_id: auth.user_id.to_string(),
        filename,
        mime_type: String::from(sniffed_mime),
        size_bytes: total_size,
        sha256_hex,
    }))
}

pub(crate) async fn download_attachment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<AttachmentPath>,
) -> Result<Response, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    if !user_can_write_channel(&state, auth.user_id, &path.guild_id, &path.channel_id).await {
        return Err(AuthFailure::Forbidden);
    }

    let record = find_attachment(&state, &path).await?;
    let object_path = ObjectPath::from(record.object_key.clone());
    let get_result = state
        .attachment_store
        .get(&object_path)
        .await
        .map_err(|_| AuthFailure::NotFound)?;
    let payload = get_result
        .bytes()
        .await
        .map_err(|_| AuthFailure::Internal)?;

    let mut response = Response::new(payload.into());
    let content_type =
        HeaderValue::from_str(&record.mime_type).map_err(|_| AuthFailure::Internal)?;
    response.headers_mut().insert(CONTENT_TYPE, content_type);
    let content_len =
        HeaderValue::from_str(&record.size_bytes.to_string()).map_err(|_| AuthFailure::Internal)?;
    response.headers_mut().insert(CONTENT_LENGTH, content_len);
    response.headers_mut().insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    response.headers_mut().insert(
        HeaderName::from_static("cache-control"),
        HeaderValue::from_static("private, no-store"),
    );
    Ok(response)
}

pub(crate) async fn delete_attachment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<AttachmentPath>,
) -> Result<StatusCode, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    let role = user_role_in_guild(&state, auth.user_id, &path.guild_id).await?;
    let record = find_attachment(&state, &path).await?;
    if record.owner_id != auth.user_id && !has_permission(role, Permission::DeleteMessage) {
        return Err(AuthFailure::Forbidden);
    }

    if let Some(pool) = &state.db_pool {
        sqlx::query(
            "DELETE FROM attachments
             WHERE attachment_id = $1 AND guild_id = $2 AND channel_id = $3",
        )
        .bind(&path.attachment_id)
        .bind(&path.guild_id)
        .bind(&path.channel_id)
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
    } else {
        state.attachments.write().await.remove(&path.attachment_id);
    }

    let object_path = ObjectPath::from(record.object_key);
    let _ = state.attachment_store.delete(&object_path).await;
    Ok(StatusCode::NO_CONTENT)
}

#[allow(clippy::too_many_lines)]
pub(crate) async fn issue_voice_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(path): Path<ChannelPath>,
    Json(payload): Json<VoiceTokenRequest>,
) -> Result<Json<VoiceTokenResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    enforce_media_token_rate_limit(&state, client_ip, auth.user_id, &path).await?;
    let (_, permissions) =
        channel_permission_snapshot(&state, auth.user_id, &path.guild_id, &path.channel_id).await?;
    if !permissions.contains(Permission::CreateMessage) {
        return Err(AuthFailure::Forbidden);
    }
    let livekit = state.livekit.clone().ok_or(AuthFailure::Internal)?;

    let room = LiveKitRoomName::try_from(format!(
        "filament.voice.{}.{}",
        path.guild_id, path.channel_id
    ))
    .map_err(|_| AuthFailure::Internal)?;
    let identity = LiveKitIdentity::try_from(format!("u.{}.{}", auth.user_id, Ulid::new()))
        .map_err(|_| AuthFailure::Internal)?;

    let mut grants = VideoGrants {
        room_join: true,
        room: room.as_str().to_owned(),
        ..VideoGrants::default()
    };
    let requested_publish = payload.can_publish.unwrap_or(true);
    let requested_subscribe = payload.can_subscribe.unwrap_or(false);
    let requested_sources = payload.publish_sources.as_deref().map_or_else(
        || vec![MediaPublishSource::Microphone],
        dedup_publish_sources,
    );
    let allowed_sources = allowed_publish_sources(permissions);
    let mut effective_sources = if requested_publish {
        requested_sources
            .into_iter()
            .filter(|source| allowed_sources.contains(source))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    effective_sources.sort_by_key(|source| match source {
        MediaPublishSource::Microphone => 0_u8,
        MediaPublishSource::Camera => 1_u8,
        MediaPublishSource::ScreenShare => 2_u8,
    });
    effective_sources.dedup();

    if effective_sources.iter().any(|source| {
        matches!(
            source,
            MediaPublishSource::Camera | MediaPublishSource::ScreenShare
        )
    }) {
        enforce_media_publish_rate_limit(&state, client_ip, auth.user_id, &path).await?;
    }

    grants.can_publish = !effective_sources.is_empty();
    grants.can_subscribe =
        requested_subscribe && permissions.contains(Permission::SubscribeStreams);
    grants.can_publish_data = grants.can_publish;
    grants.can_publish_sources = effective_sources
        .iter()
        .map(|source| String::from(source.as_livekit_source()))
        .collect();

    if !grants.can_publish && !grants.can_subscribe {
        return Err(AuthFailure::InvalidRequest);
    }

    if grants.can_subscribe {
        enforce_media_subscribe_cap(&state, auth.user_id, &path).await?;
    }

    let token = LiveKitAccessToken::with_api_key(&livekit.api_key, &livekit.api_secret)
        .with_identity(identity.as_str())
        .with_name(&auth.username)
        .with_ttl(state.runtime.livekit_token_ttl)
        .with_grants(grants.clone())
        .to_jwt()
        .map_err(|_| AuthFailure::Internal)?;

    write_audit_log(
        &state,
        Some(path.guild_id),
        auth.user_id,
        None,
        "media.token.issue",
        serde_json::json!({
            "channel_id": path.channel_id,
            "room": room.as_str(),
            "requested_publish_sources": payload.publish_sources.as_ref().map(|sources| {
                sources
                    .iter()
                    .map(|source| source.as_livekit_source())
                    .collect::<Vec<_>>()
            }),
            "effective_publish_sources": grants.can_publish_sources.clone(),
            "can_publish": grants.can_publish,
            "can_subscribe": grants.can_subscribe,
            "ttl_secs": state.runtime.livekit_token_ttl.as_secs(),
            "client_ip": client_ip.normalized(),
            "client_ip_source": client_ip.source().as_str(),
        }),
    )
    .await?;

    Ok(Json(VoiceTokenResponse {
        token,
        livekit_url: livekit.url.clone(),
        room: room.as_str().to_owned(),
        identity: identity.as_str().to_owned(),
        can_publish: grants.can_publish,
        can_subscribe: grants.can_subscribe,
        publish_sources: grants.can_publish_sources.clone(),
        expires_in_secs: state.runtime.livekit_token_ttl.as_secs(),
    }))
}
