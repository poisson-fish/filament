use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use filament_core::UserId;
use sqlx::Row;
use ulid::Ulid;

use crate::server::{
    auth::{authenticate, now_unix},
    core::{AppState, FriendshipRequestRecord},
    db::ensure_db_schema,
    errors::AuthFailure,
    types::{
        CreateFriendRequest, FriendListResponse, FriendPath, FriendRecordResponse,
        FriendRequestPath, FriendshipRequestCreateResponse, FriendshipRequestListResponse,
        FriendshipRequestResponse, ModerationResponse,
    },
};

pub(crate) fn canonical_friend_pair(user_a: UserId, user_b: UserId) -> (String, String) {
    let left = user_a.to_string();
    let right = user_b.to_string();
    if left < right {
        (left, right)
    } else {
        (right, left)
    }
}

pub(crate) async fn create_friend_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateFriendRequest>,
) -> Result<Json<FriendshipRequestCreateResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    let recipient_user_id =
        UserId::try_from(payload.recipient_user_id).map_err(|_| AuthFailure::InvalidRequest)?;
    if recipient_user_id == auth.user_id {
        return Err(AuthFailure::InvalidRequest);
    }

    let request_id = Ulid::new().to_string();
    let created_at_unix = now_unix();
    let sender_id = auth.user_id.to_string();
    let recipient_id = recipient_user_id.to_string();
    let (pair_a, pair_b) = canonical_friend_pair(auth.user_id, recipient_user_id);

    if let Some(pool) = &state.db_pool {
        let recipient_exists = sqlx::query("SELECT 1 FROM users WHERE user_id = $1")
            .bind(&recipient_id)
            .fetch_optional(pool)
            .await
            .map_err(|_| AuthFailure::Internal)?;
        if recipient_exists.is_none() {
            return Err(AuthFailure::InvalidRequest);
        }

        let existing_friendship =
            sqlx::query("SELECT 1 FROM friendships WHERE user_a_id = $1 AND user_b_id = $2")
                .bind(&pair_a)
                .bind(&pair_b)
                .fetch_optional(pool)
                .await
                .map_err(|_| AuthFailure::Internal)?;
        if existing_friendship.is_some() {
            return Err(AuthFailure::InvalidRequest);
        }

        let existing_request = sqlx::query(
            "SELECT 1
             FROM friendship_requests
             WHERE (sender_user_id = $1 AND recipient_user_id = $2)
                OR (sender_user_id = $2 AND recipient_user_id = $1)",
        )
        .bind(&sender_id)
        .bind(&recipient_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        if existing_request.is_some() {
            return Err(AuthFailure::InvalidRequest);
        }

        sqlx::query(
            "INSERT INTO friendship_requests (request_id, sender_user_id, recipient_user_id, created_at_unix)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(&request_id)
        .bind(&sender_id)
        .bind(&recipient_id)
        .bind(created_at_unix)
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
    } else {
        let users = state.user_ids.read().await;
        if !users.contains_key(&recipient_id) {
            return Err(AuthFailure::InvalidRequest);
        }
        drop(users);

        let friendships = state.friendships.read().await;
        if friendships.contains(&(pair_a.clone(), pair_b.clone())) {
            return Err(AuthFailure::InvalidRequest);
        }
        drop(friendships);

        let requests = state.friendship_requests.read().await;
        let exists = requests.values().any(|request| {
            (request.sender_user_id == auth.user_id
                && request.recipient_user_id == recipient_user_id)
                || (request.sender_user_id == recipient_user_id
                    && request.recipient_user_id == auth.user_id)
        });
        if exists {
            return Err(AuthFailure::InvalidRequest);
        }
        drop(requests);

        state.friendship_requests.write().await.insert(
            request_id.clone(),
            FriendshipRequestRecord {
                sender_user_id: auth.user_id,
                recipient_user_id,
                created_at_unix,
            },
        );
    }

    Ok(Json(FriendshipRequestCreateResponse {
        request_id,
        sender_user_id: sender_id,
        recipient_user_id: recipient_id,
        created_at_unix,
    }))
}

#[allow(clippy::too_many_lines)]
pub(crate) async fn list_friend_requests(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<FriendshipRequestListResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    let auth_user_id = auth.user_id.to_string();

    if let Some(pool) = &state.db_pool {
        let incoming_rows = sqlx::query(
            "SELECT fr.request_id, fr.sender_user_id, su.username AS sender_username,
                    fr.recipient_user_id, ru.username AS recipient_username, fr.created_at_unix
             FROM friendship_requests fr
             JOIN users su ON su.user_id = fr.sender_user_id
             JOIN users ru ON ru.user_id = fr.recipient_user_id
             WHERE fr.recipient_user_id = $1
             ORDER BY fr.created_at_unix DESC",
        )
        .bind(&auth_user_id)
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        let outgoing_rows = sqlx::query(
            "SELECT fr.request_id, fr.sender_user_id, su.username AS sender_username,
                    fr.recipient_user_id, ru.username AS recipient_username, fr.created_at_unix
             FROM friendship_requests fr
             JOIN users su ON su.user_id = fr.sender_user_id
             JOIN users ru ON ru.user_id = fr.recipient_user_id
             WHERE fr.sender_user_id = $1
             ORDER BY fr.created_at_unix DESC",
        )
        .bind(&auth_user_id)
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;

        let mut incoming = Vec::with_capacity(incoming_rows.len());
        for row in incoming_rows {
            incoming.push(FriendshipRequestResponse {
                request_id: row
                    .try_get("request_id")
                    .map_err(|_| AuthFailure::Internal)?,
                sender_user_id: row
                    .try_get("sender_user_id")
                    .map_err(|_| AuthFailure::Internal)?,
                sender_username: row
                    .try_get("sender_username")
                    .map_err(|_| AuthFailure::Internal)?,
                recipient_user_id: row
                    .try_get("recipient_user_id")
                    .map_err(|_| AuthFailure::Internal)?,
                recipient_username: row
                    .try_get("recipient_username")
                    .map_err(|_| AuthFailure::Internal)?,
                created_at_unix: row
                    .try_get("created_at_unix")
                    .map_err(|_| AuthFailure::Internal)?,
            });
        }

        let mut outgoing = Vec::with_capacity(outgoing_rows.len());
        for row in outgoing_rows {
            outgoing.push(FriendshipRequestResponse {
                request_id: row
                    .try_get("request_id")
                    .map_err(|_| AuthFailure::Internal)?,
                sender_user_id: row
                    .try_get("sender_user_id")
                    .map_err(|_| AuthFailure::Internal)?,
                sender_username: row
                    .try_get("sender_username")
                    .map_err(|_| AuthFailure::Internal)?,
                recipient_user_id: row
                    .try_get("recipient_user_id")
                    .map_err(|_| AuthFailure::Internal)?,
                recipient_username: row
                    .try_get("recipient_username")
                    .map_err(|_| AuthFailure::Internal)?,
                created_at_unix: row
                    .try_get("created_at_unix")
                    .map_err(|_| AuthFailure::Internal)?,
            });
        }

        return Ok(Json(FriendshipRequestListResponse { incoming, outgoing }));
    }

    let requests = state.friendship_requests.read().await;
    let user_ids = state.user_ids.read().await;
    let mut incoming = Vec::new();
    let mut outgoing = Vec::new();

    for (request_id, request) in &*requests {
        if request.recipient_user_id == auth.user_id || request.sender_user_id == auth.user_id {
            let sender_id = request.sender_user_id.to_string();
            let recipient_id = request.recipient_user_id.to_string();
            let sender_username = user_ids
                .get(&sender_id)
                .cloned()
                .ok_or(AuthFailure::Internal)?;
            let recipient_username = user_ids
                .get(&recipient_id)
                .cloned()
                .ok_or(AuthFailure::Internal)?;
            let response = FriendshipRequestResponse {
                request_id: request_id.clone(),
                sender_user_id: sender_id,
                sender_username,
                recipient_user_id: recipient_id,
                recipient_username,
                created_at_unix: request.created_at_unix,
            };
            if request.recipient_user_id == auth.user_id {
                incoming.push(response);
            } else {
                outgoing.push(response);
            }
        }
    }

    incoming.sort_by(|left, right| right.created_at_unix.cmp(&left.created_at_unix));
    outgoing.sort_by(|left, right| right.created_at_unix.cmp(&left.created_at_unix));
    Ok(Json(FriendshipRequestListResponse { incoming, outgoing }))
}

pub(crate) async fn accept_friend_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<FriendRequestPath>,
) -> Result<Json<ModerationResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;

    if let Some(pool) = &state.db_pool {
        let row = sqlx::query(
            "SELECT sender_user_id, recipient_user_id
             FROM friendship_requests
             WHERE request_id = $1",
        )
        .bind(&path.request_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        let row = row.ok_or(AuthFailure::NotFound)?;
        let sender_user_id: String = row
            .try_get("sender_user_id")
            .map_err(|_| AuthFailure::Internal)?;
        let recipient_user_id: String = row
            .try_get("recipient_user_id")
            .map_err(|_| AuthFailure::Internal)?;
        if recipient_user_id != auth.user_id.to_string() {
            return Err(AuthFailure::NotFound);
        }
        let sender_user_id = UserId::try_from(sender_user_id).map_err(|_| AuthFailure::Internal)?;
        let (pair_a, pair_b) = canonical_friend_pair(sender_user_id, auth.user_id);
        let mut tx = pool.begin().await.map_err(|_| AuthFailure::Internal)?;
        sqlx::query(
            "INSERT INTO friendships (user_a_id, user_b_id, created_at_unix)
             VALUES ($1, $2, $3)
             ON CONFLICT (user_a_id, user_b_id) DO NOTHING",
        )
        .bind(&pair_a)
        .bind(&pair_b)
        .bind(now_unix())
        .execute(&mut *tx)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        sqlx::query("DELETE FROM friendship_requests WHERE request_id = $1")
            .bind(&path.request_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| AuthFailure::Internal)?;
        tx.commit().await.map_err(|_| AuthFailure::Internal)?;
        return Ok(Json(ModerationResponse { accepted: true }));
    }

    let mut requests = state.friendship_requests.write().await;
    let request = requests
        .get(&path.request_id)
        .cloned()
        .ok_or(AuthFailure::NotFound)?;
    if request.recipient_user_id != auth.user_id {
        return Err(AuthFailure::NotFound);
    }
    let (pair_a, pair_b) = canonical_friend_pair(request.sender_user_id, request.recipient_user_id);
    requests.remove(&path.request_id);
    drop(requests);
    state.friendships.write().await.insert((pair_a, pair_b));
    Ok(Json(ModerationResponse { accepted: true }))
}

pub(crate) async fn delete_friend_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<FriendRequestPath>,
) -> Result<StatusCode, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;

    if let Some(pool) = &state.db_pool {
        let row = sqlx::query(
            "SELECT sender_user_id, recipient_user_id
             FROM friendship_requests
             WHERE request_id = $1",
        )
        .bind(&path.request_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        let row = row.ok_or(AuthFailure::NotFound)?;
        let sender_user_id: String = row
            .try_get("sender_user_id")
            .map_err(|_| AuthFailure::Internal)?;
        let recipient_user_id: String = row
            .try_get("recipient_user_id")
            .map_err(|_| AuthFailure::Internal)?;
        let auth_id = auth.user_id.to_string();
        if sender_user_id != auth_id && recipient_user_id != auth_id {
            return Err(AuthFailure::NotFound);
        }

        sqlx::query("DELETE FROM friendship_requests WHERE request_id = $1")
            .bind(&path.request_id)
            .execute(pool)
            .await
            .map_err(|_| AuthFailure::Internal)?;
        return Ok(StatusCode::NO_CONTENT);
    }

    let mut requests = state.friendship_requests.write().await;
    let request = requests
        .get(&path.request_id)
        .cloned()
        .ok_or(AuthFailure::NotFound)?;
    if request.sender_user_id != auth.user_id && request.recipient_user_id != auth.user_id {
        return Err(AuthFailure::NotFound);
    }
    requests.remove(&path.request_id);
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn list_friends(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<FriendListResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    let auth_user_id = auth.user_id.to_string();

    if let Some(pool) = &state.db_pool {
        let rows = sqlx::query(
            "SELECT u.user_id, u.username, f.created_at_unix
             FROM friendships f
             JOIN users u
               ON u.user_id = CASE
                   WHEN f.user_a_id = $1 THEN f.user_b_id
                   ELSE f.user_a_id
               END
             WHERE f.user_a_id = $1 OR f.user_b_id = $1
             ORDER BY f.created_at_unix DESC",
        )
        .bind(&auth_user_id)
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;

        let mut friends = Vec::with_capacity(rows.len());
        for row in rows {
            friends.push(FriendRecordResponse {
                user_id: row.try_get("user_id").map_err(|_| AuthFailure::Internal)?,
                username: row.try_get("username").map_err(|_| AuthFailure::Internal)?,
                created_at_unix: row
                    .try_get("created_at_unix")
                    .map_err(|_| AuthFailure::Internal)?,
            });
        }
        return Ok(Json(FriendListResponse { friends }));
    }

    let friendships = state.friendships.read().await;
    let user_ids = state.user_ids.read().await;
    let mut friends = Vec::new();
    for (user_a, user_b) in &*friendships {
        let friend_user_id = if user_a == &auth_user_id {
            Some(user_b.clone())
        } else if user_b == &auth_user_id {
            Some(user_a.clone())
        } else {
            None
        };
        if let Some(friend_user_id) = friend_user_id {
            let Some(username) = user_ids.get(&friend_user_id).cloned() else {
                continue;
            };
            friends.push(FriendRecordResponse {
                user_id: friend_user_id,
                username,
                created_at_unix: 0,
            });
        }
    }
    friends.sort_by(|left, right| left.user_id.cmp(&right.user_id));
    Ok(Json(FriendListResponse { friends }))
}

pub(crate) async fn remove_friend(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<FriendPath>,
) -> Result<StatusCode, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    let friend_user_id =
        UserId::try_from(path.friend_user_id).map_err(|_| AuthFailure::InvalidRequest)?;
    if friend_user_id == auth.user_id {
        return Err(AuthFailure::InvalidRequest);
    }
    let (pair_a, pair_b) = canonical_friend_pair(auth.user_id, friend_user_id);

    if let Some(pool) = &state.db_pool {
        sqlx::query("DELETE FROM friendships WHERE user_a_id = $1 AND user_b_id = $2")
            .bind(&pair_a)
            .bind(&pair_b)
            .execute(pool)
            .await
            .map_err(|_| AuthFailure::Internal)?;
        return Ok(StatusCode::NO_CONTENT);
    }

    state.friendships.write().await.remove(&(pair_a, pair_b));
    Ok(StatusCode::NO_CONTENT)
}
