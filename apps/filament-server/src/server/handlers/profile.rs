use axum::{
    body::Body,
    extract::{connect_info::ConnectInfo, Extension, Path, State},
    http::{header::CONTENT_LENGTH, header::CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue},
    response::Response,
    Json,
};
use futures_util::StreamExt;
use object_store::{path::Path as ObjectPath, ObjectStore};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::{collections::HashSet, net::SocketAddr};

use filament_core::{tokenize_markdown, ProfileAbout, UserId, Username};

use crate::server::{
    auth::{authenticate, enforce_auth_route_rate_limit, extract_client_ip, now_unix},
    core::{
        AppState, ProfileAvatarRecord, MAX_MIME_SNIFF_BYTES, MAX_PROFILE_AVATAR_MIME_CHARS,
        MAX_PROFILE_AVATAR_OBJECT_KEY_CHARS,
    },
    errors::AuthFailure,
    gateway_events,
    realtime::broadcast_user_event,
    types::{UpdateProfileRequest, UserPath, UserProfileResponse},
};

#[allow(clippy::too_many_lines)]
pub(crate) async fn update_my_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Json(payload): Json<UpdateProfileRequest>,
) -> Result<Json<UserProfileResponse>, AuthFailure> {
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    enforce_auth_route_rate_limit(&state, client_ip, "profile_update").await?;
    let auth = authenticate(&state, &headers).await?;

    let next_username = payload
        .username
        .map(Username::try_from)
        .transpose()
        .map_err(|_| AuthFailure::InvalidRequest)?;
    let next_about = payload
        .about_markdown
        .map(ProfileAbout::try_from)
        .transpose()
        .map_err(|_| AuthFailure::InvalidRequest)?;

    if next_username.is_none() && next_about.is_none() {
        return Err(AuthFailure::InvalidRequest);
    }

    if let Some(pool) = &state.db_pool {
        let row = match (next_username.as_ref(), next_about.as_ref()) {
            (Some(username), Some(about)) => sqlx::query(
                "UPDATE users
                 SET username = $2, about_markdown = $3
                 WHERE user_id = $1
                 RETURNING user_id, username, about_markdown, avatar_version",
            )
            .bind(auth.user_id.to_string())
            .bind(username.as_str())
            .bind(about.as_str())
            .fetch_optional(pool)
            .await
            .map_err(|error| {
                if is_unique_violation(&error) {
                    AuthFailure::InvalidRequest
                } else {
                    AuthFailure::Internal
                }
            })?,
            (Some(username), None) => sqlx::query(
                "UPDATE users
                 SET username = $2
                 WHERE user_id = $1
                 RETURNING user_id, username, about_markdown, avatar_version",
            )
            .bind(auth.user_id.to_string())
            .bind(username.as_str())
            .fetch_optional(pool)
            .await
            .map_err(|error| {
                if is_unique_violation(&error) {
                    AuthFailure::InvalidRequest
                } else {
                    AuthFailure::Internal
                }
            })?,
            (None, Some(about)) => sqlx::query(
                "UPDATE users
                 SET about_markdown = $2
                 WHERE user_id = $1
                 RETURNING user_id, username, about_markdown, avatar_version",
            )
            .bind(auth.user_id.to_string())
            .bind(about.as_str())
            .fetch_optional(pool)
            .await
            .map_err(|_| AuthFailure::Internal)?,
            (None, None) => None,
        };
        let row = row.ok_or(AuthFailure::Unauthorized)?;
        let response = profile_response_from_row(&row)?;
        broadcast_profile_update(
            &state,
            auth.user_id,
            &response,
            next_username.as_ref().map(filament_core::Username::as_str),
            next_about
                .as_ref()
                .map(|_| response.about_markdown.as_str()),
            next_about
                .as_ref()
                .map(|_| response.about_markdown_tokens.as_slice()),
        )
        .await?;
        return Ok(Json(response));
    }

    let current_username = auth.username;
    let mut users = state.users.write().await;
    let mut user = users
        .remove(&current_username)
        .ok_or(AuthFailure::Unauthorized)?;
    if let Some(about) = next_about.as_ref() {
        about.as_str().clone_into(&mut user.about_markdown);
    }
    if let Some(username) = next_username.as_ref() {
        if username.as_str() != user.username.as_str() && users.contains_key(username.as_str()) {
            users.insert(current_username, user);
            return Err(AuthFailure::InvalidRequest);
        }
        user.username = username.clone();
    }
    let user_id_text = user.id.to_string();
    let final_username = user.username.as_str().to_owned();
    let response = user_profile_response(
        user_id_text.clone(),
        final_username.clone(),
        &user.about_markdown,
        user.avatar_version,
    );
    users.insert(final_username.clone(), user);
    drop(users);

    state
        .user_ids
        .write()
        .await
        .insert(user_id_text, final_username);

    broadcast_profile_update(
        &state,
        auth.user_id,
        &response,
        next_username.as_ref().map(filament_core::Username::as_str),
        next_about
            .as_ref()
            .map(|_| response.about_markdown.as_str()),
        next_about
            .as_ref()
            .map(|_| response.about_markdown_tokens.as_slice()),
    )
    .await?;

    Ok(Json(response))
}

pub(crate) async fn get_user_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<UserPath>,
) -> Result<Json<UserProfileResponse>, AuthFailure> {
    let _auth = authenticate(&state, &headers).await?;
    let user_id = UserId::try_from(path.user_id).map_err(|_| AuthFailure::InvalidRequest)?;

    if let Some(pool) = &state.db_pool {
        let row = sqlx::query(
            "SELECT user_id, username, about_markdown, avatar_version
             FROM users
             WHERE user_id = $1",
        )
        .bind(user_id.to_string())
        .fetch_optional(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?
        .ok_or(AuthFailure::NotFound)?;
        return Ok(Json(profile_response_from_row(&row)?));
    }

    Ok(Json(profile_from_memory(&state, user_id).await?))
}

#[allow(clippy::too_many_lines)]
pub(crate) async fn upload_my_avatar(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    body: Body,
) -> Result<Json<UserProfileResponse>, AuthFailure> {
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    enforce_auth_route_rate_limit(&state, client_ip, "profile_avatar_upload").await?;
    let auth = authenticate(&state, &headers).await?;

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

    let object_key = format!("avatars/{}", auth.user_id);
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
    let max_avatar_bytes =
        u64::try_from(state.runtime.max_profile_avatar_bytes).map_err(|_| AuthFailure::Internal)?;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| AuthFailure::InvalidRequest)?;
        if chunk.is_empty() {
            continue;
        }
        let chunk_len = u64::try_from(chunk.len()).map_err(|_| AuthFailure::InvalidRequest)?;
        total_size = total_size
            .checked_add(chunk_len)
            .ok_or(AuthFailure::PayloadTooLarge)?;
        if total_size > max_avatar_bytes {
            let _ = upload.abort().await;
            return Err(AuthFailure::PayloadTooLarge);
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
    if !is_allowed_avatar_mime(sniffed_mime) {
        let _ = upload.abort().await;
        return Err(AuthFailure::InvalidRequest);
    }
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
        let version_floor = now_unix();
        let row = sqlx::query(
            "UPDATE users
             SET avatar_object_key = $2,
                 avatar_mime_type = $3,
                 avatar_size_bytes = $4,
                 avatar_sha256_hex = $5,
                 avatar_version = GREATEST(COALESCE(avatar_version, 0) + 1, $6)
             WHERE user_id = $1
             RETURNING user_id, username, about_markdown, avatar_version",
        )
        .bind(auth.user_id.to_string())
        .bind(&object_key)
        .bind(sniffed_mime)
        .bind(i64::try_from(total_size).map_err(|_| AuthFailure::Internal)?)
        .bind(&sha256_hex)
        .bind(version_floor)
        .fetch_optional(pool)
        .await;
        let Ok(row) = row else {
            let _ = state.attachment_store.delete(&object_path).await;
            return Err(AuthFailure::Internal);
        };
        let Some(row) = row else {
            let _ = state.attachment_store.delete(&object_path).await;
            return Err(AuthFailure::Unauthorized);
        };
        let response = profile_response_from_row(&row)?;
        broadcast_profile_avatar_update(&state, auth.user_id, &response).await?;
        return Ok(Json(response));
    }

    let current_username = auth.username;
    let mut users = state.users.write().await;
    let mut user = users
        .remove(&current_username)
        .ok_or(AuthFailure::Unauthorized)?;
    let avatar_version = next_profile_version(user.avatar_version);
    user.avatar = Some(ProfileAvatarRecord {
        object_key,
        mime_type: sniffed_mime.to_owned(),
        size_bytes: total_size,
        sha256_hex,
    });
    user.avatar_version = avatar_version;
    let response = user_profile_response(
        user.id.to_string(),
        user.username.as_str().to_owned(),
        &user.about_markdown,
        user.avatar_version,
    );
    users.insert(current_username, user);
    broadcast_profile_avatar_update(&state, auth.user_id, &response).await?;
    Ok(Json(response))
}

pub(crate) async fn download_user_avatar(
    State(state): State<AppState>,
    Path(path): Path<UserPath>,
) -> Result<Response, AuthFailure> {
    let user_id = UserId::try_from(path.user_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let avatar = if let Some(pool) = &state.db_pool {
        let row = sqlx::query(
            "SELECT avatar_object_key, avatar_mime_type, avatar_size_bytes, avatar_sha256_hex
             FROM users
             WHERE user_id = $1",
        )
        .bind(user_id.to_string())
        .fetch_optional(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?
        .ok_or(AuthFailure::NotFound)?;
        avatar_from_row(&row)?
    } else {
        let username = state
            .user_ids
            .read()
            .await
            .get(&user_id.to_string())
            .cloned()
            .ok_or(AuthFailure::NotFound)?;
        let users = state.users.read().await;
        users
            .get(&username)
            .and_then(|user| user.avatar.clone())
            .ok_or(AuthFailure::NotFound)?
    };

    let object_path = ObjectPath::from(avatar.object_key);
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
        HeaderValue::from_str(&avatar.mime_type).map_err(|_| AuthFailure::Internal)?;
    response.headers_mut().insert(CONTENT_TYPE, content_type);
    let content_len =
        HeaderValue::from_str(&avatar.size_bytes.to_string()).map_err(|_| AuthFailure::Internal)?;
    response.headers_mut().insert(CONTENT_LENGTH, content_len);
    response.headers_mut().insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    response.headers_mut().insert(
        HeaderName::from_static("cache-control"),
        HeaderValue::from_static("public, max-age=300"),
    );
    let etag = HeaderValue::from_str(&format!("\"{}\"", avatar.sha256_hex))
        .map_err(|_| AuthFailure::Internal)?;
    response
        .headers_mut()
        .insert(HeaderName::from_static("etag"), etag);
    Ok(response)
}

async fn profile_from_memory(
    state: &AppState,
    user_id: UserId,
) -> Result<UserProfileResponse, AuthFailure> {
    let username = state
        .user_ids
        .read()
        .await
        .get(&user_id.to_string())
        .cloned()
        .ok_or(AuthFailure::NotFound)?;
    let users = state.users.read().await;
    let user = users.get(&username).ok_or(AuthFailure::NotFound)?;
    Ok(user_profile_response(
        user.id.to_string(),
        user.username.as_str().to_owned(),
        &user.about_markdown,
        user.avatar_version,
    ))
}

fn profile_response_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<UserProfileResponse, AuthFailure> {
    let user_id: String = row.try_get("user_id").map_err(|_| AuthFailure::Internal)?;
    let username: String = row.try_get("username").map_err(|_| AuthFailure::Internal)?;
    let about_markdown: String = row
        .try_get("about_markdown")
        .map_err(|_| AuthFailure::Internal)?;
    let avatar_version: i64 = row
        .try_get("avatar_version")
        .map_err(|_| AuthFailure::Internal)?;
    Ok(user_profile_response(
        user_id,
        username,
        &about_markdown,
        avatar_version,
    ))
}

fn user_profile_response(
    user_id: String,
    username: String,
    about_markdown: &str,
    avatar_version: i64,
) -> UserProfileResponse {
    UserProfileResponse {
        user_id,
        username,
        about_markdown: about_markdown.to_owned(),
        about_markdown_tokens: tokenize_markdown(about_markdown),
        avatar_version,
    }
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(db_error) => db_error.code().as_deref() == Some("23505"),
        _ => false,
    }
}

fn is_allowed_avatar_mime(mime_type: &str) -> bool {
    matches!(
        mime_type,
        "image/jpeg" | "image/png" | "image/webp" | "image/gif" | "image/avif"
    )
}

fn avatar_from_row(row: &sqlx::postgres::PgRow) -> Result<ProfileAvatarRecord, AuthFailure> {
    let object_key: Option<String> = row
        .try_get("avatar_object_key")
        .map_err(|_| AuthFailure::Internal)?;
    let mime_type: Option<String> = row
        .try_get("avatar_mime_type")
        .map_err(|_| AuthFailure::Internal)?;
    let size_bytes: Option<i64> = row
        .try_get("avatar_size_bytes")
        .map_err(|_| AuthFailure::Internal)?;
    let sha256_hex: Option<String> = row
        .try_get("avatar_sha256_hex")
        .map_err(|_| AuthFailure::Internal)?;
    let (Some(object_key), Some(mime_type), Some(size_bytes), Some(sha256_hex)) =
        (object_key, mime_type, size_bytes, sha256_hex)
    else {
        return Err(AuthFailure::NotFound);
    };
    if object_key.is_empty() || object_key.len() > MAX_PROFILE_AVATAR_OBJECT_KEY_CHARS {
        return Err(AuthFailure::Internal);
    }
    if mime_type.is_empty() || mime_type.len() > MAX_PROFILE_AVATAR_MIME_CHARS {
        return Err(AuthFailure::Internal);
    }
    if !is_allowed_avatar_mime(&mime_type) {
        return Err(AuthFailure::Internal);
    }
    if sha256_hex.len() != 64 || !sha256_hex.chars().all(|value| value.is_ascii_hexdigit()) {
        return Err(AuthFailure::Internal);
    }
    Ok(ProfileAvatarRecord {
        object_key,
        mime_type,
        size_bytes: u64::try_from(size_bytes).map_err(|_| AuthFailure::Internal)?,
        sha256_hex,
    })
}

fn next_profile_version(current: i64) -> i64 {
    let now = now_unix();
    if now > current {
        now
    } else {
        current.saturating_add(1)
    }
}

async fn broadcast_profile_update(
    state: &AppState,
    actor_user_id: UserId,
    response: &UserProfileResponse,
    username: Option<&str>,
    about_markdown: Option<&str>,
    about_markdown_tokens: Option<&[filament_core::MarkdownToken]>,
) -> Result<(), AuthFailure> {
    let updated_at_unix = now_unix();
    let event = gateway_events::profile_update(
        &response.user_id,
        username,
        about_markdown,
        about_markdown_tokens,
        updated_at_unix,
    );
    broadcast_user_event(state, actor_user_id, &event).await;

    for observer in profile_observer_user_ids(state, actor_user_id).await? {
        broadcast_user_event(state, observer, &event).await;
    }
    Ok(())
}

async fn broadcast_profile_avatar_update(
    state: &AppState,
    actor_user_id: UserId,
    response: &UserProfileResponse,
) -> Result<(), AuthFailure> {
    let updated_at_unix = now_unix();
    let event = gateway_events::profile_avatar_update(
        &response.user_id,
        response.avatar_version,
        updated_at_unix,
    );
    broadcast_user_event(state, actor_user_id, &event).await;

    for observer in profile_observer_user_ids(state, actor_user_id).await? {
        broadcast_user_event(state, observer, &event).await;
    }
    Ok(())
}

async fn profile_observer_user_ids(
    state: &AppState,
    user_id: UserId,
) -> Result<Vec<UserId>, AuthFailure> {
    if let Some(pool) = &state.db_pool {
        let friend_rows = sqlx::query(
            "SELECT CASE
                 WHEN user_a_id = $1 THEN user_b_id
                 ELSE user_a_id
             END AS friend_user_id
             FROM friendships
             WHERE user_a_id = $1 OR user_b_id = $1",
        )
        .bind(user_id.to_string())
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;

        let guild_member_rows = sqlx::query(
            "SELECT DISTINCT gm_other.user_id AS observer_user_id
             FROM guild_members gm_self
             INNER JOIN guild_members gm_other
                 ON gm_other.guild_id = gm_self.guild_id
             WHERE gm_self.user_id = $1
               AND gm_other.user_id <> $1",
        )
        .bind(user_id.to_string())
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;

        let mut unique = HashSet::new();
        for row in friend_rows {
            let friend_user_id: String = row
                .try_get("friend_user_id")
                .map_err(|_| AuthFailure::Internal)?;
            let friend_user_id =
                UserId::try_from(friend_user_id).map_err(|_| AuthFailure::Internal)?;
            if friend_user_id != user_id {
                unique.insert(friend_user_id);
            }
        }
        for row in guild_member_rows {
            let observer_user_id: String = row
                .try_get("observer_user_id")
                .map_err(|_| AuthFailure::Internal)?;
            let observer_user_id =
                UserId::try_from(observer_user_id).map_err(|_| AuthFailure::Internal)?;
            if observer_user_id != user_id {
                unique.insert(observer_user_id);
            }
        }
        return Ok(unique.into_iter().collect());
    }

    let user_id_text = user_id.to_string();
    let friendships = state.friendships.read().await;
    let mut unique = HashSet::new();
    for (user_a, user_b) in &*friendships {
        if user_a == &user_id_text {
            if let Ok(friend_id) = UserId::try_from(user_b.clone()) {
                unique.insert(friend_id);
            }
        } else if user_b == &user_id_text {
            if let Ok(friend_id) = UserId::try_from(user_a.clone()) {
                unique.insert(friend_id);
            }
        }
    }
    drop(friendships);

    let guilds = state.membership_store.guilds().read().await;
    for guild in guilds.values() {
        if !guild.members.contains_key(&user_id) {
            continue;
        }
        for observer_user_id in guild.members.keys() {
            if *observer_user_id != user_id {
                unique.insert(*observer_user_id);
            }
        }
    }

    Ok(unique.into_iter().collect())
}
