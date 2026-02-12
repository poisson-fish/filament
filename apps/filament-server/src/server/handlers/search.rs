use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use filament_core::{has_permission, Permission, Role};

use crate::server::{
    auth::authenticate,
    core::{AppState, SearchOperation, DEFAULT_SEARCH_RESULT_LIMIT, MAX_SEARCH_RECONCILE_DOCS},
    db::ensure_db_schema,
    domain::user_role_in_guild,
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
    Path(path): Path<GuildPath>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    let role = user_role_in_guild(&state, auth.user_id, &path.guild_id).await?;
    if !has_permission(role, Permission::CreateMessage) {
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
    Path(path): Path<GuildPath>,
) -> Result<StatusCode, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    let role = user_role_in_guild(&state, auth.user_id, &path.guild_id).await?;
    if !matches!(role, Role::Owner | Role::Moderator) {
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
    Path(path): Path<GuildPath>,
) -> Result<Json<SearchReconcileResponse>, AuthFailure> {
    ensure_db_schema(&state).await?;
    let auth = authenticate(&state, &headers).await?;
    let role = user_role_in_guild(&state, auth.user_id, &path.guild_id).await?;
    if !matches!(role, Role::Owner | Role::Moderator) {
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
