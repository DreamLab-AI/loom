# Bench harness — legacy-Python note (2026-08-17, deleted 2026-09-03)

## What was here, and where it went

`bench_ontology_uplift.py`, `bench-uplift.py` and `bench-agentic-uplift.py`
imported `app/ontology_scaffold.py`, which was retired with the Python serving
code (RUST-ARCHITECTURE §12 deprecation map). They could not execute, so on
**2026-09-03 they were deleted** rather than left as a broken import. Deleted
alongside them:

| Deleted | Why |
|---|---|
| `bench/bench_ontology_uplift.py`, `bench/bench-uplift.py`, `bench/bench-agentic-uplift.py` | Broken import of the retired `app/ontology_scaffold.py`. |
| `bench/run-all-tests.sh` | Five of its six suites invoked files that no longer exist (`app/ontology_scaffold.py`, `app/test_proxy.py`, `tests/test_confidence_injection.py`, `app/pipeline/tests`). Superseded by `just ci`. |
| `legacy/scripts/*.py` (8 drivers) + their `run-*.sh` | Model benchmarks for retired models; llama.cpp builds they referenced are gone. |
| `legacy/diffusiongemma-docker/` | DiffusionGemma retired at the Qwen3.8 cutover. |

**The published results are untouched.** The preprint's data stays frozen in
`uplift-results/`, and the raw logs in `docs/research/evidence/`. To re-run the
Python harness, check out the pre-retirement tree:

```bash
git checkout eb678a0 -- app/ontology_scaffold.py     # the retired module
git checkout 9ce2023 -- bench/bench_ontology_uplift.py   # the harness, at its last commit
```

## Where the behaviour lives now (Rust)

| Behaviour | Rust home |
|---|---|
| Lexical matcher + confidence-gated injection policy | `crates/loom-scaffold` — byte-parity goldens (EXP-002), gate math (EXP-003) |
| `match()` p99 < 50 ms on the 8,146-class index | `crates/loom-scaffold` criterion bench (EXP-010) |
| Exposure decomposition (`tools/paper/decompose_exposure.py`) | `crates/loom-scaffold/src/exposure.rs` |
| `/health` graph / generation / confidence assertions | `crates/loom-facade/src/bin/{graph_check,generation_check,confidence_check}.rs` |
| CONCEPT-INDEX record build (`tools/ingest/build_concept_records.py`) | `crates/loom-scaffold/src/bin/build_concept_records.rs` |
| Embed + stage the corpus (`tools/ingest/embed_and_stage.py`) | `crates/loom-vector-ruvector/src/bin/stage_corpus.rs` (`--features pg-write`) |

`tests/test_confidence_injection.py` was deleted earlier for the same reason —
the Rust policy table tests supersede it.

## Kept on purpose

`bench/quality/*.py` and `bench/sweep/analyse.py` are frozen research artefacts
behind the preprint and are **not** ported. `bench/run-gemini.sh`,
`bench/sweep/run-one-model.sh` and `bench/sweep/launch-sweep.sh` are cited by
the paper (`docs/research/latex/report.tex`) as the reproduction drivers; they
are kept as provenance and carry a header noting the harness they drove is gone.
