#![forbid(unsafe_code)]

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

include!("server/core.rs");
include!("server/db.rs");
include!("server/metrics.rs");
include!("server/router.rs");
include!("server/types.rs");
include!("server/handlers.rs");
include!("server/realtime.rs");
include!("server/domain.rs");
include!("server/auth.rs");
include!("server/errors.rs");
include!("server/tests.rs");
