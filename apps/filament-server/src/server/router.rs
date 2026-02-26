use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use anyhow::anyhow;
use axum::{
    extract::ConnectInfo,
    extract::DefaultBodyLimit,
    http::{header::AUTHORIZATION, request::Request, HeaderName, StatusCode},
    routing::{delete, get, patch, post},
    Router,
};

use pasetors::{
    claims::ClaimsValidationRules, keys::SymmetricKey, local, token::UntrustedToken, version4::V4,
    Local,
};
use tower::ServiceBuilder;
use tower_governor::{
    errors::GovernorError, governor::GovernorConfigBuilder, key_extractor::KeyExtractor,
    GovernorLayer,
};
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};

use super::{
    auth::resolve_client_ip,
    core::{AppConfig, AppState, MAX_LIVEKIT_TOKEN_TTL_SECS},
    db::ensure_db_schema,
    handlers::{
        auth::{login, logout, lookup_users, me, refresh, register},
        friends::{
            accept_friend_request, create_friend_request, delete_friend_request,
            list_friend_requests, list_friends, remove_friend,
        },
        guilds::{
            add_member, assign_guild_role, ban_member, create_channel, create_guild,
            create_guild_role, delete_guild_role, join_public_guild, kick_member, list_guild_audit,
            list_guild_channels, list_guild_ip_bans, list_guild_members, list_guild_roles,
            list_guilds, list_public_guilds, remove_guild_ip_ban, reorder_guild_roles,
            set_channel_permission_override, set_channel_role_override, unassign_guild_role,
            update_guild, update_guild_default_join_role, update_guild_role, update_member_role,
            upsert_guild_ip_bans_by_user,
        },
        media::{
            delete_attachment, download_attachment, issue_voice_token, leave_voice_channel,
            update_voice_participant_state, upload_attachment,
        },
        messages::{
            add_reaction, create_message, delete_message, edit_message, get_channel_permissions,
            get_messages, remove_reaction,
        },
        profile::{
            download_user_avatar, download_user_banner, get_user_profile, update_my_profile,
            upload_my_avatar, upload_my_banner,
        },
        search::{rebuild_search_index, reconcile_search_index, search_messages},
    },
    realtime::gateway_ws,
    types::{echo, health, metrics, slow},
};

#[cfg(test)]
pub(crate) const ROUTE_MANIFEST: &[(&str, &str)] = &[
    ("GET", "/health"),
    ("GET", "/metrics"),
    ("POST", "/echo"),
    ("GET", "/slow"),
    ("POST", "/auth/register"),
    ("POST", "/auth/login"),
    ("POST", "/auth/refresh"),
    ("POST", "/auth/logout"),
    ("GET", "/auth/me"),
    ("PATCH", "/users/me/profile"),
    ("GET", "/users/{user_id}/profile"),
    ("GET", "/users/{user_id}/avatar"),
    ("GET", "/users/{user_id}/banner"),
    ("POST", "/users/lookup"),
    ("GET", "/friends"),
    ("DELETE", "/friends/{friend_user_id}"),
    ("POST", "/friends/requests"),
    ("GET", "/friends/requests"),
    ("POST", "/friends/requests/{request_id}/accept"),
    ("DELETE", "/friends/requests/{request_id}"),
    ("POST", "/guilds"),
    ("GET", "/guilds"),
    ("PATCH", "/guilds/{guild_id}"),
    ("GET", "/guilds/public"),
    ("POST", "/guilds/{guild_id}/join"),
    ("GET", "/guilds/{guild_id}/audit"),
    ("GET", "/guilds/{guild_id}/members"),
    ("GET", "/guilds/{guild_id}/roles"),
    ("POST", "/guilds/{guild_id}/roles"),
    ("POST", "/guilds/{guild_id}/roles/reorder"),
    ("POST", "/guilds/{guild_id}/roles/default"),
    ("PATCH", "/guilds/{guild_id}/roles/{role_id}"),
    ("DELETE", "/guilds/{guild_id}/roles/{role_id}"),
    (
        "POST",
        "/guilds/{guild_id}/roles/{role_id}/members/{user_id}",
    ),
    (
        "DELETE",
        "/guilds/{guild_id}/roles/{role_id}/members/{user_id}",
    ),
    ("GET", "/guilds/{guild_id}/ip-bans"),
    ("POST", "/guilds/{guild_id}/ip-bans/by-user"),
    ("DELETE", "/guilds/{guild_id}/ip-bans/{ban_id}"),
    ("POST", "/guilds/{guild_id}/channels"),
    ("GET", "/guilds/{guild_id}/channels"),
    (
        "GET",
        "/guilds/{guild_id}/channels/{channel_id}/permissions/self",
    ),
    (
        "POST",
        "/guilds/{guild_id}/channels/{channel_id}/overrides/{role}",
    ),
    (
        "POST",
        "/guilds/{guild_id}/channels/{channel_id}/permission-overrides/{target_kind}/{target_id}",
    ),
    ("POST", "/guilds/{guild_id}/channels/{channel_id}/messages"),
    ("GET", "/guilds/{guild_id}/channels/{channel_id}/messages"),
    (
        "PATCH",
        "/guilds/{guild_id}/channels/{channel_id}/messages/{message_id}",
    ),
    (
        "DELETE",
        "/guilds/{guild_id}/channels/{channel_id}/messages/{message_id}",
    ),
    (
        "POST",
        "/guilds/{guild_id}/channels/{channel_id}/messages/{message_id}/reactions/{emoji}",
    ),
    (
        "DELETE",
        "/guilds/{guild_id}/channels/{channel_id}/messages/{message_id}/reactions/{emoji}",
    ),
    (
        "POST",
        "/guilds/{guild_id}/channels/{channel_id}/voice/token",
    ),
    (
        "POST",
        "/guilds/{guild_id}/channels/{channel_id}/voice/leave",
    ),
    (
        "POST",
        "/guilds/{guild_id}/channels/{channel_id}/voice/state",
    ),
    ("GET", "/guilds/{guild_id}/search"),
    ("POST", "/guilds/{guild_id}/search/rebuild"),
    ("POST", "/guilds/{guild_id}/search/reconcile"),
    (
        "GET",
        "/guilds/{guild_id}/channels/{channel_id}/attachments/{attachment_id}",
    ),
    (
        "DELETE",
        "/guilds/{guild_id}/channels/{channel_id}/attachments/{attachment_id}",
    ),
    ("POST", "/guilds/{guild_id}/members/{user_id}"),
    ("PATCH", "/guilds/{guild_id}/members/{user_id}"),
    ("POST", "/guilds/{guild_id}/members/{user_id}/kick"),
    ("POST", "/guilds/{guild_id}/members/{user_id}/ban"),
    ("GET", "/gateway/ws"),
    (
        "POST",
        "/guilds/{guild_id}/channels/{channel_id}/attachments",
    ),
    ("POST", "/users/me/profile/avatar"),
    ("POST", "/users/me/profile/banner"),
];

#[derive(Clone)]
struct TrustedClientIpKeyExtractor {
    trusted_proxy_cidrs: Arc<Vec<super::directory_contract::IpNetwork>>,
    token_key: Arc<SymmetricKey<V4>>,
}

impl TrustedClientIpKeyExtractor {
    fn new(
        trusted_proxy_cidrs: Arc<Vec<super::directory_contract::IpNetwork>>,
        token_key: Arc<SymmetricKey<V4>>,
    ) -> Self {
        Self {
            trusted_proxy_cidrs,
            token_key,
        }
    }
}

impl KeyExtractor for TrustedClientIpKeyExtractor {
    type Key = String;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        if let Some(auth_header) = req.headers().get(AUTHORIZATION) {
            if let Ok(auth_str) = auth_header.to_str() {
                if let Some(token) = auth_str.strip_prefix("Bearer ") {
                    if let Ok(untrusted) = UntrustedToken::<Local, V4>::try_from(token) {
                        let validation_rules = ClaimsValidationRules::new();
                        if let Ok(trusted) = local::decrypt(
                            &self.token_key,
                            &untrusted,
                            &validation_rules,
                            None,
                            None,
                        ) {
                            if let Some(claims) = trusted.payload_claims() {
                                if let Some(sub) = claims.get_claim("sub").and_then(|v| v.as_str())
                                {
                                    return Ok(format!("user:{sub}"));
                                }
                            }
                        }
                    }
                }
            }
        }

        let peer_ip = req
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|value| value.0.ip())
            .or_else(|| req.extensions().get::<SocketAddr>().map(SocketAddr::ip));
        let resolved =
            resolve_client_ip(req.headers(), peer_ip, self.trusted_proxy_cidrs.as_slice());
        let ip = resolved.ip().unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
        Ok(format!("ip:{ip}"))
    }
}

/// Build the axum router with global security middleware.
///
/// # Errors
/// Returns an error if configured security limits are invalid.
#[allow(clippy::too_many_lines)]
pub fn build_router(config: &AppConfig) -> anyhow::Result<Router> {
    validate_router_config(config)?;

    let app_state = AppState::new(config)?;
    build_router_with_state(config, app_state)
}

/// Build the axum router and fail-fast if database schema bootstrap fails.
///
/// # Errors
/// Returns an error if configured security limits are invalid or schema bootstrap fails.
pub async fn build_router_with_db_bootstrap(config: &AppConfig) -> anyhow::Result<Router> {
    validate_router_config(config)?;

    let app_state = AppState::new(config)?;
    ensure_db_schema(&app_state)
        .await
        .map_err(|_| anyhow!("database schema bootstrap failed"))?;

    build_router_with_state(config, app_state)
}

fn validate_router_config(config: &AppConfig) -> anyhow::Result<()> {
    if config.rate_limit_requests_per_minute == 0 {
        return Err(anyhow!(
            "global rate limit must be at least 1 request per minute"
        ));
    }
    if config.auth_route_requests_per_minute == 0 {
        return Err(anyhow!(
            "auth route rate limit must be at least 1 request per minute"
        ));
    }
    if config.gateway_ingress_events_per_window == 0 {
        return Err(anyhow!(
            "gateway ingress rate limit must be at least 1 event per window"
        ));
    }
    if config.gateway_ingress_window.is_zero() {
        return Err(anyhow!("gateway ingress window must be at least 1 second"));
    }
    if config.max_gateway_event_bytes > filament_protocol::MAX_EVENT_BYTES {
        return Err(anyhow!(
            "gateway event limit cannot exceed protocol max of {} bytes",
            filament_protocol::MAX_EVENT_BYTES
        ));
    }
    if config.media_token_requests_per_minute == 0 {
        return Err(anyhow!(
            "media token rate limit must be at least 1 request per minute"
        ));
    }
    if config.media_publish_requests_per_minute == 0 {
        return Err(anyhow!(
            "media publish rate limit must be at least 1 request per minute"
        ));
    }
    if config.media_subscribe_token_cap_per_channel == 0 {
        return Err(anyhow!(
            "media subscribe token cap must be at least 1 active token"
        ));
    }
    if config.max_created_guilds_per_user == 0 {
        return Err(anyhow!(
            "max created guilds per user must be at least 1 guild"
        ));
    }
    if config.directory_join_requests_per_minute_per_ip == 0 {
        return Err(anyhow!(
            "directory join per-ip rate limit must be at least 1 request per minute"
        ));
    }
    if config.directory_join_requests_per_minute_per_user == 0 {
        return Err(anyhow!(
            "directory join per-user rate limit must be at least 1 request per minute"
        ));
    }
    if config.audit_list_limit_max == 0 {
        return Err(anyhow!(
            "audit list limit max must be at least 1 record per request"
        ));
    }
    if config.guild_ip_ban_max_entries == 0 {
        return Err(anyhow!(
            "guild ip ban max entries must be at least 1 record"
        ));
    }
    if config.max_profile_avatar_bytes == 0 {
        return Err(anyhow!("max profile avatar bytes must be at least 1 byte"));
    }
    if config.max_profile_banner_bytes == 0 {
        return Err(anyhow!("max profile banner bytes must be at least 1 byte"));
    }
    if config.livekit_token_ttl.is_zero()
        || config.livekit_token_ttl > Duration::from_secs(MAX_LIVEKIT_TOKEN_TTL_SECS)
    {
        return Err(anyhow!(
            "livekit token ttl must be between 1 and {MAX_LIVEKIT_TOKEN_TTL_SECS} seconds"
        ));
    }

    Ok(())
}

#[allow(clippy::too_many_lines)]
fn build_router_with_state(config: &AppConfig, app_state: AppState) -> anyhow::Result<Router> {
    tokio::spawn(crate::server::realtime::livekit_sync::start_livekit_sync(
        app_state.clone(),
    ));

    let governor_config = Arc::new(
        GovernorConfigBuilder::default()
            .period(Duration::from_secs(60))
            .burst_size(config.rate_limit_requests_per_minute)
            .key_extractor(TrustedClientIpKeyExtractor::new(
                Arc::new(config.trusted_proxy_cidrs.clone()),
                app_state.token_key.clone(),
            ))
            .finish()
            .ok_or_else(|| anyhow!("invalid governor configuration"))?,
    );
    let request_id_header = HeaderName::from_static("x-request-id");
    let governor_layer = GovernorLayer::new(governor_config);

    let routes = Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .route("/echo", post(echo))
        .route("/slow", get(slow))
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/refresh", post(refresh))
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
        .route("/users/me/profile", patch(update_my_profile))
        .route("/users/{user_id}/profile", get(get_user_profile))
        .route("/users/{user_id}/avatar", get(download_user_avatar))
        .route("/users/{user_id}/banner", get(download_user_banner))
        .route("/users/lookup", post(lookup_users))
        .route("/friends", get(list_friends))
        .route("/friends/{friend_user_id}", delete(remove_friend))
        .route(
            "/friends/requests",
            post(create_friend_request).get(list_friend_requests),
        )
        .route(
            "/friends/requests/{request_id}/accept",
            post(accept_friend_request),
        )
        .route(
            "/friends/requests/{request_id}",
            delete(delete_friend_request),
        )
        .route("/guilds", post(create_guild).get(list_guilds))
        .route("/guilds/{guild_id}", patch(update_guild))
        .route("/guilds/public", get(list_public_guilds))
        .route("/guilds/{guild_id}/join", post(join_public_guild))
        .route("/guilds/{guild_id}/audit", get(list_guild_audit))
        .route(
            "/guilds/{guild_id}/roles",
            get(list_guild_roles).post(create_guild_role),
        )
        .route(
            "/guilds/{guild_id}/roles/reorder",
            post(reorder_guild_roles),
        )
        .route(
            "/guilds/{guild_id}/roles/default",
            post(update_guild_default_join_role),
        )
        .route(
            "/guilds/{guild_id}/roles/{role_id}",
            patch(update_guild_role).delete(delete_guild_role),
        )
        .route(
            "/guilds/{guild_id}/roles/{role_id}/members/{user_id}",
            post(assign_guild_role).delete(unassign_guild_role),
        )
        .route("/guilds/{guild_id}/ip-bans", get(list_guild_ip_bans))
        .route(
            "/guilds/{guild_id}/ip-bans/by-user",
            post(upsert_guild_ip_bans_by_user),
        )
        .route(
            "/guilds/{guild_id}/ip-bans/{ban_id}",
            delete(remove_guild_ip_ban),
        )
        .route(
            "/guilds/{guild_id}/channels",
            post(create_channel).get(list_guild_channels),
        )
        .route(
            "/guilds/{guild_id}/channels/{channel_id}/permissions/self",
            get(get_channel_permissions),
        )
        .route(
            "/guilds/{guild_id}/channels/{channel_id}/overrides/{role}",
            post(set_channel_role_override),
        )
        .route(
            "/guilds/{guild_id}/channels/{channel_id}/permission-overrides/{target_kind}/{target_id}",
            post(set_channel_permission_override),
        )
        .route(
            "/guilds/{guild_id}/channels/{channel_id}/messages",
            post(create_message).get(get_messages),
        )
        .route(
            "/guilds/{guild_id}/channels/{channel_id}/messages/{message_id}",
            patch(edit_message).delete(delete_message),
        )
        .route(
            "/guilds/{guild_id}/channels/{channel_id}/messages/{message_id}/reactions/{emoji}",
            post(add_reaction).delete(remove_reaction),
        )
        .route(
            "/guilds/{guild_id}/channels/{channel_id}/voice/token",
            post(issue_voice_token),
        )
        .route(
            "/guilds/{guild_id}/channels/{channel_id}/voice/leave",
            post(leave_voice_channel),
        )
        .route(
            "/guilds/{guild_id}/channels/{channel_id}/voice/state",
            post(update_voice_participant_state),
        )
        .route("/guilds/{guild_id}/search", get(search_messages))
        .route(
            "/guilds/{guild_id}/search/rebuild",
            post(rebuild_search_index),
        )
        .route(
            "/guilds/{guild_id}/search/reconcile",
            post(reconcile_search_index),
        )
        .route(
            "/guilds/{guild_id}/channels/{channel_id}/attachments/{attachment_id}",
            get(download_attachment).delete(delete_attachment),
        )
        .route("/guilds/{guild_id}/members", get(list_guild_members))
        .route(
            "/guilds/{guild_id}/members/{user_id}",
            post(add_member).patch(update_member_role),
        )
        .route(
            "/guilds/{guild_id}/members/{user_id}/kick",
            post(kick_member),
        )
        .route("/guilds/{guild_id}/members/{user_id}/ban", post(ban_member))
        .route("/gateway/ws", get(gateway_ws));

    let upload_route = Router::new()
        .route(
            "/guilds/{guild_id}/channels/{channel_id}/attachments",
            post(upload_attachment),
        )
        .route("/users/me/profile/avatar", post(upload_my_avatar))
        .route("/users/me/profile/banner", post(upload_my_banner))
        .layer(DefaultBodyLimit::disable());

    Ok(routes
        .merge(upload_route)
        .with_state(app_state)
        .layer(DefaultBodyLimit::max(config.max_body_bytes))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(PropagateRequestIdLayer::new(request_id_header.clone()))
                .layer(SetRequestIdLayer::new(request_id_header, MakeRequestUuid))
                .layer(TimeoutLayer::with_status_code(
                    StatusCode::REQUEST_TIMEOUT,
                    config.request_timeout,
                ))
                .layer(governor_layer),
        ))
}

#[cfg(test)]
mod tests {
    use super::build_router_with_db_bootstrap;
    use crate::server::core::AppConfig;

    #[tokio::test]
    async fn build_router_with_db_bootstrap_fails_fast_when_schema_init_fails() {
        let result = build_router_with_db_bootstrap(&AppConfig {
            database_url: Some(String::from("postgres://127.0.0.1:1/filament")),
            ..AppConfig::default()
        })
        .await;

        assert!(
            result.is_err(),
            "schema bootstrap failure should fail router startup"
        );
    }
}
