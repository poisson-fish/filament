use std::{
    collections::{HashMap, HashSet},
    fmt::Write as _,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::anyhow;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::Json,
    http::{header::AUTHORIZATION, header::CONTENT_TYPE, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use filament_core::{
    apply_channel_overwrite, base_permissions, can_assign_role, can_moderate_member,
    has_permission, tokenize_markdown, ChannelKind, ChannelName, ChannelPermissionOverwrite,
    GuildName, LiveKitIdentity, LiveKitRoomName, MarkdownToken, Permission, PermissionSet, Role,
    UserId, Username,
};
use filament_protocol::{Envelope, EventType, PROTOCOL_VERSION};
use object_store::local::LocalFileSystem;
use pasetors::{
    claims::{Claims, ClaimsValidationRules},
    keys::SymmetricKey,
    local,
    token::UntrustedToken,
    version4::V4,
    Local,
};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use tantivy::schema::Field;
use tokio::sync::{mpsc, oneshot, watch, OnceCell, RwLock};
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
