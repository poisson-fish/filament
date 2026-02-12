use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
};

use axum::{
    extract::{connect_info::ConnectInfo, Extension, Path, Query, State},
    http::HeaderMap,
    Json,
};
use filament_core::{
    apply_channel_overwrite, base_permissions, can_assign_role, can_moderate_member,
    has_permission, ChannelKind, ChannelName, ChannelPermissionOverwrite, GuildName, Permission,
    Role, UserId,
};
use sqlx::Row;
use ulid::Ulid;

use crate::server::{
    auth::{
        authenticate, enforce_directory_join_rate_limit, extract_client_ip, now_unix, ClientIp,
    },
    core::{AppState, ChannelRecord, GuildRecord, GuildVisibility},
    db::{
        channel_kind_from_i16, channel_kind_to_i16, ensure_db_schema, permission_set_from_i64,
        permission_set_from_list, permission_set_to_i64, role_from_i16, role_to_i16,
        visibility_from_i16, visibility_to_i16,
    },
    directory_contract::{DirectoryJoinOutcome, IpNetwork},
    domain::{member_role_in_guild, user_role_in_guild, write_audit_log},
    errors::AuthFailure,
    types::{
        ChannelListResponse, ChannelResponse, ChannelRolePath, CreateChannelRequest,
        CreateGuildRequest, DirectoryJoinOutcomeResponse, DirectoryJoinResponse, GuildListResponse,
        GuildPath, GuildResponse, MemberPath, ModerationResponse, PublicGuildListItem,
        PublicGuildListQuery, PublicGuildListResponse, UpdateChannelRoleOverrideRequest,
        UpdateMemberRoleRequest,
    },
};

pub(crate) async fn create_guild(
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

pub(crate) const MAX_GUILD_LIST_LIMIT: usize = 200;

pub(crate) async fn list_guilds(
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

pub(crate) const DEFAULT_PUBLIC_GUILD_LIST_LIMIT: usize = 20;
pub(crate) const MAX_PUBLIC_GUILD_LIST_LIMIT: usize = 50;
pub(crate) const MAX_PUBLIC_GUILD_QUERY_CHARS: usize = 64;
const DIRECTORY_JOIN_OBSERVATION_WRITE_MIN_SECS: i64 = 60;

const fn join_outcome_response(outcome: DirectoryJoinOutcome) -> DirectoryJoinOutcomeResponse {
    match outcome {
        DirectoryJoinOutcome::Accepted => DirectoryJoinOutcomeResponse::Accepted,
        DirectoryJoinOutcome::AlreadyMember => DirectoryJoinOutcomeResponse::AlreadyMember,
        DirectoryJoinOutcome::RejectedVisibility => {
            DirectoryJoinOutcomeResponse::RejectedVisibility
        }
        DirectoryJoinOutcome::RejectedUserBan => DirectoryJoinOutcomeResponse::RejectedUserBan,
        DirectoryJoinOutcome::RejectedIpBan => DirectoryJoinOutcomeResponse::RejectedIpBan,
    }
}

#[derive(Clone, Copy)]
struct DirectoryJoinPolicyInput {
    visibility: DirectoryJoinVisibilityStatus,
    user_ban: DirectoryJoinBanStatus,
    ip_ban: DirectoryJoinBanStatus,
    membership: DirectoryJoinMembershipStatus,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DirectoryJoinVisibilityStatus {
    Public,
    NonPublic,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DirectoryJoinBanStatus {
    Banned,
    NotBanned,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DirectoryJoinMembershipStatus {
    AlreadyMember,
    NotMember,
}

const fn classify_directory_join_outcome(input: DirectoryJoinPolicyInput) -> DirectoryJoinOutcome {
    if matches!(input.visibility, DirectoryJoinVisibilityStatus::NonPublic) {
        return DirectoryJoinOutcome::RejectedVisibility;
    }
    if matches!(input.user_ban, DirectoryJoinBanStatus::Banned) {
        return DirectoryJoinOutcome::RejectedUserBan;
    }
    if matches!(input.ip_ban, DirectoryJoinBanStatus::Banned) {
        return DirectoryJoinOutcome::RejectedIpBan;
    }
    if matches!(
        input.membership,
        DirectoryJoinMembershipStatus::AlreadyMember
    ) {
        return DirectoryJoinOutcome::AlreadyMember;
    }
    DirectoryJoinOutcome::Accepted
}

async fn maybe_record_join_ip_observation(
    state: &AppState,
    user_id: UserId,
    client_ip: ClientIp,
) -> Result<(), AuthFailure> {
    let Some(ip) = client_ip.ip() else {
        return Ok(());
    };
    let network = IpNetwork::host(ip);
    let canonical = network.canonical_cidr();
    let now = now_unix();
    let key = format!("{user_id}:{canonical}");

    {
        let mut write_guard = state.user_ip_observation_writes.write().await;
        if write_guard.get(&key).is_some_and(|last_write| {
            now.saturating_sub(*last_write) < DIRECTORY_JOIN_OBSERVATION_WRITE_MIN_SECS
        }) {
            return Ok(());
        }
        write_guard.insert(key, now);
    }

    if let Some(pool) = &state.db_pool {
        sqlx::query(
            "INSERT INTO user_ip_observations (observation_id, user_id, ip_cidr, first_seen_at_unix, last_seen_at_unix)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (user_id, ip_cidr)
             DO UPDATE SET last_seen_at_unix = EXCLUDED.last_seen_at_unix",
        )
        .bind(Ulid::new().to_string())
        .bind(user_id.to_string())
        .bind(canonical)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        return Ok(());
    }

    let mut observations = state.user_ip_observations.write().await;
    observations.insert((user_id, network), now);
    Ok(())
}

async fn guild_has_active_ip_ban(
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
    Ok(guild_bans.iter().any(|(network, expires_at_unix)| {
        expires_at_unix.is_none_or(|value| value > now) && network.contains(ip)
    }))
}

async fn write_directory_join_audit(
    state: &AppState,
    guild_id: &str,
    actor_user_id: UserId,
    outcome: DirectoryJoinOutcome,
    client_ip: ClientIp,
) -> Result<(), AuthFailure> {
    write_audit_log(
        state,
        Some(guild_id.to_owned()),
        actor_user_id,
        Some(actor_user_id),
        outcome.audit_action(),
        serde_json::json!({
            "outcome": join_outcome_response(outcome),
            "client_ip_source": client_ip.source().as_str(),
        }),
    )
    .await
}

pub(crate) async fn list_public_guilds(
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

fn join_failure_from_outcome(outcome: DirectoryJoinOutcome) -> Option<AuthFailure> {
    match outcome {
        DirectoryJoinOutcome::RejectedVisibility => Some(AuthFailure::NotFound),
        DirectoryJoinOutcome::RejectedUserBan => Some(AuthFailure::DirectoryJoinUserBanned),
        DirectoryJoinOutcome::RejectedIpBan => Some(AuthFailure::DirectoryJoinIpBanned),
        DirectoryJoinOutcome::Accepted | DirectoryJoinOutcome::AlreadyMember => None,
    }
}

async fn resolve_directory_join_outcome_db(
    state: &AppState,
    pool: &sqlx::PgPool,
    guild_id: &str,
    user_id: UserId,
    client_ip: ClientIp,
) -> Result<DirectoryJoinOutcome, AuthFailure> {
    let guild_row = sqlx::query("SELECT visibility FROM guilds WHERE guild_id = $1")
        .bind(guild_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
    let Some(guild_row) = guild_row else {
        return Err(AuthFailure::NotFound);
    };
    let visibility_raw: i16 = guild_row
        .try_get("visibility")
        .map_err(|_| AuthFailure::Internal)?;
    let visibility = visibility_from_i16(visibility_raw).ok_or(AuthFailure::Internal)?;
    let user_banned = sqlx::query("SELECT 1 FROM guild_bans WHERE guild_id = $1 AND user_id = $2")
        .bind(guild_id)
        .bind(user_id.to_string())
        .fetch_optional(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?
        .is_some();
    let ip_banned = guild_has_active_ip_ban(state, guild_id, client_ip).await?;
    let already_member =
        sqlx::query("SELECT 1 FROM guild_members WHERE guild_id = $1 AND user_id = $2")
            .bind(guild_id)
            .bind(user_id.to_string())
            .fetch_optional(pool)
            .await
            .map_err(|_| AuthFailure::Internal)?
            .is_some();
    let mut outcome = classify_directory_join_outcome(DirectoryJoinPolicyInput {
        visibility: if visibility == GuildVisibility::Public {
            DirectoryJoinVisibilityStatus::Public
        } else {
            DirectoryJoinVisibilityStatus::NonPublic
        },
        user_ban: if user_banned {
            DirectoryJoinBanStatus::Banned
        } else {
            DirectoryJoinBanStatus::NotBanned
        },
        ip_ban: if ip_banned {
            DirectoryJoinBanStatus::Banned
        } else {
            DirectoryJoinBanStatus::NotBanned
        },
        membership: if already_member {
            DirectoryJoinMembershipStatus::AlreadyMember
        } else {
            DirectoryJoinMembershipStatus::NotMember
        },
    });
    if outcome != DirectoryJoinOutcome::Accepted {
        return Ok(outcome);
    }

    let insert = sqlx::query(
        "INSERT INTO guild_members (guild_id, user_id, role)
         VALUES ($1, $2, $3)
         ON CONFLICT (guild_id, user_id) DO NOTHING",
    )
    .bind(guild_id)
    .bind(user_id.to_string())
    .bind(role_to_i16(Role::Member))
    .execute(pool)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    if insert.rows_affected() == 0 {
        outcome = DirectoryJoinOutcome::AlreadyMember;
    }
    Ok(outcome)
}

async fn resolve_directory_join_outcome_in_memory(
    state: &AppState,
    guild_id: &str,
    user_id: UserId,
    client_ip: ClientIp,
) -> Result<DirectoryJoinOutcome, AuthFailure> {
    let ip_banned = guild_has_active_ip_ban(state, guild_id, client_ip).await?;
    let (visibility, user_banned, already_member) = {
        let guilds = state.guilds.read().await;
        let guild = guilds.get(guild_id).ok_or(AuthFailure::NotFound)?;
        (
            guild.visibility,
            guild.banned_members.contains(&user_id),
            guild.members.contains_key(&user_id),
        )
    };

    let mut outcome = classify_directory_join_outcome(DirectoryJoinPolicyInput {
        visibility: if visibility == GuildVisibility::Public {
            DirectoryJoinVisibilityStatus::Public
        } else {
            DirectoryJoinVisibilityStatus::NonPublic
        },
        user_ban: if user_banned {
            DirectoryJoinBanStatus::Banned
        } else {
            DirectoryJoinBanStatus::NotBanned
        },
        ip_ban: if ip_banned {
            DirectoryJoinBanStatus::Banned
        } else {
            DirectoryJoinBanStatus::NotBanned
        },
        membership: if already_member {
            DirectoryJoinMembershipStatus::AlreadyMember
        } else {
            DirectoryJoinMembershipStatus::NotMember
        },
    });
    if outcome != DirectoryJoinOutcome::Accepted {
        return Ok(outcome);
    }

    let mut guilds = state.guilds.write().await;
    let guild = guilds.get_mut(guild_id).ok_or(AuthFailure::NotFound)?;
    if let std::collections::hash_map::Entry::Vacant(entry) = guild.members.entry(user_id) {
        entry.insert(Role::Member);
    } else {
        outcome = DirectoryJoinOutcome::AlreadyMember;
    }
    Ok(outcome)
}

pub(crate) async fn join_public_guild(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(path): Path<GuildPath>,
) -> Result<Json<DirectoryJoinResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    enforce_directory_join_rate_limit(&state, client_ip, auth.user_id).await?;
    maybe_record_join_ip_observation(&state, auth.user_id, client_ip).await?;

    let outcome = if let Some(pool) = &state.db_pool {
        resolve_directory_join_outcome_db(&state, pool, &path.guild_id, auth.user_id, client_ip)
            .await?
    } else {
        resolve_directory_join_outcome_in_memory(&state, &path.guild_id, auth.user_id, client_ip)
            .await?
    };

    write_directory_join_audit(&state, &path.guild_id, auth.user_id, outcome, client_ip).await?;
    if let Some(failure) = join_failure_from_outcome(outcome) {
        return Err(failure);
    }
    Ok(Json(DirectoryJoinResponse {
        guild_id: path.guild_id,
        outcome: join_outcome_response(outcome),
    }))
}

pub(crate) const MAX_CHANNEL_LIST_LIMIT: usize = 500;

pub(crate) async fn list_guild_channels(
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

pub(crate) async fn create_channel(
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

pub(crate) async fn add_member(
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

pub(crate) async fn update_member_role(
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

pub(crate) async fn set_channel_role_override(
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

pub(crate) async fn kick_member(
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

pub(crate) async fn ban_member(
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

#[cfg(test)]
mod tests {
    use super::{
        classify_directory_join_outcome, join_outcome_response, maybe_record_join_ip_observation,
        DirectoryJoinBanStatus, DirectoryJoinMembershipStatus, DirectoryJoinPolicyInput,
        DirectoryJoinVisibilityStatus,
    };
    use crate::server::{
        auth::resolve_client_ip,
        core::{AppConfig, AppState},
        directory_contract::DirectoryJoinOutcome,
        types::DirectoryJoinOutcomeResponse,
    };
    use axum::http::HeaderMap;
    use filament_core::UserId;

    #[test]
    fn directory_join_state_transition_precedence_is_stable() {
        assert_eq!(
            classify_directory_join_outcome(DirectoryJoinPolicyInput {
                visibility: DirectoryJoinVisibilityStatus::NonPublic,
                user_ban: DirectoryJoinBanStatus::NotBanned,
                ip_ban: DirectoryJoinBanStatus::NotBanned,
                membership: DirectoryJoinMembershipStatus::NotMember,
            }),
            DirectoryJoinOutcome::RejectedVisibility
        );
        assert_eq!(
            classify_directory_join_outcome(DirectoryJoinPolicyInput {
                visibility: DirectoryJoinVisibilityStatus::Public,
                user_ban: DirectoryJoinBanStatus::Banned,
                ip_ban: DirectoryJoinBanStatus::NotBanned,
                membership: DirectoryJoinMembershipStatus::NotMember,
            }),
            DirectoryJoinOutcome::RejectedUserBan
        );
        assert_eq!(
            classify_directory_join_outcome(DirectoryJoinPolicyInput {
                visibility: DirectoryJoinVisibilityStatus::Public,
                user_ban: DirectoryJoinBanStatus::NotBanned,
                ip_ban: DirectoryJoinBanStatus::Banned,
                membership: DirectoryJoinMembershipStatus::NotMember,
            }),
            DirectoryJoinOutcome::RejectedIpBan
        );
        assert_eq!(
            classify_directory_join_outcome(DirectoryJoinPolicyInput {
                visibility: DirectoryJoinVisibilityStatus::Public,
                user_ban: DirectoryJoinBanStatus::NotBanned,
                ip_ban: DirectoryJoinBanStatus::NotBanned,
                membership: DirectoryJoinMembershipStatus::AlreadyMember,
            }),
            DirectoryJoinOutcome::AlreadyMember
        );
        assert_eq!(
            classify_directory_join_outcome(DirectoryJoinPolicyInput {
                visibility: DirectoryJoinVisibilityStatus::Public,
                user_ban: DirectoryJoinBanStatus::NotBanned,
                ip_ban: DirectoryJoinBanStatus::NotBanned,
                membership: DirectoryJoinMembershipStatus::NotMember,
            }),
            DirectoryJoinOutcome::Accepted
        );
    }

    #[test]
    fn directory_join_outcome_response_mapping_is_typed() {
        assert_eq!(
            join_outcome_response(DirectoryJoinOutcome::Accepted),
            DirectoryJoinOutcomeResponse::Accepted
        );
        assert_eq!(
            join_outcome_response(DirectoryJoinOutcome::AlreadyMember),
            DirectoryJoinOutcomeResponse::AlreadyMember
        );
        assert_eq!(
            join_outcome_response(DirectoryJoinOutcome::RejectedVisibility),
            DirectoryJoinOutcomeResponse::RejectedVisibility
        );
        assert_eq!(
            join_outcome_response(DirectoryJoinOutcome::RejectedUserBan),
            DirectoryJoinOutcomeResponse::RejectedUserBan
        );
        assert_eq!(
            join_outcome_response(DirectoryJoinOutcome::RejectedIpBan),
            DirectoryJoinOutcomeResponse::RejectedIpBan
        );
    }

    #[tokio::test]
    async fn join_ip_observation_upsert_is_write_bounded_in_memory_mode() {
        let state = AppState::new(&AppConfig::default()).expect("state should initialize");
        let user_id = UserId::new();
        let headers = HeaderMap::new();
        let client_ip = resolve_client_ip(
            &headers,
            Some("203.0.113.44".parse().expect("valid ip")),
            &[],
        );

        maybe_record_join_ip_observation(&state, user_id, client_ip)
            .await
            .expect("first observation write should succeed");
        maybe_record_join_ip_observation(&state, user_id, client_ip)
            .await
            .expect("second observation write should be bounded");

        let observations = state.user_ip_observations.read().await;
        assert_eq!(observations.len(), 1);
        let last_seen = observations.values().next().expect("observation record");
        assert!(*last_seen > 0);
        let writes = state.user_ip_observation_writes.read().await;
        assert_eq!(writes.len(), 1);
    }
}
