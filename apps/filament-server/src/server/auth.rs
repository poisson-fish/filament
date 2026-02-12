use super::*;

pub(crate) fn validate_password(value: &str) -> Result<(), AuthFailure> {
    let len = value.len();
    if (12..=128).contains(&len) {
        Ok(())
    } else {
        Err(AuthFailure::InvalidRequest)
    }
}

pub(crate) fn validate_message_content(content: &str) -> Result<(), AuthFailure> {
    let len = content.len();
    if (1..=2000).contains(&len) {
        Ok(())
    } else {
        Err(AuthFailure::InvalidRequest)
    }
}

pub(crate) fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow!("password hash failed: {e}"))?
        .to_string();
    Ok(hash)
}

pub(crate) fn verify_password(stored_hash: &str, supplied_password: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored_hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(supplied_password.as_bytes(), &parsed)
        .is_ok()
}

pub(crate) fn issue_tokens(
    state: &AppState,
    user_id: UserId,
    username: &str,
    session_id: &str,
) -> anyhow::Result<(String, String, [u8; 32])> {
    let mut claims = Claims::new_expires_in(&Duration::from_secs(ACCESS_TOKEN_TTL_SECS as u64))
        .map_err(|e| anyhow!("claims init failed: {e}"))?;
    claims
        .subject(&user_id.to_string())
        .map_err(|e| anyhow!("claim sub failed: {e}"))?;
    claims
        .add_additional("username", username)
        .map_err(|e| anyhow!("claim username failed: {e}"))?;

    let access_token = local::encrypt(&state.token_key, &claims, None, None)
        .map_err(|e| anyhow!("access token mint failed: {e}"))?;

    let mut refresh_secret = [0_u8; 32];
    OsRng.fill_bytes(&mut refresh_secret);
    let refresh_secret = URL_SAFE_NO_PAD.encode(refresh_secret);
    let refresh_token = format!("{session_id}.{refresh_secret}");
    let refresh_hash = hash_refresh_token(&refresh_token);

    Ok((access_token, refresh_token, refresh_hash))
}

pub(crate) fn verify_access_token(state: &AppState, token: &str) -> anyhow::Result<Claims> {
    let untrusted = UntrustedToken::<Local, V4>::try_from(token).map_err(|e| anyhow!("{e}"))?;
    let validation_rules = ClaimsValidationRules::new();
    let trusted = local::decrypt(&state.token_key, &untrusted, &validation_rules, None, None)
        .map_err(|e| anyhow!("token decrypt failed: {e}"))?;
    trusted
        .payload_claims()
        .cloned()
        .ok_or_else(|| anyhow!("token claims missing"))
}

pub(crate) async fn authenticate(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<AuthContext, AuthFailure> {
    let access_token = bearer_token(headers).ok_or(AuthFailure::Unauthorized)?;
    authenticate_with_token(state, access_token).await
}

pub(crate) async fn authenticate_with_token(
    state: &AppState,
    access_token: &str,
) -> Result<AuthContext, AuthFailure> {
    ensure_db_schema(state).await?;
    let claims = verify_access_token(state, access_token).map_err(|_| AuthFailure::Unauthorized)?;
    let user_id = claims
        .get_claim("sub")
        .and_then(serde_json::Value::as_str)
        .ok_or(AuthFailure::Unauthorized)?;
    let username = find_username_by_subject(state, user_id)
        .await
        .ok_or(AuthFailure::Unauthorized)?;
    let user_id = UserId::try_from(user_id.to_owned()).map_err(|_| AuthFailure::Unauthorized)?;
    Ok(AuthContext { user_id, username })
}

pub(crate) async fn find_username_by_subject(state: &AppState, user_id: &str) -> Option<String> {
    if let Some(pool) = &state.db_pool {
        let row = sqlx::query("SELECT username FROM users WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .ok()?;
        return row.and_then(|value| value.try_get("username").ok());
    }
    state.user_ids.read().await.get(user_id).cloned()
}

pub(crate) async fn find_username_by_user_id(state: &AppState, user_id: UserId) -> Option<String> {
    if let Some(pool) = &state.db_pool {
        let row = sqlx::query("SELECT username FROM users WHERE user_id = $1")
            .bind(user_id.to_string())
            .fetch_optional(pool)
            .await
            .ok()?;
        return row.and_then(|value| value.try_get("username").ok());
    }
    state
        .user_ids
        .read()
        .await
        .get(&user_id.to_string())
        .cloned()
}

pub(crate) fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let header = headers.get(AUTHORIZATION)?;
    let header = header.to_str().ok()?;
    header.strip_prefix("Bearer ")
}

pub(crate) fn hash_refresh_token(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}

pub(crate) fn now_unix() -> i64 {
    let now = SystemTime::now();
    let seconds = now
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs();
    i64::try_from(seconds).unwrap_or(i64::MAX)
}

pub(crate) fn outbound_event<T: Serialize>(event_type: &str, data: T) -> String {
    let envelope = Envelope {
        v: PROTOCOL_VERSION,
        t: EventType::try_from(event_type.to_owned()).unwrap_or_else(|_| {
            EventType::try_from(String::from("ready")).expect("valid event type")
        }),
        d: serde_json::to_value(data).unwrap_or(serde_json::Value::Null),
    };

    serde_json::to_string(&envelope)
        .unwrap_or_else(|_| String::from(r#"{"v":1,"t":"ready","d":{}}"#))
}

pub(crate) fn channel_key(guild_id: &str, channel_id: &str) -> String {
    format!("{guild_id}:{channel_id}")
}

pub(crate) fn build_livekit_config(config: &AppConfig) -> anyhow::Result<Option<LiveKitConfig>> {
    match (&config.livekit_api_key, &config.livekit_api_secret) {
        (None, None) => Ok(None),
        (Some(_), None) | (None, Some(_)) => {
            Err(anyhow!("livekit api key and secret must be set together"))
        }
        (Some(api_key), Some(api_secret)) => {
            let api_key = api_key.trim();
            let api_secret = api_secret.trim();
            if api_key.is_empty() || api_secret.is_empty() {
                return Err(anyhow!("livekit api key and secret cannot be empty"));
            }
            let url = validate_livekit_url(&config.livekit_url)?;
            Ok(Some(LiveKitConfig {
                api_key: api_key.to_owned(),
                api_secret: api_secret.to_owned(),
                url,
            }))
        }
    }
}

pub(crate) fn build_captcha_config(config: &AppConfig) -> anyhow::Result<Option<CaptchaConfig>> {
    match (
        &config.captcha_hcaptcha_site_key,
        &config.captcha_hcaptcha_secret,
    ) {
        (None, None) => Ok(None),
        (Some(_), None) | (None, Some(_)) => {
            Err(anyhow!("hcaptcha site key and secret must be set together"))
        }
        (Some(site_key), Some(secret)) => {
            let site_key = site_key.trim();
            let secret = secret.trim();
            if site_key.is_empty() || secret.is_empty() {
                return Err(anyhow!("hcaptcha site key and secret cannot be empty"));
            }
            let verify_url = validate_captcha_verify_url(&config.captcha_verify_url)?;
            if config.captcha_verify_timeout.is_zero()
                || config.captcha_verify_timeout > Duration::from_secs(10)
            {
                return Err(anyhow!(
                    "captcha verify timeout must be between 1 and 10 seconds"
                ));
            }
            Ok(Some(CaptchaConfig {
                secret: secret.to_owned(),
                verify_url,
                verify_timeout: config.captcha_verify_timeout,
            }))
        }
    }
}

pub(crate) fn validate_livekit_url(value: &str) -> anyhow::Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > 256 {
        return Err(anyhow!("livekit url is invalid"));
    }
    if !(trimmed.starts_with("ws://") || trimmed.starts_with("wss://")) {
        return Err(anyhow!("livekit url must use ws:// or wss://"));
    }
    Ok(trimmed.to_owned())
}

pub(crate) fn validate_captcha_verify_url(value: &str) -> anyhow::Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > 256 {
        return Err(anyhow!("captcha verify url is invalid"));
    }
    if trimmed.starts_with("https://")
        || trimmed.starts_with("http://127.0.0.1")
        || trimmed.starts_with("http://localhost")
    {
        return Ok(trimmed.to_owned());
    }
    Err(anyhow!(
        "captcha verify url must use https://, or localhost http:// for tests"
    ))
}

pub(crate) async fn enforce_auth_route_rate_limit(
    state: &AppState,
    headers: &HeaderMap,
    route: &str,
) -> Result<(), AuthFailure> {
    let ip = extract_client_ip(headers);
    let key = format!("{route}:{ip}");
    let now = now_unix();

    let mut hits = state.auth_route_hits.write().await;
    let route_hits = hits.entry(key).or_default();
    route_hits.retain(|timestamp| now.saturating_sub(*timestamp) < 60);
    let max_hits =
        usize::try_from(state.runtime.auth_route_requests_per_minute).unwrap_or(usize::MAX);
    if route_hits.len() >= max_hits {
        tracing::warn!(event = "auth.rate_limit", route = %route, ip = %ip);
        return Err(AuthFailure::RateLimited);
    }
    route_hits.push(now);
    Ok(())
}

pub(crate) async fn enforce_media_token_rate_limit(
    state: &AppState,
    headers: &HeaderMap,
    user_id: UserId,
    path: &ChannelPath,
) -> Result<(), AuthFailure> {
    let ip = extract_client_ip(headers);
    let key = format!("{ip}:{}:{}:{}", user_id, path.guild_id, path.channel_id);
    let now = now_unix();

    let mut hits = state.media_token_hits.write().await;
    let route_hits = hits.entry(key).or_default();
    route_hits.retain(|timestamp| now.saturating_sub(*timestamp) < 60);
    let max_hits =
        usize::try_from(state.runtime.media_token_requests_per_minute).unwrap_or(usize::MAX);
    if route_hits.len() >= max_hits {
        tracing::warn!(event = "media.token.rate_limit", ip = %ip, user_id = %user_id, guild_id = %path.guild_id, channel_id = %path.channel_id);
        return Err(AuthFailure::RateLimited);
    }
    route_hits.push(now);
    Ok(())
}

pub(crate) async fn enforce_media_publish_rate_limit(
    state: &AppState,
    headers: &HeaderMap,
    user_id: UserId,
    path: &ChannelPath,
) -> Result<(), AuthFailure> {
    let ip = extract_client_ip(headers);
    let key = format!("{ip}:{}", media_channel_user_key(user_id, path));
    let now = now_unix();

    let mut hits = state.media_publish_hits.write().await;
    let route_hits = hits.entry(key).or_default();
    route_hits.retain(|timestamp| now.saturating_sub(*timestamp) < 60);
    let max_hits =
        usize::try_from(state.runtime.media_publish_requests_per_minute).unwrap_or(usize::MAX);
    if route_hits.len() >= max_hits {
        tracing::warn!(event = "media.publish.rate_limit", ip = %ip, user_id = %user_id, guild_id = %path.guild_id, channel_id = %path.channel_id);
        return Err(AuthFailure::RateLimited);
    }
    route_hits.push(now);
    Ok(())
}

pub(crate) async fn enforce_media_subscribe_cap(
    state: &AppState,
    user_id: UserId,
    path: &ChannelPath,
) -> Result<(), AuthFailure> {
    let key = media_channel_user_key(user_id, path);
    let now = now_unix();
    let expires_at = now
        .checked_add(i64::try_from(state.runtime.livekit_token_ttl.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(i64::MAX);
    let mut leases = state.media_subscribe_leases.write().await;
    let channel_leases = leases.entry(key).or_default();
    channel_leases.retain(|timestamp| *timestamp > now);
    if channel_leases.len() >= state.runtime.media_subscribe_token_cap_per_channel {
        tracing::warn!(
            event = "media.subscribe.cap_reached",
            user_id = %user_id,
            guild_id = %path.guild_id,
            channel_id = %path.channel_id
        );
        return Err(AuthFailure::RateLimited);
    }
    channel_leases.push(expires_at);
    Ok(())
}

pub(crate) fn media_channel_user_key(user_id: UserId, path: &ChannelPath) -> String {
    format!("{}:{}:{}", user_id, path.guild_id, path.channel_id)
}

pub(crate) fn dedup_publish_sources(sources: &[MediaPublishSource]) -> Vec<MediaPublishSource> {
    let mut deduped = Vec::new();
    for source in sources {
        if !deduped.contains(source) {
            deduped.push(*source);
        }
    }
    deduped
}

pub(crate) fn allowed_publish_sources(permissions: PermissionSet) -> Vec<MediaPublishSource> {
    let mut sources = Vec::with_capacity(3);
    if permissions.contains(Permission::CreateMessage) {
        sources.push(MediaPublishSource::Microphone);
    }
    if permissions.contains(Permission::PublishVideo) {
        sources.push(MediaPublishSource::Camera);
    }
    if permissions.contains(Permission::PublishScreenShare) {
        sources.push(MediaPublishSource::ScreenShare);
    }
    sources
}

pub(crate) fn extract_client_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map_or_else(|| String::from("unknown"), ToOwned::to_owned)
}
