//! loom-facade — the composition root (RUST-ARCHITECTURE §9/§10). Binds one
//! concrete adapter per hexagonal port, sequences the fusion pipeline (§6), and
//! serves the axum router. This library target exists so the router can be
//! exercised in-memory (`tower::ServiceExt::oneshot`) with a fixture `AppState`;
//! `main.rs` is the thin binary over it.

// Port errors are documented on the domain traits (§4); prose mentions env-var
// and Python identifiers verbatim, which would otherwise trip `doc_markdown`.
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::doc_markdown)]

pub mod build_info;
pub mod bundle;
pub mod config;
pub mod error;
pub mod fallback;
pub mod fusion;
pub mod jev;
pub mod mcp_host;
pub mod mirror;
pub mod promote;
pub mod reload;
pub mod routes;
pub mod serving;
pub mod state;

use std::sync::Arc;

pub use build_info::{BuildInfo, BuildReceipt, EffectiveConfig};
pub use bundle::LoadedBundle;
pub use config::Config;
pub use routes::build_router;
pub use state::AppState;

use loom_domain::{BundleError, GraphStore, LexicalIndex, VectorIndex};
use loom_scaffold::policy::InjectionPolicy;
use loom_scaffold::LexicalRetriever;

use loom_backend_openai::OpenAiBackend;
use loom_embed_xinference::XinferenceEmbedder;
use loom_graph_oxigraph::OxigraphStore;
use loom_vector_ruvector::HnswIndex;

/// An empty scaffold index — the fail-open floor when `ONTOLOGY_INDEX` cannot be
/// loaded. Every query then returns no seeds (a NoMatch scaffold), so the node
/// still answers `/health` and delegates chat, degraded and honest.
const EMPTY_INDEX_JSON: &str = r#"{"version":1,"generated":"","classes":{}}"#;

/// Build the full `AppState` from the process environment — the composition
/// root. Each adapter loads fail-open with a `tracing` status line; the
/// embedder's startup `verify()` is spawned as a non-fatal probe (warn-only), so
/// a cold/absent Xinference never blocks the bind.
///
/// ONE step is deliberately NOT fail-open: bundle activation (ADR-135
/// closeout). The accelerators degrade because an answer without them is still
/// an honest answer; a bundle that failed verification is different in kind — it
/// would make every generation this node reports a claim it cannot support. So
/// activation is ordered FIRST and its failure is returned, not logged.
///
/// # Errors
/// [`loom_domain::BundleError`] when the data directory holds a mixed,
/// incomplete or mid-promotion artefact set.
///
/// # Panics
/// Never — the empty-index fallback guarantees a usable lexical floor, and every
/// other adapter is fail-open.
pub fn try_app_state_from_env() -> Result<AppState, loom_domain::BundleError> {
    try_app_state(Config::from_env())
}

/// Activate the data directory's bundle; if that fails and a retained
/// generation validates, activate that instead and say so. Returns the config
/// the rest of start-up must use (rebased onto the fallback directory when one
/// is served).
fn activate_or_fall_back(config: Config) -> Result<(Config, LoadedBundle), BundleError> {
    let disk = mirror::MirrorStore::new(&config.index_path);
    // A marker file that is not valid JSON counts as "no marker" to the store,
    // which would start unattested over a damaged directory. Treat it as the
    // failure it is, but keep the old degraded start when nothing can replace it.
    let garbled = disk.commit_marker_path().is_file() && !disk.has_commit_marker();
    let failure = if garbled {
        BundleError::MarkerUnreadable {
            dir: disk.data_dir().display().to_string(),
            detail: format!("{} is not valid JSON", disk.commit_marker_path().display()),
        }
    } else {
        match LoadedBundle::activate_or_degraded(&config.index_path) {
            Ok(bundle) => return Ok((config, bundle)),
            Err(failure) => failure,
        }
    };
    let failed = {
        disk.has_commit_marker().then(|| {
            <mirror::MirrorStore as loom_domain::GenerationStore>::current(&disk)
                .id
                .0
        })
    };
    let Some(selected) = fallback::select(&config, failed.as_deref()) else {
        if garbled {
            let bundle = LoadedBundle::activate_or_degraded(&config.index_path)?;
            return Ok((config, bundle));
        }
        return Err(failure);
    };
    let rebased = fallback::rebased(&config, &selected.dir);
    let info = fallback::FallbackInfo {
        failed_generation: failed,
        reason: failure.to_string(),
        serving_generation: selected.generation,
        serving_dir: selected.dir.display().to_string(),
    };
    tracing::warn!(
        failed = ?info.failed_generation,
        reason = %info.reason,
        serving = %info.serving_generation,
        dir = %info.serving_dir,
        data = %config.index_path,
        "SERVING A FALLBACK GENERATION: the current generation failed to activate; \
         the failed bundle is left untouched for inspection"
    );
    // A fallback that does not itself activate strictly is no fallback: report
    // the original failure, not a second one.
    match LoadedBundle::activate_fallback(&rebased.index_path, &config.index_path, info) {
        Ok(bundle) => Ok((rebased, bundle)),
        Err(_) if garbled => {
            let bundle = LoadedBundle::activate_or_degraded(&config.index_path)?;
            Ok((config, bundle))
        }
        Err(_) => Err(failure),
    }
}

/// The lexical adapter reads the prose index from `$ONTOLOGY_PROSE_INDEX`, not
/// from the configured path, so over a fallback it would attach the failed
/// bundle's prose to the retained bundle's index. Attach the right one.
fn with_bundle_prose(
    retriever: LexicalRetriever,
    bundle: &LoadedBundle,
    config: &Config,
) -> LexicalRetriever {
    if bundle.fallback().is_none() {
        return retriever;
    }
    retriever.with_prose(loom_scaffold::prose::load_prose(std::path::Path::new(
        &config.prose_path,
    )))
}

/// [`try_app_state_from_env`] over an explicit [`Config`].
///
/// # Errors
/// As [`try_app_state_from_env`].
///
/// # Panics
/// Never — see [`try_app_state_from_env`].
pub fn try_app_state(config: Config) -> Result<AppState, loom_domain::BundleError> {
    // --- serving identity (FIRST, and fatal on failure) --------------------
    // Verify before loading anything, so no content is read from a set that is
    // about to be rejected. `verify_atomicity` runs here, on the serving path,
    // rather than existing only as a method nothing calls. The one softening:
    // a retained previous generation that passes promotion's checks is served
    // instead, and reported as a fallback everywhere.
    let (config, bundle) = activate_or_fall_back(config)?;
    {
        let id = bundle.identity();
        tracing::info!(
            generation = %id.generation.id.0,
            content_digest = %id.content_digest,
            artefacts = id.artefacts.len(),
            attested = id.atomicity_verified,
            "serving bundle activated"
        );
    }

    // --- lexical (the hard floor) — load index + prose, else an empty floor.
    let retriever = match LexicalRetriever::load(Some(&config.index_path)) {
        Ok(r) => {
            tracing::info!(
                classes = r.class_count(),
                path = %config.index_path,
                "lexical index loaded"
            );
            r
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                path = %config.index_path,
                "lexical index NOT loaded; serving on an empty floor (degraded)"
            );
            LexicalRetriever::from_json_str(EMPTY_INDEX_JSON).expect("static empty index parses")
        }
    };

    let retriever = with_bundle_prose(retriever, &bundle, &config);

    // --- graph (accelerator) — fail-open; report availability.
    let graph = OxigraphStore::load(config.data_dir());
    {
        let s = graph.status();
        if s.available {
            tracing::info!(triples = s.triples, files = ?s.loaded_files, "graph store loaded");
        } else {
            tracing::warn!(error = ?s.error, "graph store DISABLED (fail-open → lexical)");
        }
    }

    // --- semantic (accelerator, gated) — fail-open; report QUALIFICATION.
    // Readiness now means "passed the artefact contract", not "opened": a
    // Euclidean or wrong-width artefact is rejected here rather than silently
    // relabelled at query time (ADR-137 closeout).
    let semantic =
        HnswIndex::open_with_contract(&config.hnsw_artifact, &HnswIndex::contract_from_env());
    if semantic.is_ready() {
        let q = semantic.qualification();
        tracing::info!(
            artifact = %config.hnsw_artifact,
            enabled = config.semantic_fallback,
            dimensions = q.dimensions,
            metric = q.metric.as_str(),
            model = ?q.model_id,
            "HNSW artifact qualified and ready"
        );
    } else {
        tracing::warn!(
            artifact = %config.hnsw_artifact,
            reasons = ?semantic.qualification().reasons(),
            "HNSW artifact NOT qualified (semantic fallback unavailable — fail-open)"
        );
    }

    // --- embedder — non-fatal startup probe (spawned, never blocks the bind).
    let embedder = XinferenceEmbedder::from_env();
    {
        let probe = XinferenceEmbedder::from_env();
        tokio::spawn(async move {
            match probe.verify().await {
                Ok(()) => tracing::info!("embedder verified (bge-small-en-v1.5/384)"),
                Err(e) => tracing::warn!(error = %e, "embedder verify failed (non-fatal)"),
            }
        });
    }

    // --- backend (the model-swap seam).
    let backend = OpenAiBackend::from_env();
    if config.backend_url.is_empty() {
        tracing::info!("no DISTILL_BACKEND_URL — retrieval-only node");
    } else {
        tracing::info!(endpoint = %config.backend_url, "backend seam configured");
    }

    // A publication racing adapter loading must not bind a port with an old
    // identity over newly loaded bytes. The serving objects are immutable after
    // this point; future disk changes are detected by the generation endpoint.
    if bundle.identity().atomicity_verified {
        bundle.verify_loaded()?;
        // A fallback differs from the data directory by definition.
        if bundle.fallback().is_none() && !bundle.disk_matches_loaded() {
            return Err(loom_domain::BundleError::ActivatedDrift {
                name: ".generation.json".to_owned(),
            });
        }
    }

    let policy = InjectionPolicy::from_env();

    // One retriever, two ports: it is both the lexical index and the authority
    // on what the corpus contains, and those must never disagree.
    let retriever = Arc::new(retriever);

    Ok(AppState::new(
        retriever.clone(),
        retriever,
        Arc::new(semantic),
        Arc::new(graph),
        Arc::new(embedder),
        Arc::new(backend),
        Arc::new(bundle),
        policy,
        config,
    ))
}

/// [`try_app_state_from_env`], panicking on an unservable bundle.
///
/// Retained for call sites that legitimately cannot proceed — the binary's own
/// startup — where a failed activation must stop the process rather than bind a
/// port over content that did not verify.
///
/// # Panics
/// When bundle activation fails. Every other adapter is fail-open.
#[must_use]
pub fn app_state_from_env() -> AppState {
    match try_app_state_from_env() {
        Ok(s) => s,
        Err(e) => panic!("refusing to serve: {e}"),
    }
}
