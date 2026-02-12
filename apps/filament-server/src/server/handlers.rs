async fn verify_captcha_token(
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

async fn register(
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
async fn login(
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
async fn refresh(
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

async fn logout(
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

async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<MeResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;

    Ok(Json(MeResponse {
        user_id: auth.user_id.to_string(),
        username: auth.username,
    }))
}

async fn lookup_users(
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

fn canonical_friend_pair(user_a: UserId, user_b: UserId) -> (String, String) {
    let left = user_a.to_string();
    let right = user_b.to_string();
    if left < right {
        (left, right)
    } else {
        (right, left)
    }
}

async fn create_friend_request(
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
async fn list_friend_requests(
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

async fn accept_friend_request(
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

async fn delete_friend_request(
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

async fn list_friends(
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

async fn remove_friend(
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

async fn create_guild(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateGuildRequest>,
) -> Result<Json<GuildResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    let name = GuildName::try_from(payload.name).map_err(|_| AuthFailure::InvalidRequest)?;
    let visibility = payload.visibility.unwrap_or(GuildVisibility::Private);

    let guild_id = Ulid::new().to_string();
    let creator_user_id = auth.user_id.to_string();
    let limit = state.runtime.max_created_guilds_per_user;
    if let Some(pool) = &state.db_pool {
        let mut tx = pool.begin().await.map_err(|_| AuthFailure::Internal)?;
        sqlx::query_scalar::<_, String>("SELECT user_id FROM users WHERE user_id = $1 FOR UPDATE")
            .bind(&creator_user_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| AuthFailure::Internal)?;
        let existing_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM guilds WHERE created_by_user_id = $1",
        )
        .bind(&creator_user_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        if existing_count >= i64::try_from(limit).map_err(|_| AuthFailure::Internal)? {
            tracing::warn!(
                event = "guild.create",
                outcome = "limit_reached",
                user_id = %auth.user_id,
                max_created_guilds_per_user = limit,
            );
            return Err(AuthFailure::GuildCreationLimitReached);
        }
        sqlx::query(
            "INSERT INTO guilds (guild_id, name, visibility, created_by_user_id, created_at_unix)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(&guild_id)
        .bind(name.as_str())
        .bind(visibility_to_i16(visibility))
        .bind(&creator_user_id)
        .bind(now_unix())
        .execute(&mut *tx)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        sqlx::query("INSERT INTO guild_members (guild_id, user_id, role) VALUES ($1, $2, $3)")
            .bind(&guild_id)
            .bind(&creator_user_id)
            .bind(role_to_i16(Role::Owner))
            .execute(&mut *tx)
            .await
            .map_err(|_| AuthFailure::Internal)?;
        tx.commit().await.map_err(|_| AuthFailure::Internal)?;

        return Ok(Json(GuildResponse {
            guild_id,
            name: name.as_str().to_owned(),
            visibility,
        }));
    }

    let mut members = HashMap::new();
    members.insert(auth.user_id, Role::Owner);

    let mut guilds = state.guilds.write().await;
    let current_count = guilds
        .values()
        .filter(|record| record.created_by_user_id == auth.user_id)
        .count();
    if current_count >= limit {
        tracing::warn!(
            event = "guild.create",
            outcome = "limit_reached",
            user_id = %auth.user_id,
            max_created_guilds_per_user = limit,
        );
        return Err(AuthFailure::GuildCreationLimitReached);
    }

    guilds.insert(
        guild_id.clone(),
        GuildRecord {
            name: name.as_str().to_owned(),
            visibility,
            created_by_user_id: auth.user_id,
            members,
            banned_members: HashSet::new(),
            channels: HashMap::new(),
        },
    );

    Ok(Json(GuildResponse {
        guild_id,
        name: name.as_str().to_owned(),
        visibility,
    }))
}

const MAX_GUILD_LIST_LIMIT: usize = 200;

async fn list_guilds(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<GuildListResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;

    if let Some(pool) = &state.db_pool {
        let rows = sqlx::query(
            "SELECT g.guild_id, g.name, g.visibility
             FROM guild_members gm
             JOIN guilds g ON g.guild_id = gm.guild_id
             LEFT JOIN guild_bans gb ON gb.guild_id = gm.guild_id AND gb.user_id = gm.user_id
             WHERE gm.user_id = $1
               AND gb.user_id IS NULL
             ORDER BY g.created_at_unix DESC
             LIMIT $2",
        )
        .bind(auth.user_id.to_string())
        .bind(i64::try_from(MAX_GUILD_LIST_LIMIT).map_err(|_| AuthFailure::Internal)?)
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;

        let mut guilds = Vec::with_capacity(rows.len());
        for row in rows {
            let visibility_raw: i16 = row
                .try_get("visibility")
                .map_err(|_| AuthFailure::Internal)?;
            let visibility = visibility_from_i16(visibility_raw).ok_or(AuthFailure::Internal)?;
            guilds.push(GuildResponse {
                guild_id: row.try_get("guild_id").map_err(|_| AuthFailure::Internal)?,
                name: row.try_get("name").map_err(|_| AuthFailure::Internal)?,
                visibility,
            });
        }
        return Ok(Json(GuildListResponse { guilds }));
    }

    let guilds = state.guilds.read().await;
    let mut response = guilds
        .iter()
        .filter_map(|(guild_id, guild)| {
            if guild.banned_members.contains(&auth.user_id) {
                return None;
            }
            if !guild.members.contains_key(&auth.user_id) {
                return None;
            }
            Some(GuildResponse {
                guild_id: guild_id.clone(),
                name: guild.name.clone(),
                visibility: guild.visibility,
            })
        })
        .collect::<Vec<_>>();
    response.sort_by(|left, right| right.guild_id.cmp(&left.guild_id));
    response.truncate(MAX_GUILD_LIST_LIMIT);
    Ok(Json(GuildListResponse { guilds: response }))
}

const DEFAULT_PUBLIC_GUILD_LIST_LIMIT: usize = 20;
const MAX_PUBLIC_GUILD_LIST_LIMIT: usize = 50;
const MAX_PUBLIC_GUILD_QUERY_CHARS: usize = 64;

async fn list_public_guilds(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PublicGuildListQuery>,
) -> Result<Json<PublicGuildListResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let _auth = authenticate(&state, &headers).await?;

    let limit = query.limit.unwrap_or(DEFAULT_PUBLIC_GUILD_LIST_LIMIT);
    if limit == 0 || limit > MAX_PUBLIC_GUILD_LIST_LIMIT {
        return Err(AuthFailure::InvalidRequest);
    }
    let needle = query.q.map(|value| value.trim().to_ascii_lowercase());
    if needle
        .as_ref()
        .is_some_and(|value| value.len() > MAX_PUBLIC_GUILD_QUERY_CHARS)
    {
        return Err(AuthFailure::InvalidRequest);
    }
    let has_query = needle.as_ref().is_some_and(|value| !value.is_empty());

    if let Some(pool) = &state.db_pool {
        let limit_i64 = i64::try_from(limit).map_err(|_| AuthFailure::InvalidRequest)?;
        let sql_like = needle
            .as_ref()
            .filter(|_| has_query)
            .map(|value| format!("%{value}%"));
        let rows = sqlx::query(
            "SELECT guild_id, name, visibility
             FROM guilds
             WHERE visibility = $1
               AND ($2::text IS NULL OR LOWER(name) LIKE $2)
             ORDER BY created_at_unix DESC
             LIMIT $3",
        )
        .bind(visibility_to_i16(GuildVisibility::Public))
        .bind(sql_like)
        .bind(limit_i64)
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;

        let mut guilds = Vec::with_capacity(rows.len());
        for row in rows {
            let visibility_raw: i16 = row
                .try_get("visibility")
                .map_err(|_| AuthFailure::Internal)?;
            let visibility = visibility_from_i16(visibility_raw).ok_or(AuthFailure::Internal)?;
            if visibility != GuildVisibility::Public {
                continue;
            }
            guilds.push(PublicGuildListItem {
                guild_id: row.try_get("guild_id").map_err(|_| AuthFailure::Internal)?,
                name: row.try_get("name").map_err(|_| AuthFailure::Internal)?,
                visibility,
            });
        }
        return Ok(Json(PublicGuildListResponse { guilds }));
    }

    let guilds = state.guilds.read().await;
    let query_term = needle
        .as_ref()
        .filter(|_| has_query)
        .map(std::string::String::as_str);
    let mut results = guilds
        .iter()
        .filter_map(|(guild_id, guild)| {
            if guild.visibility != GuildVisibility::Public {
                return None;
            }
            if let Some(term) = query_term {
                if !guild.name.to_ascii_lowercase().contains(term) {
                    return None;
                }
            }
            Some(PublicGuildListItem {
                guild_id: guild_id.clone(),
                name: guild.name.clone(),
                visibility: guild.visibility,
            })
        })
        .collect::<Vec<_>>();
    results.sort_by(|left, right| right.guild_id.cmp(&left.guild_id));
    results.truncate(limit);
    Ok(Json(PublicGuildListResponse { guilds: results }))
}

const MAX_CHANNEL_LIST_LIMIT: usize = 500;

async fn list_guild_channels(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<GuildPath>,
) -> Result<Json<ChannelListResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;

    if let Some(pool) = &state.db_pool {
        let rows = sqlx::query(
            "SELECT c.channel_id, c.name, c.kind, gm.role, co.allow_mask, co.deny_mask
             FROM guild_members gm
             JOIN channels c ON c.guild_id = gm.guild_id
             LEFT JOIN channel_role_overrides co
               ON co.guild_id = c.guild_id
              AND co.channel_id = c.channel_id
              AND co.role = gm.role
             LEFT JOIN guild_bans gb ON gb.guild_id = gm.guild_id AND gb.user_id = gm.user_id
             WHERE gm.guild_id = $1
               AND gm.user_id = $2
               AND gb.user_id IS NULL
             ORDER BY c.created_at_unix ASC
             LIMIT $3",
        )
        .bind(&path.guild_id)
        .bind(auth.user_id.to_string())
        .bind(i64::try_from(MAX_CHANNEL_LIST_LIMIT).map_err(|_| AuthFailure::Internal)?)
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;

        if rows.is_empty() {
            user_role_in_guild(&state, auth.user_id, &path.guild_id).await?;
            return Ok(Json(ChannelListResponse {
                channels: Vec::new(),
            }));
        }

        let mut channels = Vec::new();
        for row in rows {
            let role_value: i16 = row.try_get("role").map_err(|_| AuthFailure::Internal)?;
            let role = role_from_i16(role_value).ok_or(AuthFailure::Forbidden)?;
            let allow_mask = row.try_get::<Option<i64>, _>("allow_mask").ok().flatten();
            let deny_mask = row.try_get::<Option<i64>, _>("deny_mask").ok().flatten();
            let overwrite = if let (Some(allow), Some(deny)) = (allow_mask, deny_mask) {
                Some(ChannelPermissionOverwrite {
                    allow: permission_set_from_i64(allow)?,
                    deny: permission_set_from_i64(deny)?,
                })
            } else {
                None
            };
            let permissions = apply_channel_overwrite(base_permissions(role), overwrite);
            if !permissions.contains(Permission::CreateMessage) {
                continue;
            }
            let channel_kind_raw: i16 = row.try_get("kind").map_err(|_| AuthFailure::Internal)?;
            let kind = channel_kind_from_i16(channel_kind_raw).ok_or(AuthFailure::Internal)?;
            channels.push(ChannelResponse {
                channel_id: row
                    .try_get("channel_id")
                    .map_err(|_| AuthFailure::Internal)?,
                name: row.try_get("name").map_err(|_| AuthFailure::Internal)?,
                kind,
            });
        }
        return Ok(Json(ChannelListResponse { channels }));
    }

    let guilds = state.guilds.read().await;
    let guild = guilds.get(&path.guild_id).ok_or(AuthFailure::NotFound)?;
    let role = guild
        .members
        .get(&auth.user_id)
        .copied()
        .ok_or(AuthFailure::Forbidden)?;
    if guild.banned_members.contains(&auth.user_id) {
        return Err(AuthFailure::Forbidden);
    }

    let mut channels = guild
        .channels
        .iter()
        .filter_map(|(channel_id, channel)| {
            let overwrite = channel.role_overrides.get(&role).copied();
            let permissions = apply_channel_overwrite(base_permissions(role), overwrite);
            if !permissions.contains(Permission::CreateMessage) {
                return None;
            }
            Some(ChannelResponse {
                channel_id: channel_id.clone(),
                name: channel.name.clone(),
                kind: channel.kind,
            })
        })
        .collect::<Vec<_>>();
    channels.sort_by(|left, right| left.channel_id.cmp(&right.channel_id));
    channels.truncate(MAX_CHANNEL_LIST_LIMIT);
    Ok(Json(ChannelListResponse { channels }))
}

async fn create_channel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<GuildPath>,
    Json(payload): Json<CreateChannelRequest>,
) -> Result<Json<ChannelResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    let name = ChannelName::try_from(payload.name).map_err(|_| AuthFailure::InvalidRequest)?;
    let kind = payload.kind.unwrap_or(ChannelKind::Text);

    if let Some(pool) = &state.db_pool {
        let role_row =
            sqlx::query("SELECT role FROM guild_members WHERE guild_id = $1 AND user_id = $2")
                .bind(&path.guild_id)
                .bind(auth.user_id.to_string())
                .fetch_optional(pool)
                .await
                .map_err(|_| AuthFailure::Internal)?;
        let role_row = role_row.ok_or(AuthFailure::Forbidden)?;
        let role_value: i16 = role_row
            .try_get("role")
            .map_err(|_| AuthFailure::Internal)?;
        let role = role_from_i16(role_value).ok_or(AuthFailure::Forbidden)?;
        if !matches!(role, Role::Owner | Role::Moderator) {
            return Err(AuthFailure::Forbidden);
        }

        let channel_id = Ulid::new().to_string();
        sqlx::query(
            "INSERT INTO channels (channel_id, guild_id, name, kind, created_at_unix) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(&channel_id)
        .bind(&path.guild_id)
        .bind(name.as_str())
        .bind(channel_kind_to_i16(kind))
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

        return Ok(Json(ChannelResponse {
            channel_id,
            name: name.as_str().to_owned(),
            kind,
        }));
    }

    let mut guilds = state.guilds.write().await;
    let guild = guilds
        .get_mut(&path.guild_id)
        .ok_or(AuthFailure::NotFound)?;
    let role = guild
        .members
        .get(&auth.user_id)
        .copied()
        .ok_or(AuthFailure::Forbidden)?;
    if !matches!(role, Role::Owner | Role::Moderator) {
        return Err(AuthFailure::Forbidden);
    }

    let channel_id = Ulid::new().to_string();
    guild.channels.insert(
        channel_id.clone(),
        ChannelRecord {
            name: name.as_str().to_owned(),
            kind,
            messages: Vec::new(),
            role_overrides: HashMap::new(),
        },
    );

    Ok(Json(ChannelResponse {
        channel_id,
        name: name.as_str().to_owned(),
        kind,
    }))
}

async fn create_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<ChannelPath>,
    Json(payload): Json<CreateMessageRequest>,
) -> Result<Json<MessageResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;
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

async fn get_channel_permissions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<ChannelPath>,
) -> Result<Json<ChannelPermissionsResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;
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
async fn get_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<ChannelPath>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<MessageHistoryResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
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
async fn search_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<GuildPath>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    let role = user_role_in_guild(&state, auth.user_id, &path.guild_id).await?;
    if !has_permission(role, Permission::CreateMessage) {
        return Err(AuthFailure::Forbidden);
    }

    validate_search_query(&state, &query)?;
    ensure_search_bootstrapped(&state).await?;
    let limit = query.limit.unwrap_or(DEFAULT_SEARCH_RESULT_LIMIT);
    let channel_id = query.channel_id.clone();
    let message_ids = run_search_query(
        &state,
        &path.guild_id,
        channel_id.as_deref(),
        &query.q,
        limit,
    )
    .await?;
    let messages =
        hydrate_messages_by_id(&state, &path.guild_id, channel_id.as_deref(), &message_ids).await?;

    Ok(Json(SearchResponse {
        message_ids,
        messages,
    }))
}

async fn rebuild_search_index(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<GuildPath>,
) -> Result<StatusCode, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    let role = user_role_in_guild(&state, auth.user_id, &path.guild_id).await?;
    if !matches!(role, Role::Owner | Role::Moderator) {
        return Err(AuthFailure::Forbidden);
    }

    let docs = collect_all_indexed_messages(&state).await?;
    enqueue_search_operation(&state, SearchOperation::Rebuild { docs }, true).await?;
    state.search_bootstrapped.set(()).ok();
    Ok(StatusCode::NO_CONTENT)
}

async fn reconcile_search_index(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<GuildPath>,
) -> Result<Json<SearchReconcileResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    let role = user_role_in_guild(&state, auth.user_id, &path.guild_id).await?;
    if !matches!(role, Role::Owner | Role::Moderator) {
        return Err(AuthFailure::Forbidden);
    }

    ensure_search_bootstrapped(&state).await?;
    let (upserts, delete_message_ids) =
        plan_search_reconciliation(&state, &path.guild_id, MAX_SEARCH_RECONCILE_DOCS).await?;
    let upserted = upserts.len();
    let deleted = delete_message_ids.len();
    if upserted > 0 || deleted > 0 {
        enqueue_search_operation(
            &state,
            SearchOperation::Reconcile {
                upserts,
                delete_message_ids,
            },
            true,
        )
        .await?;
    }

    Ok(Json(SearchReconcileResponse { upserted, deleted }))
}

#[allow(clippy::too_many_lines)]
async fn edit_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<MessagePath>,
    Json(payload): Json<EditMessageRequest>,
) -> Result<Json<MessageResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
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
    Ok(Json(response))
}

#[allow(clippy::too_many_lines)]
async fn delete_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<MessagePath>,
) -> Result<StatusCode, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
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
            message_id: path.message_id,
        },
        true,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[allow(clippy::too_many_lines)]
async fn upload_attachment(
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

async fn download_attachment(
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

async fn delete_attachment(
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

async fn add_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<MemberPath>,
) -> Result<Json<ModerationResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    let actor_role = user_role_in_guild(&state, auth.user_id, &path.guild_id).await?;
    if !has_permission(actor_role, Permission::ManageRoles) {
        return Err(AuthFailure::Forbidden);
    }
    let target_user_id = UserId::try_from(path.user_id).map_err(|_| AuthFailure::InvalidRequest)?;

    if let Some(pool) = &state.db_pool {
        let banned = sqlx::query("SELECT 1 FROM guild_bans WHERE guild_id = $1 AND user_id = $2")
            .bind(&path.guild_id)
            .bind(target_user_id.to_string())
            .fetch_optional(pool)
            .await
            .map_err(|_| AuthFailure::Internal)?;
        if banned.is_some() {
            return Err(AuthFailure::Forbidden);
        }

        sqlx::query(
            "INSERT INTO guild_members (guild_id, user_id, role)
             VALUES ($1, $2, $3)
             ON CONFLICT (guild_id, user_id) DO NOTHING",
        )
        .bind(&path.guild_id)
        .bind(target_user_id.to_string())
        .bind(role_to_i16(Role::Member))
        .execute(pool)
        .await
        .map_err(|e| {
            if matches!(e, sqlx::Error::Database(_)) {
                AuthFailure::NotFound
            } else {
                AuthFailure::Internal
            }
        })?;
    } else {
        let mut guilds = state.guilds.write().await;
        let guild = guilds
            .get_mut(&path.guild_id)
            .ok_or(AuthFailure::NotFound)?;
        if guild.banned_members.contains(&target_user_id) {
            return Err(AuthFailure::Forbidden);
        }
        guild.members.entry(target_user_id).or_insert(Role::Member);
    }

    Ok(Json(ModerationResponse { accepted: true }))
}

async fn update_member_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<MemberPath>,
    Json(payload): Json<UpdateMemberRoleRequest>,
) -> Result<Json<ModerationResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    let actor_role = user_role_in_guild(&state, auth.user_id, &path.guild_id).await?;
    let target_user_id = UserId::try_from(path.user_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let target_role = member_role_in_guild(&state, target_user_id, &path.guild_id).await?;

    if !can_assign_role(actor_role, target_role, payload.role) {
        return Err(AuthFailure::Forbidden);
    }

    if let Some(pool) = &state.db_pool {
        let result =
            sqlx::query("UPDATE guild_members SET role = $3 WHERE guild_id = $1 AND user_id = $2")
                .bind(&path.guild_id)
                .bind(target_user_id.to_string())
                .bind(role_to_i16(payload.role))
                .execute(pool)
                .await
                .map_err(|_| AuthFailure::Internal)?;
        if result.rows_affected() == 0 {
            return Err(AuthFailure::NotFound);
        }
    } else {
        let mut guilds = state.guilds.write().await;
        let guild = guilds
            .get_mut(&path.guild_id)
            .ok_or(AuthFailure::NotFound)?;
        let Some(role) = guild.members.get_mut(&target_user_id) else {
            return Err(AuthFailure::NotFound);
        };
        *role = payload.role;
    }

    write_audit_log(
        &state,
        Some(path.guild_id),
        auth.user_id,
        Some(target_user_id),
        "member.role.update",
        serde_json::json!({"role": payload.role}),
    )
    .await?;
    Ok(Json(ModerationResponse { accepted: true }))
}

async fn set_channel_role_override(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<ChannelRolePath>,
    Json(payload): Json<UpdateChannelRoleOverrideRequest>,
) -> Result<Json<ModerationResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    let actor_role = user_role_in_guild(&state, auth.user_id, &path.guild_id).await?;
    if !has_permission(actor_role, Permission::ManageChannelOverrides) {
        return Err(AuthFailure::Forbidden);
    }

    let allow = permission_set_from_list(&payload.allow);
    let deny = permission_set_from_list(&payload.deny);
    if allow.bits() & deny.bits() != 0 {
        return Err(AuthFailure::InvalidRequest);
    }

    if let Some(pool) = &state.db_pool {
        let result = sqlx::query(
            "INSERT INTO channel_role_overrides (guild_id, channel_id, role, allow_mask, deny_mask)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (guild_id, channel_id, role)
             DO UPDATE SET allow_mask = EXCLUDED.allow_mask, deny_mask = EXCLUDED.deny_mask",
        )
        .bind(&path.guild_id)
        .bind(&path.channel_id)
        .bind(role_to_i16(path.role))
        .bind(permission_set_to_i64(allow)?)
        .bind(permission_set_to_i64(deny)?)
        .execute(pool)
        .await
        .map_err(|e| {
            if matches!(e, sqlx::Error::Database(_)) {
                AuthFailure::NotFound
            } else {
                AuthFailure::Internal
            }
        })?;
        if result.rows_affected() == 0 {
            return Err(AuthFailure::NotFound);
        }
    } else {
        let mut guilds = state.guilds.write().await;
        let guild = guilds
            .get_mut(&path.guild_id)
            .ok_or(AuthFailure::NotFound)?;
        let channel = guild
            .channels
            .get_mut(&path.channel_id)
            .ok_or(AuthFailure::NotFound)?;
        channel
            .role_overrides
            .insert(path.role, ChannelPermissionOverwrite { allow, deny });
    }

    write_audit_log(
        &state,
        Some(path.guild_id),
        auth.user_id,
        None,
        "channel.override.update",
        serde_json::json!({
            "channel_id": path.channel_id,
            "role": path.role,
            "allow_bits": allow.bits(),
            "deny_bits": deny.bits(),
        }),
    )
    .await?;
    Ok(Json(ModerationResponse { accepted: true }))
}

async fn add_reaction(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<ReactionPath>,
) -> Result<Json<ReactionResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
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
        return Ok(Json(ReactionResponse {
            emoji: path.emoji,
            count,
        }));
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

    Ok(Json(ReactionResponse {
        emoji: path.emoji,
        count: users.len(),
    }))
}

async fn remove_reaction(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<ReactionPath>,
) -> Result<Json<ReactionResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
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
        return Ok(Json(ReactionResponse {
            emoji: path.emoji,
            count,
        }));
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

    Ok(Json(ReactionResponse {
        emoji: path.emoji,
        count,
    }))
}

async fn kick_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<MemberPath>,
) -> Result<Json<ModerationResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    let actor_role = user_role_in_guild(&state, auth.user_id, &path.guild_id).await?;
    if !has_permission(actor_role, Permission::BanMember) {
        return Err(AuthFailure::Forbidden);
    }
    let target_user_id = UserId::try_from(path.user_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let target_role = member_role_in_guild(&state, target_user_id, &path.guild_id).await?;
    if !can_moderate_member(actor_role, target_role) {
        return Err(AuthFailure::Forbidden);
    }

    if let Some(pool) = &state.db_pool {
        let deleted = sqlx::query("DELETE FROM guild_members WHERE guild_id = $1 AND user_id = $2")
            .bind(&path.guild_id)
            .bind(target_user_id.to_string())
            .execute(pool)
            .await
            .map_err(|_| AuthFailure::Internal)?;
        if deleted.rows_affected() == 0 {
            return Err(AuthFailure::NotFound);
        }
    } else {
        let mut guilds = state.guilds.write().await;
        let guild = guilds
            .get_mut(&path.guild_id)
            .ok_or(AuthFailure::NotFound)?;
        if guild.members.remove(&target_user_id).is_none() {
            return Err(AuthFailure::NotFound);
        }
    }

    write_audit_log(
        &state,
        Some(path.guild_id),
        auth.user_id,
        Some(target_user_id),
        "member.kick",
        serde_json::json!({}),
    )
    .await?;
    Ok(Json(ModerationResponse { accepted: true }))
}

async fn ban_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<MemberPath>,
) -> Result<Json<ModerationResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    let actor_role = user_role_in_guild(&state, auth.user_id, &path.guild_id).await?;
    if !has_permission(actor_role, Permission::BanMember) {
        return Err(AuthFailure::Forbidden);
    }
    let target_user_id = UserId::try_from(path.user_id).map_err(|_| AuthFailure::InvalidRequest)?;
    if let Ok(target_role) = member_role_in_guild(&state, target_user_id, &path.guild_id).await {
        if !can_moderate_member(actor_role, target_role) {
            return Err(AuthFailure::Forbidden);
        }
    }

    if let Some(pool) = &state.db_pool {
        let mut tx = pool.begin().await.map_err(|_| AuthFailure::Internal)?;
        sqlx::query(
            "INSERT INTO guild_bans (guild_id, user_id, banned_by_user_id, created_at_unix)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (guild_id, user_id) DO UPDATE SET banned_by_user_id = EXCLUDED.banned_by_user_id, created_at_unix = EXCLUDED.created_at_unix",
        )
        .bind(&path.guild_id)
        .bind(target_user_id.to_string())
        .bind(auth.user_id.to_string())
        .bind(now_unix())
        .execute(&mut *tx)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        sqlx::query("DELETE FROM guild_members WHERE guild_id = $1 AND user_id = $2")
            .bind(&path.guild_id)
            .bind(target_user_id.to_string())
            .execute(&mut *tx)
            .await
            .map_err(|_| AuthFailure::Internal)?;
        tx.commit().await.map_err(|_| AuthFailure::Internal)?;
    } else {
        let mut guilds = state.guilds.write().await;
        let guild = guilds
            .get_mut(&path.guild_id)
            .ok_or(AuthFailure::NotFound)?;
        guild.members.remove(&target_user_id);
        guild.banned_members.insert(target_user_id);
    }

    write_audit_log(
        &state,
        Some(path.guild_id),
        auth.user_id,
        Some(target_user_id),
        "member.ban",
        serde_json::json!({}),
    )
    .await?;
    Ok(Json(ModerationResponse { accepted: true }))
}

#[allow(clippy::too_many_lines)]
async fn issue_voice_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<ChannelPath>,
    Json(payload): Json<VoiceTokenRequest>,
) -> Result<Json<VoiceTokenResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    enforce_media_token_rate_limit(&state, &headers, auth.user_id, &path).await?;
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
        enforce_media_publish_rate_limit(&state, &headers, auth.user_id, &path).await?;
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
            "client_ip": extract_client_ip(&headers),
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

