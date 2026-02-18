use std::{
    net::IpAddr,
    sync::atomic::Ordering,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::anyhow;
use argon2::{
    password_hash::rand_core::{OsRng, RngCore},
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::http::{header::AUTHORIZATION, HeaderMap};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use filament_core::{Permission, PermissionSet, UserId};
use filament_protocol::{Envelope, EventType, PROTOCOL_VERSION};
use pasetors::{
    claims::{Claims, ClaimsValidationRules},
    local,
    token::UntrustedToken,
    version4::V4,
    Local,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::Row;

use super::{
    core::{
        AppConfig, AppState, AuthContext, CaptchaConfig, LiveKitConfig, ACCESS_TOKEN_TTL_SECS,
        RATE_LIMIT_SWEEP_INTERVAL_SECS,
    },
    directory_contract::IpNetwork,
    errors::AuthFailure,
    types::{ChannelPath, MediaPublishSource},
};

const MAX_X_FORWARDED_FOR_HEADER_CHARS: usize = 512;
const MAX_X_FORWARDED_FOR_ENTRY_CHARS: usize = 64;
const UNKNOWN_CLIENT_IP: &str = "unknown";
const RATE_LIMIT_WINDOW_SECS: i64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClientIpSource {
    Peer,
    Forwarded,
}

impl ClientIpSource {
    #[must_use]
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Peer => "peer",
            Self::Forwarded => "forwarded",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ClientIp {
    ip: Option<IpAddr>,
    source: ClientIpSource,
}

impl ClientIp {
    #[must_use]
    pub(crate) fn ip(self) -> Option<IpAddr> {
        self.ip
    }

    #[must_use]
    pub(crate) fn source(self) -> ClientIpSource {
        self.source
    }

    #[must_use]
    pub(crate) fn normalized(self) -> String {
        self.ip
            .map_or_else(|| String::from(UNKNOWN_CLIENT_IP), |ip| ip.to_string())
    }

    fn peer(ip: Option<IpAddr>) -> Self {
        Self {
            ip,
            source: ClientIpSource::Peer,
        }
    }

    fn forwarded(ip: IpAddr) -> Self {
        Self {
            ip: Some(ip),
            source: ClientIpSource::Forwarded,
        }
    }
}

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

async fn maybe_sweep_rate_limit_state(state: &AppState, now: i64) {
    let last = state.rate_limit_last_sweep_unix.load(Ordering::Relaxed);
    if now.saturating_sub(last) < RATE_LIMIT_SWEEP_INTERVAL_SECS {
        return;
    }
    if state
        .rate_limit_last_sweep_unix
        .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
        .is_err()
    {
        return;
    }

    {
        let mut hits = state.auth_route_hits.write().await;
        hits.retain(|_, route_hits| {
            route_hits.retain(|timestamp| now.saturating_sub(*timestamp) < RATE_LIMIT_WINDOW_SECS);
            !route_hits.is_empty()
        });
    }
    {
        let mut hits = state.directory_join_ip_hits.write().await;
        hits.retain(|_, route_hits| {
            route_hits.retain(|timestamp| now.saturating_sub(*timestamp) < RATE_LIMIT_WINDOW_SECS);
            !route_hits.is_empty()
        });
    }
    {
        let mut hits = state.directory_join_user_hits.write().await;
        hits.retain(|_, route_hits| {
            route_hits.retain(|timestamp| now.saturating_sub(*timestamp) < RATE_LIMIT_WINDOW_SECS);
            !route_hits.is_empty()
        });
    }
    {
        let mut hits = state.media_token_hits.write().await;
        hits.retain(|_, route_hits| {
            route_hits.retain(|timestamp| now.saturating_sub(*timestamp) < RATE_LIMIT_WINDOW_SECS);
            !route_hits.is_empty()
        });
    }
    {
        let mut hits = state.media_publish_hits.write().await;
        hits.retain(|_, route_hits| {
            route_hits.retain(|timestamp| now.saturating_sub(*timestamp) < RATE_LIMIT_WINDOW_SECS);
            !route_hits.is_empty()
        });
    }
    {
        let mut leases = state.media_subscribe_leases.write().await;
        leases.retain(|_, channel_leases| {
            channel_leases.retain(|timestamp| *timestamp > now);
            !channel_leases.is_empty()
        });
    }
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
                site_key: site_key.to_owned(),
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
    client_ip: ClientIp,
    route: &str,
) -> Result<(), AuthFailure> {
    let ip = client_ip.normalized();
    let key = format!("{route}:{ip}");
    let now = now_unix();
    maybe_sweep_rate_limit_state(state, now).await;

    let mut hits = state.auth_route_hits.write().await;
    let route_hits = hits.entry(key).or_default();
    route_hits.retain(|timestamp| now.saturating_sub(*timestamp) < RATE_LIMIT_WINDOW_SECS);
    let max_hits =
        usize::try_from(state.runtime.auth_route_requests_per_minute).unwrap_or(usize::MAX);
    if route_hits.len() >= max_hits {
        tracing::warn!(
            event = "auth.rate_limit",
            route = %route,
            client_ip = %ip,
            client_ip_source = client_ip.source().as_str()
        );
        return Err(AuthFailure::RateLimited);
    }
    route_hits.push(now);
    Ok(())
}

pub(crate) async fn enforce_directory_join_rate_limit(
    state: &AppState,
    client_ip: ClientIp,
    user_id: UserId,
) -> Result<(), AuthFailure> {
    let ip = client_ip.normalized();
    let now = now_unix();
    maybe_sweep_rate_limit_state(state, now).await;
    {
        let mut ip_hits = state.directory_join_ip_hits.write().await;
        let route_hits = ip_hits.entry(ip.clone()).or_default();
        route_hits.retain(|timestamp| now.saturating_sub(*timestamp) < RATE_LIMIT_WINDOW_SECS);
        let max_hits = usize::try_from(state.runtime.directory_join_requests_per_minute_per_ip)
            .unwrap_or(usize::MAX);
        if route_hits.len() >= max_hits {
            tracing::warn!(
                event = "directory.join.rate_limit",
                limiter = "ip",
                client_ip = %ip,
                client_ip_source = client_ip.source().as_str()
            );
            return Err(AuthFailure::RateLimited);
        }
        route_hits.push(now);
    }

    let user_key = user_id.to_string();
    let mut user_hits = state.directory_join_user_hits.write().await;
    let route_hits = user_hits.entry(user_key.clone()).or_default();
    route_hits.retain(|timestamp| now.saturating_sub(*timestamp) < RATE_LIMIT_WINDOW_SECS);
    let max_hits = usize::try_from(state.runtime.directory_join_requests_per_minute_per_user)
        .unwrap_or(usize::MAX);
    if route_hits.len() >= max_hits {
        tracing::warn!(
            event = "directory.join.rate_limit",
            limiter = "user",
            user_id = %user_id,
            client_ip = %ip,
            client_ip_source = client_ip.source().as_str()
        );
        return Err(AuthFailure::RateLimited);
    }
    route_hits.push(now);
    Ok(())
}

pub(crate) async fn enforce_media_token_rate_limit(
    state: &AppState,
    client_ip: ClientIp,
    user_id: UserId,
    path: &ChannelPath,
) -> Result<(), AuthFailure> {
    let ip = client_ip.normalized();
    let key = format!("{ip}:{}:{}:{}", user_id, path.guild_id, path.channel_id);
    let now = now_unix();
    maybe_sweep_rate_limit_state(state, now).await;

    let mut hits = state.media_token_hits.write().await;
    let route_hits = hits.entry(key).or_default();
    route_hits.retain(|timestamp| now.saturating_sub(*timestamp) < RATE_LIMIT_WINDOW_SECS);
    let max_hits =
        usize::try_from(state.runtime.media_token_requests_per_minute).unwrap_or(usize::MAX);
    if route_hits.len() >= max_hits {
        tracing::warn!(
            event = "media.token.rate_limit",
            client_ip = %ip,
            client_ip_source = client_ip.source().as_str(),
            user_id = %user_id,
            guild_id = %path.guild_id,
            channel_id = %path.channel_id
        );
        return Err(AuthFailure::RateLimited);
    }
    route_hits.push(now);
    Ok(())
}

pub(crate) async fn enforce_media_publish_rate_limit(
    state: &AppState,
    client_ip: ClientIp,
    user_id: UserId,
    path: &ChannelPath,
) -> Result<(), AuthFailure> {
    let ip = client_ip.normalized();
    let key = format!("{ip}:{}", media_channel_user_key(user_id, path));
    let now = now_unix();
    maybe_sweep_rate_limit_state(state, now).await;

    let mut hits = state.media_publish_hits.write().await;
    let route_hits = hits.entry(key).or_default();
    route_hits.retain(|timestamp| now.saturating_sub(*timestamp) < RATE_LIMIT_WINDOW_SECS);
    let max_hits =
        usize::try_from(state.runtime.media_publish_requests_per_minute).unwrap_or(usize::MAX);
    if route_hits.len() >= max_hits {
        tracing::warn!(
            event = "media.publish.rate_limit",
            client_ip = %ip,
            client_ip_source = client_ip.source().as_str(),
            user_id = %user_id,
            guild_id = %path.guild_id,
            channel_id = %path.channel_id
        );
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
    maybe_sweep_rate_limit_state(state, now).await;
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

pub(crate) fn resolve_client_ip(
    headers: &HeaderMap,
    peer_ip: Option<IpAddr>,
    trusted_proxy_cidrs: &[IpNetwork],
) -> ClientIp {
    let Some(peer_ip) = peer_ip else {
        return ClientIp::peer(None);
    };
    let peer_is_trusted = trusted_proxy_cidrs
        .iter()
        .any(|network| network.contains(peer_ip));
    if peer_is_trusted {
        if let Some(forwarded_ip) = parse_forwarded_ip(headers) {
            return ClientIp::forwarded(forwarded_ip);
        }
    }
    ClientIp::peer(Some(peer_ip))
}

#[must_use]
pub(crate) fn extract_client_ip(
    state: &AppState,
    headers: &HeaderMap,
    peer_ip: Option<IpAddr>,
) -> ClientIp {
    resolve_client_ip(
        headers,
        peer_ip,
        state.runtime.trusted_proxy_cidrs.as_slice(),
    )
}

fn parse_forwarded_ip(headers: &HeaderMap) -> Option<IpAddr> {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.len() <= MAX_X_FORWARDED_FOR_HEADER_CHARS)
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= MAX_X_FORWARDED_FOR_ENTRY_CHARS)
        .and_then(|value| value.parse::<IpAddr>().ok())
}

#[cfg(test)]
mod tests {
    use super::{
        build_captcha_config, enforce_auth_route_rate_limit, resolve_client_ip, ClientIp,
        ClientIpSource,
    };
    use crate::server::core::{AppConfig, AppState};
    use crate::server::directory_contract::IpNetwork;
    use axum::http::HeaderMap;

    #[test]
    fn captcha_config_includes_site_key_for_siteverify_binding() {
        let mut config = AppConfig::default();
        config.captcha_hcaptcha_site_key = Some(String::from("10000000-ffff-ffff-ffff-000000000001"));
        config.captcha_hcaptcha_secret = Some(String::from("0x0000000000000000000000000000000000000000"));

        let captcha = build_captcha_config(&config)
            .expect("captcha config should build")
            .expect("captcha should be enabled");

        assert_eq!(captcha.site_key, "10000000-ffff-ffff-ffff-000000000001");
        assert_eq!(captcha.secret, "0x0000000000000000000000000000000000000000");
        assert_eq!(captcha.verify_url, "https://api.hcaptcha.com/siteverify");
    }

    #[test]
    fn client_ip_defaults_to_peer_when_proxy_is_untrusted() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            "198.51.100.21".parse().expect("valid header"),
        );
        let resolved = resolve_client_ip(
            &headers,
            Some("10.10.0.4".parse().expect("valid ip")),
            &Vec::new(),
        );
        assert_eq!(resolved.source(), ClientIpSource::Peer);
        assert_eq!(
            resolved
                .ip()
                .expect("peer ip should be present")
                .to_string(),
            "10.10.0.4"
        );
    }

    #[test]
    fn client_ip_uses_forwarded_value_when_peer_proxy_is_trusted() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            "198.51.100.44, 203.0.113.10".parse().expect("valid header"),
        );
        let trusted = vec![IpNetwork::try_from(String::from("10.0.0.0/8")).expect("valid cidr")];
        let resolved = resolve_client_ip(
            &headers,
            Some("10.2.0.8".parse().expect("valid ip")),
            &trusted,
        );
        assert_eq!(resolved.source(), ClientIpSource::Forwarded);
        assert_eq!(
            resolved
                .ip()
                .expect("forwarded ip should be present")
                .to_string(),
            "198.51.100.44"
        );
    }

    #[test]
    fn client_ip_rejects_malformed_forwarded_value() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            "198.51.100.44:80".parse().expect("valid header"),
        );
        let trusted = vec![IpNetwork::try_from(String::from("10.0.0.0/8")).expect("valid cidr")];
        let resolved = resolve_client_ip(
            &headers,
            Some("10.2.0.8".parse().expect("valid ip")),
            &trusted,
        );
        assert_eq!(resolved.source(), ClientIpSource::Peer);
        assert_eq!(
            resolved
                .ip()
                .expect("peer ip should be present")
                .to_string(),
            "10.2.0.8"
        );
    }

    #[test]
    fn client_ip_rejects_oversized_forwarded_header() {
        let mut headers = HeaderMap::new();
        let oversized = format!("{},{}", "198.51.100.1", "9".repeat(600));
        headers.insert("x-forwarded-for", oversized.parse().expect("valid header"));
        let trusted = vec![IpNetwork::try_from(String::from("10.0.0.0/8")).expect("valid cidr")];
        let resolved = resolve_client_ip(
            &headers,
            Some("10.2.0.8".parse().expect("valid ip")),
            &trusted,
        );
        assert_eq!(resolved.source(), ClientIpSource::Peer);
        assert_eq!(
            resolved
                .ip()
                .expect("peer ip should be present")
                .to_string(),
            "10.2.0.8"
        );
    }

    #[tokio::test]
    async fn auth_rate_limit_sweep_prunes_stale_keys() {
        let state = AppState::new(&AppConfig::default()).expect("state should initialize");
        state
            .auth_route_hits
            .write()
            .await
            .insert(String::from("register:198.51.100.9"), vec![0]);

        let client_ip = ClientIp::peer(Some("198.51.100.10".parse().expect("valid ip")));
        enforce_auth_route_rate_limit(&state, client_ip, "register")
            .await
            .expect("rate limit should allow fresh key");

        let hits = state.auth_route_hits.read().await;
        assert!(
            !hits.contains_key("register:198.51.100.9"),
            "stale key should be swept"
        );
        assert!(
            hits.contains_key("register:198.51.100.10"),
            "fresh key should remain"
        );
    }

    #[tokio::test]
    async fn rate_limit_sweep_keeps_maps_bounded_under_many_unique_stale_keys() {
        let state = AppState::new(&AppConfig::default()).expect("state should initialize");

        {
            let mut auth_hits = state.auth_route_hits.write().await;
            for index in 0..256 {
                auth_hits.insert(format!("register:198.51.100.{index}"), vec![0]);
            }
        }
        {
            let mut ip_hits = state.directory_join_ip_hits.write().await;
            for index in 0..256 {
                ip_hits.insert(format!("198.51.101.{index}"), vec![0]);
            }
        }
        {
            let mut user_hits = state.directory_join_user_hits.write().await;
            for index in 0..256 {
                user_hits.insert(format!("user-{index}"), vec![0]);
            }
        }
        {
            let mut token_hits = state.media_token_hits.write().await;
            for index in 0..256 {
                token_hits.insert(format!("token-key-{index}"), vec![0]);
            }
        }
        {
            let mut publish_hits = state.media_publish_hits.write().await;
            for index in 0..256 {
                publish_hits.insert(format!("publish-key-{index}"), vec![0]);
            }
        }
        {
            let mut subscribe_leases = state.media_subscribe_leases.write().await;
            for index in 0..256 {
                subscribe_leases.insert(format!("lease-key-{index}"), vec![0]);
            }
        }

        let client_ip = ClientIp::peer(Some("203.0.113.25".parse().expect("valid ip")));
        enforce_auth_route_rate_limit(&state, client_ip, "register")
            .await
            .expect("fresh key should be accepted");

        let auth_hits = state.auth_route_hits.read().await;
        assert_eq!(auth_hits.len(), 1, "stale auth keys should be fully swept");
        drop(auth_hits);

        let ip_hits = state.directory_join_ip_hits.read().await;
        assert!(
            ip_hits.is_empty(),
            "stale directory ip keys should be swept"
        );
        drop(ip_hits);

        let user_hits = state.directory_join_user_hits.read().await;
        assert!(
            user_hits.is_empty(),
            "stale directory user keys should be swept"
        );
        drop(user_hits);

        let token_hits = state.media_token_hits.read().await;
        assert!(
            token_hits.is_empty(),
            "stale media token keys should be swept"
        );
        drop(token_hits);

        let publish_hits = state.media_publish_hits.read().await;
        assert!(
            publish_hits.is_empty(),
            "stale media publish keys should be swept"
        );
        drop(publish_hits);

        let subscribe_leases = state.media_subscribe_leases.read().await;
        assert!(
            subscribe_leases.is_empty(),
            "expired subscribe leases should be swept"
        );
    }
}
