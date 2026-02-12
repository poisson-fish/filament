#[allow(clippy::wildcard_imports)]
use crate::server::*;

pub(crate) async fn verify_captcha_token(
    state: &AppState,
    headers: &HeaderMap,
    token: Option<String>,
) -> Result<(), AuthFailure> {
    let Some(config) = state.runtime.captcha.clone() else {
        return Ok(());
    };

    let token = token
        .ok_or(AuthFailure::CaptchaFailed)
        .and_then(|raw| CaptchaToken::try_from(raw).map_err(|()| AuthFailure::CaptchaFailed))?;

    let client = Client::builder()
        .timeout(config.verify_timeout)
        .build()
        .map_err(|_| AuthFailure::Internal)?;
    let remote_ip = extract_client_ip(headers);
    let response = client
        .post(&config.verify_url)
        .form(&[
            ("secret", config.secret.as_str()),
            ("response", token.as_str()),
            ("remoteip", remote_ip.as_str()),
        ])
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
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, AuthFailure> {
    enforce_auth_route_rate_limit(&state, &headers, "register").await?;
    ensure_db_schema(&state).await?;
    verify_captcha_token(&state, &headers, payload.captcha_token).await?;

    let username = Username::try_from(payload.username).map_err(|_| AuthFailure::InvalidRequest)?;
    validate_password(&payload.password).map_err(|_| AuthFailure::InvalidRequest)?;

    if let Some(pool) = &state.db_pool {
        let password_hash = hash_password(&payload.password).map_err(|_| AuthFailure::Internal)?;
        let user_id = UserId::new();
        let insert_result = sqlx::query(
            "INSERT INTO users (user_id, username, password_hash, failed_logins, locked_until_unix)
             VALUES ($1, $2, $3, 0, NULL)
             ON CONFLICT (username) DO NOTHING",
        )
        .bind(user_id.to_string())
        .bind(username.as_str())
        .bind(password_hash)
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;

        if insert_result.rows_affected() == 0 {
            tracing::info!(event = "auth.register", outcome = "existing_user");
        } else {
            tracing::info!(event = "auth.register", outcome = "created", user_id = %user_id);
        }
        return Ok(Json(RegisterResponse { accepted: true }));
    }

    let mut users = state.users.write().await;
    if users.contains_key(username.as_str()) {
        tracing::info!(event = "auth.register", outcome = "existing_user");
        return Ok(Json(RegisterResponse { accepted: true }));
    }

    let password_hash = hash_password(&payload.password).map_err(|_| AuthFailure::Internal)?;
    let user_id = UserId::new();
    users.insert(
        username.as_str().to_owned(),
        UserRecord {
            id: user_id,
            username: username.clone(),
            password_hash,
            failed_logins: 0,
            locked_until_unix: None,
        },
    );
    drop(users);

    state
        .user_ids
        .write()
        .await
        .insert(user_id.to_string(), username.as_str().to_owned());
    tracing::info!(event = "auth.register", outcome = "created", user_id = %user_id);

    Ok(Json(RegisterResponse { accepted: true }))
}

#[allow(clippy::too_many_lines)]
pub(crate) async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, AuthFailure> {
    enforce_auth_route_rate_limit(&state, &headers, "login").await?;
    ensure_db_schema(&state).await?;

    let username = Username::try_from(payload.username).map_err(|_| AuthFailure::Unauthorized)?;
    validate_password(&payload.password).map_err(|_| AuthFailure::Unauthorized)?;

    if let Some(pool) = &state.db_pool {
        let row = sqlx::query(
            "SELECT user_id, password_hash, failed_logins, locked_until_unix
             FROM users WHERE username = $1",
        )
        .bind(username.as_str())
        .fetch_optional(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;

        let now = now_unix();
        let (user_id, verified) = if let Some(row) = row {
            let user_id_text: String = row.try_get("user_id").map_err(|_| AuthFailure::Internal)?;
            let user_id = UserId::try_from(user_id_text).map_err(|_| AuthFailure::Internal)?;
            let password_hash: String = row
                .try_get("password_hash")
                .map_err(|_| AuthFailure::Internal)?;
            let failed_logins: i16 = row
                .try_get("failed_logins")
                .map_err(|_| AuthFailure::Internal)?;
            let locked_until_unix: Option<i64> = row
                .try_get("locked_until_unix")
                .map_err(|_| AuthFailure::Internal)?;

            if locked_until_unix.is_some_and(|lock_until| lock_until > now) {
                tracing::warn!(event = "auth.login", outcome = "locked", username = %username.as_str());
                return Err(AuthFailure::Unauthorized);
            }

            let verified = verify_password(&password_hash, &payload.password);
            if verified {
                sqlx::query(
                    "UPDATE users SET failed_logins = 0, locked_until_unix = NULL WHERE user_id = $1",
                )
                .bind(user_id.to_string())
                .execute(pool)
                .await
                .map_err(|_| AuthFailure::Internal)?;
                (Some(user_id), true)
            } else {
                let mut updated_failed = i32::from(failed_logins) + 1;
                let mut lock_until = None;
                if updated_failed >= i32::from(LOGIN_LOCK_THRESHOLD) {
                    updated_failed = 0;
                    lock_until = Some(now + LOGIN_LOCK_SECS);
                    tracing::warn!(event = "auth.login", outcome = "lockout", username = %username.as_str());
                } else {
                    tracing::warn!(event = "auth.login", outcome = "bad_password", username = %username.as_str());
                }
                sqlx::query(
                    "UPDATE users SET failed_logins = $2, locked_until_unix = $3 WHERE user_id = $1",
                )
                .bind(user_id.to_string())
                .bind(i16::try_from(updated_failed).unwrap_or(i16::MAX))
                .bind(lock_until)
                .execute(pool)
                .await
                .map_err(|_| AuthFailure::Internal)?;
                (None, false)
            }
        } else {
            let _ = verify_password(&state.dummy_password_hash, &payload.password);
            tracing::warn!(event = "auth.login", outcome = "invalid_credentials");
            (None, false)
        };

        if !verified {
            return Err(AuthFailure::Unauthorized);
        }
        let user_id = user_id.ok_or(AuthFailure::Internal)?;

        let session_id = Ulid::new().to_string();
        let (access_token, refresh_token, refresh_hash) =
            issue_tokens(&state, user_id, username.as_str(), &session_id)
                .map_err(|_| AuthFailure::Internal)?;
        sqlx::query(
            "INSERT INTO sessions (session_id, user_id, refresh_token_hash, expires_at_unix, revoked)
             VALUES ($1, $2, $3, $4, FALSE)",
        )
        .bind(&session_id)
        .bind(user_id.to_string())
        .bind(refresh_hash.as_slice())
        .bind(now + REFRESH_TOKEN_TTL_SECS)
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        tracing::info!(event = "auth.login", outcome = "success", user_id = %user_id);

        return Ok(Json(AuthResponse {
            access_token,
            refresh_token,
            expires_in_secs: ACCESS_TOKEN_TTL_SECS,
        }));
    }

    let mut users = state.users.write().await;
    let now = now_unix();
    let mut user_id = None;
    let mut username_text = None;
    let mut verified = false;

    if let Some(user) = users.get_mut(username.as_str()) {
        if user
            .locked_until_unix
            .is_some_and(|lock_until| lock_until > now)
        {
            tracing::warn!(event = "auth.login", outcome = "locked", username = %username.as_str());
            return Err(AuthFailure::Unauthorized);
        }

        verified = verify_password(&user.password_hash, &payload.password);
        if verified {
            user.failed_logins = 0;
            user.locked_until_unix = None;
            user_id = Some(user.id);
            username_text = Some(user.username.as_str().to_owned());
        } else {
            user.failed_logins = user.failed_logins.saturating_add(1);
            if user.failed_logins >= LOGIN_LOCK_THRESHOLD {
                user.locked_until_unix = Some(now + LOGIN_LOCK_SECS);
                user.failed_logins = 0;
                tracing::warn!(event = "auth.login", outcome = "lockout", username = %username.as_str());
            } else {
                tracing::warn!(event = "auth.login", outcome = "bad_password", username = %username.as_str());
            }
        }
    } else {
        let _ = verify_password(&state.dummy_password_hash, &payload.password);
        tracing::warn!(event = "auth.login", outcome = "invalid_credentials");
    }
    drop(users);

    if !verified {
        return Err(AuthFailure::Unauthorized);
    }

    let user_id = user_id.ok_or(AuthFailure::Internal)?;
    let username_text = username_text.ok_or(AuthFailure::Internal)?;
    let session_id = Ulid::new().to_string();
    let (access_token, refresh_token, refresh_hash) =
        issue_tokens(&state, user_id, &username_text, &session_id)
            .map_err(|_| AuthFailure::Internal)?;
    state.sessions.write().await.insert(
        session_id,
        SessionRecord {
            user_id,
            refresh_token_hash: refresh_hash,
            expires_at_unix: now + REFRESH_TOKEN_TTL_SECS,
            revoked: false,
        },
    );
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
    Json(payload): Json<RefreshRequest>,
) -> Result<Json<AuthResponse>, AuthFailure> {
    enforce_auth_route_rate_limit(&state, &headers, "refresh").await?;
    ensure_db_schema(&state).await?;

    if payload.refresh_token.is_empty() || payload.refresh_token.len() > 512 {
        tracing::warn!(event = "auth.refresh", outcome = "invalid_token_format");
        return Err(AuthFailure::Unauthorized);
    }

    if let Some(pool) = &state.db_pool {
        let token_hash = hash_refresh_token(&payload.refresh_token);
        if let Some(row) =
            sqlx::query("SELECT session_id FROM used_refresh_tokens WHERE token_hash = $1")
                .bind(token_hash.as_slice())
                .fetch_optional(pool)
                .await
                .map_err(|_| AuthFailure::Internal)?
        {
            let replay_session_id: String = row
                .try_get("session_id")
                .map_err(|_| AuthFailure::Internal)?;
            sqlx::query("UPDATE sessions SET revoked = TRUE WHERE session_id = $1")
                .bind(&replay_session_id)
                .execute(pool)
                .await
                .map_err(|_| AuthFailure::Internal)?;
            tracing::warn!(event = "auth.refresh", outcome = "replay_detected", session_id = %replay_session_id);
            return Err(AuthFailure::Unauthorized);
        }

        let session_id = payload
            .refresh_token
            .split('.')
            .next()
            .ok_or(AuthFailure::Unauthorized)?
            .to_owned();

        let row = sqlx::query(
            "SELECT user_id, refresh_token_hash, expires_at_unix, revoked
             FROM sessions WHERE session_id = $1",
        )
        .bind(&session_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        let row = row.ok_or(AuthFailure::Unauthorized)?;

        let session_user_id: String = row.try_get("user_id").map_err(|_| AuthFailure::Internal)?;
        let refresh_hash: Vec<u8> = row
            .try_get("refresh_token_hash")
            .map_err(|_| AuthFailure::Internal)?;
        let expires_at_unix: i64 = row
            .try_get("expires_at_unix")
            .map_err(|_| AuthFailure::Internal)?;
        let revoked: bool = row.try_get("revoked").map_err(|_| AuthFailure::Internal)?;

        if revoked
            || expires_at_unix < now_unix()
            || refresh_hash.as_slice() != token_hash.as_slice()
        {
            tracing::warn!(event = "auth.refresh", outcome = "rejected", session_id = %session_id);
            return Err(AuthFailure::Unauthorized);
        }

        let user_id = UserId::try_from(session_user_id).map_err(|_| AuthFailure::Internal)?;
        let username = find_username_by_user_id(&state, user_id)
            .await
            .ok_or(AuthFailure::Unauthorized)?;

        let (access_token, refresh_token, rotated_hash) =
            issue_tokens(&state, user_id, &username, &session_id)
                .map_err(|_| AuthFailure::Internal)?;
        sqlx::query(
            "UPDATE sessions SET refresh_token_hash = $2, expires_at_unix = $3 WHERE session_id = $1",
        )
        .bind(&session_id)
        .bind(rotated_hash.as_slice())
        .bind(now_unix() + REFRESH_TOKEN_TTL_SECS)
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;

        sqlx::query(
            "INSERT INTO used_refresh_tokens (token_hash, session_id) VALUES ($1, $2)
             ON CONFLICT (token_hash) DO NOTHING",
        )
        .bind(token_hash.as_slice())
        .bind(&session_id)
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;

        tracing::info!(event = "auth.refresh", outcome = "success", session_id = %session_id, user_id = %user_id);
        return Ok(Json(AuthResponse {
            access_token,
            refresh_token,
            expires_in_secs: ACCESS_TOKEN_TTL_SECS,
        }));
    }

    let token_hash = hash_refresh_token(&payload.refresh_token);
    if let Some(session_id) = state
        .used_refresh_tokens
        .read()
        .await
        .get(&token_hash)
        .cloned()
    {
        if let Some(session) = state.sessions.write().await.get_mut(&session_id) {
            session.revoked = true;
        }
        tracing::warn!(event = "auth.refresh", outcome = "replay_detected", session_id = %session_id);
        return Err(AuthFailure::Unauthorized);
    }

    let session_id = payload
        .refresh_token
        .split('.')
        .next()
        .ok_or(AuthFailure::Unauthorized)?
        .to_owned();

    let mut sessions = state.sessions.write().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or(AuthFailure::Unauthorized)?;
    if session.revoked
        || session.expires_at_unix < now_unix()
        || session.refresh_token_hash != token_hash
    {
        tracing::warn!(event = "auth.refresh", outcome = "rejected", session_id = %session_id);
        return Err(AuthFailure::Unauthorized);
    }

    let user_id = session.user_id;
    let username = find_username_by_user_id(&state, user_id)
        .await
        .ok_or(AuthFailure::Unauthorized)?;

    let old_hash = session.refresh_token_hash;
    let (access_token, refresh_token, refresh_hash) =
        issue_tokens(&state, user_id, &username, &session_id).map_err(|_| AuthFailure::Internal)?;
    session.refresh_token_hash = refresh_hash;
    session.expires_at_unix = now_unix() + REFRESH_TOKEN_TTL_SECS;
    drop(sessions);

    state
        .used_refresh_tokens
        .write()
        .await
        .insert(old_hash, session_id.clone());

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
    ensure_db_schema(&state).await?;

    if payload.refresh_token.is_empty() || payload.refresh_token.len() > 512 {
        tracing::warn!(event = "auth.logout", outcome = "invalid_token_format");
        return Err(AuthFailure::Unauthorized);
    }

    if let Some(pool) = &state.db_pool {
        let session_id = payload
            .refresh_token
            .split('.')
            .next()
            .ok_or(AuthFailure::Unauthorized)?
            .to_owned();
        let token_hash = hash_refresh_token(&payload.refresh_token);
        let row =
            sqlx::query("SELECT user_id, refresh_token_hash FROM sessions WHERE session_id = $1")
                .bind(&session_id)
                .fetch_optional(pool)
                .await
                .map_err(|_| AuthFailure::Internal)?;
        let row = row.ok_or(AuthFailure::Unauthorized)?;
        let user_id: String = row.try_get("user_id").map_err(|_| AuthFailure::Internal)?;
        let session_hash: Vec<u8> = row
            .try_get("refresh_token_hash")
            .map_err(|_| AuthFailure::Internal)?;

        if session_hash.as_slice() != token_hash.as_slice() {
            tracing::warn!(event = "auth.logout", outcome = "hash_mismatch", session_id = %session_id);
            return Err(AuthFailure::Unauthorized);
        }

        sqlx::query("UPDATE sessions SET revoked = TRUE WHERE session_id = $1")
            .bind(&session_id)
            .execute(pool)
            .await
            .map_err(|_| AuthFailure::Internal)?;
        tracing::info!(event = "auth.logout", outcome = "success", session_id = %session_id, user_id = %user_id);
        return Ok(StatusCode::NO_CONTENT);
    }

    let session_id = payload
        .refresh_token
        .split('.')
        .next()
        .ok_or(AuthFailure::Unauthorized)?
        .to_owned();
    let token_hash = hash_refresh_token(&payload.refresh_token);
    let mut sessions = state.sessions.write().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or(AuthFailure::Unauthorized)?;
    if session.refresh_token_hash != token_hash {
        tracing::warn!(event = "auth.logout", outcome = "hash_mismatch", session_id = %session_id);
        return Err(AuthFailure::Unauthorized);
    }
    session.revoked = true;
    tracing::info!(event = "auth.logout", outcome = "success", session_id = %session_id, user_id = %session.user_id);
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<MeResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;

    Ok(Json(MeResponse {
        user_id: auth.user_id.to_string(),
        username: auth.username,
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
        let rows = sqlx::query("SELECT user_id, username FROM users WHERE user_id = ANY($1)")
            .bind(&user_ids)
            .fetch_all(pool)
            .await
            .map_err(|_| AuthFailure::Internal)?;

        let mut usernames_by_id = HashMap::with_capacity(rows.len());
        for row in rows {
            let user_id: String = row.try_get("user_id").map_err(|_| AuthFailure::Internal)?;
            let username: String = row.try_get("username").map_err(|_| AuthFailure::Internal)?;
            usernames_by_id.insert(user_id, username);
        }

        let users = user_ids
            .iter()
            .filter_map(|user_id| {
                usernames_by_id.get(user_id).map(|username| UserLookupItem {
                    user_id: user_id.clone(),
                    username: username.clone(),
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
                })
        })
        .collect();

    Ok(Json(UserLookupResponse { users }))
}
