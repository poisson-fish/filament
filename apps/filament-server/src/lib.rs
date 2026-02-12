#![forbid(unsafe_code)]

mod server;

pub use server::{build_router, init_tracing, AppConfig, MAX_LIVEKIT_TOKEN_TTL_SECS};
