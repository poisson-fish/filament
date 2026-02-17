use sqlx::{PgPool, Row};

use filament_core::{UserId, Username};

use crate::server::{
    auth::{hash_refresh_token, verify_password},
    core::{
        AppState, SessionRecord, LOGIN_LOCK_SECS, LOGIN_LOCK_THRESHOLD, REFRESH_TOKEN_TTL_SECS,
    },
    errors::AuthFailure,
};

pub(crate) struct RefreshCheck {
    pub(crate) session_id: String,
    pub(crate) user_id: UserId,
    pub(crate) presented_hash: [u8; 32],
}

pub(crate) enum RefreshCheckError {
    ReplayDetected { session_id: String },
    Unauthorized { session_id: String },
    Internal,
}

pub(crate) trait AuthPersistence {
    async fn create_user_if_missing(
        &self,
        username: &Username,
        password_hash: &str,
    ) -> Result<bool, AuthFailure>;

    async fn verify_credentials(
        &self,
        username: &Username,
        password: &str,
        dummy_password_hash: &str,
        now_unix: i64,
    ) -> Result<Option<UserId>, AuthFailure>;

    async fn insert_session(
        &self,
        session_id: &str,
        user_id: UserId,
        refresh_hash: [u8; 32],
        expires_at_unix: i64,
    ) -> Result<(), AuthFailure>;

    async fn check_refresh_token(
        &self,
        refresh_token: &str,
        now_unix: i64,
    ) -> Result<RefreshCheck, RefreshCheckError>;

    async fn rotate_refresh_token(
        &self,
        session_id: &str,
        presented_hash: [u8; 32],
        next_hash: [u8; 32],
        now_unix: i64,
        next_expires_at_unix: i64,
    ) -> Result<(), AuthFailure>;

    async fn revoke_session_with_token(
        &self,
        session_id: &str,
        token_hash: [u8; 32],
    ) -> Result<UserId, AuthFailure>;
}

pub(crate) struct PostgresAuthRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> PostgresAuthRepository<'a> {
    pub(crate) fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }
}

impl AuthPersistence for PostgresAuthRepository<'_> {
    async fn create_user_if_missing(
        &self,
        username: &Username,
        password_hash: &str,
    ) -> Result<bool, AuthFailure> {
        let user_id = UserId::new();
        let insert_result = sqlx::query(
            "INSERT INTO users (user_id, username, password_hash, failed_logins, locked_until_unix)
             VALUES ($1, $2, $3, 0, NULL)
             ON CONFLICT (username) DO NOTHING",
        )
        .bind(user_id.to_string())
        .bind(username.as_str())
        .bind(password_hash)
        .execute(self.pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        Ok(insert_result.rows_affected() > 0)
    }

    async fn verify_credentials(
        &self,
        username: &Username,
        password: &str,
        dummy_password_hash: &str,
        now_unix: i64,
    ) -> Result<Option<UserId>, AuthFailure> {
        let row = sqlx::query(
            "SELECT user_id, password_hash, failed_logins, locked_until_unix
             FROM users WHERE username = $1",
        )
        .bind(username.as_str())
        .fetch_optional(self.pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;

        let Some(row) = row else {
            let _ = verify_password(dummy_password_hash, password);
            return Ok(None);
        };

        let user_id_text: String = row.try_get("user_id").map_err(|_| AuthFailure::Internal)?;
        let user_id = UserId::try_from(user_id_text).map_err(|_| AuthFailure::Internal)?;
        let stored_password_hash: String = row
            .try_get("password_hash")
            .map_err(|_| AuthFailure::Internal)?;
        let failed_logins: i16 = row
            .try_get("failed_logins")
            .map_err(|_| AuthFailure::Internal)?;
        let locked_until_unix: Option<i64> = row
            .try_get("locked_until_unix")
            .map_err(|_| AuthFailure::Internal)?;

        if locked_until_unix.is_some_and(|lock_until| lock_until > now_unix) {
            return Ok(None);
        }

        if verify_password(&stored_password_hash, password) {
            sqlx::query(
                "UPDATE users SET failed_logins = 0, locked_until_unix = NULL WHERE user_id = $1",
            )
            .bind(user_id.to_string())
            .execute(self.pool)
            .await
            .map_err(|_| AuthFailure::Internal)?;
            return Ok(Some(user_id));
        }

        let mut updated_failed = i32::from(failed_logins) + 1;
        let mut lock_until = None;
        if updated_failed >= i32::from(LOGIN_LOCK_THRESHOLD) {
            updated_failed = 0;
            lock_until = Some(now_unix + LOGIN_LOCK_SECS);
        }
        sqlx::query(
            "UPDATE users SET failed_logins = $2, locked_until_unix = $3 WHERE user_id = $1",
        )
        .bind(user_id.to_string())
        .bind(i16::try_from(updated_failed).unwrap_or(i16::MAX))
        .bind(lock_until)
        .execute(self.pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        Ok(None)
    }

    async fn insert_session(
        &self,
        session_id: &str,
        user_id: UserId,
        refresh_hash: [u8; 32],
        expires_at_unix: i64,
    ) -> Result<(), AuthFailure> {
        sqlx::query(
            "INSERT INTO sessions (session_id, user_id, refresh_token_hash, expires_at_unix, revoked)
             VALUES ($1, $2, $3, $4, FALSE)",
        )
        .bind(session_id)
        .bind(user_id.to_string())
        .bind(refresh_hash.as_slice())
        .bind(expires_at_unix)
        .execute(self.pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        Ok(())
    }

    async fn check_refresh_token(
        &self,
        refresh_token: &str,
        now_unix: i64,
    ) -> Result<RefreshCheck, RefreshCheckError> {
        let presented_hash = hash_refresh_token(refresh_token);
        if let Some(row) =
            sqlx::query("SELECT session_id FROM used_refresh_tokens WHERE token_hash = $1")
                .bind(presented_hash.as_slice())
                .fetch_optional(self.pool)
                .await
                .map_err(|_| RefreshCheckError::Internal)?
        {
            let replay_session_id: String = row
                .try_get("session_id")
                .map_err(|_| RefreshCheckError::Internal)?;
            sqlx::query("UPDATE sessions SET revoked = TRUE WHERE session_id = $1")
                .bind(&replay_session_id)
                .execute(self.pool)
                .await
                .map_err(|_| RefreshCheckError::Internal)?;
            return Err(RefreshCheckError::ReplayDetected {
                session_id: replay_session_id,
            });
        }

        let session_id = refresh_token
            .split('.')
            .next()
            .ok_or_else(|| RefreshCheckError::Unauthorized {
                session_id: String::from("unknown"),
            })?
            .to_owned();
        let row = sqlx::query(
            "SELECT user_id, refresh_token_hash, expires_at_unix, revoked
             FROM sessions WHERE session_id = $1",
        )
        .bind(&session_id)
        .fetch_optional(self.pool)
        .await
        .map_err(|_| RefreshCheckError::Internal)?;
        let Some(row) = row else {
            return Err(RefreshCheckError::Unauthorized { session_id });
        };

        let session_user_id: String = row
            .try_get("user_id")
            .map_err(|_| RefreshCheckError::Internal)?;
        let stored_hash: Vec<u8> = row
            .try_get("refresh_token_hash")
            .map_err(|_| RefreshCheckError::Internal)?;
        let expires_at_unix: i64 = row
            .try_get("expires_at_unix")
            .map_err(|_| RefreshCheckError::Internal)?;
        let revoked: bool = row
            .try_get("revoked")
            .map_err(|_| RefreshCheckError::Internal)?;

        if revoked || expires_at_unix < now_unix || stored_hash.as_slice() != presented_hash {
            return Err(RefreshCheckError::Unauthorized { session_id });
        }

        let user_id = UserId::try_from(session_user_id).map_err(|_| RefreshCheckError::Internal)?;
        Ok(RefreshCheck {
            session_id,
            user_id,
            presented_hash,
        })
    }

    async fn rotate_refresh_token(
        &self,
        session_id: &str,
        presented_hash: [u8; 32],
        next_hash: [u8; 32],
        _now_unix: i64,
        next_expires_at_unix: i64,
    ) -> Result<(), AuthFailure> {
        sqlx::query(
            "UPDATE sessions SET refresh_token_hash = $2, expires_at_unix = $3 WHERE session_id = $1",
        )
        .bind(session_id)
        .bind(next_hash.as_slice())
        .bind(next_expires_at_unix)
        .execute(self.pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;

        sqlx::query(
            "INSERT INTO used_refresh_tokens (token_hash, session_id) VALUES ($1, $2)
             ON CONFLICT (token_hash) DO NOTHING",
        )
        .bind(presented_hash.as_slice())
        .bind(session_id)
        .execute(self.pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        Ok(())
    }

    async fn revoke_session_with_token(
        &self,
        session_id: &str,
        token_hash: [u8; 32],
    ) -> Result<UserId, AuthFailure> {
        let row =
            sqlx::query("SELECT user_id, refresh_token_hash FROM sessions WHERE session_id = $1")
                .bind(session_id)
                .fetch_optional(self.pool)
                .await
                .map_err(|_| AuthFailure::Internal)?;
        let row = row.ok_or(AuthFailure::Unauthorized)?;
        let user_id: String = row.try_get("user_id").map_err(|_| AuthFailure::Internal)?;
        let session_hash: Vec<u8> = row
            .try_get("refresh_token_hash")
            .map_err(|_| AuthFailure::Internal)?;
        if session_hash.as_slice() != token_hash {
            return Err(AuthFailure::Unauthorized);
        }
        sqlx::query("UPDATE sessions SET revoked = TRUE WHERE session_id = $1")
            .bind(session_id)
            .execute(self.pool)
            .await
            .map_err(|_| AuthFailure::Internal)?;
        UserId::try_from(user_id).map_err(|_| AuthFailure::Internal)
    }
}

pub(crate) struct InMemoryAuthRepository<'a> {
    state: &'a AppState,
}

impl<'a> InMemoryAuthRepository<'a> {
    pub(crate) fn new(state: &'a AppState) -> Self {
        Self { state }
    }
}

impl AuthPersistence for InMemoryAuthRepository<'_> {
    async fn create_user_if_missing(
        &self,
        username: &Username,
        password_hash: &str,
    ) -> Result<bool, AuthFailure> {
        let mut users = self.state.users.write().await;
        if users.contains_key(username.as_str()) {
            return Ok(false);
        }

        let user_id = UserId::new();
        users.insert(
            username.as_str().to_owned(),
            crate::server::core::UserRecord {
                id: user_id,
                username: username.clone(),
                about_markdown: String::new(),
                avatar: None,
                avatar_version: 0,
                password_hash: password_hash.to_owned(),
                failed_logins: 0,
                locked_until_unix: None,
            },
        );
        drop(users);

        self.state
            .user_ids
            .write()
            .await
            .insert(user_id.to_string(), username.as_str().to_owned());
        Ok(true)
    }

    async fn verify_credentials(
        &self,
        username: &Username,
        password: &str,
        dummy_password_hash: &str,
        now_unix: i64,
    ) -> Result<Option<UserId>, AuthFailure> {
        let mut users = self.state.users.write().await;
        let mut user_id = None;
        let mut verified = false;

        if let Some(user) = users.get_mut(username.as_str()) {
            if user
                .locked_until_unix
                .is_some_and(|lock_until| lock_until > now_unix)
            {
                return Ok(None);
            }

            verified = verify_password(&user.password_hash, password);
            if verified {
                user.failed_logins = 0;
                user.locked_until_unix = None;
                user_id = Some(user.id);
            } else {
                user.failed_logins = user.failed_logins.saturating_add(1);
                if user.failed_logins >= LOGIN_LOCK_THRESHOLD {
                    user.locked_until_unix = Some(now_unix + LOGIN_LOCK_SECS);
                    user.failed_logins = 0;
                }
            }
        } else {
            let _ = verify_password(dummy_password_hash, password);
        }
        drop(users);

        if !verified {
            return Ok(None);
        }
        Ok(user_id)
    }

    async fn insert_session(
        &self,
        session_id: &str,
        user_id: UserId,
        refresh_hash: [u8; 32],
        expires_at_unix: i64,
    ) -> Result<(), AuthFailure> {
        self.state
            .session_store
            .insert(
                session_id.to_owned(),
                SessionRecord {
                    user_id,
                    refresh_token_hash: refresh_hash,
                    expires_at_unix,
                    revoked: false,
                },
            )
            .await;
        Ok(())
    }

    async fn check_refresh_token(
        &self,
        refresh_token: &str,
        now_unix: i64,
    ) -> Result<RefreshCheck, RefreshCheckError> {
        let presented_hash = hash_refresh_token(refresh_token);
        if let Some(session_id) = self
            .state
            .session_store
            .revoke_if_replayed_token(presented_hash)
            .await
        {
            return Err(RefreshCheckError::ReplayDetected { session_id });
        }

        let session_id = refresh_token
            .split('.')
            .next()
            .ok_or_else(|| RefreshCheckError::Unauthorized {
                session_id: String::from("unknown"),
            })?
            .to_owned();
        let user_id = self
            .state
            .session_store
            .validate_refresh_token(&session_id, presented_hash, now_unix)
            .await
            .map_err(|_| RefreshCheckError::Unauthorized {
                session_id: session_id.clone(),
            })?;
        Ok(RefreshCheck {
            session_id,
            user_id,
            presented_hash,
        })
    }

    async fn rotate_refresh_token(
        &self,
        session_id: &str,
        presented_hash: [u8; 32],
        next_hash: [u8; 32],
        now_unix: i64,
        next_expires_at_unix: i64,
    ) -> Result<(), AuthFailure> {
        self.state
            .session_store
            .rotate_refresh_hash(
                session_id,
                presented_hash,
                next_hash,
                now_unix,
                next_expires_at_unix,
            )
            .await
            .map_err(|_| AuthFailure::Unauthorized)?;
        Ok(())
    }

    async fn revoke_session_with_token(
        &self,
        session_id: &str,
        token_hash: [u8; 32],
    ) -> Result<UserId, AuthFailure> {
        self.state
            .session_store
            .revoke_with_token(session_id, token_hash)
            .await
            .map_err(|_| AuthFailure::Unauthorized)
    }
}

pub(crate) enum AuthRepository<'a> {
    Postgres(PostgresAuthRepository<'a>),
    InMemory(InMemoryAuthRepository<'a>),
}

impl AuthRepository<'_> {
    pub(crate) fn from_state(state: &AppState) -> AuthRepository<'_> {
        if let Some(pool) = &state.db_pool {
            AuthRepository::Postgres(PostgresAuthRepository::new(pool))
        } else {
            AuthRepository::InMemory(InMemoryAuthRepository::new(state))
        }
    }
}

impl AuthPersistence for AuthRepository<'_> {
    async fn create_user_if_missing(
        &self,
        username: &Username,
        password_hash: &str,
    ) -> Result<bool, AuthFailure> {
        match self {
            Self::Postgres(repo) => repo.create_user_if_missing(username, password_hash).await,
            Self::InMemory(repo) => repo.create_user_if_missing(username, password_hash).await,
        }
    }

    async fn verify_credentials(
        &self,
        username: &Username,
        password: &str,
        dummy_password_hash: &str,
        now_unix: i64,
    ) -> Result<Option<UserId>, AuthFailure> {
        match self {
            Self::Postgres(repo) => {
                repo.verify_credentials(username, password, dummy_password_hash, now_unix)
                    .await
            }
            Self::InMemory(repo) => {
                repo.verify_credentials(username, password, dummy_password_hash, now_unix)
                    .await
            }
        }
    }

    async fn insert_session(
        &self,
        session_id: &str,
        user_id: UserId,
        refresh_hash: [u8; 32],
        expires_at_unix: i64,
    ) -> Result<(), AuthFailure> {
        match self {
            Self::Postgres(repo) => {
                repo.insert_session(session_id, user_id, refresh_hash, expires_at_unix)
                    .await
            }
            Self::InMemory(repo) => {
                repo.insert_session(session_id, user_id, refresh_hash, expires_at_unix)
                    .await
            }
        }
    }

    async fn check_refresh_token(
        &self,
        refresh_token: &str,
        now_unix: i64,
    ) -> Result<RefreshCheck, RefreshCheckError> {
        match self {
            Self::Postgres(repo) => repo.check_refresh_token(refresh_token, now_unix).await,
            Self::InMemory(repo) => repo.check_refresh_token(refresh_token, now_unix).await,
        }
    }

    async fn rotate_refresh_token(
        &self,
        session_id: &str,
        presented_hash: [u8; 32],
        next_hash: [u8; 32],
        now_unix: i64,
        next_expires_at_unix: i64,
    ) -> Result<(), AuthFailure> {
        match self {
            Self::Postgres(repo) => {
                repo.rotate_refresh_token(
                    session_id,
                    presented_hash,
                    next_hash,
                    now_unix,
                    next_expires_at_unix,
                )
                .await
            }
            Self::InMemory(repo) => {
                repo.rotate_refresh_token(
                    session_id,
                    presented_hash,
                    next_hash,
                    now_unix,
                    next_expires_at_unix,
                )
                .await
            }
        }
    }

    async fn revoke_session_with_token(
        &self,
        session_id: &str,
        token_hash: [u8; 32],
    ) -> Result<UserId, AuthFailure> {
        match self {
            Self::Postgres(repo) => repo.revoke_session_with_token(session_id, token_hash).await,
            Self::InMemory(repo) => repo.revoke_session_with_token(session_id, token_hash).await,
        }
    }
}

pub(crate) fn refresh_session_ttl_unix(now_unix: i64) -> i64 {
    now_unix + REFRESH_TOKEN_TTL_SECS
}
