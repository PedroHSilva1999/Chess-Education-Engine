use std::{net::SocketAddr, str::FromStr};

use anyhow::{Context, Result};
use chess_api::{AppState, application_service, grpc, router};
use tokio::net::TcpListener;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let http_address = http_address()?;
    let grpc_address = address_from_env("GRPC_ADDR", "0.0.0.0:50051")?;
    let service = application_service();
    let app = router(AppState {
        service: service.clone(),
    });
    let listener = TcpListener::bind(http_address)
        .await
        .with_context(|| format!("failed to bind HTTP server to {http_address}"))?;
    info!(%http_address, %grpc_address, "chess education engine started");

    tokio::select! {
        result = axum::serve(listener, app) => {
            result.context("HTTP server stopped unexpectedly")?;
        }
        result = grpc::serve(grpc_address, service) => {
            result.context("gRPC server stopped unexpectedly")?;
        }
        result = tokio::signal::ctrl_c() => {
            if let Err(error) = result {
                error!(%error, "failed to listen for shutdown signal");
            }
            info!("shutdown signal received");
        }
    }
    Ok(())
}

fn http_address() -> Result<SocketAddr> {
    if let Ok(addr) = std::env::var("HTTP_ADDR") {
        let addr = addr.trim();
        if !addr.is_empty() {
            return SocketAddr::from_str(addr).context("invalid HTTP_ADDR");
        }
    }
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_owned());
    SocketAddr::from_str(&format!("0.0.0.0:{port}")).context("invalid PORT")
}

fn address_from_env(name: &str, fallback: &str) -> Result<SocketAddr> {
    SocketAddr::from_str(&std::env::var(name).unwrap_or_else(|_| fallback.to_owned()))
        .with_context(|| format!("invalid {name}"))
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .json()
        .with_current_span(true)
        .init();
}
