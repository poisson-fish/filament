use axum::{
    extract::{connect_info::ConnectInfo, Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use filament_core::Permission;
use std::net::SocketAddr;

use crate::server::{
    auth::{authenticate, extract_client_ip},
    core::{AppState, SearchOperation, DEFAULT_SEARCH_RESULT_LIMIT, MAX_SEARCH_RECONCILE_DOCS},
    domain::{enforce_guild_ip_ban_for_request, guild_permission_snapshot},
    errors::AuthFailure,
    realtime::{
        collect_all_indexed_messages, enqueue_search_operation, ensure_search_bootstrapped,
        hydrate_messages_by_id, plan_search_reconciliation, run_search_query,
        validate_search_query,
    },
    types::{GuildPath, SearchQuery, SearchReconcileResponse, SearchResponse},
};

#[allow(clippy::too_many_lines)]
pub(crate) async fn search_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(path): Path<GuildPath>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, AuthFailure> {
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
        "search.messages",
    )
    .await?;
    let (_, permissions) = guild_permission_snapshot(&state, auth.user_id, &path.guild_id).await?;
    if !permissions.contains(Permission::CreateMessage) {
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

pub(crate) async fn rebuild_search_index(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(path): Path<GuildPath>,
) -> Result<StatusCode, AuthFailure> {
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
        "search.rebuild",
    )
    .await?;
    let (_, permissions) = guild_permission_snapshot(&state, auth.user_id, &path.guild_id).await?;
    if !permissions.contains(Permission::ManageWorkspaceRoles) {
        return Err(AuthFailure::Forbidden);
    }

    let docs = collect_all_indexed_messages(&state).await?;
    enqueue_search_operation(&state, SearchOperation::Rebuild { docs }, true).await?;
    state.search_bootstrapped.set(()).ok();
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn reconcile_search_index(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(path): Path<GuildPath>,
) -> Result<Json<SearchReconcileResponse>, AuthFailure> {
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
        "search.reconcile",
    )
    .await?;
    let (_, permissions) = guild_permission_snapshot(&state, auth.user_id, &path.guild_id).await?;
    if !permissions.contains(Permission::ManageWorkspaceRoles) {
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
