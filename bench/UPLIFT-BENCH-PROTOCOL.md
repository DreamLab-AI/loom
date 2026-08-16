# Ontology Uplift Benchmark — Run Protocol (HP-Desktop)

How to run the real muse-glimmer vs gemma uplift comparison with
`bench_ontology_uplift.py`. Everything is stdlib-only Python; the only inputs
are the scaffold index and the two llama-server endpoints.

## Prerequisites

```bash
cd ~/githubs/loom/bench
./mirror.sh                                   # refresh data/scaffold-index.json
python3 ontology_scaffold.py --stats          # expect ~8k classes
bash run-all-tests.sh                         # all suites (incl. bench selftest) must PASS
```

The bench NEVER defaults to a live port — `--base-url` is required on `run`,
because the models on this box are often mid-benchmark. Check what is actually
serving before you point at it (`curl -s http://127.0.0.1:8085/v1/models`).

## Step 1 — generate one question set (used by every run)

```bash
python3 bench_ontology_uplift.py generate \
    --index data/scaffold-index.json --seed 42 --out questions.jsonl
```

Gold answers are derived from the graph itself (relation targets, parents,
inferred ancestors), stratified across every domain with >=50 classes, only
classes with quality >=0.6, >=2 relation types and a real definition. The set
is deterministic for a given `--seed` — regenerate identically anywhere.
Templates: `T-REL` (what does X require/use/depend on/enable, what are its
parts), `T-TAX` (parent + ancestors), `T-COMMON` (shared ancestor of a pair).

## Step 2 — the 4 runs (2 models x raw|scaffold, one question set)

**The two llama-server services are usually mutually exclusive** — muse on
:8085 and gemma on :8084 rarely serve at the same time. Run this as two
per-model sessions, and only ever point at a port whose model you have just
verified is serving (`curl -s http://127.0.0.1:8085/v1/models`). Pointing at
the wrong port either fails loudly (fine) or, worse, benchmarks whatever model
happens to be loaded there under the wrong label.

```bash
mkdir -p uplift-results

# ---- SESSION A: muse-glimmer serving on :8085 ----
python3 bench_ontology_uplift.py run --questions questions.jsonl \
    --base-url http://127.0.0.1:8085/v1 --model-name muse-glimmer \
    --mode raw --outdir uplift-results
python3 bench_ontology_uplift.py run --questions questions.jsonl \
    --base-url http://127.0.0.1:8085/v1 --model-name muse-glimmer \
    --mode scaffold --index data/scaffold-index.json --outdir uplift-results

# ---- SESSION B (after swapping services): gemma serving on :8084 ----
python3 bench_ontology_uplift.py run --questions questions.jsonl \
    --base-url http://127.0.0.1:8084/v1 --model-name gemma \
    --mode raw --outdir uplift-results
python3 bench_ontology_uplift.py run --questions questions.jsonl \
    --base-url http://127.0.0.1:8084/v1 --model-name gemma \
    --mode scaffold --index data/scaffold-index.json --outdir uplift-results
```

Defaults: `--temp 0 --max-tokens 400 --timeout 120 --retries 2 --sleep 0`,
scaffold `--budget 1500`. Keep the budget identical across models so the
comparison is fair. Per-question failures are recorded as error rows and never
abort a run. Each run writes `results-<model>-<mode>.jsonl` with the answer,
latency, `scaffold_engaged` and the injected-token estimate.

The one-shot `all` subcommand (generate + runs + scoring + report per
`--endpoint name=url`) exists, but on this box it is only usable when both
endpoints genuinely serve at the same time — which they usually do not.
Prefer the per-model sessions above; `score` and `report` stitch the sessions
together afterwards because every run shares the same `questions.jsonl`:

```bash
# only if BOTH models are verified serving simultaneously:
python3 bench_ontology_uplift.py all --index data/scaffold-index.json \
    --endpoint muse-glimmer=http://127.0.0.1:8085/v1 \
    --endpoint gemma=http://127.0.0.1:8084/v1 \
    --outdir uplift-results
```

## Step 3 — score (objective; optional LLM judge)

```bash
for f in uplift-results/results-*.jsonl; do
  python3 bench_ontology_uplift.py score --questions questions.jsonl \
      --results "$f" --outdir uplift-results
done
```

Scoring is lexical and needs no LLM: a gold item is a hit when its title
appears in the answer (substring, or >=80% of its length-4+ words). Optional
judge pass adds a 0-5 groundedness grade:

```bash
# judging muse-glimmer answers -> use gemma as judge (NEVER the model under test)
python3 bench_ontology_uplift.py score --questions questions.jsonl \
    --results uplift-results/results-muse-glimmer-scaffold.jsonl \
    --judge-base-url http://127.0.0.1:8084/v1 --judge-model gemma \
    --outdir uplift-results
```

Judge failures degrade gracefully to objective-only scoring.

## Step 4 — one report over all four score files

```bash
python3 bench_ontology_uplift.py report \
    --scores muse-glimmer/raw=uplift-results/scores-muse-glimmer-raw.jsonl \
    --scores muse-glimmer/scaffold=uplift-results/scores-muse-glimmer-scaffold.jsonl \
    --scores gemma/raw=uplift-results/scores-gemma-raw.jsonl \
    --scores gemma/scaffold=uplift-results/scores-gemma-scaffold.jsonl \
    --out uplift-results/report.md
```

`--scores` is repeatable; the `model/mode=` label is optional (bare paths use
the model/mode recorded in the rows) but keeping it explicit makes the report
self-describing.

## What the report means

- **Summary table** — mean recall per model x mode, plus the T-TAX ancestor
  extra-recall (gold_extra is credited separately and never touches headline
  recall), judge mean if run, mean injected tokens and latency. Per-domain and
  per-template breakdowns follow.
- **PAIRED UPLIFT lines** — the headline numbers. For each model, the paired
  per-question delta (scaffold recall − raw recall) over the intersection of
  question ids, with a seeded 10k-resample bootstrap 95% CI. If the CI
  excludes zero, the uplift is real at that sample size.
- **Not-engaged questions** are counted and excluded from the paired delta:
  when the scaffold found no ontology match, both arms saw the identical
  prompt, so those pairs measure nothing about uplift.
- **Error rows** (timeouts etc.) are excluded from means and from pairing, and
  reported as counts — infra noise must not masquerade as model signal.

### The honesty notes (auto-included in every report)

1. The scaffold contains gold-adjacent facts **by design** — questions and
   gold both come from the graph the scaffold injects. Scaffolded scores
   measure grounded-answer capability and retrieval quality; raw scores
   measure parametric knowledge. Deltas are "uplift available from
   grounding", not "model quality".
2. Substring scoring undercounts paraphrases — treat absolute recall as a
   lower bound; the paired deltas are the signal.
3. If a scaffolded run looks *worse*, that is a real result. Report it; do
   not tune the scaffold per model (budget and seed count are the only
   intended knobs).

## Later: adding the proxy's `tools` mode as a third axis

Tools mode measures *agentic traversal* (tool-call quality), which adds an
agency variable on top of knowledge uplift — keep it in its own column, never
mixed into the scaffold comparison prose.

```bash
# 1. start the proxy in tools mode in front of the model under test
ONTOLOGY_MODE=tools ONTOLOGY_UPSTREAM=http://127.0.0.1:8085 \
ONTOLOGY_INDEX=data/scaffold-index.json ONTOLOGY_PROXY_PORT=8086 \
    python3 ontology_proxy.py &

# 2. run the SAME question set through the proxy: raw mode client-side
#    (the proxy does the ontology work), but label the rows 'tools'
python3 bench_ontology_uplift.py run --questions questions.jsonl \
    --base-url http://127.0.0.1:8086/v1 --model-name muse-glimmer \
    --mode raw --mode-label tools --outdir uplift-results

# 3. score + re-report with the extra file
python3 bench_ontology_uplift.py score --questions questions.jsonl \
    --results uplift-results/results-muse-glimmer-tools.jsonl --outdir uplift-results
python3 bench_ontology_uplift.py report \
    --scores muse-glimmer/raw=uplift-results/scores-muse-glimmer-raw.jsonl \
    --scores muse-glimmer/scaffold=uplift-results/scores-muse-glimmer-scaffold.jsonl \
    --scores muse-glimmer/tools=uplift-results/scores-muse-glimmer-tools.jsonl \
    --out uplift-results/report.md
```

The run command records the proxy's `ontology` response annotation
(`tool_calls` / `injected_tokens`) per question; rows with zero tool calls
count as not-engaged and drop out of the tools-vs-raw paired delta, exactly
like non-engaging scaffolds. The report automatically adds a
`PAIRED UPLIFT <model> (tools - raw)` line for any model that has both runs.

## Sanity checks before trusting a result

- `bash run-all-tests.sh` still passes on the box.
- The four runs used the SAME `questions.jsonl` (same seed, same index).
- Error counts in the summary table are near zero; if not, the endpoints were
  contended — rerun rather than interpret.
- `mean injected tok` for scaffold runs is well under the model context and
  roughly constant across models (same budget).

## Mode: scaffold-prose (fourth axis)

Run exactly as the scaffold mode but with `--mode scaffold-prose`; score and
report identically (label model/scaffold-prose=path). Requires
data/prose-index.json (in mirror.sh). Keep --budget identical across all
modes; the landscape prose is appended LAST per section so the clamp drops it
first under pressure — injected_tokens tells you what actually fit.

## Mode: confidence-aware injection A/B (optimisation #2)

This axis is not a bench `--mode`; it is a **server-side** toggle on the Loom.
The façade decides how much scaffold to inject from the retrieval score, so you
hold the bench mode fixed at `scaffold` and vary the Loom's environment between
two container recycles. No rebuild — the switch is read at request time.

```
# one shared question set
python3 bench_ontology_uplift.py generate --seed 7 --out questions.jsonl

# A — blanket injection (baseline / legacy behaviour)
LOOM_CONFIDENCE_INJECTION=0 docker compose up -d
python3 bench_ontology_uplift.py run --mode scaffold --questions questions.jsonl --out A.jsonl

# B — confidence-aware selective injection
LOOM_CONFIDENCE_INJECTION=1 docker compose up -d
python3 bench_ontology_uplift.py run --mode scaffold --questions questions.jsonl --out B.jsonl
```

Score A and B with the SAME scorer/judge, then read two signals:

1. **Quality delta** — recall/quality should be *unchanged* on strong on-ontology
   questions (those keep full budget) and must not regress on the set overall.
2. **Efficiency / interference** — per-answer `loom.grounding` reports `top_score`
   and `effective_budget`; on weak / off-ontology questions B should show a lower
   `effective_budget` (or `injected=false`), i.e. fewer injected tokens for equal
   or better answers. That reduction is the context-interference the switch avoids.

Keep `--budget` and `LOOM_STRONG_MATCH_SCORE`/`LOOM_MIN_INJECT_SCORE`/
`LOOM_MIN_INJECT_FRACTION` fixed across A and B so the only variable is the master
switch. Unit coverage for the gate/scaling math: `tests/test_confidence_injection.py`.

### Status — verified live, A/B pending (2026-08-11)

The feature is deployed and verified on HP (loom `4beba5f`, rebuilt container, flag on):

- **Standing tests** — toolkit suite (`loom/bench/run-all-tests.sh`, with the new
  scaffold synced in) 6/6 PASS: scaffold, proxy, bench, confidence, mcp, pipeline. Loom
  repo suite (`bench/run-all-tests.sh`) PASS on all vendored suites (scaffold, bench,
  confidence, mcp).
- **Live HTTP probes** through `/v1/chat/completions`: an on-ontology query ("what is a
  rollup in blockchain scaling?") scored `top_score` 10.75 → full `effective_budget` 1500
  (683 tokens injected); an off-ontology query ("best recipe for banana pancakes?") scored
  0.0 → `injected: false`, 0 tokens. Gate and full-budget paths both confirmed in
  production.

The quality A/B itself (steps above) has **not** been run yet — those two runs plus scoring
are the open item. Grounding for the interference claim: Lin et al. 2026
(arXiv:2506.05154, contextual interference / parametric-knowledge displacement), Yoran et
al. 2024 (arXiv:2310.01558), Shi et al. 2023 (arXiv:2302.00093).

## Axis: cloud OpenAI-compat models (e.g. Gemini 3.7 Flash)

The same harness benches a **cloud** model with no code change beyond the base URL — the
façade contract *is* OpenAI-compatible, so any provider that speaks `/v1/chat/completions`
is a drop-in endpoint. Scaffold injection still happens **client-side** from the local
mirror (`app/data/scaffold-index.json`), so grounding is identical to what the deployed Loom
serves; only generation is delegated to the cloud. This is the portability the façade design
promises, exercised against a model behind someone else's door.

Three harness flags (added 2026-08-16) make a cloud provider work:

| Flag | Why it exists |
|---|---|
| `--auth-bearer-env ENV_VAR` | sends `Authorization: Bearer <token>` from the **named** env var (never argv), for keyed cloud endpoints |
| `--reasoning-effort low\|medium\|high` | OpenAI-compat `reasoning_effort`; on Gemini 3.x it maps to `thinking_level` |
| smarter `--base-url` | a base ending in `/openai` (Gemini's `…/v1beta/openai/`) is used as-is; it is **not** force-suffixed with `/v1` |

**Run it — Gemini 3.7 Flash** (auth via `GOOGLE_API_KEY`; generate the shared question set
first exactly as Step 1):

```bash
BASE=https://generativelanguage.googleapis.com/v1beta/openai/
COMMON="--base-url $BASE --model-name gemini-3.7-flash --auth-bearer-env GOOGLE_API_KEY \
  --reasoning-effort low --temp 1.0 --max-tokens 2048 --timeout 120 --retries 3 --sleep 0.4"
PYTHONPATH=app python3 bench/bench_ontology_uplift.py run --questions uplift-results/questions.jsonl \
  --mode raw --outdir uplift-results $COMMON
PYTHONPATH=app python3 bench/bench_ontology_uplift.py run --questions uplift-results/questions.jsonl \
  --mode scaffold --index app/data/scaffold-index.json --outdir uplift-results $COMMON
```

Then score + report exactly as Steps 3–4. A ready driver is `bench/run-gemini.sh`.

### Gemini 3.x gotchas (verified live 2026-08-16, `gemini-3.7-flash`)

1. **Thinking cannot be disabled** (floor `low`, default `medium`) and thinking tokens are
   drawn from the **output** budget. At the harness default `--max-tokens 400` the model
   returned `finish_reason: length` truncated to 74 chars — an empty-ish answer that would
   silently poison a whole arm. **Use `--max-tokens 2048` (or more) and `--reasoning-effort
   low`.** This is the same trap the workspace records for local reasoners (Muse empties at
   400); cloud reasoners hit it harder because thinking is mandatory.
2. **`temperature=0` is off-label for Gemini 3.x.** Google recommends the default `1.0` and
   warns that lower values can loop or degrade. The paired uplift is a *within-model* delta
   (same temp both arms) so it stays valid, but we run **`--temp 1.0`** and record the
   deviation from the historical local-model runs (which used `temp 0`). Do not silently mix
   the two temperatures into one comparison.
3. Thinking tokens are **billed as output** (Gemini 3.7 Flash intro pricing $0.75 / $3.75 per
   1M in/out through 2026-12-31) — a full 510-question raw+scaffold sweep is ~$1–2, but keep
   `--reasoning-effort low` to keep both cost and truncation risk down.
4. The judge, if run, must **never** be the model under test — use the LAN Loom/Qwen or a
   different cloud model as judge (auth for a cloud judge is not yet wired; use a local judge).

## Methodology hardening (adversarial pass, 2026-08-16)

An external adversarial review (Codex, red-teaming the estimand and statistics) surfaced real
weaknesses. The harness now instruments and reports the following; read them before quoting a
headline. What each guards against is in brackets.

**The estimand is *lexical gold-title recall*, not "grounding".** It counts whether expected
class titles appear in the answer — not relation direction, negation, correctness of
explanation, or contradiction. Reserve words like "grounded" / "decisive" for a composite that
also checks precision and factual consistency. [over-claiming from recall alone]

**Copy ceiling (the deepest issue).** Because the scaffold injects the gold titles by design, a
no-op extractor that echoed the injected context would already score high. Every scaffold row
now records `n_gold_exposed` / `n_gold`; the summary reports `mean_gold_exposed_recall` (the
copy ceiling) and `recall_gain_over_exposure` (mean recall − copy ceiling). **Report the gain
over copy, not just the headline recall.** For a stronger claim, add negative controls in future
runs: irrelevant scaffold, relation-target-shuffled scaffold, and entity-label-masked
definitions. [circularity / copy-from-context masquerading as reasoning]

**Per-row observability.** Each row now carries `finish_reason`, a normalised token breakdown
(`tokens.{prompt,completion,total,reasoning}`), `attempts` (retry count) and `response_model`.
The summary reports `n_truncated_finish_length` and `n_with_retries_gt1`. A run with truncated
rows or hidden retries is not trustworthy — check these before interpreting. [thinking-token
truncation and transport flakiness confounding recall and latency]

**Clustered CI + intention-to-treat.** Questions cluster by class and domain, so the naive
per-question bootstrap is optimistic. The report now prints a **domain-clustered 95% CI** beside
the naive one (expect it wider) and an **intention-to-treat delta** that keeps non-engaged pairs
(at their real ~0 delta) rather than excluding them. [too-narrow CI; selection on treatment
delivery]

**Single-sample-at-temperature caveat.** Gemini runs at `temp=1.0` (one completion per arm), so
run-to-run generation variance is unmeasured. Do **not** claim "convergence" from close point
estimates across models run at different temperatures / set sizes. For a convergence claim, run
**3–5 replicates per arm**, interleave/counterbalance raw-vs-scaffold order per question (not all
raw then all scaffold — that confounds latency with time-of-day load), and pre-register an
equivalence margin. [unmeasured sampling noise; order/latency confound; cross-condition
conflation]

### Minimum before the next headline run
- [ ] Report `recall_gain_over_exposure`, not bare recall, and run at least one negative-control
      scaffold (shuffled targets) to bound copy.
- [ ] Confirm `n_truncated_finish_length == 0` and note `n_with_retries_gt1`.
- [ ] Quote the **domain-clustered** CI as primary; keep the naive one as a footnote.
- [ ] For any cross-model "convergence" claim: same frozen index, questions, temperature and
      token budget, with ≥3 replicates and interleaved arm order.
