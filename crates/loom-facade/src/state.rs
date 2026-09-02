//! `AppState` — the composition-root bundle: one `Arc<dyn Port>` per hexagonal
//! port, plus the `Config`, the injection `InjectionPolicy` (the gate's
//! authority, read once from env) and the rolling confidence window `/health`
//! reports. Cloneable (an `Arc` inside), so axum can hand a cheap copy to every
//! handler.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use loom_domain::{
    EmbeddingProvider, GenerationStore, GraphStore, InjectionDecision, LexicalIndex, ModelBackend,
    VectorIndex,
};
use loom_scaffold::policy::InjectionPolicy;

use crate::config::Config;

/// How many of the most recent grounded requests the `/health` confidence block
/// summarises. A fixed window keeps the block O(1) in memory and makes
/// `mean_confidence` a *recent* signal rather than a since-boot average that a
/// long-lived node can never move.
pub const CONFIDENCE_WINDOW: usize = 1000;

/// The rolling record of the last [`CONFIDENCE_WINDOW`] injection decisions, one
/// `(decision, confidence)` pair per `/loom/scaffold` and `/v1/chat/completions`
/// request.
///
/// Telemetry must never break a request, so every operation degrades rather than
/// propagates: a poisoned lock (a panic while another handler held it) makes
/// `record` a no-op and `snapshot` return zeros. One uncontended `Mutex` touch
/// per request is far below the cost of the retrieval it measures.
#[derive(Debug, Default)]
pub struct ConfidenceWindow {
    inner: Mutex<VecDeque<(InjectionDecision, f64)>>,
}

impl ConfidenceWindow {
    /// Record one settled decision. Confidences are clamped into `[0, 1]` on the
    /// way in, so `mean_confidence` cannot leave the unit interval whatever a
    /// future scale hands us.
    pub fn record(&self, decision: InjectionDecision, confidence: f64) {
        let Ok(mut q) = self.inner.lock() else {
            return; // poisoned — drop the sample, never fail the request
        };
        if q.len() >= CONFIDENCE_WINDOW {
            q.pop_front();
        }
        let c = if confidence.is_finite() {
            confidence.clamp(0.0, 1.0)
        } else {
            0.0
        };
        q.push_back((decision, c));
    }

    /// Summarise the window for `/health`. Zeros (with the window size still
    /// reported, it being a constant rather than a measurement) on a poisoned
    /// lock.
    #[must_use]
    pub fn snapshot(&self) -> ConfidenceStats {
        let base = ConfidenceStats {
            window: CONFIDENCE_WINDOW,
            ..ConfidenceStats::default()
        };
        let Ok(q) = self.inner.lock() else {
            return base;
        };
        let mut stats = base;
        let mut total = 0.0_f64;
        for (decision, confidence) in q.iter() {
            stats.requests += 1;
            total += *confidence;
            match decision {
                InjectionDecision::Skipped => stats.skipped += 1,
                InjectionDecision::Scaled => stats.scaled += 1,
                InjectionDecision::Full => stats.full += 1,
                InjectionDecision::Verbatim => stats.verbatim += 1,
            }
        }
        stats.engaged = stats.requests - stats.skipped;
        if stats.requests > 0 {
            #[allow(clippy::cast_precision_loss)]
            {
                stats.mean_confidence = total / stats.requests as f64;
            }
        }
        stats
    }
}

/// The `/health` `confidence` block: how the gate has been deciding lately.
/// `requests` is the number of samples currently in the window (≤ `window`),
/// `engaged` the non-skipped ones, and the four decision counters partition
/// `requests` exactly.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConfidenceStats {
    pub window: usize,
    pub requests: usize,
    pub engaged: usize,
    pub skipped: usize,
    pub scaled: usize,
    pub full: usize,
    pub verbatim: usize,
    pub mean_confidence: f64,
}

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
    /// Rolling per-request gate telemetry, summarised by `/health`.
    pub confidence: ConfidenceWindow,
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
            confidence: ConfidenceWindow::default(),
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
