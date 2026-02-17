use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use axum::{
    extract::{connect_info::ConnectInfo, Extension, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use sqlx::Row;
use ulid::Ulid;

use filament_core::{tokenize_markdown, UserId, Username};

use crate::server::{
    auth::{
        authenticate, enforce_auth_route_rate_limit, extract_client_ip, find_username_by_user_id,
        hash_password, hash_refresh_token, issue_tokens, now_unix, validate_password, ClientIp,
    },
    auth_repository::{
        refresh_session_ttl_unix, AuthPersistence, AuthRepository, RefreshCheckError,
    },
    core::{AppState, ACCESS_TOKEN_TTL_SECS, MAX_USER_LOOKUP_IDS},
    errors::AuthFailure,
    types::{
        AuthResponse, CaptchaToken, HcaptchaVerifyResponse, LoginRequest, MeResponse,
        RefreshRequest, RegisterRequest, RegisterResponse, UserLookupItem, UserLookupRequest,
        UserLookupResponse,
    },
};

pub(crate) async fn verify_captcha_token(
    state: &AppState,
    client_ip: ClientIp,
    token: Option<String>,
) -> Result<(), AuthFailure> {
    let Some(config) = state.runtime.captcha.clone() else {
        return Ok(());
    };

    let token = token
        .ok_or(AuthFailure::CaptchaFailed)
        .and_then(|raw| CaptchaToken::try_from(raw).map_err(|()| AuthFailure::CaptchaFailed))?;

    let mut form_data = vec![
        ("secret", config.secret.clone()),
        ("response", token.as_str().to_owned()),
    ];
    if let Some(remote_ip) = client_ip.ip() {
        form_data.push(("remoteip", remote_ip.to_string()));
    }
    let response = state
        .http_client
        .post(&config.verify_url)
        .timeout(config.verify_timeout)
        .form(&form_data)
        .send()
        .await
        .map_err(|_| AuthFailure::CaptchaFailed)?;
    let verify: HcaptchaVerifyResponse = response
        .json()
        .await
        .map_err(|_| AuthFailure::CaptchaFailed)?;
    if !verify.success {
        return Err(AuthFailure::CaptchaFailed);
    }
    Ok(())
}

pub(crate) async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, AuthFailure> {
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    enforce_auth_route_rate_limit(&state, client_ip, "register").await?;
    verify_captcha_token(&state, client_ip, payload.captcha_token).await?;

    let username = Username::try_from(payload.username).map_err(|_| AuthFailure::InvalidRequest)?;
    validate_password(&payload.password).map_err(|_| AuthFailure::InvalidRequest)?;
    let password_hash = hash_password(&payload.password).map_err(|_| AuthFailure::Internal)?;
    let repository = AuthRepository::from_state(&state);

    let created = repository
        .create_user_if_missing(&username, &password_hash)
        .await?;

    if !created {
        tracing::info!(event = "auth.register", outcome = "existing_user");
        return Ok(Json(RegisterResponse { accepted: true }));
    }

    tracing::info!(event = "auth.register", outcome = "created");

    Ok(Json(RegisterResponse { accepted: true }))
}

#[allow(clippy::too_many_lines)]
pub(crate) async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, AuthFailure> {
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    enforce_auth_route_rate_limit(&state, client_ip, "login").await?;

    let username = Username::try_from(payload.username).map_err(|_| AuthFailure::Unauthorized)?;
    validate_password(&payload.password).map_err(|_| AuthFailure::Unauthorized)?;
    let now = now_unix();
    let repository = AuthRepository::from_state(&state);
    let user_id = repository
        .verify_credentials(
            &username,
            &payload.password,
            &state.dummy_password_hash,
            now,
        )
        .await?;
    let Some(user_id) = user_id else {
        tracing::warn!(event = "auth.login", outcome = "invalid_credentials");
        return Err(AuthFailure::Unauthorized);
    };

    let session_id = Ulid::new().to_string();
    let (access_token, refresh_token, refresh_hash) =
        issue_tokens(&state, user_id, username.as_str(), &session_id)
            .map_err(|_| AuthFailure::Internal)?;
    repository
        .insert_session(
            &session_id,
            user_id,
            refresh_hash,
            refresh_session_ttl_unix(now),
        )
        .await?;

    tracing::info!(event = "auth.login", outcome = "success", user_id = %user_id);

    Ok(Json(AuthResponse {
        access_token,
        refresh_token,
        expires_in_secs: ACCESS_TOKEN_TTL_SECS,
    }))
}

#[allow(clippy::too_many_lines)]
pub(crate) async fn refresh(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Json(payload): Json<RefreshRequest>,
) -> Result<Json<AuthResponse>, AuthFailure> {
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    enforce_auth_route_rate_limit(&state, client_ip, "refresh").await?;

    if payload.refresh_token.is_empty() || payload.refresh_token.len() > 512 {
        tracing::warn!(event = "auth.refresh", outcome = "invalid_token_format");
        return Err(AuthFailure::Unauthorized);
    }

    let now = now_unix();
    let repository = AuthRepository::from_state(&state);
    let refresh_check = repository
        .check_refresh_token(&payload.refresh_token, now)
        .await
        .map_err(|error| match error {
            RefreshCheckError::ReplayDetected { session_id } => {
                tracing::warn!(event = "auth.refresh", outcome = "replay_detected", session_id = %session_id);
                AuthFailure::Unauthorized
            }
            RefreshCheckError::Unauthorized { session_id } => {
                tracing::warn!(event = "auth.refresh", outcome = "rejected", session_id = %session_id);
                AuthFailure::Unauthorized
            }
            RefreshCheckError::Internal => AuthFailure::Internal,
        })?;

    let session_id = refresh_check.session_id;
    let user_id = refresh_check.user_id;
    let token_hash = refresh_check.presented_hash;
    let now = now_unix();
    let username = find_username_by_user_id(&state, user_id)
        .await
        .ok_or(AuthFailure::Unauthorized)?;

    let (access_token, refresh_token, refresh_hash) =
        issue_tokens(&state, user_id, &username, &session_id).map_err(|_| AuthFailure::Internal)?;
    repository
        .rotate_refresh_token(
            &session_id,
            token_hash,
            refresh_hash,
            now,
            refresh_session_ttl_unix(now),
        )
        .await?;

    tracing::info!(event = "auth.refresh", outcome = "success", session_id = %session_id, user_id = %user_id);

    Ok(Json(AuthResponse {
        access_token,
        refresh_token,
        expires_in_secs: ACCESS_TOKEN_TTL_SECS,
    }))
}

pub(crate) async fn logout(
    State(state): State<AppState>,
    Json(payload): Json<RefreshRequest>,
) -> Result<StatusCode, AuthFailure> {
    if payload.refresh_token.is_empty() || payload.refresh_token.len() > 512 {
        tracing::warn!(event = "auth.logout", outcome = "invalid_token_format");
        return Err(AuthFailure::Unauthorized);
    }

    let session_id = payload
        .refresh_token
        .split('.')
        .next()
        .ok_or(AuthFailure::Unauthorized)?
        .to_owned();
    let token_hash = hash_refresh_token(&payload.refresh_token);
    let repository = AuthRepository::from_state(&state);
    let user_id = repository
        .revoke_session_with_token(&session_id, token_hash)
        .await
        .map_err(|_| {
            tracing::warn!(event = "auth.logout", outcome = "hash_mismatch", session_id = %session_id);
            AuthFailure::Unauthorized
        })?;
    tracing::info!(event = "auth.logout", outcome = "success", session_id = %session_id, user_id = %user_id);
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<MeResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;

    if let Some(pool) = &state.db_pool {
        let row = sqlx::query(
            "SELECT username, about_markdown, avatar_version
             FROM users
             WHERE user_id = $1",
        )
        .bind(auth.user_id.to_string())
        .fetch_optional(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?
        .ok_or(AuthFailure::Unauthorized)?;
        let username: String = row.try_get("username").map_err(|_| AuthFailure::Internal)?;
        let about_markdown: String = row
            .try_get("about_markdown")
            .map_err(|_| AuthFailure::Internal)?;
        let avatar_version: i64 = row
            .try_get("avatar_version")
            .map_err(|_| AuthFailure::Internal)?;

        return Ok(Json(MeResponse {
            user_id: auth.user_id.to_string(),
            username,
            about_markdown: about_markdown.clone(),
            about_markdown_tokens: tokenize_markdown(&about_markdown),
            avatar_version,
        }));
    }

    let users = state.users.read().await;
    let user = users.get(&auth.username).ok_or(AuthFailure::Unauthorized)?;

    Ok(Json(MeResponse {
        user_id: auth.user_id.to_string(),
        username: user.username.as_str().to_owned(),
        about_markdown: user.about_markdown.clone(),
        about_markdown_tokens: tokenize_markdown(&user.about_markdown),
        avatar_version: user.avatar_version,
    }))
}

pub(crate) async fn lookup_users(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UserLookupRequest>,
) -> Result<Json<UserLookupResponse>, AuthFailure> {
    let _auth = authenticate(&state, &headers).await?;
    if payload.user_ids.is_empty() || payload.user_ids.len() > MAX_USER_LOOKUP_IDS {
        return Err(AuthFailure::InvalidRequest);
    }

    let mut deduped = Vec::with_capacity(payload.user_ids.len());
    let mut seen = HashSet::with_capacity(payload.user_ids.len());
    for raw_user_id in payload.user_ids {
        let user_id = UserId::try_from(raw_user_id).map_err(|_| AuthFailure::InvalidRequest)?;
        if seen.insert(user_id) {
            deduped.push(user_id);
        }
    }
    if deduped.is_empty() {
        return Err(AuthFailure::InvalidRequest);
    }

    if let Some(pool) = &state.db_pool {
        let user_ids: Vec<String> = deduped.iter().map(ToString::to_string).collect();
        let rows = sqlx::query(
            "SELECT user_id, username, avatar_version
             FROM users
             WHERE user_id = ANY($1)",
        )
        .bind(&user_ids)
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;

        let mut users_by_id = HashMap::with_capacity(rows.len());
        for row in rows {
            let user_id: String = row.try_get("user_id").map_err(|_| AuthFailure::Internal)?;
            let username: String = row.try_get("username").map_err(|_| AuthFailure::Internal)?;
            let avatar_version: i64 = row
                .try_get("avatar_version")
                .map_err(|_| AuthFailure::Internal)?;
            users_by_id.insert(user_id, (username, avatar_version));
        }

        let users = user_ids
            .iter()
            .filter_map(|user_id| {
                users_by_id
                    .get(user_id)
                    .map(|(username, avatar_version)| UserLookupItem {
                        user_id: user_id.clone(),
                        username: username.clone(),
                        avatar_version: *avatar_version,
                    })
            })
            .collect();
        return Ok(Json(UserLookupResponse { users }));
    }

    let user_ids_map = state.user_ids.read().await;
    let users = deduped
        .into_iter()
        .filter_map(|user_id| {
            let user_id_text = user_id.to_string();
            user_ids_map
                .get(&user_id_text)
                .cloned()
                .map(|username| UserLookupItem {
                    user_id: user_id_text,
                    username,
                    avatar_version: 0,
                })
        })
        .collect();

    Ok(Json(UserLookupResponse { users }))
}
