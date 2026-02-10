#![forbid(unsafe_code)]

use std::{sync::Arc, time::Duration};

use anyhow::anyhow;
use axum::{extract::DefaultBodyLimit, routing::get, Json, Router};
use http::{HeaderName, StatusCode};
use serde::{Deserialize, Serialize};
use tower::ServiceBuilder;
use tower_governor::{
    governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor, GovernorLayer,
};
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};

pub const DEFAULT_JSON_BODY_LIMIT_BYTES: usize = 1_048_576;
pub const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 10;
pub const DEFAULT_RATE_LIMIT_REQUESTS_PER_MINUTE: u32 = 60;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub max_body_bytes: usize,
    pub request_timeout: Duration,
    pub rate_limit_requests_per_minute: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            max_body_bytes: DEFAULT_JSON_BODY_LIMIT_BYTES,
            request_timeout: Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS),
            rate_limit_requests_per_minute: DEFAULT_RATE_LIMIT_REQUESTS_PER_MINUTE,
        }
    }
}

/// Build the axum router with global security middleware.
///
/// # Errors
/// Returns an error if the configured governor rate-limiter settings are invalid.
pub fn build_router(config: &AppConfig) -> anyhow::Result<Router> {
    let governor_config = Arc::new(
        GovernorConfigBuilder::default()
            .period(Duration::from_secs(60))
            .burst_size(config.rate_limit_requests_per_minute)
            .key_extractor(SmartIpKeyExtractor)
            .finish()
            .ok_or_else(|| anyhow!("invalid governor configuration"))?,
    );

    let request_id_header = HeaderName::from_static("x-request-id");

    Ok(Router::new()
        .route("/health", get(health))
        .route("/echo", axum::routing::post(echo))
        .route("/slow", get(slow))
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
                .layer(GovernorLayer::new(governor_config)),
        ))
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EchoRequest {
    message: String,
}

#[derive(Debug, Serialize)]
struct EchoResponse {
    message: String,
}

async fn echo(Json(payload): Json<EchoRequest>) -> Result<Json<EchoResponse>, StatusCode> {
    if payload.message.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    Ok(Json(EchoResponse {
        message: payload.message,
    }))
}

async fn slow() -> Json<HealthResponse> {
    tokio::time::sleep(Duration::from_millis(200)).await;
    Json(HealthResponse { status: "ok" })
}

pub fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .json()
        .with_env_filter(filter)
        .with_current_span(true)
        .with_span_list(true)
        .init();
}
