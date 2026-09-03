# `tools/ingest/` — the ontology-corpus → RuVector write channel

> **Ported to Rust, 2026-09-03.** `build_concept_records.py` and
> `embed_and_stage.py` are gone; the two halves are now workspace bins. This
> directory keeps the doc because the *discipline* below (the two ground-truth
> laws, the build/write split) is what matters, not the language.

The two bins here build and stage the **ontology-corpus** RuVector namespace: the
8,146 IRI-keyed `bge-small-en-v1.5`/384 records the (default-off, benchmark-gated)
HNSW semantic fallback searches. This is the **build/off-turn write channel** — it
is *never* on the query hot path. The serving Loom reads an in-process projection
of this namespace; it never writes it (ADR-136 §3, DDD §6.1,
`RUST-ARCHITECTURE.md` §11.2).

## The two halves (deliberately separated)

| Bin | Half | What it does | Purity |
|---|---|---|---|
| `build-concept-records`<br>(`crates/loom-scaffold`) | **build** | One record per ontology class — the seed-finding surface a semantic query matches against. The embedded text is the human-scrutible unit's *header*: title + definition + verbalised typed relations + taxonomy + the `dfull` summary, so a vector query lands on the right **IRI**. Reads the build-derived projections (`scaffold-index.json` + `prose-index.json`, themselves single-source per ADR-136 D4) and emits `concept-records.jsonl`. | **Pure + deterministic**: no network, no embedding, no infra write — testable and reviewable. |
| `stage_corpus`<br>(`crates/loom-vector-ruvector`, `--features pg-write`) | **write** | Embeds the records via Xinference (`bge-small-en-v1.5`/384, LOCKED) and stages idempotent upserts into `ruvector-postgres` namespace `ontology-corpus`. Emits a `.sql` file (streamed via `docker exec -i psql`); `ON CONFLICT (id)` makes it idempotent. | Impure (network + write), kept separate so the build half stays pure. |

The build half lives in `loom-scaffold` because that crate **owns the index
format** (`index.rs` defines `RawIndex`/`ClassEntry`, `prose.rs` the prose
index); the write half rides `loom-vector-ruvector`'s existing `pg-write`
feature gate, so neither is linked into the serving binary.

```bash
cargo run -p loom-scaffold --bin build-concept-records -- \
    --scaffold app/data/scaffold-index.json \
    --prose app/data/prose-index.json \
    --out uplift-results/ingest/concept-records.jsonl

cargo run -p loom-vector-ruvector --features pg-write --bin stage_corpus -- \
    --records uplift-results/ingest/concept-records.jsonl \
    --out uplift-results/ingest/ontology-corpus.sql \
    --namespace ontology-corpus --batch 96
```

Both take the same flags the Python did. `build-concept-records` is pinned
**byte-identical** to the Python it replaced: the golden
`tests/golden-python/golden_concept_records.jsonl` is asserted on every
`cargo test`, and the full 8,146-class corpus reproduced the retired script's
output to the byte (sha256 `bdf536ef…`) at port time.

## Record shape (matches the live `memory_entries` schema)

- `key` = `urn:ngm:class:<slug>` — the **IRI**, the join key across ttl / scaffold / prose / HNSW.
- `id` = `loom:ontology-corpus:<key>` (PK; `ON CONFLICT (id)` ⇒ idempotent).
- `value` = the record text as `jsonb` (exactly how `memory_store` stores it).
- `embedding` = `ruvector(384)`, unit-norm, cosine HNSW (`idx_memory_embedding_hnsw`).
- `metadata` = slug / title / domain / maturity / quality / has_prose / **generation** — the
  generation stamp is what makes the close-the-loop step (re-embed on promotion) a cheap
  per-IRI diff.
- `source_type = loom`, `project_id = NULL`.

## Two ground-truth laws these bins obey

1. **Embedder LOCK (ops law).** The embedder is `bge-small-en-v1.5`/384 and **only** that —
   the whole namespace is embedded with it and cosine-comparability requires one model.
   Qwen3 / bge-m3 were rejected. A different embedder silently invalidates the index.
   In Rust this is no longer a convention: `loom-embed-xinference` hard-codes `MODEL_ID`
   and `DIMENSIONS` as constants and length-checks every returned vector, so a mismatch
   is an error rather than a quietly-wrong answer.
2. **HNSW index-law (verified, load-bearing).** After a bulk ingest/delete, rebuild the HNSW
   index **non-concurrently** (`m=16`, `ef_construction=128`). **Never
   `CREATE INDEX CONCURRENTLY`** on the ruvector HNSW access method — it double-inserts
   (verified). The bins stage the rows; the HNSW rebuild is a **separate** step, never folded
   into the upsert (a unit test asserts the emitted SQL contains no `CREATE INDEX`).

> **Write only through the embedding pipeline.** Raw SQL `INSERT` and the `claude-flow memory`
> CLI bypass the embedding pipeline, so rows become invisible to HNSW search. `stage_corpus`
> embeds via Xinference and writes the vector explicitly; that is why it is a legitimate write
> path where a bare `INSERT` is not.

## Where this connects

- **Serving side:** `loom-vector-ruvector` reads an **in-process** `ruvector-core` projection of
  this namespace on the query hot path (network-free); its `pg-write` feature is the only thing
  that talks to `ruvector-postgres`, and only off-turn. See `RUST-ARCHITECTURE.md` §11.2.
  `export_corpus` (same feature gate) is the third step: it reads the verified embeddings back
  out of Postgres and builds the `.rvdb` artifact the serving binary opens.
- **The gate:** the fallback these records feed is **default-off** until the recall gate clears
  (currently RED, `0.816 < 0.87`; `.claude/evidence/EXP-008.evidence.md`).
- **Cross-refs:** RuVector **ADR-001** (HNSW production index); ADR-136 D3/D4; the estate
  ops-law in `~/workspace/CLAUDE.md` ("RuVector memory").
