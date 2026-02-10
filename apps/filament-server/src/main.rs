#![forbid(unsafe_code)]

use std::net::SocketAddr;

use filament_server::{build_router, init_tracing, AppConfig};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let app_config = AppConfig::default();
    let app = build_router(&app_config)?;
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "filament-server listening");

    axum::serve(listener, app).await?;
    Ok(())
}
