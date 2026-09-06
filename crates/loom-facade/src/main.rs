//! loom-facade binary — the thin composition-root over the library: build the
//! `AppState` from env, layer the router, bind `LOOM_FACADE_PORT` (default 8080;
//! Profile A DNATs `:8084` to this), serve. All wiring lives in the library so
//! the router is oneshot-testable in-memory.

use std::net::SocketAddr;

use tracing_subscriber::EnvFilter;

/// `loom-facade --build-info` prints the release receipt and exits.
///
/// The receipt (ADR-137 closeout) is the artefact CI keeps: it binds the Loom
/// revision, the SIBLING `RuVector` revision and features, the compiler, and the
/// effective configuration this process resolved. A Loom commit sha alone does
/// not identify the build, because `ruvector-core` is a path dependency with no
/// lockfile revision — so a release that cannot print a complete receipt is a
/// release that cannot be reproduced, and this exits non-zero to say so.
fn print_build_info() -> anyhow::Result<std::process::ExitCode> {
    let build = loom_facade::BuildInfo::current();
    let receipt = build.with_effective_config(&loom_facade::Config::from_env());
    println!("{}", serde_json::to_string_pretty(&receipt)?);
    Ok(if build.source_identity_complete() {
        std::process::ExitCode::SUCCESS
    } else {
        eprintln!(
            "incomplete source identity: loom={:?} ruvector={:?}",
            build.loom_revision, build.ruvector_revision
        );
        std::process::ExitCode::FAILURE
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<std::process::ExitCode> {
    if std::env::args().any(|a| a == "--build-info") {
        return print_build_info();
    }

    // env-filter tracing; default to `info` when RUST_LOG is unset.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // Activation is the one fatal startup step: a bundle that did not verify
    // must stop the process, not bind a port over content it cannot vouch for.
    let state = loom_facade::try_app_state_from_env()?;
    let port = state.config.facade_port;
    let profile = state.config.deploy_profile.clone();
    let app = loom_facade::build_router(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, profile = %profile, "loom-facade listening");
    axum::serve(listener, app).await?;
    Ok(std::process::ExitCode::SUCCESS)
}
