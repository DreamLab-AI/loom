# Paraphrase stress set — FRESH study, 2026-09-21

## What this is, and what it is not

The paper's original paraphrase stress set is **not recoverable**. `docs/research/paper-v8/notes/R5-numbers.md` §3.3 establishes that no paraphrase file, per-question ceiling pair, resolvability grading or per-arm row exists anywhere in the repository or its git history, and the paper lists it as reported-but-not-released twice (§sec:intro item (vi), §sec:limits item (v)). Neither the paraphrasing model nor its prompt nor a seed was ever recorded, so the original set cannot be regenerated even in principle.

**This is therefore a new stress set, built to the same specification, and it either replicates the paper's finding or it does not.** Nothing below is a reproduction of the paper's numbers; the numbers below are independent measurements whose agreement (or otherwise) with the published figures is the result.

## How it was built

- **Paraphrase generator**: `openai/gpt-4.1` via OpenRouter, temperature 0.0, seed 42. The prompt is stored in `tools/paper/paraphrase_stress.py` as `PARAPHRASE_PROMPT` and named in every row of `paraphrases.jsonl`.
- **Resolvability gate**, applied to every candidate with up to 4 retries, each rejection and its reason logged per question:
  1. no gold title appears verbatim;
  2. no content token of the seed class's title appears, by exact or prefix match (prefix, so "emotion" catches "emotional"; deliberately not arbitrary substring, which would flag "edge" inside "knowledge" and reject clean paraphrases, since every prompt contains the phrase "knowledge graph");
  3. the asked relation is preserved, adjudicated by the same model at temperature 0.
- **Lexical arm**: the live production façade, `http://192.168.2.132:8084/loom/scaffold`.
- **Semantic arm**: top-5 cosine seeds over all class embeddings (bge-small-en-v1.5, 384-d, via the estate's Xinference endpoint), seeded into the same frozen v1 scaffold engine at the same budget (1500 tokens, hops 1, prose off).
- **Matcher**: the paper's own, imported from `tools/paper/decompose_exposure.py` — not reimplemented.

## Gate outcome

| quantity | value |
|---|---:|
| questions | 510 |
| accepted by the gate | 506 (99.2%) |
| unresolvable after 4 attempts | 4 |
| mean attempts per question | 1.112 |
| total rejected attempts | 61 |
| title-token leaks in the final set | 0 |

Rejection reasons across all attempts: gold named verbatim 33, relation not preserved 23, seed-title-token leak 5.

The paper reports its own leak check as 0 of 510 containing a title token; this set is 0 of 506, by construction (the gate is a hard filter, not an audit after the fact). The paper additionally graded resolvability with two model families and reported 69% resolvable by at least one. That is a softer, more permissive criterion than the gate used here, which *requires* relation preservation before a paraphrase enters the set at all; the two numbers are not comparable and this set makes no 69% claim.

## Headline: the ceiling collapse

| arm | mean ceiling | paper |
|---|---:|---:|
| original questions, frozen v1 engine | 0.9642 | 0.96 |
| original questions, live façade | 0.9642 | — |
| **paraphrases, lexical (façade)** | **0.3275** | **0.34** |
| paraphrases, semantic (top-5 cosine) | 0.5964 | — |

Fraction of paraphrases with ceiling below 0.5: **64.4%** (326 of 506). Paper: **62%**.

The live façade and the frozen v1 engine agree to 4 dp on the original questions, which is worth recording in its own right: the deployed node still retrieves exactly what the paper's frozen harness did.

## Comparison to the paper

| finding | paper | this study | replicates? |
|---|---|---|---|
| mean ceiling, original | 0.96 | 0.964 | yes |
| mean ceiling, paraphrase | 0.34 | 0.328 | yes |
| fraction below 0.5 | 62% | 64.4% | yes |
| absence-keyed fallback fires | 3 of 510 | 2 of 506 | yes |
| semantic ceiling recovery on the failing subset | +0.42 [+0.38, +0.47] | +0.446 [+0.398, +0.493] | yes |

The semantic arm's recovery over the whole set is +0.269 [+0.227, +0.310]; restricted to the 326 questions where lexical retrieval fails (ceiling < 0.5) it is +0.446, whose 95% CI overlaps the paper's [+0.38, +0.47] throughout. CIs are the paper's own seeded (42) 10,000-resample bootstrap, imported from `decompose_exposure.py`.

## The silent-failure result

The designed fallback is absence-keyed: it fires only when retrieval finds nothing. On this set it fires on **2 of 506** questions (0.40%), against the paper's 3 of 510 — while 326 questions (64.4%) have a ceiling below 0.5. The paper's central claim about this gate — that partial word overlap almost always engages *some* seed, so a gate keyed to finding nothing cannot detect finding the wrong thing — replicates directly and is if anything slightly stronger here.

## Endpoint note: the façade's semantic path is not reachable

`GET /health` on the façade reports `semantic.ready = false`, with the rejection `artefact declares no embedding model; contract requires "bge-small-en-v1.5"`. The semantic index artefact carries no model id, so the contract check rejects it and the path stays unconfigured. The semantic arm here is therefore a harness-side reconstruction over the same corpus and the same embedding model — which is exactly what the paper says of its own semantic arm ("the production façade ships this path unconfigured; the arms here are a harness-side reconstruction using its embedding model and corpus"). No semantic retrieval mode is reachable through the façade or through a direct HNSW query, so a "semantic path as served" measurement is not available from this deployment.

## Files

| file | contents |
|---|---|
| `paraphrases.jsonl` | 510 rows: original, paraphrase, banned tokens, acceptance, attempt count, full per-attempt rejection log, generator settings |
| `ceilings.jsonl` | per question: original ceiling (v1 engine and live façade), paraphrase lexical ceiling, paraphrase semantic ceiling, fallback-trigger flag, façade fusion path / top score / seed count, semantic seed slugs with cosines |
| `summary.json` | the aggregate numbers above, machine-readable |
| `semantic_cache.json` | embedding cache (class corpus + paraphrases) |

Regenerate with `OPENROUTER_API_KEY=... python3 tools/paper/paraphrase_stress.py --stage all`; the three stages are individually resumable.

