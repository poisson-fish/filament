#![forbid(unsafe_code)]

use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use filament_core::UserId;
use filament_server::{build_router, directory_contract::IpNetwork, init_tracing, AppConfig};
use tokio::net::TcpListener;

fn parse_usize_env_or_default(var_name: &str, default: usize) -> anyhow::Result<usize> {
    std::env::var(var_name).map_or_else(
        |_| Ok(default),
        |value| {
            value
                .parse::<usize>()
                .map_err(|e| anyhow::anyhow!("invalid {var_name} value {value:?}: {e}"))
        },
    )
}

fn parse_u32_env_or_default(var_name: &str, default: u32) -> anyhow::Result<u32> {
    std::env::var(var_name).map_or_else(
        |_| Ok(default),
        |value| {
            value
                .parse::<u32>()
                .map_err(|e| anyhow::anyhow!("invalid {var_name} value {value:?}: {e}"))
        },
    )
}

fn parse_u64_env_or_default(var_name: &str, default: u64) -> anyhow::Result<u64> {
    std::env::var(var_name).map_or_else(
        |_| Ok(default),
        |value| {
            value
                .parse::<u64>()
                .map_err(|e| anyhow::anyhow!("invalid {var_name} value {value:?}: {e}"))
        },
    )
}

fn parse_rate_limit_requests_per_minute_from_env(defaults: &AppConfig) -> anyhow::Result<u32> {
    parse_u32_env_or_default(
        "FILAMENT_RATE_LIMIT_REQUESTS_PER_MINUTE",
        defaults.rate_limit_requests_per_minute,
    )
}

fn parse_rate_runtime_limits_from_env(
    defaults: &AppConfig,
) -> anyhow::Result<(u32, u32, Duration, u32, u32)> {
    let auth_route_requests_per_minute = parse_u32_env_or_default(
        "FILAMENT_AUTH_ROUTE_REQUESTS_PER_MINUTE",
        defaults.auth_route_requests_per_minute,
    )?;
    let gateway_ingress_events_per_window = parse_u32_env_or_default(
        "FILAMENT_GATEWAY_INGRESS_EVENTS_PER_WINDOW",
        defaults.gateway_ingress_events_per_window,
    )?;
    let gateway_ingress_window_secs = parse_u64_env_or_default(
        "FILAMENT_GATEWAY_INGRESS_WINDOW_SECS",
        defaults.gateway_ingress_window.as_secs(),
    )?;
    let media_token_requests_per_minute = parse_u32_env_or_default(
        "FILAMENT_MEDIA_TOKEN_REQUESTS_PER_MINUTE",
        defaults.media_token_requests_per_minute,
    )?;
    let media_publish_requests_per_minute = parse_u32_env_or_default(
        "FILAMENT_MEDIA_PUBLISH_REQUESTS_PER_MINUTE",
        defaults.media_publish_requests_per_minute,
    )?;
    Ok((
        auth_route_requests_per_minute,
        gateway_ingress_events_per_window,
        Duration::from_secs(gateway_ingress_window_secs),
        media_token_requests_per_minute,
        media_publish_requests_per_minute,
    ))
}

fn parse_directory_runtime_limits_from_env(
    defaults: &AppConfig,
) -> anyhow::Result<(u32, u32, usize, usize)> {
    let join_per_ip = parse_u32_env_or_default(
        "FILAMENT_DIRECTORY_JOIN_REQUESTS_PER_MINUTE_PER_IP",
        defaults.directory_join_requests_per_minute_per_ip,
    )?;
    let join_per_user = parse_u32_env_or_default(
        "FILAMENT_DIRECTORY_JOIN_REQUESTS_PER_MINUTE_PER_USER",
        defaults.directory_join_requests_per_minute_per_user,
    )?;
    let audit_list_limit_max = parse_usize_env_or_default(
        "FILAMENT_AUDIT_LIST_LIMIT_MAX",
        defaults.audit_list_limit_max,
    )?;
    let guild_ip_ban_max_entries = parse_usize_env_or_default(
        "FILAMENT_GUILD_IP_BAN_MAX_ENTRIES",
        defaults.guild_ip_ban_max_entries,
    )?;
    Ok((
        join_per_ip,
        join_per_user,
        audit_list_limit_max,
        guild_ip_ban_max_entries,
    ))
}

fn parse_trusted_proxy_cidrs_from_env(defaults: &AppConfig) -> anyhow::Result<Vec<IpNetwork>> {
    std::env::var("FILAMENT_TRUSTED_PROXY_CIDRS").map_or_else(
        |_| Ok(defaults.trusted_proxy_cidrs.clone()),
        |raw| {
            if raw.trim().is_empty() {
                return Ok(Vec::new());
            }
            raw.split(',')
                .enumerate()
                .map(|(index, value)| {
                    let candidate = value.trim();
                    if candidate.is_empty() {
                        return Err(anyhow::anyhow!(
                            "invalid FILAMENT_TRUSTED_PROXY_CIDRS entry at position {}: empty value",
                            index + 1
                        ));
                    }
                    IpNetwork::from_str(candidate).map_err(|_| {
                        anyhow::anyhow!(
                            "invalid FILAMENT_TRUSTED_PROXY_CIDRS entry at position {}: {candidate:?}",
                            index + 1
                        )
                    })
                })
                .collect()
        },
    )
}

fn parse_server_owner_user_id_from_env(defaults: &AppConfig) -> anyhow::Result<Option<UserId>> {
    std::env::var("FILAMENT_SERVER_OWNER_USER_ID").map_or_else(
        |_| Ok(defaults.server_owner_user_id),
        |value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            UserId::try_from(trimmed.to_owned())
                .map(Some)
                .map_err(|_| anyhow::anyhow!("invalid FILAMENT_SERVER_OWNER_USER_ID"))
        },
    )
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let database_url = std::env::var("FILAMENT_DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("FILAMENT_DATABASE_URL is required for runtime"))?;
    let livekit_api_key = std::env::var("FILAMENT_LIVEKIT_API_KEY")
        .map_err(|_| anyhow::anyhow!("FILAMENT_LIVEKIT_API_KEY is required for runtime"))?;
    let livekit_api_secret = std::env::var("FILAMENT_LIVEKIT_API_SECRET")
        .map_err(|_| anyhow::anyhow!("FILAMENT_LIVEKIT_API_SECRET is required for runtime"))?;
    let defaults = AppConfig::default();
    let rate_limit_requests_per_minute = parse_rate_limit_requests_per_minute_from_env(&defaults)?;
    let (
        auth_route_requests_per_minute,
        gateway_ingress_events_per_window,
        gateway_ingress_window,
        media_token_requests_per_minute,
        media_publish_requests_per_minute,
    ) = parse_rate_runtime_limits_from_env(&defaults)?;
    let max_created_guilds_per_user = parse_usize_env_or_default(
        "FILAMENT_MAX_CREATED_GUILDS_PER_USER",
        defaults.max_created_guilds_per_user,
    )?;
    let (
        directory_join_requests_per_minute_per_ip,
        directory_join_requests_per_minute_per_user,
        audit_list_limit_max,
        guild_ip_ban_max_entries,
    ) = parse_directory_runtime_limits_from_env(&defaults)?;
    let trusted_proxy_cidrs = parse_trusted_proxy_cidrs_from_env(&defaults)?;
    let server_owner_user_id = parse_server_owner_user_id_from_env(&defaults)?;
    let captcha_hcaptcha_site_key = std::env::var("FILAMENT_HCAPTCHA_SITE_KEY").ok();
    let captcha_hcaptcha_secret = std::env::var("FILAMENT_HCAPTCHA_SECRET").ok();
    let app_config = AppConfig {
        attachment_root: std::env::var("FILAMENT_ATTACHMENT_ROOT")
            .map_or_else(|_| PathBuf::from("./data/attachments"), PathBuf::from),
        livekit_url: std::env::var("FILAMENT_LIVEKIT_URL")
            .unwrap_or_else(|_| String::from("ws://127.0.0.1:7880")),
        livekit_api_key: Some(livekit_api_key),
        livekit_api_secret: Some(livekit_api_secret),
        rate_limit_requests_per_minute,
        auth_route_requests_per_minute,
        gateway_ingress_events_per_window,
        gateway_ingress_window,
        media_token_requests_per_minute,
        media_publish_requests_per_minute,
        max_created_guilds_per_user,
        directory_join_requests_per_minute_per_ip,
        directory_join_requests_per_minute_per_user,
        audit_list_limit_max,
        guild_ip_ban_max_entries,
        trusted_proxy_cidrs,
        server_owner_user_id,
        captcha_hcaptcha_site_key,
        captcha_hcaptcha_secret,
        captcha_verify_url: std::env::var("FILAMENT_HCAPTCHA_VERIFY_URL")
            .unwrap_or_else(|_| String::from("https://hcaptcha.com/siteverify")),
        database_url: Some(database_url),
        ..AppConfig::default()
    };
    let app = build_router(&app_config)?;
    let addr = std::env::var("FILAMENT_BIND_ADDR")
        .unwrap_or_else(|_| String::from("0.0.0.0:3000"))
        .parse::<SocketAddr>()
        .map_err(|e| anyhow::anyhow!("invalid FILAMENT_BIND_ADDR: {e}"))?;
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "filament-server listening");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        parse_directory_runtime_limits_from_env, parse_rate_limit_requests_per_minute_from_env,
        parse_rate_runtime_limits_from_env, parse_server_owner_user_id_from_env,
        parse_trusted_proxy_cidrs_from_env, parse_u32_env_or_default, parse_u64_env_or_default,
        parse_usize_env_or_default,
    };
    use filament_core::UserId;
    use filament_server::{directory_contract::IpNetwork, AppConfig};
    use std::{
        sync::{Mutex, OnceLock},
        time::Duration,
    };

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock should not be poisoned")
    }

    #[test]
    fn parse_usize_env_or_default_rejects_invalid_values() {
        let _guard = lock_env();
        let key = "FILAMENT_TEST_PARSE_USIZE_INVALID";
        std::env::set_var(key, "not-a-number");
        let result = parse_usize_env_or_default(key, 10);
        std::env::remove_var(key);
        assert!(result.is_err());
    }

    #[test]
    fn parse_u32_env_or_default_rejects_invalid_values() {
        let _guard = lock_env();
        let key = "FILAMENT_TEST_PARSE_U32_INVALID";
        std::env::set_var(key, "NaN");
        let result = parse_u32_env_or_default(key, 10);
        std::env::remove_var(key);
        assert!(result.is_err());
    }

    #[test]
    fn parse_u64_env_or_default_rejects_invalid_values() {
        let _guard = lock_env();
        let key = "FILAMENT_TEST_PARSE_U64_INVALID";
        std::env::set_var(key, "NaN");
        let result = parse_u64_env_or_default(key, 10);
        std::env::remove_var(key);
        assert!(result.is_err());
    }

    #[test]
    fn rate_limit_env_override_is_parsed() {
        let _guard = lock_env();
        std::env::remove_var("FILAMENT_RATE_LIMIT_REQUESTS_PER_MINUTE");
        std::env::set_var("FILAMENT_RATE_LIMIT_REQUESTS_PER_MINUTE", "240");

        let parsed = parse_rate_limit_requests_per_minute_from_env(&AppConfig::default())
            .expect("rate limit env should parse");

        std::env::remove_var("FILAMENT_RATE_LIMIT_REQUESTS_PER_MINUTE");
        assert_eq!(parsed, 240);
    }

    #[test]
    fn rate_limit_env_rejects_invalid_values() {
        let _guard = lock_env();
        std::env::remove_var("FILAMENT_RATE_LIMIT_REQUESTS_PER_MINUTE");
        std::env::set_var("FILAMENT_RATE_LIMIT_REQUESTS_PER_MINUTE", "bogus");

        let result = parse_rate_limit_requests_per_minute_from_env(&AppConfig::default());

        std::env::remove_var("FILAMENT_RATE_LIMIT_REQUESTS_PER_MINUTE");
        assert!(result.is_err());
    }

    #[test]
    fn rate_runtime_limits_env_overrides_are_parsed() {
        let _guard = lock_env();
        std::env::remove_var("FILAMENT_AUTH_ROUTE_REQUESTS_PER_MINUTE");
        std::env::remove_var("FILAMENT_GATEWAY_INGRESS_EVENTS_PER_WINDOW");
        std::env::remove_var("FILAMENT_GATEWAY_INGRESS_WINDOW_SECS");
        std::env::remove_var("FILAMENT_MEDIA_TOKEN_REQUESTS_PER_MINUTE");
        std::env::remove_var("FILAMENT_MEDIA_PUBLISH_REQUESTS_PER_MINUTE");
        std::env::set_var("FILAMENT_AUTH_ROUTE_REQUESTS_PER_MINUTE", "90");
        std::env::set_var("FILAMENT_GATEWAY_INGRESS_EVENTS_PER_WINDOW", "75");
        std::env::set_var("FILAMENT_GATEWAY_INGRESS_WINDOW_SECS", "12");
        std::env::set_var("FILAMENT_MEDIA_TOKEN_REQUESTS_PER_MINUTE", "120");
        std::env::set_var("FILAMENT_MEDIA_PUBLISH_REQUESTS_PER_MINUTE", "40");

        let parsed = parse_rate_runtime_limits_from_env(&AppConfig::default())
            .expect("runtime rate limits should parse");

        std::env::remove_var("FILAMENT_AUTH_ROUTE_REQUESTS_PER_MINUTE");
        std::env::remove_var("FILAMENT_GATEWAY_INGRESS_EVENTS_PER_WINDOW");
        std::env::remove_var("FILAMENT_GATEWAY_INGRESS_WINDOW_SECS");
        std::env::remove_var("FILAMENT_MEDIA_TOKEN_REQUESTS_PER_MINUTE");
        std::env::remove_var("FILAMENT_MEDIA_PUBLISH_REQUESTS_PER_MINUTE");

        assert_eq!(parsed, (90, 75, Duration::from_secs(12), 120, 40));
    }

    #[test]
    fn rate_runtime_limits_env_rejects_invalid_values() {
        let _guard = lock_env();
        std::env::remove_var("FILAMENT_GATEWAY_INGRESS_WINDOW_SECS");
        std::env::set_var("FILAMENT_GATEWAY_INGRESS_WINDOW_SECS", "bad");

        let result = parse_rate_runtime_limits_from_env(&AppConfig::default());

        std::env::remove_var("FILAMENT_GATEWAY_INGRESS_WINDOW_SECS");
        assert!(result.is_err());
    }

    #[test]
    fn directory_runtime_limits_env_overrides_are_parsed() {
        let _guard = lock_env();
        std::env::remove_var("FILAMENT_DIRECTORY_JOIN_REQUESTS_PER_MINUTE_PER_IP");
        std::env::remove_var("FILAMENT_DIRECTORY_JOIN_REQUESTS_PER_MINUTE_PER_USER");
        std::env::remove_var("FILAMENT_AUDIT_LIST_LIMIT_MAX");
        std::env::remove_var("FILAMENT_GUILD_IP_BAN_MAX_ENTRIES");
        std::env::set_var("FILAMENT_DIRECTORY_JOIN_REQUESTS_PER_MINUTE_PER_IP", "31");
        std::env::set_var("FILAMENT_DIRECTORY_JOIN_REQUESTS_PER_MINUTE_PER_USER", "19");
        std::env::set_var("FILAMENT_AUDIT_LIST_LIMIT_MAX", "250");
        std::env::set_var("FILAMENT_GUILD_IP_BAN_MAX_ENTRIES", "1200");

        let parsed = parse_directory_runtime_limits_from_env(&AppConfig::default())
            .expect("directory env limits should parse");

        std::env::remove_var("FILAMENT_DIRECTORY_JOIN_REQUESTS_PER_MINUTE_PER_IP");
        std::env::remove_var("FILAMENT_DIRECTORY_JOIN_REQUESTS_PER_MINUTE_PER_USER");
        std::env::remove_var("FILAMENT_AUDIT_LIST_LIMIT_MAX");
        std::env::remove_var("FILAMENT_GUILD_IP_BAN_MAX_ENTRIES");

        assert_eq!(parsed, (31, 19, 250, 1200));
    }

    #[test]
    fn directory_runtime_limits_env_rejects_invalid_values() {
        let _guard = lock_env();
        std::env::remove_var("FILAMENT_AUDIT_LIST_LIMIT_MAX");
        std::env::set_var("FILAMENT_AUDIT_LIST_LIMIT_MAX", "bogus");
        let result = parse_directory_runtime_limits_from_env(&AppConfig::default());
        std::env::remove_var("FILAMENT_AUDIT_LIST_LIMIT_MAX");
        assert!(result.is_err());
    }

    #[test]
    fn trusted_proxy_cidrs_env_overrides_are_parsed() {
        let _guard = lock_env();
        std::env::remove_var("FILAMENT_TRUSTED_PROXY_CIDRS");
        std::env::set_var(
            "FILAMENT_TRUSTED_PROXY_CIDRS",
            "10.0.0.0/8, 192.0.2.10, 2001:db8::/64",
        );
        let parsed = parse_trusted_proxy_cidrs_from_env(&AppConfig::default())
            .expect("trusted proxy cidrs should parse");
        std::env::remove_var("FILAMENT_TRUSTED_PROXY_CIDRS");
        assert_eq!(
            parsed,
            vec![
                IpNetwork::try_from(String::from("10.0.0.0/8")).expect("valid cidr"),
                IpNetwork::try_from(String::from("192.0.2.10")).expect("valid host"),
                IpNetwork::try_from(String::from("2001:db8::/64")).expect("valid cidr"),
            ]
        );
    }

    #[test]
    fn trusted_proxy_cidrs_env_rejects_invalid_values() {
        let _guard = lock_env();
        std::env::remove_var("FILAMENT_TRUSTED_PROXY_CIDRS");
        std::env::set_var("FILAMENT_TRUSTED_PROXY_CIDRS", "10.0.0.0/8,bad-cidr");
        let result = parse_trusted_proxy_cidrs_from_env(&AppConfig::default());
        std::env::remove_var("FILAMENT_TRUSTED_PROXY_CIDRS");
        assert!(result.is_err());
    }

    #[test]
    fn server_owner_user_id_env_override_is_parsed() {
        let _guard = lock_env();
        std::env::remove_var("FILAMENT_SERVER_OWNER_USER_ID");
        std::env::set_var(
            "FILAMENT_SERVER_OWNER_USER_ID",
            "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        );
        let parsed = parse_server_owner_user_id_from_env(&AppConfig::default())
            .expect("server owner user id should parse");
        std::env::remove_var("FILAMENT_SERVER_OWNER_USER_ID");
        assert_eq!(
            parsed,
            Some(UserId::try_from(String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV")).expect("valid ulid"))
        );
    }

    #[test]
    fn server_owner_user_id_env_rejects_invalid_values() {
        let _guard = lock_env();
        std::env::remove_var("FILAMENT_SERVER_OWNER_USER_ID");
        std::env::set_var("FILAMENT_SERVER_OWNER_USER_ID", "not-ulid");
        let result = parse_server_owner_user_id_from_env(&AppConfig::default());
        std::env::remove_var("FILAMENT_SERVER_OWNER_USER_ID");
        assert!(result.is_err());
    }
}
