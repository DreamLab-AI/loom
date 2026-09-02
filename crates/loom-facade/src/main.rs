//! loom-facade binary — the thin composition-root over the library: build the
//! `AppState` from env, layer the router, bind `LOOM_FACADE_PORT` (default 8080;
//! Profile A DNATs `:8084` to this), serve. All wiring lives in the library so
//! the router is oneshot-testable in-memory.

use std::net::SocketAddr;

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // env-filter tracing; default to `info` when RUST_LOG is unset.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let state = loom_facade::app_state_from_env();
    let port = state.config.facade_port;
    let profile = state.config.deploy_profile.clone();
    let app = loom_facade::build_router(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, profile = %profile, "loom-facade listening");
    axum::serve(listener, app).await?;
    Ok(())
}
