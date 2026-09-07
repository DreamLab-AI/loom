# Dream-cycle ADR candidates

Architectural decisions the nightly cycle has surfaced but which are **not** taken here.
Each entry is a title and the case for opening it. Numbering is the operator's: these
carry no ADR id until one is assigned.

---

## Proposed: Evaluation self-containment — replace the `ruvector` sibling path dependency with a rev-pinned git dependency or a vendored `ruvector-core`

**Raised by** the 2026-09-03 → 2026-09-06 nights (deeps `graph-engine`, `scaffold-retrieval`,
`model-facade`, `confidence-injection`), all four INCONCLUSIVE for one environmental reason.

**Context.** `Cargo.toml` declares `ruvector-core = { path = "../ruvector/crates/ruvector-core", … }`.
`crates/loom-vector-ruvector` consumes it and `crates/loom-facade` pulls that in, so the entire
workspace — including the facade's own tests — resolves through a checkout that lives *outside this
repository*. When the sibling is absent, cargo fails at **manifest load**, before a single crate is
compiled, and all five sanctioned evaluators go dark at once. The failure never reproduces for a
human at a keyboard, because `../ruvector` exists in a normal working checkout.

The blackout is mitigated as of 2026-09-07 — `dream.config.json` ships the sibling to the annexe via
`annexeInclude`, and a control-plane probe plus a build-step fence name its absence in phase 0 rather
than leaving it to be rediscovered from a stack trace in phase 3. That is a fence around the hazard,
not its removal: the dependency graph still reaches outside the repository, so any consumer that is
not the dream annexe (a fresh clone, CI, a contributor) meets the same wall.

**Options.**

1. **Rev-pinned git dependency** — `ruvector-core = { git = "…", rev = "…", … }`. Self-contained and
   reproducible; costs a pin to advance deliberately, and loses the edit-sibling-and-rebuild loop.
2. **Vendor `ruvector-core`** into this repository. Maximally self-contained; incurs a real
   maintenance duty to track upstream.
3. **Status quo plus the fence.** Cheapest, and honest about the hazard, but leaves the workspace
   unbuildable outside a prepared environment.

**Why it needs a decision rather than a patch.** It changes the dependency graph and the MSRV/feature
surface: `Cargo.toml` documents MSRV 1.89 driven by `ruvector-core`'s AVX-512 `simd` feature, and the
explicit non-default feature set (`hnsw`, `storage`, `simd`, `parallel`) exists to keep the
`api-embeddings` default off, since it drags in reqwest 0.11 → rustls 0.21 and three RUSTSEC
advisories that `cargo-deny` gates. Any of these options has to preserve that.

**Status.** Proposed. Not implemented.
