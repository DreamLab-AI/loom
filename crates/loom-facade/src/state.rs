//! `AppState` — the composition-root bundle: one `Arc<dyn Port>` per hexagonal
//! port, plus the `Config` and the injection `InjectionPolicy` (the gate's
//! authority, read once from env). Cloneable (an `Arc` inside), so axum can hand
//! a cheap copy to every handler.

use std::sync::Arc;

use loom_domain::{
    EmbeddingProvider, GenerationStore, GraphStore, LexicalIndex, ModelBackend, VectorIndex,
};
use loom_scaffold::policy::InjectionPolicy;

use crate::config::Config;

/// The port bundle behind an `Arc` so `AppState` clones are pointer-cheap.
pub struct AppStateInner {
    pub retriever: Arc<dyn LexicalIndex>,
    pub semantic: Arc<dyn VectorIndex>,
    pub graph: Arc<dyn GraphStore>,
    pub embedder: Arc<dyn EmbeddingProvider>,
    pub backend: Arc<dyn ModelBackend>,
    pub generation: Arc<dyn GenerationStore>,
    /// The confidence gate's authority — its `min_inject_score` is the lexical
    /// hot-path short-circuit in the fusion pipeline (§6 step 2).
    pub policy: InjectionPolicy,
    pub config: Config,
}

/// Cheap-to-clone handle to the port bundle (axum extension state).
#[derive(Clone)]
pub struct AppState(pub Arc<AppStateInner>);

impl AppState {
    /// Assemble from already-constructed ports — the seam the router tests build
    /// their in-memory app through (fixture index, stub/absent accelerators).
    ///
    /// One argument per hexagonal port + config + policy; the arity IS the ring.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        retriever: Arc<dyn LexicalIndex>,
        semantic: Arc<dyn VectorIndex>,
        graph: Arc<dyn GraphStore>,
        embedder: Arc<dyn EmbeddingProvider>,
        backend: Arc<dyn ModelBackend>,
        generation: Arc<dyn GenerationStore>,
        policy: InjectionPolicy,
        config: Config,
    ) -> Self {
        Self(Arc::new(AppStateInner {
            retriever,
            semantic,
            graph,
            embedder,
            backend,
            generation,
            policy,
            config,
        }))
    }

    /// Whether the semantic fallback may run at all: the master switch AND a
    /// ready index (either alone is not enough — §6 step 3).
    #[must_use]
    pub fn semantic_fallback_enabled(&self) -> bool {
        self.0.config.semantic_fallback
    }
}

impl std::ops::Deref for AppState {
    type Target = AppStateInner;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
