#![forbid(unsafe_code)]

use std::net::SocketAddr;
use std::path::PathBuf;

use filament_server::{build_router, init_tracing, AppConfig};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let database_url = std::env::var("FILAMENT_DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("FILAMENT_DATABASE_URL is required for runtime"))?;
    let livekit_api_key = std::env::var("FILAMENT_LIVEKIT_API_KEY")
        .map_err(|_| anyhow::anyhow!("FILAMENT_LIVEKIT_API_KEY is required for runtime"))?;
    let livekit_api_secret = std::env::var("FILAMENT_LIVEKIT_API_SECRET")
        .map_err(|_| anyhow::anyhow!("FILAMENT_LIVEKIT_API_SECRET is required for runtime"))?;
    let max_created_guilds_per_user = std::env::var("FILAMENT_MAX_CREATED_GUILDS_PER_USER")
        .map_or_else(
            |_| Ok(AppConfig::default().max_created_guilds_per_user),
            |value| {
                value.parse::<usize>().map_err(|e| {
                    anyhow::anyhow!(
                        "invalid FILAMENT_MAX_CREATED_GUILDS_PER_USER value {value:?}: {e}"
                    )
                })
            },
        )?;
    let app_config = AppConfig {
        attachment_root: std::env::var("FILAMENT_ATTACHMENT_ROOT")
            .map_or_else(|_| PathBuf::from("./data/attachments"), PathBuf::from),
        livekit_url: std::env::var("FILAMENT_LIVEKIT_URL")
            .unwrap_or_else(|_| String::from("ws://127.0.0.1:7880")),
        livekit_api_key: Some(livekit_api_key),
        livekit_api_secret: Some(livekit_api_secret),
        max_created_guilds_per_user,
        database_url: Some(database_url),
        ..AppConfig::default()
    };
    let app = build_router(&app_config)?;
    let addr = std::env::var("FILAMENT_BIND_ADDR")
        .unwrap_or_else(|_| String::from("0.0.0.0:3000"))
        .parse::<SocketAddr>()
        .map_err(|e| anyhow::anyhow!("invalid FILAMENT_BIND_ADDR: {e}"))?;
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "filament-server listening");

    axum::serve(listener, app).await?;
    Ok(())
}
