use axum::{http::StatusCode, response::IntoResponse, Json};

use super::{
    directory_contract::{
        AUDIT_ACCESS_DENIED_ERROR, DIRECTORY_JOIN_IP_BANNED_ERROR, DIRECTORY_JOIN_USER_BANNED_ERROR,
    },
    metrics::{record_auth_failure, record_rate_limit_hit},
    types::AuthError,
};

#[derive(Debug)]
pub(crate) enum AuthFailure {
    InvalidRequest,
    CaptchaFailed,
    Unauthorized,
    Forbidden,
    AuditAccessDenied,
    DirectoryJoinUserBanned,
    DirectoryJoinIpBanned,
    GuildCreationLimitReached,
    NotFound,
    RateLimited,
    PayloadTooLarge,
    QuotaExceeded,
    Internal,
}

impl std::fmt::Display for AuthFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl IntoResponse for AuthFailure {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::Unauthorized => record_auth_failure("unauthorized"),
            Self::Forbidden
            | Self::AuditAccessDenied
            | Self::DirectoryJoinUserBanned
            | Self::DirectoryJoinIpBanned => {
                record_auth_failure("forbidden");
            }
            Self::RateLimited => record_rate_limit_hit("http", "auth_failure"),
            Self::InvalidRequest
            | Self::CaptchaFailed
            | Self::GuildCreationLimitReached
            | Self::NotFound
            | Self::PayloadTooLarge
            | Self::QuotaExceeded
            | Self::Internal => {}
        }

        match self {
            Self::InvalidRequest => (
                StatusCode::BAD_REQUEST,
                Json(AuthError {
                    error: "invalid_request",
                }),
            )
                .into_response(),
            Self::CaptchaFailed => (
                StatusCode::FORBIDDEN,
                Json(AuthError {
                    error: "captcha_failed",
                }),
            )
                .into_response(),
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                Json(AuthError {
                    error: "invalid_credentials",
                }),
            )
                .into_response(),
            Self::Forbidden => (
                StatusCode::FORBIDDEN,
                Json(AuthError { error: "forbidden" }),
            )
                .into_response(),
            Self::AuditAccessDenied => (
                StatusCode::FORBIDDEN,
                Json(AuthError {
                    error: AUDIT_ACCESS_DENIED_ERROR,
                }),
            )
                .into_response(),
            Self::DirectoryJoinUserBanned => (
                StatusCode::FORBIDDEN,
                Json(AuthError {
                    error: DIRECTORY_JOIN_USER_BANNED_ERROR,
                }),
            )
                .into_response(),
            Self::DirectoryJoinIpBanned => (
                StatusCode::FORBIDDEN,
                Json(AuthError {
                    error: DIRECTORY_JOIN_IP_BANNED_ERROR,
                }),
            )
                .into_response(),
            Self::GuildCreationLimitReached => (
                StatusCode::FORBIDDEN,
                Json(AuthError {
                    error: "guild_creation_limit_reached",
                }),
            )
                .into_response(),
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                Json(AuthError { error: "not_found" }),
            )
                .into_response(),
            Self::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                Json(AuthError {
                    error: "rate_limited",
                }),
            )
                .into_response(),
            Self::PayloadTooLarge => (
                StatusCode::PAYLOAD_TOO_LARGE,
                Json(AuthError {
                    error: "payload_too_large",
                }),
            )
                .into_response(),
            Self::QuotaExceeded => (
                StatusCode::CONFLICT,
                Json(AuthError {
                    error: "quota_exceeded",
                }),
            )
                .into_response(),
            Self::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthError {
                    error: "internal_error",
                }),
            )
                .into_response(),
        }
    }
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
