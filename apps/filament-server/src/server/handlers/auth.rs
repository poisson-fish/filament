use std::collections::HashSet;
use std::error::Error as StdError;
use std::net::SocketAddr;

use axum::{
    extract::{connect_info::ConnectInfo, Extension, State},
    http::{HeaderMap, StatusCode},
    Json,
};
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
        RefreshRequest, RegisterRequest, RegisterResponse, UserLookupRequest,
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
        .ok_or_else(|| {
            tracing::warn!(
                event = "auth.captcha.verify",
                outcome = "missing_token",
                client_ip_source = client_ip.source().as_str()
            );
            AuthFailure::CaptchaFailed
        })
        .and_then(|raw| {
            CaptchaToken::try_from(raw).map_err(|()| {
                tracing::warn!(
                    event = "auth.captcha.verify",
                    outcome = "invalid_token_format",
                    client_ip_source = client_ip.source().as_str()
                );
                AuthFailure::CaptchaFailed
            })
        })?;

    let mut form_data = vec![
        ("sitekey", config.site_key.clone()),
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
        .map_err(|error| {
            let error_chain = format_error_chain(&error);
            tracing::warn!(
                event = "auth.captcha.verify",
                outcome = "request_error",
                error = %error,
                error_debug = ?error,
                error_chain = %error_chain,
                verify_url = %config.verify_url,
                client_ip_source = client_ip.source().as_str()
            );
            AuthFailure::CaptchaFailed
        })?;
    
    let status = response.status();
    let verify: HcaptchaVerifyResponse = response
        .json()
        .await
        .map_err(|error| {
            tracing::warn!(
                event = "auth.captcha.verify",
                outcome = "response_parse_error",
                status = %status,
                error = %error,
                verify_url = %config.verify_url,
                client_ip_source = client_ip.source().as_str()
            );
            AuthFailure::CaptchaFailed
        })?;

    validate_captcha_response(status, &verify, &config.verify_url, client_ip.source().as_str())
}

fn validate_captcha_response(
    status: StatusCode,
    verify: &HcaptchaVerifyResponse,
    verify_url: &str,
    client_ip_source: &str,
) -> Result<(), AuthFailure> {
    if !status.is_success() {
        tracing::warn!(
            event = "auth.captcha.verify",
            outcome = "non_success_status",
            status = %status,
            verify_url = %verify_url,
            error_codes = ?verify.error_codes,
            hostname = ?verify.hostname,
            challenge_ts = ?verify.challenge_ts,
            score = ?verify.score,
            score_reason = ?verify.score_reason,
            credit = ?verify.credit,
            client_ip_source = %client_ip_source
        );
        return Err(AuthFailure::CaptchaFailed);
    }
    if !verify.success {
        tracing::warn!(
            event = "auth.captcha.verify",
            outcome = "verify_failed",
            status = %status,
            verify_url = %verify_url,
            error_codes = ?verify.error_codes,
            hostname = ?verify.hostname,
            challenge_ts = ?verify.challenge_ts,
            score = ?verify.score,
            score_reason = ?verify.score_reason,
            credit = ?verify.credit,
            client_ip_source = %client_ip_source
        );
        return Err(AuthFailure::CaptchaFailed);
    }

    tracing::info!(
        event = "auth.captcha.verify",
        outcome = "success",
        status = %status,
        hostname = ?verify.hostname,
        challenge_ts = ?verify.challenge_ts,
        score = ?verify.score,
        score_reason = ?verify.score_reason,
        credit = ?verify.credit,
        client_ip_source = %client_ip_source
    );

    Ok(())
}

fn format_error_chain(error: &dyn StdError) -> String {
    let mut chain = Vec::new();
    chain.push(error.to_string());
    let mut source = error.source();
    while let Some(err) = source {
        chain.push(err.to_string());
        source = err.source();
    }
    chain.join(" | caused_by: ")
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
    let typed_username = Username::try_from(auth.username.clone()).map_err(|_| AuthFailure::Unauthorized)?;

    let repository = AuthRepository::from_state(&state);
    let profile = repository
        .get_user_profile(auth.user_id, &typed_username)
        .await?
        .ok_or(AuthFailure::Unauthorized)?;

    Ok(Json(MeResponse {
        user_id: auth.user_id.to_string(),
        username: profile.0,
        about_markdown: profile.1.clone(),
        about_markdown_tokens: tokenize_markdown(&profile.1),
        avatar_version: profile.2,
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

    let repository = AuthRepository::from_state(&state);
    let users = repository.lookup_users(&deduped).await?;
    Ok(Json(UserLookupResponse { users }))
}
