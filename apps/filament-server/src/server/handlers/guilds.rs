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
    can_assign_role_legacy, can_moderate_member_legacy, has_permission_legacy, ChannelKind,
    ChannelName, ChannelPermissionOverwrite, GuildName, Permission, Role, UserId,
};
use sqlx::Row;
use ulid::Ulid;

use crate::server::{
    auth::{
        authenticate, enforce_directory_join_rate_limit, extract_client_ip, now_unix, ClientIp,
    },
    core::{AppState, ChannelRecord, GuildRecord, GuildVisibility},
    db::{
        channel_kind_from_i16, channel_kind_to_i16, permission_set_from_list,
        permission_set_to_i64, role_to_i16, seed_hierarchical_permissions_for_new_guild,
        visibility_from_i16, visibility_to_i16,
    },
    directory_contract::{
        validate_workspace_role_name, AuditListQuery, AuditListQueryDto, DirectoryContractError,
        DirectoryJoinOutcome, GuildIpBanByUserRequest, GuildIpBanByUserRequestDto, GuildIpBanId,
        GuildIpBanListQuery, GuildIpBanListQueryDto, GuildMemberListQuery, GuildMemberListQueryDto,
        IpNetwork, WorkspaceRoleId,
    },
    domain::{
        enforce_guild_ip_ban_for_request, guild_has_active_ip_ban_for_client,
        guild_permission_snapshot, member_role_in_guild, user_role_in_guild, write_audit_log,
    },
    errors::AuthFailure,
    gateway_events,
    permissions::{
        DEFAULT_ROLE_MEMBER, DEFAULT_ROLE_MODERATOR, MAX_GUILD_ROLES, MAX_MEMBER_ROLE_ASSIGNMENTS,
        MAX_ROLE_NAME_CHARS, SYSTEM_ROLE_EVERYONE, SYSTEM_ROLE_WORKSPACE_OWNER,
    },
    realtime::broadcast_guild_event,
    types::{
        ChannelListResponse, ChannelPermissionOverridePath, ChannelResponse, ChannelRolePath,
        CreateChannelRequest, CreateGuildRequest, CreateGuildRoleRequest,
        DirectoryJoinOutcomeResponse, DirectoryJoinResponse, GuildAuditEventResponse,
        GuildAuditListResponse, GuildIpBanApplyResponse, GuildIpBanListResponse, GuildIpBanPath,
        GuildIpBanRecordResponse, GuildListResponse, GuildMemberListResponse,
        GuildMemberRecordResponse, GuildPath, GuildResponse, GuildRoleListResponse,
        GuildRoleMemberPath, GuildRolePath, GuildRoleResponse, MemberPath, ModerationResponse,
        PublicGuildListItem, PublicGuildListQuery, PublicGuildListResponse,
        ReorderGuildRolesRequest, UpdateChannelPermissionOverrideRequest,
        UpdateChannelRoleOverrideRequest, UpdateGuildDefaultJoinRoleRequest, UpdateGuildRequest,
        UpdateGuildRoleRequest, UpdateMemberRoleRequest,
    },
};

pub(crate) async fn create_guild(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateGuildRequest>,
) -> Result<Json<GuildResponse>, AuthFailure> {
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
            "INSERT INTO guilds
                (guild_id, name, visibility, created_by_user_id, default_join_role_id, created_at_unix)
             VALUES ($1, $2, $3, $4, NULL, $5)",
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
        seed_hierarchical_permissions_for_new_guild(&mut tx, &guild_id, &creator_user_id)
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

    let mut guilds = state.membership_store.guilds().write().await;
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
            default_join_role_id: None,
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

    let guilds = state.membership_store.guilds().read().await;
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

pub(crate) async fn update_guild(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<GuildPath>,
    Json(payload): Json<UpdateGuildRequest>,
) -> Result<Json<GuildResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;
    let (_, permissions) = guild_permission_snapshot(&state, auth.user_id, &path.guild_id).await?;
    if !permissions.contains(Permission::ManageRoles) {
        return Err(AuthFailure::Forbidden);
    }

    let name = payload
        .name
        .map(|raw| GuildName::try_from(raw).map(|value| value.as_str().to_owned()))
        .transpose()
        .map_err(|_| AuthFailure::InvalidRequest)?;
    let visibility = payload.visibility;
    if name.is_none() && visibility.is_none() {
        return Err(AuthFailure::InvalidRequest);
    }

    let mut changed_name: Option<String> = None;
    let mut changed_visibility: Option<GuildVisibility> = None;
    let updated_at_unix = now_unix();
    let response = if let Some(pool) = &state.db_pool {
        let current = sqlx::query("SELECT name, visibility FROM guilds WHERE guild_id = $1")
            .bind(&path.guild_id)
            .fetch_optional(pool)
            .await
            .map_err(|_| AuthFailure::Internal)?
            .ok_or(AuthFailure::NotFound)?;

        let current_name: String = current.try_get("name").map_err(|_| AuthFailure::Internal)?;
        let current_visibility_raw: i16 = current
            .try_get("visibility")
            .map_err(|_| AuthFailure::Internal)?;
        let current_visibility =
            visibility_from_i16(current_visibility_raw).ok_or(AuthFailure::Internal)?;

        let next_name = name.clone().unwrap_or(current_name.clone());
        let next_visibility = visibility.unwrap_or(current_visibility);
        if next_name != current_name {
            changed_name = Some(next_name.clone());
        }
        if next_visibility != current_visibility {
            changed_visibility = Some(next_visibility);
        }

        let update = sqlx::query(
            "UPDATE guilds
             SET name = $2, visibility = $3
             WHERE guild_id = $1",
        )
        .bind(&path.guild_id)
        .bind(&next_name)
        .bind(visibility_to_i16(next_visibility))
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        if update.rows_affected() == 0 {
            return Err(AuthFailure::NotFound);
        }

        GuildResponse {
            guild_id: path.guild_id.clone(),
            name: next_name,
            visibility: next_visibility,
        }
    } else {
        let mut guilds = state.membership_store.guilds().write().await;
        let guild = guilds
            .get_mut(&path.guild_id)
            .ok_or(AuthFailure::NotFound)?;

        if let Some(next_name) = name {
            if next_name != guild.name {
                changed_name = Some(next_name.clone());
                guild.name = next_name;
            }
        }
        if let Some(next_visibility) = visibility {
            if next_visibility != guild.visibility {
                changed_visibility = Some(next_visibility);
                guild.visibility = next_visibility;
            }
        }

        GuildResponse {
            guild_id: path.guild_id.clone(),
            name: guild.name.clone(),
            visibility: guild.visibility,
        }
    };

    if changed_name.is_some() || changed_visibility.is_some() {
        let event = gateway_events::workspace_update(
            &path.guild_id,
            changed_name.as_deref(),
            changed_visibility,
            updated_at_unix,
            Some(auth.user_id),
        );
        broadcast_guild_event(&state, &path.guild_id, &event).await;
    }

    Ok(Json(response))
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
    guild_has_active_ip_ban_for_client(state, guild_id, client_ip).await
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

#[derive(Debug, Clone)]
struct GuildAuditEventRecord {
    audit_id: String,
    actor_user_id: String,
    target_user_id: Option<String>,
    action: String,
    created_at_unix: i64,
}

#[derive(Debug, Clone)]
struct AuditCursorPosition {
    created_at_unix: i64,
    audit_id: String,
}

#[derive(Debug, Clone)]
struct GuildMemberRecord {
    user_id: String,
    role_ids: Vec<String>,
}

const fn map_directory_contract_error_to_auth_failure(
    _error: DirectoryContractError,
) -> AuthFailure {
    AuthFailure::InvalidRequest
}

fn parse_audit_cursor_position(cursor: &str) -> Result<AuditCursorPosition, AuthFailure> {
    let (created_at_raw, audit_id_raw) =
        cursor.split_once('_').ok_or(AuthFailure::InvalidRequest)?;
    let created_at_unix = created_at_raw
        .parse::<i64>()
        .map_err(|_| AuthFailure::InvalidRequest)?;
    if Ulid::from_string(audit_id_raw).is_err() {
        return Err(AuthFailure::InvalidRequest);
    }
    Ok(AuditCursorPosition {
        created_at_unix,
        audit_id: audit_id_raw.to_owned(),
    })
}

fn build_audit_cursor(created_at_unix: i64, audit_id: &str) -> String {
    format!("{created_at_unix}_{audit_id}")
}

fn action_has_ip_ban_match(action: &str) -> bool {
    action == "directory.join.rejected.ip_ban" || action == "moderation.ip_ban.hit"
}

fn guild_audit_response_from_record(record: GuildAuditEventRecord) -> GuildAuditEventResponse {
    GuildAuditEventResponse {
        audit_id: record.audit_id,
        actor_user_id: record.actor_user_id,
        target_user_id: record.target_user_id,
        ip_ban_match: action_has_ip_ban_match(&record.action),
        action: record.action,
        created_at_unix: record.created_at_unix,
    }
}

fn guild_member_response_from_record(record: GuildMemberRecord) -> GuildMemberRecordResponse {
    GuildMemberRecordResponse {
        user_id: record.user_id,
        role_ids: record.role_ids,
    }
}

async fn enforce_audit_access(
    state: &AppState,
    guild_id: &str,
    user_id: UserId,
) -> Result<(), AuthFailure> {
    if let Some(pool) = &state.db_pool {
        let guild_exists = sqlx::query("SELECT 1 FROM guilds WHERE guild_id = $1")
            .bind(guild_id)
            .fetch_optional(pool)
            .await
            .map_err(|_| AuthFailure::Internal)?
            .is_some();
        if !guild_exists {
            return Err(AuthFailure::NotFound);
        }
    } else if !state
        .membership_store
        .guilds()
        .read()
        .await
        .contains_key(guild_id)
    {
        return Err(AuthFailure::NotFound);
    }

    let (_, permissions) = match guild_permission_snapshot(state, user_id, guild_id).await {
        Ok(value) => value,
        Err(AuthFailure::Forbidden) => return Err(AuthFailure::AuditAccessDenied),
        Err(AuthFailure::NotFound) => return Err(AuthFailure::NotFound),
        Err(error) => return Err(error),
    };
    if !permissions.contains(Permission::ViewAuditLog) {
        return Err(AuthFailure::AuditAccessDenied);
    }
    Ok(())
}

async fn enforce_guild_ip_ban_moderation_access(
    state: &AppState,
    guild_id: &str,
    user_id: UserId,
) -> Result<(), AuthFailure> {
    if let Some(pool) = &state.db_pool {
        let guild_exists = sqlx::query("SELECT 1 FROM guilds WHERE guild_id = $1")
            .bind(guild_id)
            .fetch_optional(pool)
            .await
            .map_err(|_| AuthFailure::Internal)?
            .is_some();
        if !guild_exists {
            return Err(AuthFailure::NotFound);
        }
    } else if !state
        .membership_store
        .guilds()
        .read()
        .await
        .contains_key(guild_id)
    {
        return Err(AuthFailure::NotFound);
    }

    let (_, permissions) = match guild_permission_snapshot(state, user_id, guild_id).await {
        Ok(value) => value,
        Err(AuthFailure::Forbidden) => return Err(AuthFailure::Forbidden),
        Err(AuthFailure::NotFound) => return Err(AuthFailure::NotFound),
        Err(error) => return Err(error),
    };
    if permissions.contains(Permission::ManageIpBans) {
        Ok(())
    } else {
        Err(AuthFailure::Forbidden)
    }
}

#[derive(Debug, Clone)]
struct GuildRoleModel {
    role_id: String,
    name: String,
    position: i32,
    is_system: bool,
    system_key: Option<String>,
    permissions: filament_core::PermissionSet,
    color_hex: Option<String>,
    created_at_unix: i64,
}

#[derive(Debug, Clone, Copy)]
struct RoleManageContext {
    is_server_owner: bool,
    is_workspace_owner: bool,
    highest_position: i32,
}

fn role_model_to_response(model: GuildRoleModel) -> GuildRoleResponse {
    GuildRoleResponse {
        role_id: model.role_id,
        name: model.name,
        position: model.position,
        is_system: model.is_system,
        permissions: crate::server::db::permission_list_from_set(model.permissions),
        color_hex: model.color_hex,
    }
}

fn parse_role_id(value: String) -> Result<String, AuthFailure> {
    WorkspaceRoleId::try_from(value)
        .map(|id| id.to_string())
        .map_err(map_directory_contract_error_to_auth_failure)
}

fn validate_role_name_input(value: &str) -> Result<String, AuthFailure> {
    validate_workspace_role_name(value)
        .map_err(map_directory_contract_error_to_auth_failure)
        .and_then(|name| {
            if name == "@everyone" || name.eq_ignore_ascii_case(SYSTEM_ROLE_WORKSPACE_OWNER) {
                return Err(AuthFailure::InvalidRequest);
            }
            if name.len() > MAX_ROLE_NAME_CHARS {
                return Err(AuthFailure::InvalidRequest);
            }
            Ok(name)
        })
}

fn validate_role_color_hex_input(value: &str) -> Result<String, AuthFailure> {
    let normalized = value.trim();
    if normalized.len() != 7 || !normalized.starts_with('#') {
        return Err(AuthFailure::InvalidRequest);
    }

    let mut output = String::with_capacity(7);
    output.push('#');
    for ch in normalized.chars().skip(1) {
        if !ch.is_ascii_hexdigit() {
            return Err(AuthFailure::InvalidRequest);
        }
        output.push(ch.to_ascii_uppercase());
    }

    Ok(output)
}

fn can_manage_role(model: &GuildRoleModel, context: RoleManageContext) -> bool {
    if context.is_server_owner {
        return true;
    }
    if model.system_key.as_deref() == Some(SYSTEM_ROLE_WORKSPACE_OWNER)
        || model.system_key.as_deref() == Some(SYSTEM_ROLE_EVERYONE)
    {
        return false;
    }
    if context.is_workspace_owner {
        return true;
    }
    model.position < context.highest_position
}

async fn guild_roles_db(
    pool: &sqlx::PgPool,
    guild_id: &str,
) -> Result<Vec<GuildRoleModel>, AuthFailure> {
    let rows = sqlx::query(
        "SELECT role_id, name, position, is_system, system_key, permissions_allow_mask, color_hex, created_at_unix
         FROM guild_roles
         WHERE guild_id = $1
         ORDER BY position DESC, created_at_unix ASC, role_id ASC",
    )
    .bind(guild_id)
    .fetch_all(pool)
    .await
    .map_err(|_| AuthFailure::Internal)?;

    let mut roles = Vec::with_capacity(rows.len());
    for row in rows {
        let mask: i64 = row
            .try_get("permissions_allow_mask")
            .map_err(|_| AuthFailure::Internal)?;
        let bits = u64::try_from(mask).map_err(|_| AuthFailure::Internal)?;
        let (permissions, _) = crate::server::permissions::mask_permissions(bits);
        roles.push(GuildRoleModel {
            role_id: row.try_get("role_id").map_err(|_| AuthFailure::Internal)?,
            name: row.try_get("name").map_err(|_| AuthFailure::Internal)?,
            position: row.try_get("position").map_err(|_| AuthFailure::Internal)?,
            is_system: row
                .try_get("is_system")
                .map_err(|_| AuthFailure::Internal)?,
            system_key: row
                .try_get("system_key")
                .map_err(|_| AuthFailure::Internal)?,
            permissions,
            color_hex: row
                .try_get("color_hex")
                .map_err(|_| AuthFailure::Internal)?,
            created_at_unix: row
                .try_get("created_at_unix")
                .map_err(|_| AuthFailure::Internal)?,
        });
    }
    Ok(roles)
}

async fn guild_roles_in_memory(
    state: &AppState,
    guild_id: &str,
) -> Result<Vec<GuildRoleModel>, AuthFailure> {
    let role_maps = state.membership_store.guild_roles().read().await;
    let roles = role_maps.get(guild_id).ok_or(AuthFailure::NotFound)?;
    let mut list = roles
        .values()
        .map(|role| GuildRoleModel {
            role_id: role.role_id.clone(),
            name: role.name.clone(),
            position: role.position,
            is_system: role.is_system,
            system_key: role.system_key.clone(),
            permissions: role.permissions_allow,
            color_hex: role.color_hex.clone(),
            created_at_unix: role.created_at_unix,
        })
        .collect::<Vec<_>>();
    list.sort_by(|left, right| {
        right
            .position
            .cmp(&left.position)
            .then_with(|| left.created_at_unix.cmp(&right.created_at_unix))
            .then_with(|| left.role_id.cmp(&right.role_id))
    });
    Ok(list)
}

async fn load_actor_role_context(
    state: &AppState,
    guild_id: &str,
    user_id: UserId,
    required_permission: Permission,
) -> Result<RoleManageContext, AuthFailure> {
    let is_server_owner = state
        .runtime
        .server_owner_user_id
        .is_some_and(|owner| owner == user_id);
    let (_, permissions) = guild_permission_snapshot(state, user_id, guild_id).await?;
    if !permissions.contains(required_permission) && !is_server_owner {
        return Err(AuthFailure::Forbidden);
    }

    let roles = if let Some(pool) = &state.db_pool {
        guild_roles_db(pool, guild_id).await?
    } else {
        guild_roles_in_memory(state, guild_id).await?
    };

    if is_server_owner {
        return Ok(RoleManageContext {
            is_server_owner: true,
            is_workspace_owner: true,
            highest_position: i32::MAX,
        });
    }

    let mut assigned_role_ids = if let Some(pool) = &state.db_pool {
        let rows = sqlx::query(
            "SELECT role_id
             FROM guild_role_members
             WHERE guild_id = $1 AND user_id = $2",
        )
        .bind(guild_id)
        .bind(user_id.to_string())
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        rows.into_iter()
            .filter_map(|row| row.try_get::<String, _>("role_id").ok())
            .collect::<HashSet<_>>()
    } else {
        let assignments = state.membership_store.guild_role_assignments().read().await;
        assignments
            .get(guild_id)
            .and_then(|map| map.get(&user_id))
            .cloned()
            .unwrap_or_default()
    };

    let mut highest_position = i32::MIN;
    let mut is_workspace_owner = false;
    let workspace_owner_role_id = roles
        .iter()
        .find(|role| role.system_key.as_deref() == Some(SYSTEM_ROLE_WORKSPACE_OWNER))
        .map(|role| role.role_id.clone());
    let moderator_role_id = roles
        .iter()
        .find(|role| {
            role.system_key.as_deref() == Some(DEFAULT_ROLE_MODERATOR)
                || role.name.eq_ignore_ascii_case(DEFAULT_ROLE_MODERATOR)
        })
        .map(|role| role.role_id.clone());
    if let Ok(legacy_role) = user_role_in_guild(state, user_id, guild_id).await {
        match legacy_role {
            Role::Owner => {
                if let Some(role_id) = &workspace_owner_role_id {
                    assigned_role_ids.insert(role_id.clone());
                }
            }
            Role::Moderator => {
                if let Some(role_id) = &moderator_role_id {
                    assigned_role_ids.insert(role_id.clone());
                }
            }
            Role::Member => {}
        }
    }

    for role in &roles {
        if assigned_role_ids.contains(&role.role_id) {
            highest_position = highest_position.max(role.position);
            if role.system_key.as_deref() == Some(SYSTEM_ROLE_WORKSPACE_OWNER) {
                is_workspace_owner = true;
            }
        }
    }
    Ok(RoleManageContext {
        is_server_owner: false,
        is_workspace_owner,
        highest_position,
    })
}

async fn sync_legacy_role_from_assignments_db(
    pool: &sqlx::PgPool,
    guild_id: &str,
    user_id: UserId,
) -> Result<Role, AuthFailure> {
    let rows = sqlx::query(
        "SELECT gr.system_key
         FROM guild_role_members grm
         JOIN guild_roles gr ON gr.role_id = grm.role_id
         WHERE grm.guild_id = $1 AND grm.user_id = $2",
    )
    .bind(guild_id)
    .bind(user_id.to_string())
    .fetch_all(pool)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let mut has_workspace_owner = false;
    let mut has_moderator = false;
    for row in rows {
        let system_key = row
            .try_get::<Option<String>, _>("system_key")
            .ok()
            .flatten();
        if system_key.as_deref() == Some(SYSTEM_ROLE_WORKSPACE_OWNER) {
            has_workspace_owner = true;
        } else if system_key.as_deref() == Some("moderator") {
            has_moderator = true;
        }
    }
    let legacy = if has_workspace_owner {
        Role::Owner
    } else if has_moderator {
        Role::Moderator
    } else {
        Role::Member
    };
    sqlx::query(
        "UPDATE guild_members
         SET role = $3
         WHERE guild_id = $1 AND user_id = $2",
    )
    .bind(guild_id)
    .bind(user_id.to_string())
    .bind(role_to_i16(legacy))
    .execute(pool)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    Ok(legacy)
}

async fn sync_legacy_role_from_assignments_in_memory(
    state: &AppState,
    guild_id: &str,
    user_id: UserId,
) -> Result<Role, AuthFailure> {
    let role_ids = {
        let roles = state.membership_store.guild_roles().read().await;
        let roles = roles.get(guild_id).ok_or(AuthFailure::NotFound)?;
        let mut workspace_owner = None;
        let mut moderator = None;
        for role in roles.values() {
            if role.system_key.as_deref() == Some(SYSTEM_ROLE_WORKSPACE_OWNER) {
                workspace_owner = Some(role.role_id.clone());
            }
            if role.system_key.as_deref() == Some("moderator")
                || role.name.eq_ignore_ascii_case("moderator")
            {
                moderator = Some(role.role_id.clone());
            }
        }
        (
            workspace_owner.ok_or(AuthFailure::Internal)?,
            moderator.ok_or(AuthFailure::Internal)?,
        )
    };

    let assignments = state.membership_store.guild_role_assignments().read().await;
    let assigned = assignments
        .get(guild_id)
        .and_then(|map| map.get(&user_id))
        .cloned()
        .unwrap_or_default();
    drop(assignments);

    let legacy = if assigned.contains(&role_ids.0) {
        Role::Owner
    } else if assigned.contains(&role_ids.1) {
        Role::Moderator
    } else {
        Role::Member
    };

    let mut guilds = state.membership_store.guilds().write().await;
    let guild = guilds.get_mut(guild_id).ok_or(AuthFailure::NotFound)?;
    if let Some(role) = guild.members.get_mut(&user_id) {
        *role = legacy;
    }
    Ok(legacy)
}

fn parse_in_memory_audit_event(
    value: &serde_json::Value,
    guild_id: &str,
) -> Option<GuildAuditEventRecord> {
    let object = value.as_object()?;
    if object.get("guild_id")?.as_str()? != guild_id {
        return None;
    }
    let audit_id = object.get("audit_id")?.as_str()?.to_owned();
    if Ulid::from_string(&audit_id).is_err() {
        return None;
    }

    let actor_user_id = object.get("actor_user_id")?.as_str()?.to_owned();
    let target_user_id = object
        .get("target_user_id")
        .and_then(|raw| raw.as_str().map(str::to_owned));
    let action = object.get("action")?.as_str()?.to_owned();
    let created_at_unix = object.get("created_at_unix")?.as_i64()?;

    Some(GuildAuditEventRecord {
        audit_id,
        actor_user_id,
        target_user_id,
        action,
        created_at_unix,
    })
}

async fn list_guild_audit_db(
    pool: &sqlx::PgPool,
    guild_id: &str,
    query: &AuditListQuery,
) -> Result<GuildAuditListResponse, AuthFailure> {
    let cursor = query
        .cursor
        .as_ref()
        .map(|value| parse_audit_cursor_position(value.as_str()))
        .transpose()?;
    let action_pattern = query
        .action_prefix
        .as_ref()
        .map(|prefix| format!("{prefix}%"));
    let limit_plus_one = query
        .limit
        .checked_add(1)
        .ok_or(AuthFailure::InvalidRequest)?;

    let rows = sqlx::query(
        "SELECT audit_id, actor_user_id, target_user_id, action, created_at_unix
         FROM audit_logs
         WHERE guild_id = $1
           AND ($2::text IS NULL OR action LIKE $2)
           AND (
                $3::bigint IS NULL
                OR created_at_unix < $3
                OR (created_at_unix = $3 AND audit_id < $4)
           )
         ORDER BY created_at_unix DESC, audit_id DESC
         LIMIT $5",
    )
    .bind(guild_id)
    .bind(action_pattern)
    .bind(cursor.as_ref().map(|value| value.created_at_unix))
    .bind(cursor.as_ref().map(|value| value.audit_id.as_str()))
    .bind(i64::try_from(limit_plus_one).map_err(|_| AuthFailure::Internal)?)
    .fetch_all(pool)
    .await
    .map_err(|_| AuthFailure::Internal)?;

    let mut records = Vec::with_capacity(rows.len());
    for row in rows {
        records.push(GuildAuditEventRecord {
            audit_id: row.try_get("audit_id").map_err(|_| AuthFailure::Internal)?,
            actor_user_id: row
                .try_get("actor_user_id")
                .map_err(|_| AuthFailure::Internal)?,
            target_user_id: row
                .try_get::<Option<String>, _>("target_user_id")
                .map_err(|_| AuthFailure::Internal)?,
            action: row.try_get("action").map_err(|_| AuthFailure::Internal)?,
            created_at_unix: row
                .try_get("created_at_unix")
                .map_err(|_| AuthFailure::Internal)?,
        });
    }

    let next_cursor = if records.len() > query.limit {
        let cursor_record = records
            .get(query.limit)
            .cloned()
            .ok_or(AuthFailure::Internal)?;
        records.truncate(query.limit);
        Some(build_audit_cursor(
            cursor_record.created_at_unix,
            &cursor_record.audit_id,
        ))
    } else {
        None
    };

    Ok(GuildAuditListResponse {
        events: records
            .into_iter()
            .map(guild_audit_response_from_record)
            .collect(),
        next_cursor,
    })
}

async fn list_guild_audit_in_memory(
    state: &AppState,
    guild_id: &str,
    query: &AuditListQuery,
) -> Result<GuildAuditListResponse, AuthFailure> {
    let cursor = query
        .cursor
        .as_ref()
        .map(|value| parse_audit_cursor_position(value.as_str()))
        .transpose()?;
    let logs = state.audit_logs.read().await;

    let mut records = logs
        .iter()
        .filter_map(|entry| parse_in_memory_audit_event(entry, guild_id))
        .filter(|entry| {
            query
                .action_prefix
                .as_ref()
                .is_none_or(|prefix| entry.action.starts_with(prefix))
        })
        .filter(|entry| {
            cursor.as_ref().is_none_or(|value| {
                entry.created_at_unix < value.created_at_unix
                    || (entry.created_at_unix == value.created_at_unix
                        && entry.audit_id < value.audit_id)
            })
        })
        .collect::<Vec<_>>();

    records.sort_by(|left, right| {
        right
            .created_at_unix
            .cmp(&left.created_at_unix)
            .then_with(|| right.audit_id.cmp(&left.audit_id))
    });

    let next_cursor = if records.len() > query.limit {
        let cursor_record = records
            .get(query.limit)
            .cloned()
            .ok_or(AuthFailure::Internal)?;
        records.truncate(query.limit);
        Some(build_audit_cursor(
            cursor_record.created_at_unix,
            &cursor_record.audit_id,
        ))
    } else {
        None
    };

    Ok(GuildAuditListResponse {
        events: records
            .into_iter()
            .map(guild_audit_response_from_record)
            .collect(),
        next_cursor,
    })
}

pub(crate) async fn list_guild_audit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<GuildPath>,
    Query(query): Query<AuditListQueryDto>,
) -> Result<Json<GuildAuditListResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;
    let audit_query =
        AuditListQuery::try_from_with_limit_max(query, state.runtime.audit_list_limit_max)
            .map_err(map_directory_contract_error_to_auth_failure)?;
    enforce_audit_access(&state, &path.guild_id, auth.user_id).await?;

    let response = if let Some(pool) = &state.db_pool {
        list_guild_audit_db(pool, &path.guild_id, &audit_query).await?
    } else {
        list_guild_audit_in_memory(&state, &path.guild_id, &audit_query).await?
    };

    Ok(Json(response))
}

async fn list_guild_members_db(
    pool: &sqlx::PgPool,
    guild_id: &str,
    query: &GuildMemberListQuery,
) -> Result<GuildMemberListResponse, AuthFailure> {
    let cursor = query.cursor.as_ref().map(ToString::to_string);
    let limit_plus_one = query
        .limit
        .checked_add(1)
        .ok_or(AuthFailure::InvalidRequest)?;

    let rows = sqlx::query(
        "SELECT user_id
         FROM guild_members
         WHERE guild_id = $1
           AND ($2::text IS NULL OR user_id > $2)
         ORDER BY user_id ASC
         LIMIT $3",
    )
    .bind(guild_id)
    .bind(cursor)
    .bind(i64::try_from(limit_plus_one).map_err(|_| AuthFailure::Internal)?)
    .fetch_all(pool)
    .await
    .map_err(|_| AuthFailure::Internal)?;

    let mut user_ids = Vec::with_capacity(rows.len());
    for row in rows {
        user_ids.push(row.try_get("user_id").map_err(|_| AuthFailure::Internal)?);
    }

    let next_cursor = if user_ids.len() > query.limit {
        let cursor = user_ids
            .get(query.limit)
            .cloned()
            .ok_or(AuthFailure::Internal)?;
        user_ids.truncate(query.limit);
        Some(cursor)
    } else {
        None
    };

    let mut role_ids_by_user: HashMap<String, Vec<String>> = HashMap::new();
    if !user_ids.is_empty() {
        let rows = sqlx::query(
            "SELECT user_id, role_id
             FROM guild_role_members
             WHERE guild_id = $1 AND user_id = ANY($2)",
        )
        .bind(guild_id)
        .bind(&user_ids)
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;

        for row in rows {
            let user_id: String = row.try_get("user_id").map_err(|_| AuthFailure::Internal)?;
            let role_id: String = row.try_get("role_id").map_err(|_| AuthFailure::Internal)?;
            role_ids_by_user.entry(user_id).or_default().push(role_id);
        }
    }

    for role_ids in role_ids_by_user.values_mut() {
        role_ids.sort();
        role_ids.dedup();
        if role_ids.len() > MAX_MEMBER_ROLE_ASSIGNMENTS {
            role_ids.truncate(MAX_MEMBER_ROLE_ASSIGNMENTS);
        }
    }

    Ok(GuildMemberListResponse {
        members: user_ids
            .into_iter()
            .map(|user_id| GuildMemberRecord {
                role_ids: role_ids_by_user.remove(&user_id).unwrap_or_default(),
                user_id,
            })
            .map(guild_member_response_from_record)
            .collect(),
        next_cursor,
    })
}

async fn list_guild_members_in_memory(
    state: &AppState,
    guild_id: &str,
    query: &GuildMemberListQuery,
) -> Result<GuildMemberListResponse, AuthFailure> {
    let cursor = query.cursor.as_ref().map(ToString::to_string);
    let guilds = state.membership_store.guilds().read().await;
    let guild = guilds.get(guild_id).ok_or(AuthFailure::NotFound)?;
    let mut user_ids = guild
        .members
        .keys()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    drop(guilds);

    user_ids.sort();
    if let Some(cursor) = cursor.as_ref() {
        user_ids.retain(|user_id| user_id > cursor);
    }

    let next_cursor = if user_ids.len() > query.limit {
        let cursor = user_ids
            .get(query.limit)
            .cloned()
            .ok_or(AuthFailure::Internal)?;
        user_ids.truncate(query.limit);
        Some(cursor)
    } else {
        None
    };

    let assignments = state.membership_store.guild_role_assignments().read().await;
    let guild_assignments = assignments.get(guild_id);
    let mut role_ids_by_user: HashMap<String, Vec<String>> = HashMap::new();
    if let Some(guild_assignments) = guild_assignments {
        for (user_id, role_ids) in guild_assignments {
            let mut roles = role_ids.iter().cloned().collect::<Vec<_>>();
            roles.sort();
            roles.dedup();
            if roles.len() > MAX_MEMBER_ROLE_ASSIGNMENTS {
                roles.truncate(MAX_MEMBER_ROLE_ASSIGNMENTS);
            }
            role_ids_by_user.insert(user_id.to_string(), roles);
        }
    }

    Ok(GuildMemberListResponse {
        members: user_ids
            .into_iter()
            .map(|user_id| GuildMemberRecord {
                role_ids: role_ids_by_user.remove(&user_id).unwrap_or_default(),
                user_id,
            })
            .map(guild_member_response_from_record)
            .collect(),
        next_cursor,
    })
}

pub(crate) async fn list_guild_members(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<GuildPath>,
    Query(query): Query<GuildMemberListQueryDto>,
) -> Result<Json<GuildMemberListResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;
    let member_query = GuildMemberListQuery::try_from(query)
        .map_err(map_directory_contract_error_to_auth_failure)?;
    let (_, permissions) = guild_permission_snapshot(&state, auth.user_id, &path.guild_id).await?;
    if !permissions.contains(Permission::ManageMemberRoles)
        && !permissions.contains(Permission::ManageWorkspaceRoles)
        && !permissions.contains(Permission::ManageRoles)
    {
        return Err(AuthFailure::Forbidden);
    }

    let response = if let Some(pool) = &state.db_pool {
        list_guild_members_db(pool, &path.guild_id, &member_query).await?
    } else {
        list_guild_members_in_memory(&state, &path.guild_id, &member_query).await?
    };

    Ok(Json(response))
}

fn parse_role_id_list(role_ids: &[String]) -> Result<Vec<String>, AuthFailure> {
    if role_ids.is_empty() {
        return Err(AuthFailure::InvalidRequest);
    }
    let mut parsed = Vec::with_capacity(role_ids.len());
    let mut seen = HashSet::new();
    for role_id in role_ids {
        let parsed_id = parse_role_id(role_id.clone())?;
        if !seen.insert(parsed_id.clone()) {
            return Err(AuthFailure::InvalidRequest);
        }
        parsed.push(parsed_id);
    }
    Ok(parsed)
}

async fn guild_default_join_role_id_db(
    pool: &sqlx::PgPool,
    guild_id: &str,
) -> Result<Option<String>, AuthFailure> {
    sqlx::query_scalar::<_, Option<String>>(
        "SELECT default_join_role_id
         FROM guilds
         WHERE guild_id = $1",
    )
    .bind(guild_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| AuthFailure::Internal)?
    .ok_or(AuthFailure::NotFound)
}

async fn guild_default_join_role_id_in_memory(
    state: &AppState,
    guild_id: &str,
) -> Result<Option<String>, AuthFailure> {
    let guilds = state.membership_store.guilds().read().await;
    let guild = guilds.get(guild_id).ok_or(AuthFailure::NotFound)?;
    Ok(guild.default_join_role_id.clone())
}

pub(crate) async fn list_guild_roles(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<GuildPath>,
) -> Result<Json<GuildRoleListResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;
    let (_, permissions) = guild_permission_snapshot(&state, auth.user_id, &path.guild_id).await?;
    if !permissions.contains(Permission::ManageWorkspaceRoles)
        && !permissions.contains(Permission::ManageMemberRoles)
    {
        return Err(AuthFailure::Forbidden);
    }

    let roles = if let Some(pool) = &state.db_pool {
        guild_roles_db(pool, &path.guild_id).await?
    } else {
        guild_roles_in_memory(&state, &path.guild_id).await?
    };
    let default_join_role_id = if let Some(pool) = &state.db_pool {
        guild_default_join_role_id_db(pool, &path.guild_id).await?
    } else {
        guild_default_join_role_id_in_memory(&state, &path.guild_id).await?
    };

    Ok(Json(GuildRoleListResponse {
        roles: roles.into_iter().map(role_model_to_response).collect(),
        default_join_role_id,
    }))
}

pub(crate) async fn update_guild_default_join_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<GuildPath>,
    Json(payload): Json<UpdateGuildDefaultJoinRoleRequest>,
) -> Result<Json<ModerationResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;
    let context = load_actor_role_context(
        &state,
        &path.guild_id,
        auth.user_id,
        Permission::ManageWorkspaceRoles,
    )
    .await?;

    let next_role_id = payload.role_id.map(parse_role_id).transpose()?;
    if let Some(ref role_id) = next_role_id {
        let roles = if let Some(pool) = &state.db_pool {
            guild_roles_db(pool, &path.guild_id).await?
        } else {
            guild_roles_in_memory(&state, &path.guild_id).await?
        };
        let role = roles
            .iter()
            .find(|entry| &entry.role_id == role_id)
            .cloned()
            .ok_or(AuthFailure::InvalidRequest)?;
        if role.is_system || !can_manage_role(&role, context) {
            return Err(AuthFailure::Forbidden);
        }
    }

    if let Some(pool) = &state.db_pool {
        let result = sqlx::query(
            "UPDATE guilds
             SET default_join_role_id = $2
             WHERE guild_id = $1",
        )
        .bind(&path.guild_id)
        .bind(next_role_id.clone())
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        if result.rows_affected() == 0 {
            return Err(AuthFailure::NotFound);
        }
    } else {
        let mut guilds = state.membership_store.guilds().write().await;
        let guild = guilds
            .get_mut(&path.guild_id)
            .ok_or(AuthFailure::NotFound)?;
        guild.default_join_role_id = next_role_id.clone();
    }

    write_audit_log(
        &state,
        Some(path.guild_id.clone()),
        auth.user_id,
        None,
        "role.default_join.update",
        serde_json::json!({ "role_id": next_role_id }),
    )
    .await?;

    Ok(Json(ModerationResponse { accepted: true }))
}

pub(crate) async fn create_guild_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<GuildPath>,
    Json(payload): Json<CreateGuildRoleRequest>,
) -> Result<Json<GuildRoleResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;
    let context = load_actor_role_context(
        &state,
        &path.guild_id,
        auth.user_id,
        Permission::ManageWorkspaceRoles,
    )
    .await?;
    let name = validate_role_name_input(&payload.name)?;
    let permissions = permission_set_from_list(&payload.permissions);
    let color_hex = payload
        .color_hex
        .as_deref()
        .map(validate_role_color_hex_input)
        .transpose()?;

    let position = payload
        .position
        .unwrap_or_else(|| context.highest_position.saturating_sub(1).max(1));
    if !context.is_server_owner
        && !context.is_workspace_owner
        && position >= context.highest_position
    {
        return Err(AuthFailure::Forbidden);
    }

    let role_id = WorkspaceRoleId::new().to_string();
    if let Some(pool) = &state.db_pool {
        let existing_count =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM guild_roles WHERE guild_id = $1")
                .bind(&path.guild_id)
                .fetch_one(pool)
                .await
                .map_err(|_| AuthFailure::Internal)?;
        if usize::try_from(existing_count).map_err(|_| AuthFailure::Internal)? >= MAX_GUILD_ROLES {
            return Err(AuthFailure::QuotaExceeded);
        }

        sqlx::query(
            "INSERT INTO guild_roles
                (role_id, guild_id, name, position, permissions_allow_mask, is_system, system_key, color_hex, created_at_unix)
             VALUES
                ($1, $2, $3, $4, $5, FALSE, NULL, $6, $7)",
        )
        .bind(&role_id)
        .bind(&path.guild_id)
        .bind(&name)
        .bind(position)
        .bind(permission_set_to_i64(permissions)?)
        .bind(color_hex.clone())
        .bind(now_unix())
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
    } else {
        let mut role_maps = state.membership_store.guild_roles().write().await;
        let guild_roles = role_maps
            .get_mut(&path.guild_id)
            .ok_or(AuthFailure::NotFound)?;
        if guild_roles.len() >= MAX_GUILD_ROLES {
            return Err(AuthFailure::QuotaExceeded);
        }
        guild_roles.insert(
            role_id.clone(),
            crate::server::core::WorkspaceRoleRecord {
                role_id: role_id.clone(),
                name: name.clone(),
                position,
                is_system: false,
                system_key: None,
                permissions_allow: permissions,
                color_hex: color_hex.clone(),
                created_at_unix: now_unix(),
            },
        );
    }

    write_audit_log(
        &state,
        Some(path.guild_id.clone()),
        auth.user_id,
        None,
        "role.create",
        serde_json::json!({
            "role_id": role_id,
            "name": name,
            "position": position,
            "permissions": payload.permissions,
            "color_hex": color_hex.clone(),
        }),
    )
    .await?;

    let event = gateway_events::workspace_role_create(
        &path.guild_id,
        &role_id,
        &name,
        position,
        false,
        payload.permissions.clone(),
        color_hex.clone(),
        Some(auth.user_id),
    );
    broadcast_guild_event(&state, &path.guild_id, &event).await;

    Ok(Json(GuildRoleResponse {
        role_id,
        name,
        position,
        is_system: false,
        permissions: payload.permissions,
        color_hex,
    }))
}

#[allow(clippy::too_many_lines)]
pub(crate) async fn update_guild_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<GuildRolePath>,
    Json(payload): Json<UpdateGuildRoleRequest>,
) -> Result<Json<GuildRoleResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;
    let role_id = parse_role_id(path.role_id)?;
    if payload.name.is_none() && payload.permissions.is_none() && payload.color_hex.is_none() {
        return Err(AuthFailure::InvalidRequest);
    }
    let context = load_actor_role_context(
        &state,
        &path.guild_id,
        auth.user_id,
        Permission::ManageWorkspaceRoles,
    )
    .await?;

    let roles = if let Some(pool) = &state.db_pool {
        guild_roles_db(pool, &path.guild_id).await?
    } else {
        guild_roles_in_memory(&state, &path.guild_id).await?
    };
    let current = roles
        .iter()
        .find(|role| role.role_id == role_id)
        .cloned()
        .ok_or(AuthFailure::NotFound)?;
    if !can_manage_role(&current, context) {
        return Err(AuthFailure::Forbidden);
    }

    let next_name = payload
        .name
        .as_deref()
        .map(validate_role_name_input)
        .transpose()?
        .unwrap_or_else(|| current.name.clone());
    let next_permissions = payload
        .permissions
        .as_ref()
        .map_or(current.permissions, |values| {
            permission_set_from_list(values)
        });
    let next_color_hex = match payload.color_hex.as_ref() {
        Some(Some(value)) => Some(validate_role_color_hex_input(value)?),
        Some(None) => None,
        None => current.color_hex.clone(),
    };

    if let Some(pool) = &state.db_pool {
        sqlx::query(
            "UPDATE guild_roles
             SET name = $3, permissions_allow_mask = $4, color_hex = $5
             WHERE guild_id = $1 AND role_id = $2",
        )
        .bind(&path.guild_id)
        .bind(&role_id)
        .bind(&next_name)
        .bind(permission_set_to_i64(next_permissions)?)
        .bind(next_color_hex.clone())
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
    } else {
        let mut role_maps = state.membership_store.guild_roles().write().await;
        let role = role_maps
            .get_mut(&path.guild_id)
            .and_then(|roles| roles.get_mut(&role_id))
            .ok_or(AuthFailure::NotFound)?;
        role.name.clone_from(&next_name);
        role.permissions_allow = next_permissions;
        role.color_hex = next_color_hex.clone();
    }

    if payload.permissions.is_some() {
        write_audit_log(
            &state,
            Some(path.guild_id.clone()),
            auth.user_id,
            None,
            "role.permissions.update",
            serde_json::json!({
                "role_id": role_id,
                "permissions": payload.permissions.clone().unwrap_or_default(),
            }),
        )
        .await?;
    }
    if payload.name.is_some() {
        write_audit_log(
            &state,
            Some(path.guild_id.clone()),
            auth.user_id,
            None,
            "role.update",
            serde_json::json!({
                "role_id": role_id,
                "name": next_name,
            }),
        )
        .await?;
    }
    if payload.color_hex.is_some() {
        write_audit_log(
            &state,
            Some(path.guild_id.clone()),
            auth.user_id,
            None,
            "role.color.update",
            serde_json::json!({
                "role_id": role_id,
                "color_hex": next_color_hex.clone(),
            }),
        )
        .await?;
    }

    let updated_at_unix = now_unix();
    let event_color_hex = if payload.color_hex.is_some() {
        Some(next_color_hex.clone())
    } else {
        None
    };
    let event = gateway_events::workspace_role_update(
        &path.guild_id,
        &role_id,
        payload.name.as_deref(),
        payload.permissions.clone(),
        event_color_hex,
        updated_at_unix,
        Some(auth.user_id),
    );
    broadcast_guild_event(&state, &path.guild_id, &event).await;

    Ok(Json(GuildRoleResponse {
        role_id: role_id.clone(),
        name: next_name,
        position: current.position,
        is_system: current.is_system,
        permissions: crate::server::db::permission_list_from_set(next_permissions),
        color_hex: next_color_hex,
    }))
}

pub(crate) async fn delete_guild_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<GuildRolePath>,
) -> Result<Json<ModerationResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;
    let role_id = parse_role_id(path.role_id)?;
    let context = load_actor_role_context(
        &state,
        &path.guild_id,
        auth.user_id,
        Permission::ManageWorkspaceRoles,
    )
    .await?;
    let roles = if let Some(pool) = &state.db_pool {
        guild_roles_db(pool, &path.guild_id).await?
    } else {
        guild_roles_in_memory(&state, &path.guild_id).await?
    };
    let current = roles
        .iter()
        .find(|role| role.role_id == role_id)
        .cloned()
        .ok_or(AuthFailure::NotFound)?;
    if current.is_system {
        return Err(AuthFailure::Forbidden);
    }
    if !can_manage_role(&current, context) {
        return Err(AuthFailure::Forbidden);
    }

    if let Some(pool) = &state.db_pool {
        let result = sqlx::query(
            "DELETE FROM guild_roles
             WHERE guild_id = $1 AND role_id = $2",
        )
        .bind(&path.guild_id)
        .bind(&role_id)
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        if result.rows_affected() == 0 {
            return Err(AuthFailure::NotFound);
        }
    } else {
        let mut role_maps = state.membership_store.guild_roles().write().await;
        let roles = role_maps
            .get_mut(&path.guild_id)
            .ok_or(AuthFailure::NotFound)?;
        if roles.remove(&role_id).is_none() {
            return Err(AuthFailure::NotFound);
        }
        drop(role_maps);
        let mut assignments = state
            .membership_store
            .guild_role_assignments()
            .write()
            .await;
        if let Some(guild_assignments) = assignments.get_mut(&path.guild_id) {
            for assigned in guild_assignments.values_mut() {
                assigned.remove(&role_id);
            }
        }
    }

    write_audit_log(
        &state,
        Some(path.guild_id.clone()),
        auth.user_id,
        None,
        "role.delete",
        serde_json::json!({ "role_id": role_id }),
    )
    .await?;

    let event = gateway_events::workspace_role_delete(
        &path.guild_id,
        &role_id,
        now_unix(),
        Some(auth.user_id),
    );
    broadcast_guild_event(&state, &path.guild_id, &event).await;

    crate::server::realtime::livekit_sync::schedule_livekit_permission_reevaluation_for_guild(
        &state,
        &path.guild_id,
    );

    Ok(Json(ModerationResponse { accepted: true }))
}

pub(crate) async fn reorder_guild_roles(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<GuildPath>,
    Json(payload): Json<ReorderGuildRolesRequest>,
) -> Result<Json<ModerationResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;
    let context = load_actor_role_context(
        &state,
        &path.guild_id,
        auth.user_id,
        Permission::ManageWorkspaceRoles,
    )
    .await?;
    let requested_ids = parse_role_id_list(&payload.role_ids)?;
    if requested_ids.len() > MAX_GUILD_ROLES {
        return Err(AuthFailure::InvalidRequest);
    }

    let roles = if let Some(pool) = &state.db_pool {
        guild_roles_db(pool, &path.guild_id).await?
    } else {
        guild_roles_in_memory(&state, &path.guild_id).await?
    };
    for role_id in &requested_ids {
        let role = roles
            .iter()
            .find(|value| &value.role_id == role_id)
            .cloned()
            .ok_or(AuthFailure::InvalidRequest)?;
        if role.is_system || !can_manage_role(&role, context) {
            return Err(AuthFailure::Forbidden);
        }
    }

    if let Some(pool) = &state.db_pool {
        let mut tx = pool.begin().await.map_err(|_| AuthFailure::Internal)?;
        for (index, role_id) in requested_ids.iter().enumerate() {
            let position = i32::try_from(requested_ids.len().saturating_sub(index))
                .map_err(|_| AuthFailure::Internal)?;
            sqlx::query(
                "UPDATE guild_roles
                 SET position = $3
                 WHERE guild_id = $1 AND role_id = $2",
            )
            .bind(&path.guild_id)
            .bind(role_id)
            .bind(position)
            .execute(&mut *tx)
            .await
            .map_err(|_| AuthFailure::Internal)?;
        }
        tx.commit().await.map_err(|_| AuthFailure::Internal)?;
    } else {
        let mut role_maps = state.membership_store.guild_roles().write().await;
        let roles = role_maps
            .get_mut(&path.guild_id)
            .ok_or(AuthFailure::NotFound)?;
        for (index, role_id) in requested_ids.iter().enumerate() {
            let position = i32::try_from(requested_ids.len().saturating_sub(index))
                .map_err(|_| AuthFailure::Internal)?;
            let role = roles.get_mut(role_id).ok_or(AuthFailure::InvalidRequest)?;
            role.position = position;
        }
    }

    write_audit_log(
        &state,
        Some(path.guild_id.clone()),
        auth.user_id,
        None,
        "role.reorder",
        serde_json::json!({ "role_ids": requested_ids }),
    )
    .await?;

    let event = gateway_events::workspace_role_reorder(
        &path.guild_id,
        requested_ids.clone(),
        now_unix(),
        Some(auth.user_id),
    );
    broadcast_guild_event(&state, &path.guild_id, &event).await;

    Ok(Json(ModerationResponse { accepted: true }))
}

fn is_workspace_owner_role(model: &GuildRoleModel) -> bool {
    model.system_key.as_deref() == Some(SYSTEM_ROLE_WORKSPACE_OWNER)
}

async fn workspace_owner_count_db(pool: &sqlx::PgPool, guild_id: &str) -> Result<i64, AuthFailure> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*)
         FROM guild_role_members grm
         JOIN guild_roles gr ON gr.role_id = grm.role_id
         WHERE grm.guild_id = $1
           AND gr.system_key = $2",
    )
    .bind(guild_id)
    .bind(SYSTEM_ROLE_WORKSPACE_OWNER)
    .fetch_one(pool)
    .await
    .map_err(|_| AuthFailure::Internal)
}

async fn workspace_owner_count_in_memory(
    state: &AppState,
    guild_id: &str,
) -> Result<usize, AuthFailure> {
    let workspace_owner_role_id = {
        let role_maps = state.membership_store.guild_roles().read().await;
        let roles = role_maps.get(guild_id).ok_or(AuthFailure::NotFound)?;
        roles
            .values()
            .find(|role| role.system_key.as_deref() == Some(SYSTEM_ROLE_WORKSPACE_OWNER))
            .map(|role| role.role_id.clone())
            .ok_or(AuthFailure::Internal)?
    };
    let assignments = state.membership_store.guild_role_assignments().read().await;
    Ok(assignments
        .get(guild_id)
        .into_iter()
        .flat_map(|value| value.values())
        .filter(|assigned| assigned.contains(&workspace_owner_role_id))
        .count())
}

#[allow(clippy::too_many_lines)]
pub(crate) async fn assign_guild_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<GuildRoleMemberPath>,
) -> Result<Json<ModerationResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;
    let role_id = parse_role_id(path.role_id)?;
    let target_user_id = UserId::try_from(path.user_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let context = load_actor_role_context(
        &state,
        &path.guild_id,
        auth.user_id,
        Permission::ManageMemberRoles,
    )
    .await?;

    let roles = if let Some(pool) = &state.db_pool {
        guild_roles_db(pool, &path.guild_id).await?
    } else {
        guild_roles_in_memory(&state, &path.guild_id).await?
    };
    let role = roles
        .iter()
        .find(|entry| entry.role_id == role_id)
        .cloned()
        .ok_or(AuthFailure::NotFound)?;
    if role.system_key.as_deref() == Some(SYSTEM_ROLE_EVERYONE) {
        return Err(AuthFailure::InvalidRequest);
    }
    if is_workspace_owner_role(&role) && !context.is_server_owner {
        return Err(AuthFailure::Forbidden);
    }
    if !context.is_workspace_owner
        && !context.is_server_owner
        && role.position >= context.highest_position
    {
        return Err(AuthFailure::Forbidden);
    }

    if let Some(pool) = &state.db_pool {
        let target_exists =
            sqlx::query("SELECT 1 FROM guild_members WHERE guild_id = $1 AND user_id = $2")
                .bind(&path.guild_id)
                .bind(target_user_id.to_string())
                .fetch_optional(pool)
                .await
                .map_err(|_| AuthFailure::Internal)?
                .is_some();
        if !target_exists {
            return Err(AuthFailure::NotFound);
        }
        let assignment_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)
             FROM guild_role_members
             WHERE guild_id = $1 AND user_id = $2",
        )
        .bind(&path.guild_id)
        .bind(target_user_id.to_string())
        .fetch_one(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        if usize::try_from(assignment_count).map_err(|_| AuthFailure::Internal)?
            >= MAX_MEMBER_ROLE_ASSIGNMENTS
        {
            return Err(AuthFailure::QuotaExceeded);
        }
        sqlx::query(
            "INSERT INTO guild_role_members (guild_id, role_id, user_id, assigned_at_unix)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (guild_id, role_id, user_id) DO NOTHING",
        )
        .bind(&path.guild_id)
        .bind(&role_id)
        .bind(target_user_id.to_string())
        .bind(now_unix())
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        let _ = sync_legacy_role_from_assignments_db(pool, &path.guild_id, target_user_id).await?;
    } else {
        let mut guilds = state.membership_store.guilds().write().await;
        let guild = guilds
            .get_mut(&path.guild_id)
            .ok_or(AuthFailure::NotFound)?;
        if !guild.members.contains_key(&target_user_id) {
            return Err(AuthFailure::NotFound);
        }
        drop(guilds);

        let mut assignments = state
            .membership_store
            .guild_role_assignments()
            .write()
            .await;
        let guild_assignments = assignments
            .get_mut(&path.guild_id)
            .ok_or(AuthFailure::NotFound)?;
        let assigned = guild_assignments.entry(target_user_id).or_default();
        if assigned.len() >= MAX_MEMBER_ROLE_ASSIGNMENTS {
            return Err(AuthFailure::QuotaExceeded);
        }
        assigned.insert(role_id.clone());
        drop(assignments);
        let _ = sync_legacy_role_from_assignments_in_memory(&state, &path.guild_id, target_user_id)
            .await?;
    }

    write_audit_log(
        &state,
        Some(path.guild_id.clone()),
        auth.user_id,
        Some(target_user_id),
        "role.assign",
        serde_json::json!({
            "role_id": role_id,
        }),
    )
    .await?;

    let event = gateway_events::workspace_role_assignment_add(
        &path.guild_id,
        target_user_id,
        &role_id,
        now_unix(),
        Some(auth.user_id),
    );
    broadcast_guild_event(&state, &path.guild_id, &event).await;

    crate::server::realtime::livekit_sync::schedule_livekit_permission_reevaluation_for_guild(
        &state,
        &path.guild_id,
    );

    Ok(Json(ModerationResponse { accepted: true }))
}

pub(crate) async fn unassign_guild_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<GuildRoleMemberPath>,
) -> Result<Json<ModerationResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;
    let role_id = parse_role_id(path.role_id)?;
    let target_user_id = UserId::try_from(path.user_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let context = load_actor_role_context(
        &state,
        &path.guild_id,
        auth.user_id,
        Permission::ManageMemberRoles,
    )
    .await?;

    let roles = if let Some(pool) = &state.db_pool {
        guild_roles_db(pool, &path.guild_id).await?
    } else {
        guild_roles_in_memory(&state, &path.guild_id).await?
    };
    let role = roles
        .iter()
        .find(|entry| entry.role_id == role_id)
        .cloned()
        .ok_or(AuthFailure::NotFound)?;
    if role.system_key.as_deref() == Some(SYSTEM_ROLE_EVERYONE) {
        return Err(AuthFailure::InvalidRequest);
    }
    if is_workspace_owner_role(&role) && !context.is_server_owner {
        return Err(AuthFailure::Forbidden);
    }
    if !context.is_workspace_owner
        && !context.is_server_owner
        && role.position >= context.highest_position
    {
        return Err(AuthFailure::Forbidden);
    }

    if let Some(pool) = &state.db_pool {
        if is_workspace_owner_role(&role) {
            let owner_count = workspace_owner_count_db(pool, &path.guild_id).await?;
            if owner_count <= 1 {
                return Err(AuthFailure::Forbidden);
            }
        }
        sqlx::query(
            "DELETE FROM guild_role_members
             WHERE guild_id = $1 AND role_id = $2 AND user_id = $3",
        )
        .bind(&path.guild_id)
        .bind(&role_id)
        .bind(target_user_id.to_string())
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        let _ = sync_legacy_role_from_assignments_db(pool, &path.guild_id, target_user_id).await?;
    } else {
        if is_workspace_owner_role(&role)
            && workspace_owner_count_in_memory(&state, &path.guild_id).await? <= 1
        {
            return Err(AuthFailure::Forbidden);
        }
        let mut assignments = state
            .membership_store
            .guild_role_assignments()
            .write()
            .await;
        let guild_assignments = assignments
            .get_mut(&path.guild_id)
            .ok_or(AuthFailure::NotFound)?;
        if let Some(assigned) = guild_assignments.get_mut(&target_user_id) {
            assigned.remove(&role_id);
        }
        drop(assignments);
        let _ = sync_legacy_role_from_assignments_in_memory(&state, &path.guild_id, target_user_id)
            .await?;
    }

    write_audit_log(
        &state,
        Some(path.guild_id.clone()),
        auth.user_id,
        Some(target_user_id),
        "role.unassign",
        serde_json::json!({
            "role_id": role_id,
        }),
    )
    .await?;

    let event = gateway_events::workspace_role_assignment_remove(
        &path.guild_id,
        target_user_id,
        &role_id,
        now_unix(),
        Some(auth.user_id),
    );
    broadcast_guild_event(&state, &path.guild_id, &event).await;
    crate::server::realtime::livekit_sync::schedule_livekit_permission_reevaluation_for_guild(
        &state,
        &path.guild_id,
    );

    Ok(Json(ModerationResponse { accepted: true }))
}

#[derive(Debug, Clone)]
struct GuildIpBanRecord {
    ban_id: String,
    source_user_id: Option<String>,
    reason: Option<String>,
    created_at_unix: i64,
    expires_at_unix: Option<i64>,
}

fn normalize_reason(reason: &str) -> Option<String> {
    if reason.is_empty() {
        None
    } else {
        Some(reason.to_owned())
    }
}

fn guild_ip_ban_response_from_record(record: GuildIpBanRecord) -> GuildIpBanRecordResponse {
    GuildIpBanRecordResponse {
        ban_id: record.ban_id,
        source_user_id: record.source_user_id,
        reason: record.reason,
        created_at_unix: record.created_at_unix,
        expires_at_unix: record.expires_at_unix,
    }
}

fn resolve_expiry_unix(expires_in_secs: Option<u64>) -> Result<Option<i64>, AuthFailure> {
    expires_in_secs
        .map(|secs| i64::try_from(secs).map_err(|_| AuthFailure::InvalidRequest))
        .transpose()?
        .map(|secs| {
            now_unix()
                .checked_add(secs)
                .ok_or(AuthFailure::InvalidRequest)
        })
        .transpose()
}

async fn list_guild_ip_bans_db(
    pool: &sqlx::PgPool,
    guild_id: &str,
    query: &GuildIpBanListQuery,
) -> Result<GuildIpBanListResponse, AuthFailure> {
    let cursor = query
        .cursor
        .as_ref()
        .map(|value| parse_audit_cursor_position(value.as_str()))
        .transpose()?;
    let limit_plus_one = query
        .limit
        .checked_add(1)
        .ok_or(AuthFailure::InvalidRequest)?;
    let now = now_unix();

    let rows = sqlx::query(
        "SELECT ban_id, source_user_id, reason, created_at_unix, expires_at_unix
         FROM guild_ip_bans
         WHERE guild_id = $1
           AND (expires_at_unix IS NULL OR expires_at_unix > $2)
           AND (
                $3::bigint IS NULL
                OR created_at_unix < $3
                OR (created_at_unix = $3 AND ban_id < $4)
           )
         ORDER BY created_at_unix DESC, ban_id DESC
         LIMIT $5",
    )
    .bind(guild_id)
    .bind(now)
    .bind(cursor.as_ref().map(|value| value.created_at_unix))
    .bind(cursor.as_ref().map(|value| value.audit_id.as_str()))
    .bind(i64::try_from(limit_plus_one).map_err(|_| AuthFailure::Internal)?)
    .fetch_all(pool)
    .await
    .map_err(|_| AuthFailure::Internal)?;

    let mut records = Vec::with_capacity(rows.len());
    for row in rows {
        let reason: String = row.try_get("reason").map_err(|_| AuthFailure::Internal)?;
        records.push(GuildIpBanRecord {
            ban_id: row.try_get("ban_id").map_err(|_| AuthFailure::Internal)?,
            source_user_id: row
                .try_get::<Option<String>, _>("source_user_id")
                .map_err(|_| AuthFailure::Internal)?,
            reason: normalize_reason(&reason),
            created_at_unix: row
                .try_get("created_at_unix")
                .map_err(|_| AuthFailure::Internal)?,
            expires_at_unix: row
                .try_get("expires_at_unix")
                .map_err(|_| AuthFailure::Internal)?,
        });
    }

    let next_cursor = if records.len() > query.limit {
        let cursor_record = records
            .get(query.limit)
            .cloned()
            .ok_or(AuthFailure::Internal)?;
        records.truncate(query.limit);
        Some(build_audit_cursor(
            cursor_record.created_at_unix,
            &cursor_record.ban_id,
        ))
    } else {
        None
    };

    Ok(GuildIpBanListResponse {
        bans: records
            .into_iter()
            .map(guild_ip_ban_response_from_record)
            .collect(),
        next_cursor,
    })
}

async fn list_guild_ip_bans_in_memory(
    state: &AppState,
    guild_id: &str,
    query: &GuildIpBanListQuery,
) -> Result<GuildIpBanListResponse, AuthFailure> {
    let cursor = query
        .cursor
        .as_ref()
        .map(|value| parse_audit_cursor_position(value.as_str()))
        .transpose()?;
    let now = now_unix();
    let bans = state.guild_ip_bans.read().await;
    let mut records = bans
        .get(guild_id)
        .into_iter()
        .flat_map(|value| value.iter())
        .filter(|entry| entry.expires_at_unix.is_none_or(|expires| expires > now))
        .filter(|entry| {
            cursor.as_ref().is_none_or(|value| {
                entry.created_at_unix < value.created_at_unix
                    || (entry.created_at_unix == value.created_at_unix
                        && entry.ban_id < value.audit_id)
            })
        })
        .map(|entry| GuildIpBanRecord {
            ban_id: entry.ban_id.clone(),
            source_user_id: entry.source_user_id.map(|value| value.to_string()),
            reason: normalize_reason(&entry.reason),
            created_at_unix: entry.created_at_unix,
            expires_at_unix: entry.expires_at_unix,
        })
        .collect::<Vec<_>>();

    records.sort_by(|left, right| {
        right
            .created_at_unix
            .cmp(&left.created_at_unix)
            .then_with(|| right.ban_id.cmp(&left.ban_id))
    });

    let next_cursor = if records.len() > query.limit {
        let cursor_record = records
            .get(query.limit)
            .cloned()
            .ok_or(AuthFailure::Internal)?;
        records.truncate(query.limit);
        Some(build_audit_cursor(
            cursor_record.created_at_unix,
            &cursor_record.ban_id,
        ))
    } else {
        None
    };

    Ok(GuildIpBanListResponse {
        bans: records
            .into_iter()
            .map(guild_ip_ban_response_from_record)
            .collect(),
        next_cursor,
    })
}

async fn resolve_target_user_observed_networks_db(
    pool: &sqlx::PgPool,
    target_user_id: UserId,
    limit: usize,
) -> Result<Vec<IpNetwork>, AuthFailure> {
    let rows = sqlx::query(
        "SELECT ip_cidr
         FROM user_ip_observations
         WHERE user_id = $1
         ORDER BY last_seen_at_unix DESC
         LIMIT $2",
    )
    .bind(target_user_id.to_string())
    .bind(i64::try_from(limit).map_err(|_| AuthFailure::InvalidRequest)?)
    .fetch_all(pool)
    .await
    .map_err(|_| AuthFailure::Internal)?;

    let mut deduped = HashSet::new();
    let mut resolved = Vec::new();
    for row in rows {
        let ip_cidr: String = row.try_get("ip_cidr").map_err(|_| AuthFailure::Internal)?;
        let network = IpNetwork::try_from(ip_cidr).map_err(|_| AuthFailure::Internal)?;
        if deduped.insert(network) {
            resolved.push(network);
        }
    }
    Ok(resolved)
}

async fn resolve_target_user_observed_networks_in_memory(
    state: &AppState,
    target_user_id: UserId,
    limit: usize,
) -> Vec<IpNetwork> {
    let observations = state.user_ip_observations.read().await;
    let mut entries = observations
        .iter()
        .filter(|((user_id, _), _)| *user_id == target_user_id)
        .map(|((_, network), last_seen)| (*network, *last_seen))
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| right.1.cmp(&left.1));

    let mut deduped = HashSet::new();
    let mut resolved = Vec::new();
    for (network, _) in entries {
        if resolved.len() >= limit {
            break;
        }
        if deduped.insert(network) {
            resolved.push(network);
        }
    }
    resolved
}

pub(crate) async fn list_guild_ip_bans(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<GuildPath>,
    Query(query): Query<GuildIpBanListQueryDto>,
) -> Result<Json<GuildIpBanListResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;
    let query = GuildIpBanListQuery::try_from(query)
        .map_err(map_directory_contract_error_to_auth_failure)?;
    enforce_guild_ip_ban_moderation_access(&state, &path.guild_id, auth.user_id).await?;

    let response = if let Some(pool) = &state.db_pool {
        list_guild_ip_bans_db(pool, &path.guild_id, &query).await?
    } else {
        list_guild_ip_bans_in_memory(&state, &path.guild_id, &query).await?
    };

    Ok(Json(response))
}

#[allow(clippy::too_many_lines)]
pub(crate) async fn upsert_guild_ip_bans_by_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<GuildPath>,
    Json(payload): Json<GuildIpBanByUserRequestDto>,
) -> Result<Json<GuildIpBanApplyResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;
    let request = GuildIpBanByUserRequest::try_from(payload)
        .map_err(map_directory_contract_error_to_auth_failure)?;
    enforce_guild_ip_ban_moderation_access(&state, &path.guild_id, auth.user_id).await?;

    let expires_at_unix = resolve_expiry_unix(request.expires_in_secs)?;
    let reason_text = request.reason.clone().unwrap_or_default();
    let now = now_unix();
    let max_entries = state.runtime.guild_ip_ban_max_entries;

    let ban_ids = if let Some(pool) = &state.db_pool {
        let observed =
            resolve_target_user_observed_networks_db(pool, request.target_user_id, max_entries)
                .await?;
        if observed.is_empty() {
            Vec::new()
        } else {
            let mut tx = pool.begin().await.map_err(|_| AuthFailure::Internal)?;
            let count_row =
                sqlx::query("SELECT COUNT(*) AS count FROM guild_ip_bans WHERE guild_id = $1")
                    .bind(&path.guild_id)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|_| AuthFailure::Internal)?;
            let existing_total: i64 = count_row
                .try_get("count")
                .map_err(|_| AuthFailure::Internal)?;

            let existing_rows = sqlx::query(
                "SELECT ip_cidr
                 FROM guild_ip_bans
                 WHERE guild_id = $1
                   AND (expires_at_unix IS NULL OR expires_at_unix > $2)
                 ORDER BY created_at_unix DESC
                 LIMIT $3",
            )
            .bind(&path.guild_id)
            .bind(now)
            .bind(i64::try_from(max_entries).map_err(|_| AuthFailure::Internal)?)
            .fetch_all(&mut *tx)
            .await
            .map_err(|_| AuthFailure::Internal)?;

            let mut existing_active = HashSet::new();
            for row in existing_rows {
                let ip_cidr: String = row.try_get("ip_cidr").map_err(|_| AuthFailure::Internal)?;
                if let Ok(network) = IpNetwork::try_from(ip_cidr) {
                    existing_active.insert(network);
                }
            }

            let to_create = observed
                .into_iter()
                .filter(|network| !existing_active.contains(network))
                .collect::<Vec<_>>();
            let projected_total = usize::try_from(existing_total)
                .map_err(|_| AuthFailure::Internal)?
                .saturating_add(to_create.len());
            if projected_total > max_entries {
                return Err(AuthFailure::QuotaExceeded);
            }

            let mut created_ids = Vec::with_capacity(to_create.len());
            for network in to_create {
                let ban_id = GuildIpBanId::new().to_string();
                sqlx::query(
                    "INSERT INTO guild_ip_bans
                     (ban_id, guild_id, ip_cidr, source_user_id, reason, created_by_user_id, created_at_unix, expires_at_unix)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                )
                .bind(&ban_id)
                .bind(&path.guild_id)
                .bind(network.canonical_cidr())
                .bind(request.target_user_id.to_string())
                .bind(&reason_text)
                .bind(auth.user_id.to_string())
                .bind(now)
                .bind(expires_at_unix)
                .execute(&mut *tx)
                .await
                .map_err(|_| AuthFailure::Internal)?;
                created_ids.push(ban_id);
            }
            tx.commit().await.map_err(|_| AuthFailure::Internal)?;
            created_ids
        }
    } else {
        let observed = resolve_target_user_observed_networks_in_memory(
            &state,
            request.target_user_id,
            max_entries,
        )
        .await;
        if observed.is_empty() {
            Vec::new()
        } else {
            let mut bans = state.guild_ip_bans.write().await;
            let guild_entries = bans.entry(path.guild_id.clone()).or_default();
            let active_networks = guild_entries
                .iter()
                .filter(|entry| entry.expires_at_unix.is_none_or(|expires| expires > now))
                .map(|entry| entry.ip_network)
                .collect::<HashSet<_>>();
            let to_create = observed
                .into_iter()
                .filter(|network| !active_networks.contains(network))
                .collect::<Vec<_>>();
            let projected_total = guild_entries.len().saturating_add(to_create.len());
            if projected_total > max_entries {
                return Err(AuthFailure::QuotaExceeded);
            }

            let mut created_ids = Vec::with_capacity(to_create.len());
            for network in to_create {
                let ban_id = GuildIpBanId::new().to_string();
                created_ids.push(ban_id.clone());
                guild_entries.push(crate::server::core::GuildIpBanRecord {
                    ban_id,
                    ip_network: network,
                    source_user_id: Some(request.target_user_id),
                    reason: reason_text.clone(),
                    created_at_unix: now,
                    expires_at_unix,
                });
            }
            created_ids
        }
    };

    if !ban_ids.is_empty() {
        write_audit_log(
            &state,
            Some(path.guild_id.clone()),
            auth.user_id,
            Some(request.target_user_id),
            "moderation.ip_ban.add",
            serde_json::json!({
                "created_count": ban_ids.len(),
                "ban_ids": ban_ids.clone(),
                "expires_at_unix": expires_at_unix,
            }),
        )
        .await?;

        let event = gateway_events::workspace_ip_ban_sync(
            &path.guild_id,
            "upsert",
            ban_ids.len(),
            now,
            Some(auth.user_id),
        );
        broadcast_guild_event(&state, &path.guild_id, &event).await;
    }

    Ok(Json(GuildIpBanApplyResponse {
        created_count: ban_ids.len(),
        ban_ids,
    }))
}

pub(crate) async fn remove_guild_ip_ban(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<GuildIpBanPath>,
) -> Result<Json<ModerationResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;
    let ban_id = GuildIpBanId::try_from(path.ban_id)
        .map_err(map_directory_contract_error_to_auth_failure)?;
    enforce_guild_ip_ban_moderation_access(&state, &path.guild_id, auth.user_id).await?;

    let mut target_user_id = None;
    if let Some(pool) = &state.db_pool {
        let row = sqlx::query(
            "DELETE FROM guild_ip_bans
             WHERE guild_id = $1 AND ban_id = $2
             RETURNING source_user_id",
        )
        .bind(&path.guild_id)
        .bind(ban_id.to_string())
        .fetch_optional(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        let Some(row) = row else {
            return Err(AuthFailure::NotFound);
        };
        let source_user_id = row
            .try_get::<Option<String>, _>("source_user_id")
            .map_err(|_| AuthFailure::Internal)?;
        target_user_id = source_user_id.and_then(|value| UserId::try_from(value).ok());
    } else {
        let mut bans = state.guild_ip_bans.write().await;
        let guild_bans = bans.get_mut(&path.guild_id).ok_or(AuthFailure::NotFound)?;
        let before = guild_bans.len();
        guild_bans.retain(|entry| {
            if entry.ban_id == ban_id.to_string() {
                target_user_id = entry.source_user_id;
                false
            } else {
                true
            }
        });
        if guild_bans.len() == before {
            return Err(AuthFailure::NotFound);
        }
    }

    write_audit_log(
        &state,
        Some(path.guild_id.clone()),
        auth.user_id,
        target_user_id,
        "moderation.ip_ban.remove",
        serde_json::json!({
            "ban_id": ban_id.to_string(),
        }),
    )
    .await?;

    let event = gateway_events::workspace_ip_ban_sync(
        &path.guild_id,
        "remove",
        1,
        now_unix(),
        Some(auth.user_id),
    );
    broadcast_guild_event(&state, &path.guild_id, &event).await;

    Ok(Json(ModerationResponse { accepted: true }))
}

pub(crate) async fn list_public_guilds(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PublicGuildListQuery>,
) -> Result<Json<PublicGuildListResponse>, AuthFailure> {
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

    let guilds = state.membership_store.guilds().read().await;
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

async fn assign_default_join_role_db(
    pool: &sqlx::PgPool,
    guild_id: &str,
    user_id: UserId,
) -> Result<(), AuthFailure> {
    let role_id = sqlx::query_scalar::<_, Option<String>>(
        "SELECT default_join_role_id
         FROM guilds
         WHERE guild_id = $1",
    )
    .bind(guild_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| AuthFailure::Internal)?
    .ok_or(AuthFailure::NotFound)?;
    let Some(role_id) = role_id else {
        return Ok(());
    };
    let role = sqlx::query(
        "SELECT role_id, is_system, system_key
         FROM guild_roles
         WHERE guild_id = $1 AND role_id = $2",
    )
    .bind(guild_id)
    .bind(&role_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let Some(role) = role else {
        return Ok(());
    };
    let is_system: bool = role
        .try_get("is_system")
        .map_err(|_| AuthFailure::Internal)?;
    let system_key = role
        .try_get::<Option<String>, _>("system_key")
        .map_err(|_| AuthFailure::Internal)?;
    if is_system || system_key.as_deref() == Some(SYSTEM_ROLE_EVERYONE) {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO guild_role_members (guild_id, role_id, user_id, assigned_at_unix)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (guild_id, role_id, user_id) DO NOTHING",
    )
    .bind(guild_id)
    .bind(role_id)
    .bind(user_id.to_string())
    .bind(now_unix())
    .execute(pool)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let _ = sync_legacy_role_from_assignments_db(pool, guild_id, user_id).await?;
    Ok(())
}

async fn assign_default_join_role_in_memory(
    state: &AppState,
    guild_id: &str,
    user_id: UserId,
) -> Result<(), AuthFailure> {
    let default_join_role_id = {
        let guilds = state.membership_store.guilds().read().await;
        guilds
            .get(guild_id)
            .ok_or(AuthFailure::NotFound)?
            .default_join_role_id
            .clone()
    };
    let Some(role_id) = default_join_role_id else {
        return Ok(());
    };
    let is_assignable = {
        let role_maps = state.membership_store.guild_roles().read().await;
        role_maps
            .get(guild_id)
            .and_then(|roles| roles.get(&role_id))
            .is_some_and(|role| {
                !(role.is_system || role.system_key.as_deref() == Some(SYSTEM_ROLE_EVERYONE))
            })
    };
    if !is_assignable {
        return Ok(());
    }
    let mut assignments = state
        .membership_store
        .guild_role_assignments()
        .write()
        .await;
    assignments
        .entry(guild_id.to_owned())
        .or_default()
        .entry(user_id)
        .or_default()
        .insert(role_id);
    Ok(())
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
    } else {
        assign_default_join_role_db(pool, guild_id, user_id).await?;
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
        let guilds = state.membership_store.guilds().read().await;
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

    let mut guilds = state.membership_store.guilds().write().await;
    let guild = guilds.get_mut(guild_id).ok_or(AuthFailure::NotFound)?;
    if let std::collections::hash_map::Entry::Vacant(entry) = guild.members.entry(user_id) {
        entry.insert(Role::Member);
        drop(guilds);
        assign_default_join_role_in_memory(state, guild_id, user_id).await?;
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
    if outcome == DirectoryJoinOutcome::Accepted {
        let joined_at_unix = now_unix();
        let event = gateway_events::workspace_member_add(
            &path.guild_id,
            auth.user_id,
            Role::Member,
            joined_at_unix,
            Some(auth.user_id),
        );
        broadcast_guild_event(&state, &path.guild_id, &event).await;
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
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(path): Path<GuildPath>,
) -> Result<Json<ChannelListResponse>, AuthFailure> {
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    enforce_guild_ip_ban_for_request(
        &state,
        &path.guild_id,
        auth.user_id,
        client_ip,
        "guild.channels.list",
    )
    .await?;
    guild_permission_snapshot(&state, auth.user_id, &path.guild_id).await?;

    let channel_candidates = if let Some(pool) = &state.db_pool {
        let rows = sqlx::query(
            "SELECT channel_id, name, kind
             FROM channels
             WHERE guild_id = $1
             ORDER BY created_at_unix ASC
             LIMIT $2",
        )
        .bind(&path.guild_id)
        .bind(i64::try_from(MAX_CHANNEL_LIST_LIMIT).map_err(|_| AuthFailure::Internal)?)
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        let mut entries = Vec::with_capacity(rows.len());
        for row in rows {
            let kind_raw: i16 = row.try_get("kind").map_err(|_| AuthFailure::Internal)?;
            let kind = channel_kind_from_i16(kind_raw).ok_or(AuthFailure::Internal)?;
            entries.push(ChannelResponse {
                channel_id: row
                    .try_get("channel_id")
                    .map_err(|_| AuthFailure::Internal)?,
                name: row.try_get("name").map_err(|_| AuthFailure::Internal)?,
                kind,
            });
        }
        entries
    } else {
        let guilds = state.membership_store.guilds().read().await;
        let guild = guilds.get(&path.guild_id).ok_or(AuthFailure::NotFound)?;
        let mut entries = guild
            .channels
            .iter()
            .map(|(channel_id, channel)| ChannelResponse {
                channel_id: channel_id.clone(),
                name: channel.name.clone(),
                kind: channel.kind,
            })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| left.channel_id.cmp(&right.channel_id));
        entries.truncate(MAX_CHANNEL_LIST_LIMIT);
        entries
    };

    let mut channels = Vec::new();
    for channel in channel_candidates {
        let (_, permissions) = crate::server::domain::channel_permission_snapshot(
            &state,
            auth.user_id,
            &path.guild_id,
            &channel.channel_id,
        )
        .await?;
        if permissions.contains(Permission::CreateMessage) {
            channels.push(channel);
        }
    }

    Ok(Json(ChannelListResponse { channels }))
}

pub(crate) async fn create_channel(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(path): Path<GuildPath>,
    Json(payload): Json<CreateChannelRequest>,
) -> Result<Json<ChannelResponse>, AuthFailure> {
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    enforce_guild_ip_ban_for_request(
        &state,
        &path.guild_id,
        auth.user_id,
        client_ip,
        "guild.channels.create",
    )
    .await?;
    let name = ChannelName::try_from(payload.name).map_err(|_| AuthFailure::InvalidRequest)?;
    let kind = payload.kind.unwrap_or(ChannelKind::Text);
    let (_, actor_permissions) =
        guild_permission_snapshot(&state, auth.user_id, &path.guild_id).await?;
    if !actor_permissions.contains(Permission::ManageChannelOverrides) {
        return Err(AuthFailure::Forbidden);
    }

    let channel_id = Ulid::new().to_string();
    if let Some(pool) = &state.db_pool {
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
    } else {
        let mut guilds = state.membership_store.guilds().write().await;
        let guild = guilds
            .get_mut(&path.guild_id)
            .ok_or(AuthFailure::NotFound)?;

        guild.channels.insert(
            channel_id.clone(),
            ChannelRecord {
                name: name.as_str().to_owned(),
                kind,
                messages: Vec::new(),
                role_overrides: HashMap::new(),
            },
        );
    }

    let response = ChannelResponse {
        channel_id,
        name: name.as_str().to_owned(),
        kind,
    };
    let event = gateway_events::channel_create(&path.guild_id, &response);
    broadcast_guild_event(&state, &path.guild_id, &event).await;

    Ok(Json(response))
}

pub(crate) async fn add_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<MemberPath>,
) -> Result<Json<ModerationResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;
    let (_, actor_permissions) =
        guild_permission_snapshot(&state, auth.user_id, &path.guild_id).await?;
    if !actor_permissions.contains(Permission::ManageMemberRoles) {
        return Err(AuthFailure::Forbidden);
    }
    let target_user_id = UserId::try_from(path.user_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let mut added = false;

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

        let insert = sqlx::query(
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
        added = insert.rows_affected() > 0;
        if added {
            assign_default_join_role_db(pool, &path.guild_id, target_user_id).await?;
        }
    } else {
        let mut guilds = state.membership_store.guilds().write().await;
        let guild = guilds
            .get_mut(&path.guild_id)
            .ok_or(AuthFailure::NotFound)?;
        if guild.banned_members.contains(&target_user_id) {
            return Err(AuthFailure::Forbidden);
        }
        if let std::collections::hash_map::Entry::Vacant(entry) =
            guild.members.entry(target_user_id)
        {
            entry.insert(Role::Member);
            added = true;
        }
        drop(guilds);
        if added {
            assign_default_join_role_in_memory(&state, &path.guild_id, target_user_id).await?;
        }
    }

    if added {
        let joined_at_unix = now_unix();
        let event = gateway_events::workspace_member_add(
            &path.guild_id,
            target_user_id,
            Role::Member,
            joined_at_unix,
            Some(auth.user_id),
        );
        broadcast_guild_event(&state, &path.guild_id, &event).await;
    }

    Ok(Json(ModerationResponse { accepted: true }))
}

#[allow(clippy::too_many_lines)]
pub(crate) async fn update_member_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<MemberPath>,
    Json(payload): Json<UpdateMemberRoleRequest>,
) -> Result<Json<ModerationResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;
    let actor_role = user_role_in_guild(&state, auth.user_id, &path.guild_id).await?;
    let target_user_id = UserId::try_from(path.user_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let target_role = member_role_in_guild(&state, target_user_id, &path.guild_id).await?;

    if !can_assign_role_legacy(actor_role, target_role, payload.role) {
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

        let role_rows = sqlx::query(
            "SELECT role_id, system_key, name
             FROM guild_roles
             WHERE guild_id = $1",
        )
        .bind(&path.guild_id)
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        let mut workspace_owner_role_id = None;
        let mut moderator_role_id = None;
        let mut member_role_id = None;
        for row in role_rows {
            let role_id: String = row.try_get("role_id").map_err(|_| AuthFailure::Internal)?;
            let system_key = row
                .try_get::<Option<String>, _>("system_key")
                .ok()
                .flatten();
            let name = row
                .try_get::<String, _>("name")
                .map_err(|_| AuthFailure::Internal)?;
            if system_key.as_deref() == Some(SYSTEM_ROLE_WORKSPACE_OWNER) {
                workspace_owner_role_id = Some(role_id.clone());
            }
            if system_key.as_deref() == Some(DEFAULT_ROLE_MODERATOR)
                || name.eq_ignore_ascii_case(DEFAULT_ROLE_MODERATOR)
            {
                moderator_role_id = Some(role_id.clone());
            }
            if system_key.as_deref() == Some(DEFAULT_ROLE_MEMBER)
                || name.eq_ignore_ascii_case(DEFAULT_ROLE_MEMBER)
            {
                member_role_id = Some(role_id.clone());
            }
        }
        if let Some(workspace_owner_role_id) = workspace_owner_role_id {
            sqlx::query(
                "DELETE FROM guild_role_members
                 WHERE guild_id = $1 AND user_id = $2 AND role_id = $3",
            )
            .bind(&path.guild_id)
            .bind(target_user_id.to_string())
            .bind(workspace_owner_role_id)
            .execute(pool)
            .await
            .map_err(|_| AuthFailure::Internal)?;
        }
        if let Some(moderator_role_id) = moderator_role_id {
            sqlx::query(
                "DELETE FROM guild_role_members
                 WHERE guild_id = $1 AND user_id = $2 AND role_id = $3",
            )
            .bind(&path.guild_id)
            .bind(target_user_id.to_string())
            .bind(&moderator_role_id)
            .execute(pool)
            .await
            .map_err(|_| AuthFailure::Internal)?;
            if payload.role == Role::Moderator {
                sqlx::query(
                    "INSERT INTO guild_role_members (guild_id, role_id, user_id, assigned_at_unix)
                     VALUES ($1, $2, $3, $4)
                     ON CONFLICT (guild_id, role_id, user_id) DO NOTHING",
                )
                .bind(&path.guild_id)
                .bind(moderator_role_id)
                .bind(target_user_id.to_string())
                .bind(now_unix())
                .execute(pool)
                .await
                .map_err(|_| AuthFailure::Internal)?;
            }
            if let Some(member_role_id) = member_role_id {
                sqlx::query(
                    "DELETE FROM guild_role_members
                 WHERE guild_id = $1 AND user_id = $2 AND role_id = $3",
                )
                .bind(&path.guild_id)
                .bind(target_user_id.to_string())
                .bind(&member_role_id)
                .execute(pool)
                .await
                .map_err(|_| AuthFailure::Internal)?;
                if payload.role == Role::Member {
                    sqlx::query(
                    "INSERT INTO guild_role_members (guild_id, role_id, user_id, assigned_at_unix)
                     VALUES ($1, $2, $3, $4)
                     ON CONFLICT (guild_id, role_id, user_id) DO NOTHING",
                )
                .bind(&path.guild_id)
                .bind(member_role_id)
                .bind(target_user_id.to_string())
                .bind(now_unix())
                .execute(pool)
                .await
                .map_err(|_| AuthFailure::Internal)?;
                }
            }
        }
    } else {
        let mut guilds = state.membership_store.guilds().write().await;
        let guild = guilds
            .get_mut(&path.guild_id)
            .ok_or(AuthFailure::NotFound)?;
        let Some(role) = guild.members.get_mut(&target_user_id) else {
            return Err(AuthFailure::NotFound);
        };
        *role = payload.role;
        drop(guilds);
        let mut assignments = state
            .membership_store
            .guild_role_assignments()
            .write()
            .await;
        if let Some(guild_assignments) = assignments.get_mut(&path.guild_id) {
            let assigned = guild_assignments.entry(target_user_id).or_default();
            let role_map = state.membership_store.guild_roles().read().await;
            let roles = role_map.get(&path.guild_id).ok_or(AuthFailure::NotFound)?;
            let workspace_owner_role_id = roles
                .values()
                .find(|role| role.system_key.as_deref() == Some(SYSTEM_ROLE_WORKSPACE_OWNER))
                .map(|role| role.role_id.clone());
            let moderator_role_id = roles
                .values()
                .find(|role| {
                    role.system_key.as_deref() == Some(DEFAULT_ROLE_MODERATOR)
                        || role.name.eq_ignore_ascii_case(DEFAULT_ROLE_MODERATOR)
                })
                .map(|role| role.role_id.clone());
            let member_role_id = roles
                .values()
                .find(|role| {
                    role.system_key.as_deref() == Some(DEFAULT_ROLE_MEMBER)
                        || role.name.eq_ignore_ascii_case(DEFAULT_ROLE_MEMBER)
                })
                .map(|role| role.role_id.clone());
            drop(role_map);
            if let Some(role_id) = workspace_owner_role_id {
                assigned.remove(&role_id);
            }
            if let Some(role_id) = moderator_role_id {
                assigned.remove(&role_id);
                if payload.role == Role::Moderator {
                    assigned.insert(role_id);
                }
            }
            if let Some(role_id) = member_role_id {
                assigned.remove(&role_id);
                if payload.role == Role::Member {
                    assigned.insert(role_id);
                }
            }
        }
    }

    let updated_at_unix = now_unix();
    let event = gateway_events::workspace_member_update(
        &path.guild_id,
        target_user_id,
        Some(payload.role),
        updated_at_unix,
        Some(auth.user_id),
    );
    broadcast_guild_event(&state, &path.guild_id, &event).await;

    crate::server::realtime::livekit_sync::schedule_livekit_permission_reevaluation_for_guild(
        &state,
        &path.guild_id,
    );

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
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(path): Path<ChannelRolePath>,
    Json(payload): Json<UpdateChannelRoleOverrideRequest>,
) -> Result<Json<ModerationResponse>, AuthFailure> {
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    enforce_guild_ip_ban_for_request(
        &state,
        &path.guild_id,
        auth.user_id,
        client_ip,
        "guild.channel_overrides.update",
    )
    .await?;
    let actor_role = user_role_in_guild(&state, auth.user_id, &path.guild_id).await?;
    if !has_permission_legacy(actor_role, Permission::ManageChannelOverrides) {
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
        let mut guilds = state.membership_store.guilds().write().await;
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
        Some(path.guild_id.clone()),
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

    let event = gateway_events::workspace_channel_override_update(
        &path.guild_id,
        &path.channel_id,
        path.role,
        payload.allow.clone(),
        payload.deny.clone(),
        now_unix(),
        Some(auth.user_id),
    );
    broadcast_guild_event(&state, &path.guild_id, &event).await;

    crate::server::realtime::livekit_sync::schedule_livekit_permission_reevaluation_for_guild(
        &state,
        &path.guild_id,
    );

    Ok(Json(ModerationResponse { accepted: true }))
}

fn parse_channel_permission_override_masks(
    payload: &UpdateChannelPermissionOverrideRequest,
) -> Result<(filament_core::PermissionSet, filament_core::PermissionSet), AuthFailure> {
    let allow = permission_set_from_list(&payload.allow);
    let deny = permission_set_from_list(&payload.deny);
    if allow.bits() & deny.bits() != 0 {
        return Err(AuthFailure::InvalidRequest);
    }
    Ok((allow, deny))
}

async fn apply_channel_permission_override(
    state: &AppState,
    path: &ChannelPermissionOverridePath,
    allow: filament_core::PermissionSet,
    deny: filament_core::PermissionSet,
) -> Result<(), AuthFailure> {
    let target_kind_i16 = path.target_kind.to_i16();

    if let Some(pool) = &state.db_pool {
        let result = sqlx::query(
            "INSERT INTO channel_permission_overrides (guild_id, channel_id, target_kind, target_id, allow_mask, deny_mask)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (guild_id, channel_id, target_kind, target_id)
             DO UPDATE SET allow_mask = EXCLUDED.allow_mask, deny_mask = EXCLUDED.deny_mask",
        )
        .bind(&path.guild_id)
        .bind(&path.channel_id)
        .bind(target_kind_i16)
        .bind(&path.target_id)
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
        let mut overrides_map = state
            .membership_store
            .guild_channel_permission_overrides()
            .write()
            .await;
        let guild_overrides = overrides_map
            .get_mut(&path.guild_id)
            .ok_or(AuthFailure::NotFound)?;
        let channel_override = guild_overrides.entry(path.channel_id.clone()).or_default();

        let permissions = filament_core::ChannelPermissionOverwrite { allow, deny };

        if path.target_kind == crate::server::types::PermissionOverrideTargetKind::Role {
            channel_override
                .role_overrides
                .insert(path.target_id.clone(), permissions);
        } else {
            let parsed_user_id = filament_core::UserId::try_from(path.target_id.clone())
                .map_err(|_| AuthFailure::InvalidRequest)?;
            channel_override
                .member_overrides
                .insert(parsed_user_id, permissions);
        }
    }

    Ok(())
}

pub(crate) async fn set_channel_permission_override(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(path): Path<ChannelPermissionOverridePath>,
    Json(payload): Json<UpdateChannelPermissionOverrideRequest>,
) -> Result<Json<ModerationResponse>, AuthFailure> {
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    enforce_guild_ip_ban_for_request(
        &state,
        &path.guild_id,
        auth.user_id,
        client_ip,
        "guild.channel_permission_overrides.update",
    )
    .await?;

    crate::server::domain::check_channel_permission(
        &state,
        auth.user_id,
        &path.guild_id,
        &path.channel_id,
        Permission::ManageChannelOverrides,
    )
    .await?;

    let (allow, deny) = parse_channel_permission_override_masks(&payload)?;
    apply_channel_permission_override(&state, &path, allow, deny).await?;

    write_audit_log(
        &state,
        Some(path.guild_id.clone()),
        auth.user_id,
        None,
        "channel.permission_override.update",
        serde_json::json!({
            "channel_id": path.channel_id,
            "target_kind": path.target_kind,
            "target_id": path.target_id,
            "allow": payload.allow,
            "deny": payload.deny,
        }),
    )
    .await?;

    // Dual emit during migration to keep older clients that only observe the legacy
    // event name functional while introducing explicit permission-override contract.
    let legacy_event = gateway_events::workspace_channel_permission_override_update_legacy(
        &path.guild_id,
        &path.channel_id,
        path.target_kind,
        &path.target_id,
        gateway_events::WorkspaceChannelOverrideFieldsPayload::new(
            payload.allow.clone(),
            payload.deny.clone(),
        ),
        now_unix(),
        Some(auth.user_id),
    );
    broadcast_guild_event(&state, &path.guild_id, &legacy_event).await;

    let event = gateway_events::workspace_channel_permission_override_update(
        &path.guild_id,
        &path.channel_id,
        path.target_kind,
        &path.target_id,
        gateway_events::WorkspaceChannelOverrideFieldsPayload::new(
            payload.allow.clone(),
            payload.deny.clone(),
        ),
        now_unix(),
        Some(auth.user_id),
    );
    broadcast_guild_event(&state, &path.guild_id, &event).await;

    crate::server::realtime::livekit_sync::schedule_livekit_permission_reevaluation_for_guild(
        &state,
        &path.guild_id,
    );

    Ok(Json(ModerationResponse { accepted: true }))
}

async fn remove_member_from_voice_channels(
    state: &AppState,
    guild_id: &str,
    user_id: UserId,
    removed_at_unix: i64,
) {
    let prefix = format!("{guild_id}:");
    let active_voice = state.realtime_registry.voice_participants().read().await;
    let mut to_remove_voice = Vec::new();
    for (key, users) in active_voice.iter() {
        if key.starts_with(&prefix) && users.contains_key(&user_id) {
            if let Some((entry_guild_id, channel_id)) = key.split_once(':') {
                to_remove_voice.push((entry_guild_id.to_owned(), channel_id.to_owned()));
            }
        }
    }
    drop(active_voice);
    for (entry_guild_id, channel_id) in to_remove_voice {
        crate::server::realtime::remove_voice_participant_for_channel(
            state,
            user_id,
            &entry_guild_id,
            &channel_id,
            removed_at_unix,
        )
        .await;
    }
}

pub(crate) async fn kick_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<MemberPath>,
) -> Result<Json<ModerationResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;
    let actor_role = user_role_in_guild(&state, auth.user_id, &path.guild_id).await?;
    if !has_permission_legacy(actor_role, Permission::BanMember) {
        return Err(AuthFailure::Forbidden);
    }
    let target_user_id = UserId::try_from(path.user_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let target_role = member_role_in_guild(&state, target_user_id, &path.guild_id).await?;
    if !can_moderate_member_legacy(actor_role, target_role) {
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
        sqlx::query(
            "DELETE FROM guild_role_members
             WHERE guild_id = $1 AND user_id = $2",
        )
        .bind(&path.guild_id)
        .bind(target_user_id.to_string())
        .execute(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?;
    } else {
        let mut guilds = state.membership_store.guilds().write().await;
        let guild = guilds
            .get_mut(&path.guild_id)
            .ok_or(AuthFailure::NotFound)?;
        if guild.members.remove(&target_user_id).is_none() {
            return Err(AuthFailure::NotFound);
        }
        drop(guilds);
        let mut assignments = state
            .membership_store
            .guild_role_assignments()
            .write()
            .await;
        if let Some(guild_assignments) = assignments.get_mut(&path.guild_id) {
            guild_assignments.remove(&target_user_id);
        }
    }

    let removed_at_unix = now_unix();
    let event = gateway_events::workspace_member_remove(
        &path.guild_id,
        target_user_id,
        "kick",
        removed_at_unix,
        Some(auth.user_id),
    );
    broadcast_guild_event(&state, &path.guild_id, &event).await;

    remove_member_from_voice_channels(&state, &path.guild_id, target_user_id, removed_at_unix)
        .await;

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

async fn persist_member_ban(
    state: &AppState,
    guild_id: &str,
    target_user_id: UserId,
    banned_by_user_id: UserId,
    banned_at_unix: i64,
) -> Result<(), AuthFailure> {
    if let Some(pool) = &state.db_pool {
        let mut tx = pool.begin().await.map_err(|_| AuthFailure::Internal)?;
        sqlx::query(
            "INSERT INTO guild_bans (guild_id, user_id, banned_by_user_id, created_at_unix)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (guild_id, user_id) DO UPDATE SET banned_by_user_id = EXCLUDED.banned_by_user_id, created_at_unix = EXCLUDED.created_at_unix",
        )
        .bind(guild_id)
        .bind(target_user_id.to_string())
        .bind(banned_by_user_id.to_string())
        .bind(banned_at_unix)
        .execute(&mut *tx)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        sqlx::query("DELETE FROM guild_members WHERE guild_id = $1 AND user_id = $2")
            .bind(guild_id)
            .bind(target_user_id.to_string())
            .execute(&mut *tx)
            .await
            .map_err(|_| AuthFailure::Internal)?;
        sqlx::query(
            "DELETE FROM guild_role_members
             WHERE guild_id = $1 AND user_id = $2",
        )
        .bind(guild_id)
        .bind(target_user_id.to_string())
        .execute(&mut *tx)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        tx.commit().await.map_err(|_| AuthFailure::Internal)?;
    } else {
        let mut guilds = state.membership_store.guilds().write().await;
        let guild = guilds.get_mut(guild_id).ok_or(AuthFailure::NotFound)?;
        guild.members.remove(&target_user_id);
        guild.banned_members.insert(target_user_id);
        drop(guilds);
        let mut assignments = state
            .membership_store
            .guild_role_assignments()
            .write()
            .await;
        if let Some(guild_assignments) = assignments.get_mut(guild_id) {
            guild_assignments.remove(&target_user_id);
        }
    }

    Ok(())
}

pub(crate) async fn ban_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<MemberPath>,
) -> Result<Json<ModerationResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;
    let actor_role = user_role_in_guild(&state, auth.user_id, &path.guild_id).await?;
    if !has_permission_legacy(actor_role, Permission::BanMember) {
        return Err(AuthFailure::Forbidden);
    }
    let target_user_id = UserId::try_from(path.user_id).map_err(|_| AuthFailure::InvalidRequest)?;
    if let Ok(target_role) = member_role_in_guild(&state, target_user_id, &path.guild_id).await {
        if !can_moderate_member_legacy(actor_role, target_role) {
            return Err(AuthFailure::Forbidden);
        }
    }

    let banned_at_unix = now_unix();
    persist_member_ban(
        &state,
        &path.guild_id,
        target_user_id,
        auth.user_id,
        banned_at_unix,
    )
    .await?;
    let ban_event = gateway_events::workspace_member_ban(
        &path.guild_id,
        target_user_id,
        banned_at_unix,
        Some(auth.user_id),
    );
    broadcast_guild_event(&state, &path.guild_id, &ban_event).await;
    let remove_event = gateway_events::workspace_member_remove(
        &path.guild_id,
        target_user_id,
        "ban",
        banned_at_unix,
        Some(auth.user_id),
    );
    broadcast_guild_event(&state, &path.guild_id, &remove_event).await;

    remove_member_from_voice_channels(&state, &path.guild_id, target_user_id, banned_at_unix).await;

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
        assign_default_join_role_in_memory, classify_directory_join_outcome, guild_roles_in_memory,
        join_outcome_response, maybe_record_join_ip_observation, validate_role_color_hex_input,
        workspace_owner_count_in_memory, DirectoryJoinBanStatus, DirectoryJoinMembershipStatus,
        DirectoryJoinPolicyInput, DirectoryJoinVisibilityStatus,
    };
    use crate::server::{
        auth::resolve_client_ip,
        core::{AppConfig, AppState, GuildRecord, GuildVisibility, WorkspaceRoleRecord},
        directory_contract::DirectoryJoinOutcome,
        types::DirectoryJoinOutcomeResponse,
    };
    use axum::http::HeaderMap;
    use filament_core::{Role, UserId};
    use std::collections::{HashMap, HashSet};

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

    #[tokio::test]
    async fn guild_roles_in_memory_reads_roles_through_membership_store() {
        let state = AppState::new(&AppConfig::default()).expect("state should initialize");
        let guild_id = String::from("guild-roles-membership-store");

        let mut roles = HashMap::new();
        roles.insert(
            String::from("member"),
            WorkspaceRoleRecord {
                role_id: String::from("member"),
                name: String::from("Member"),
                position: 1,
                is_system: true,
                system_key: Some(String::from("member")),
                permissions_allow: filament_core::PermissionSet::empty(),
                color_hex: None,
                created_at_unix: 20,
            },
        );
        roles.insert(
            String::from("owner"),
            WorkspaceRoleRecord {
                role_id: String::from("owner"),
                name: String::from("Owner"),
                position: 9,
                is_system: true,
                system_key: Some(String::from("workspace_owner")),
                permissions_allow: filament_core::PermissionSet::empty(),
                color_hex: None,
                created_at_unix: 10,
            },
        );

        state
            .membership_store
            .guild_roles()
            .write()
            .await
            .insert(guild_id.clone(), roles);

        let listed = guild_roles_in_memory(&state, &guild_id)
            .await
            .expect("guild role listing should succeed");
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].role_id, "owner");
        assert_eq!(listed[1].role_id, "member");
    }

    #[tokio::test]
    async fn workspace_owner_count_in_memory_uses_membership_store_assignments() {
        let state = AppState::new(&AppConfig::default()).expect("state should initialize");
        let guild_id = String::from("guild-owner-count-membership-store");
        let owner_role_id = String::from("owner-role");
        let owner_user = UserId::new();

        state.membership_store.guild_roles().write().await.insert(
            guild_id.clone(),
            HashMap::from([(
                owner_role_id.clone(),
                WorkspaceRoleRecord {
                    role_id: owner_role_id.clone(),
                    name: String::from("Owner"),
                    position: 10,
                    is_system: true,
                    system_key: Some(String::from("workspace_owner")),
                    permissions_allow: filament_core::PermissionSet::empty(),
                    color_hex: None,
                    created_at_unix: 1,
                },
            )]),
        );
        state
            .membership_store
            .guild_role_assignments()
            .write()
            .await
            .insert(
                guild_id.clone(),
                HashMap::from([(owner_user, std::collections::HashSet::from([owner_role_id]))]),
            );

        let count = workspace_owner_count_in_memory(&state, &guild_id)
            .await
            .expect("workspace owner count should resolve");
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn assign_default_join_role_in_memory_applies_non_system_default_role() {
        let state = AppState::new(&AppConfig::default()).expect("state should initialize");
        let guild_id = String::from("guild-default-join-role-in-memory");
        let role_id = String::from("custom-default-role");
        let user_id = UserId::new();

        state.membership_store.guilds().write().await.insert(
            guild_id.clone(),
            GuildRecord {
                name: String::from("default-role-test"),
                visibility: GuildVisibility::Private,
                created_by_user_id: user_id,
                default_join_role_id: Some(role_id.clone()),
                members: HashMap::from([(user_id, Role::Member)]),
                banned_members: HashSet::new(),
                channels: HashMap::new(),
            },
        );
        state.membership_store.guild_roles().write().await.insert(
            guild_id.clone(),
            HashMap::from([(
                role_id.clone(),
                WorkspaceRoleRecord {
                    role_id: role_id.clone(),
                    name: String::from("Default"),
                    position: 1,
                    is_system: false,
                    system_key: None,
                    permissions_allow: filament_core::PermissionSet::empty(),
                    color_hex: None,
                    created_at_unix: 1,
                },
            )]),
        );

        assign_default_join_role_in_memory(&state, &guild_id, user_id)
            .await
            .expect("default role assignment should succeed");

        let assignments = state.membership_store.guild_role_assignments().read().await;
        let assigned = assignments
            .get(&guild_id)
            .and_then(|guild| guild.get(&user_id))
            .cloned()
            .unwrap_or_default();
        assert!(assigned.contains(&role_id));
    }

    #[test]
    fn role_color_hex_validation_normalizes_and_rejects_invalid_values() {
        assert_eq!(
            validate_role_color_hex_input(" #a1b2c3 ").expect("hex color should normalize"),
            "#A1B2C3"
        );
        assert!(validate_role_color_hex_input("#12345").is_err());
        assert!(validate_role_color_hex_input("#12Z45G").is_err());
    }
}
