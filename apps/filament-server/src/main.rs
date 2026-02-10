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
    let app_config = AppConfig {
        attachment_root: std::env::var("FILAMENT_ATTACHMENT_ROOT")
            .map_or_else(|_| PathBuf::from("./data/attachments"), PathBuf::from),
        database_url: Some(database_url),
        ..AppConfig::default()
    };
    let app = build_router(&app_config)?;
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "filament-server listening");

    axum::serve(listener, app).await?;
    Ok(())
}
