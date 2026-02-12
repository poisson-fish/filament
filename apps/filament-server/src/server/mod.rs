use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt::Write as _,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::anyhow;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    body::Body,
    extract::{
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade},
        DefaultBodyLimit, Json, Path, Query, State,
    },
    http::{
        header::AUTHORIZATION, header::CONTENT_LENGTH, header::CONTENT_TYPE, HeaderMap, HeaderName,
        HeaderValue, StatusCode,
    },
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post},
    Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use filament_core::{
    apply_channel_overwrite, base_permissions, can_assign_role, can_moderate_member,
    has_permission, tokenize_markdown, ChannelKind, ChannelName, ChannelPermissionOverwrite,
    GuildName, LiveKitIdentity, LiveKitRoomName, MarkdownToken, Permission, PermissionSet, Role,
    UserId, Username,
};
use filament_protocol::{parse_envelope, Envelope, EventType, PROTOCOL_VERSION};
use futures_util::{SinkExt, StreamExt};
use livekit_api::access_token::{AccessToken as LiveKitAccessToken, VideoGrants};
use object_store::{local::LocalFileSystem, path::Path as ObjectPath, ObjectStore};
use pasetors::{
    claims::{Claims, ClaimsValidationRules},
    keys::SymmetricKey,
    local,
    token::UntrustedToken,
    version4::V4,
    Local,
};
use rand::{rngs::OsRng, RngCore};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use tantivy::{
    collector::{Count, TopDocs},
    query::{BooleanQuery, Occur, QueryParser, TermQuery},
    schema::{
        Field, IndexRecordOption, NumericOptions, Schema, TextFieldIndexing, TextOptions, Value,
        STORED, STRING,
    },
    TantivyDocument, Term,
};
use tokio::sync::{mpsc, oneshot, watch, OnceCell, RwLock};
use tower::ServiceBuilder;
use tower_governor::{
    governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor, GovernorLayer,
};
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use ulid::Ulid;
use uuid::Uuid;

pub(crate) mod auth;
pub(crate) mod core;
pub(crate) mod db;
pub(crate) mod domain;
pub(crate) mod errors;
pub(crate) mod handlers;
pub(crate) mod metrics;
pub(crate) mod realtime;
pub(crate) mod router;
#[cfg(test)]
mod tests;
pub(crate) mod types;

pub(crate) use auth::*;
pub(crate) use core::*;
pub use core::{AppConfig, MAX_LIVEKIT_TOKEN_TTL_SECS};
pub(crate) use db::*;
pub(crate) use domain::*;
pub use errors::init_tracing;
pub(crate) use errors::*;
pub(crate) use handlers::*;
pub(crate) use metrics::*;
pub(crate) use realtime::*;
pub use router::build_router;
pub(crate) use types::*;
