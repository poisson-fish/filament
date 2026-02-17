use crate::server::{
    auth::{now_unix, ClientIp},
    core::AppState,
    directory_contract::IpNetwork,
    errors::AuthFailure,
};
use filament_core::UserId;
use sqlx::Row;

use super::write_audit_log;

pub(crate) async fn guild_has_active_ip_ban_for_client(
    state: &AppState,
    guild_id: &str,
    client_ip: ClientIp,
) -> Result<bool, AuthFailure> {
    let Some(ip) = client_ip.ip() else {
        return Ok(false);
    };
    let now = now_unix();

    if let Some(pool) = &state.db_pool {
        let rows = sqlx::query(
            "SELECT ip_cidr
             FROM guild_ip_bans
             WHERE guild_id = $1
               AND (expires_at_unix IS NULL OR expires_at_unix > $2)
             ORDER BY created_at_unix DESC
             LIMIT $3",
        )
        .bind(guild_id)
        .bind(now)
        .bind(
            i64::try_from(state.runtime.guild_ip_ban_max_entries)
                .map_err(|_| AuthFailure::Internal)?,
        )
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        for row in rows {
            let cidr: String = row.try_get("ip_cidr").map_err(|_| AuthFailure::Internal)?;
            let Ok(network) = IpNetwork::try_from(cidr) else {
                continue;
            };
            if network.contains(ip) {
                return Ok(true);
            }
        }
        return Ok(false);
    }

    let bans = state.guild_ip_bans.read().await;
    let Some(guild_bans) = bans.get(guild_id) else {
        return Ok(false);
    };
    Ok(guild_bans.iter().any(|entry| {
        entry.expires_at_unix.is_none_or(|expires| expires > now) && entry.ip_network.contains(ip)
    }))
}

pub(crate) async fn enforce_guild_ip_ban_for_request(
    state: &AppState,
    guild_id: &str,
    user_id: UserId,
    client_ip: ClientIp,
    surface: &'static str,
) -> Result<(), AuthFailure> {
    if !guild_has_active_ip_ban_for_client(state, guild_id, client_ip).await? {
        return Ok(());
    }
    write_audit_log(
        state,
        Some(guild_id.to_owned()),
        user_id,
        Some(user_id),
        "moderation.ip_ban.hit",
        serde_json::json!({
            "surface": surface,
            "client_ip_source": client_ip.source().as_str(),
        }),
    )
    .await?;
    Err(AuthFailure::Forbidden)
}

#[cfg(test)]
mod tests {
    use super::guild_has_active_ip_ban_for_client;
    use crate::server::{
        auth::resolve_client_ip,
        core::{AppConfig, AppState, GuildIpBanRecord},
        directory_contract::IpNetwork,
    };
    use axum::http::HeaderMap;
    use filament_core::UserId;
    use ulid::Ulid;

    #[tokio::test]
    async fn guild_ip_ban_matching_handles_ipv4_and_ipv6_host_observations() {
        let state = AppState::new(&AppConfig::default()).expect("state initializes");
        let guild_id = String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV");
        let source_user_id = UserId::new();
        let now = crate::server::auth::now_unix();

        state.guild_ip_bans.write().await.insert(
            guild_id.clone(),
            vec![
                GuildIpBanRecord {
                    ban_id: Ulid::new().to_string(),
                    ip_network: IpNetwork::host("203.0.113.41".parse().expect("ipv4 parses")),
                    source_user_id: Some(source_user_id),
                    reason: String::from("ipv4 host"),
                    created_at_unix: now,
                    expires_at_unix: None,
                },
                GuildIpBanRecord {
                    ban_id: Ulid::new().to_string(),
                    ip_network: IpNetwork::host("2001:db8::42".parse().expect("ipv6 parses")),
                    source_user_id: Some(source_user_id),
                    reason: String::from("ipv6 host"),
                    created_at_unix: now,
                    expires_at_unix: None,
                },
            ],
        );

        let headers = HeaderMap::new();
        let ipv4_client = resolve_client_ip(
            &headers,
            Some("203.0.113.41".parse().expect("peer ip parses")),
            &[],
        );
        let ipv6_client = resolve_client_ip(
            &headers,
            Some("2001:db8::42".parse().expect("peer ip parses")),
            &[],
        );
        let other_client = resolve_client_ip(
            &headers,
            Some("198.51.100.91".parse().expect("peer ip parses")),
            &[],
        );

        assert!(
            guild_has_active_ip_ban_for_client(&state, &guild_id, ipv4_client)
                .await
                .expect("ipv4 check succeeds")
        );
        assert!(
            guild_has_active_ip_ban_for_client(&state, &guild_id, ipv6_client)
                .await
                .expect("ipv6 check succeeds")
        );
        assert!(
            !guild_has_active_ip_ban_for_client(&state, &guild_id, other_client)
                .await
                .expect("non-matching check succeeds")
        );
    }
}
