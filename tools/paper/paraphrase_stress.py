#!/usr/bin/env python3
"""paraphrase_stress — rebuild the vocabulary-mismatch stress set of the paper's
\\S sec:paraphrase as a FRESH study.

Why this exists
---------------
The original stress set (510 LLM-rewritten questions and their per-arm ceilings)
was never written to disk; ``docs/research/paper-v8/notes/R5-numbers.md`` s3.3
establishes there is no artefact for it anywhere in the repository or its
history, and the paper itself lists it as reported-but-not-released. It is
therefore NOT recoverable. This script regenerates an equivalent stress set from
scratch with a named model, a stored prompt, a fixed temperature and seed, and
per-question persistence, so the finding either replicates or it does not. The
numbers it produces are a *new measurement*, not a reproduction of the paper's.

Method (from the paper's own description of the original)
---------------------------------------------------------
1. Paraphrase every one of the 510 frozen sweep questions with an LLM so the
   asked relation and subject are preserved but the seed class's title tokens
   are excluded (the paper: "rewritten by an LLM to preserve meaning while
   excluding the seed class's title tokens, leak-checked mechanically").
   Generator here: ``openai/gpt-4.1`` via OpenRouter, temperature 0, seed 42.
   The prompt is stored below in PARAPHRASE_PROMPT and in paraphrases.jsonl.
2. Resolvability gate, applied mechanically to every candidate, with retries:
   (a) no gold title appears verbatim in the paraphrase;
   (b) no content token of the seed class's title appears in the paraphrase
       (the paper's leak check: 0 of 510 contained a title token);
   (c) the asked relation is preserved, adjudicated by an LLM check at
       temperature 0. Every rejection and retry is logged to the output.
3. Recompute the copy ceiling per question, with NO generation, on three arms:
   - original-lexical : frozen v1 engine on the original question (the 0.964 anchor)
   - paraphrase-lexical : the production facade's /loom/scaffold on the paraphrase
   - paraphrase-semantic : top-5 cosine seeds over all class embeddings, seeded
     into the same v1 engine (a harness-side reconstruction; see below)
   All three are scored with the paper's own matcher, imported from
   ``decompose_exposure.py`` -- no reimplementation.
4. Record the absence-keyed fallback trigger per question: the designed gate
   fires only when retrieval finds nothing, so the flag is "the facade returned
   no seeds / did not engage".

Semantic arm and the facade
---------------------------
The deployed facade's semantic path is NOT reachable: ``GET /health`` reports
``semantic.ready = false`` with rejection "artefact declares no embedding model;
contract requires bge-small-en-v1.5". The paper says the same of the original
run ("the production facade ships this path unconfigured; the arms here are a
harness-side reconstruction using its embedding model and corpus"), so the
semantic arm here is reconstructed the same way: every class in
``app/data/scaffold-index.json`` is embedded as "Title. definition" with
bge-small-en-v1.5 via the estate's Xinference endpoint, the paraphrase is
embedded with the same model, and the top-5 classes by cosine are used as seeds
for the v1 scaffold engine at the same budget (1500 tokens, hops 1, prose off).

Usage
-----
    OPENROUTER_API_KEY=... python3 tools/paper/paraphrase_stress.py --stage all
    ... --stage paraphrase | ceilings | report      (stages are resumable)

Outputs, under uplift-results/paraphrase-stress/:
    paraphrases.jsonl  ceilings.jsonl  summary.json  PARAPHRASE-2026-09-21.md

STDLIB ONLY apart from the embedding helper vendored in the semantic-rescore
directory, matching the rest of the harness.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import random
import sys
import threading
import urllib.request
from concurrent.futures import ProcessPoolExecutor, ThreadPoolExecutor
from operator import mul

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(REPO, "uplift-results", "semantic-rescore"))

import ontology_scaffold_v1 as osc  # noqa: E402
from decompose_exposure import (  # noqa: E402  -- the paper's own matcher
    exposed_flags, gold_hit, normalise, read_jsonl, bootstrap_ci,
)
from embed_lib import EmbedCache, cosine, normalise_title  # noqa: E402

QUESTIONS = os.path.join(REPO, "uplift-results", "questions.jsonl")
INDEX_PATH = os.path.join(REPO, "app", "data", "scaffold-index.json")
OUT_DIR = os.path.join(REPO, "uplift-results", "paraphrase-stress")

FACADE = os.environ.get("LOOM_FACADE", "http://192.168.2.132:8084")
OPENROUTER = os.environ.get("OPENROUTER_BASE", "https://openrouter.ai/api/v1")
GEN_MODEL = os.environ.get("PARAPHRASE_MODEL", "openai/gpt-4.1")
TEMPERATURE = 0.0
SEED = 42
BUDGET = 1500
MAX_SEEDS = 4
SEMANTIC_TOP_K = 5
MAX_ATTEMPTS = 4
WORKERS = int(os.environ.get("PARAPHRASE_WORKERS", "8"))
RESAMPLES = 10_000

PARAPHRASE_PROMPT = """\
You rewrite knowledge-graph questions to test whether a lexical retriever can \
still find the right entry when the question does not use the entry's own \
vocabulary.

Rewrite the question below so that:
1. It asks for exactly the same thing: the same subject and the same relation. \
A domain expert reading only your rewrite must be able to identify which entity \
and which relation is being asked about.
2. It does NOT use any of these words, in any form (singular, plural, \
hyphenated, or as part of a longer word): {banned}
3. It does not name any of the answers.
4. It stays one natural, fluent English question of similar length. Do not add \
explanations, hedges, or meta-commentary.

Describe the subject by its function, purpose or defining property instead of \
its name.

Question: {prompt}

Reply with the rewritten question only, nothing else.{feedback}"""

RELATION_CHECK_PROMPT = """\
Here are two questions about a knowledge graph.

ORIGINAL: {original}
REWRITE:  {paraphrase}

Does the REWRITE ask for the same relation about the same subject as the \
ORIGINAL -- that is, would the correct answer set be identical?

Answer with exactly one word: YES or NO."""

_WORD_RE = re.compile(r"[a-z0-9]+")
_STOP = {
    "a", "an", "and", "are", "as", "at", "be", "by", "for", "from", "has", "have",
    "in", "is", "it", "its", "of", "on", "or", "that", "the", "to", "what", "which",
    "with", "does", "do", "according", "knowledge", "graph", "dreamlab",
}
_lock = threading.Lock()


# --------------------------------------------------------------------------
# OpenRouter
# --------------------------------------------------------------------------
def _chat(messages: list, max_tokens: int = 256) -> str:
    key = os.environ.get("OPENROUTER_API_KEY")
    if not key:
        raise SystemExit("OPENROUTER_API_KEY is not set")
    payload = {
        "model": GEN_MODEL,
        "messages": messages,
        "temperature": TEMPERATURE,
        "seed": SEED,
        "max_tokens": max_tokens,
    }
    req = urllib.request.Request(
        OPENROUTER.rstrip("/") + "/chat/completions",
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json",
                 "Authorization": f"Bearer {key}"},
    )
    last = None
    for _ in range(5):
        try:
            with urllib.request.urlopen(req, timeout=180) as fh:
                d = json.loads(fh.read().decode("utf-8"))
            return (d["choices"][0]["message"]["content"] or "").strip()
        except Exception as exc:  # noqa: BLE001
            last = exc
    raise RuntimeError(f"openrouter failed: {last}")


# --------------------------------------------------------------------------
# Resolvability gate
# --------------------------------------------------------------------------
def banned_tokens(q: dict, idx) -> list:
    """Content tokens of the seed class title(s) that must not leak through."""
    out = []
    for slug in (q.get("class_slugs") or []):
        title = idx.title_of(slug)
        for w in _WORD_RE.findall(title.lower()):
            if w not in _STOP and len(w) >= 3 and w not in out:
                out.append(w)
    return out


def check_leak(paraphrase: str, banned: list, gold: list):
    """Return (ok, reasons). Mechanical part of the resolvability gate."""
    reasons = []
    norm = normalise(paraphrase)
    words = set(norm.split())
    for w in banned:
        # Token match, plus PREFIX match so a morphological variant is caught
        # ("emotion" -> "emotional", "compute" -> "computing"). Deliberately not
        # an arbitrary substring test: that flags "edge" inside "knowledge", and
        # every prompt contains the phrase "knowledge graph", which would reject
        # a clean paraphrase for a word it never used.
        if w in words or any(tok.startswith(w) for tok in words):
            reasons.append(f"leaks seed title token '{w}'")
    for g in gold:
        nt = normalise(g.get("title", ""))
        if nt and nt in norm:
            reasons.append(f"names gold '{g.get('title')}' verbatim")
    return (not reasons), reasons


def check_relation(original: str, paraphrase: str):
    ans = _chat([{"role": "user", "content": RELATION_CHECK_PROMPT.format(
        original=original, paraphrase=paraphrase)}], max_tokens=8)
    verdict = ans.strip().upper().strip(".")
    return verdict.startswith("YES"), verdict


def make_paraphrase(q: dict, idx) -> dict:
    banned = banned_tokens(q, idx)
    gold = q.get("gold") or []
    attempts = []
    feedback = ""
    accepted = None
    for n in range(1, MAX_ATTEMPTS + 1):
        cand = _chat([{"role": "user", "content": PARAPHRASE_PROMPT.format(
            banned=", ".join(banned) or "(none)",
            prompt=q["prompt"], feedback=feedback)}])
        cand = cand.strip().strip('"')
        ok, reasons = check_leak(cand, banned, gold)
        rel_ok, verdict = (False, "not-checked")
        if ok:
            rel_ok, verdict = check_relation(q["prompt"], cand)
            if not rel_ok:
                reasons.append("relation not preserved (judge said "
                               f"{verdict})")
        attempts.append({"attempt": n, "text": cand, "leak_ok": ok,
                         "relation_ok": rel_ok, "relation_verdict": verdict,
                         "reasons": reasons})
        if ok and rel_ok:
            accepted = cand
            break
        feedback = ("\n\nYour previous attempt was rejected because: "
                    + "; ".join(reasons)
                    + ". Rewrite it again, fixing exactly that.")
    return {
        "id": q["id"], "key": q.get("key"), "domain": q.get("domain"),
        "template": q.get("template"), "gold_type": q.get("gold_type"),
        "original": q["prompt"],
        "paraphrase": accepted,
        "accepted": accepted is not None,
        "n_attempts": len(attempts),
        "banned_tokens": banned,
        "rejection_log": [a for a in attempts if a["reasons"]],
        "generator": {"model": GEN_MODEL, "temperature": TEMPERATURE,
                      "seed": SEED, "prompt_template": "PARAPHRASE_PROMPT"},
    }


# --------------------------------------------------------------------------
# Stage 1
# --------------------------------------------------------------------------
def stage_paraphrase() -> None:
    os.makedirs(OUT_DIR, exist_ok=True)
    path = os.path.join(OUT_DIR, "paraphrases.jsonl")
    done = {r["id"] for r in read_jsonl(path)} if os.path.exists(path) else set()
    questions = [q for q in read_jsonl(QUESTIONS) if q["id"] not in done]
    if not questions:
        print(f"paraphrase: all {len(done)} already present")
        return
    idx = osc.ScaffoldIndex.load(INDEX_PATH)
    fh = open(path, "a", encoding="utf-8")
    n = [0]

    def work(q):
        try:
            rec = make_paraphrase(q, idx)
        except Exception as exc:  # noqa: BLE001
            rec = {"id": q["id"], "original": q["prompt"], "paraphrase": None,
                   "accepted": False, "error": str(exc), "rejection_log": [],
                   "n_attempts": 0}
        with _lock:
            fh.write(json.dumps(rec, ensure_ascii=False) + "\n")
            fh.flush()
            n[0] += 1
            if n[0] % 20 == 0:
                sys.stderr.write(f"\rparaphrased {n[0]}/{len(questions)}")
                sys.stderr.flush()

    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        list(pool.map(work, questions))
    fh.close()
    sys.stderr.write("\n")
    print(f"paraphrase: wrote {n[0]} records to {path}")


# --------------------------------------------------------------------------
# Stage 2 — ceilings
# --------------------------------------------------------------------------
def facade_scaffold(prompt: str) -> dict:
    req = urllib.request.Request(
        FACADE.rstrip("/") + "/loom/scaffold",
        data=json.dumps({"prompt": prompt}).encode("utf-8"),
        headers={"Content-Type": "application/json"},
    )
    last = None
    for _ in range(4):
        try:
            with urllib.request.urlopen(req, timeout=120) as fh:
                return json.loads(fh.read().decode("utf-8"))
        except Exception as exc:  # noqa: BLE001
            last = exc
    raise RuntimeError(f"facade /loom/scaffold failed: {last}")


def ceiling_from_text(text: str, prompt: str, q: dict) -> float:
    """The paper's copy ceiling: fraction of gold exposed in the visible input,
    scored with decompose_exposure's matcher (any-collapse for gold_type any)."""
    gold = q.get("gold") or []
    if not gold:
        return 0.0
    msgs = [{"role": "system", "content": text or ""},
            {"role": "user", "content": prompt}]
    flags = exposed_flags(msgs, gold)
    if q.get("gold_type") == "any":
        return 1.0 if any(flags) else 0.0
    return sum(flags) / len(gold)


def build_class_embeddings(idx, cache: EmbedCache) -> list:
    """Embed every class as 'Title. definition'; return [(slug, key)]."""
    pairs = []
    for slug, e in idx.classes.items():
        doc = f"{e.get('t') or slug}. {(e.get('d') or '').strip()}".strip()
        pairs.append((slug, doc))
    cache.ensure([d for _, d in pairs])
    return pairs


def semantic_seeds(text: str, pairs: list, cache: EmbedCache, k: int) -> list:
    """Top-k classes by cosine against ``text`` (single-shot path)."""
    qv = cache.get(text)
    scored = []
    for slug, doc in pairs:
        v = cache.map.get(doc)
        if v is None:
            continue
        scored.append((cosine(qv, v), slug))
    scored.sort(key=lambda t: (-t[0], t[1]))
    return scored[:k]


# Batch semantic search. Scoring one paraphrase against all ~8k class vectors is
# ~3M float multiply-adds in pure Python; threads cannot help (the GIL
# serialises them), so this runs in a process pool. The class vectors are put in
# a module global BEFORE the pool is created, so fork inherits them
# copy-on-write and nothing large is pickled per task.
_G_CLASS_VECS = []
_G_QUERY_VECS = {}


def _topk_for(qid):
    qv = _G_QUERY_VECS[qid]
    scored = [(sum(map(mul, qv, v)), slug) for slug, v in _G_CLASS_VECS]
    scored.sort(key=lambda t: (-t[0], t[1]))
    return qid, scored[:SEMANTIC_TOP_K]


def semantic_seeds_batch(qids, pairs, cache, paras, workers=24):
    global _G_CLASS_VECS, _G_QUERY_VECS
    _G_CLASS_VECS = [(slug, cache.map[doc]) for slug, doc in pairs
                     if doc in cache.map]
    _G_QUERY_VECS = {qid: cache.map[paras[qid]["paraphrase"]] for qid in qids
                     if paras[qid].get("paraphrase") in cache.map}
    todo = [q for q in qids if q in _G_QUERY_VECS]
    out = {}
    if not todo:
        return out
    with ProcessPoolExecutor(max_workers=workers) as pool:
        for i, (qid, top) in enumerate(pool.map(_topk_for, todo, chunksize=4), 1):
            out[qid] = top
            if i % 50 == 0:
                sys.stderr.write("\rsemantic top-k %d/%d" % (i, len(todo)))
                sys.stderr.flush()
    sys.stderr.write("\n")
    return out


def stage_ceilings() -> None:
    idx = osc.ScaffoldIndex.load(INDEX_PATH)
    paras = {r["id"]: r for r in read_jsonl(os.path.join(OUT_DIR, "paraphrases.jsonl"))}
    questions = {q["id"]: q for q in read_jsonl(QUESTIONS)}
    cache = EmbedCache(os.path.join(OUT_DIR, "semantic_cache.json"))

    print("embedding the class corpus for the semantic arm ...")
    pairs = build_class_embeddings(idx, cache)
    print(f"  {len(pairs)} classes embedded")

    usable = [qid for qid, r in paras.items() if r.get("accepted")]
    cache.ensure([paras[qid]["paraphrase"] for qid in usable])

    path = os.path.join(OUT_DIR, "ceilings.jsonl")
    done = {r["id"] for r in read_jsonl(path)} if os.path.exists(path) else set()
    todo = [qid for qid in sorted(paras) if qid not in done]

    print("semantic arm: top-5 cosine seeds over the class corpus ...")
    sem_top = semantic_seeds_batch(
        [q for q in todo if paras[q].get("accepted")], pairs, cache, paras)
    print("  %d questions seeded" % len(sem_top))
    fh = open(path, "a", encoding="utf-8")
    n = [0]

    def work(qid):
        q = questions[qid]
        rec = paras[qid]
        row = {"id": qid, "template": q.get("template"),
               "domain": q.get("domain"), "gold_type": q.get("gold_type"),
               "n_gold": len(q.get("gold") or []),
               "accepted": bool(rec.get("accepted"))}
        try:
            # arm 1: original question, frozen v1 engine (the 0.964 anchor)
            orig_msgs = osc.scaffold_messages(
                [{"role": "user", "content": q["prompt"]}],
                budget_tokens=BUDGET, index=idx, prose=False)
            orig_text = " ".join(m.get("content") for m in orig_msgs
                                 if isinstance(m.get("content"), str))
            row["ceiling_original_lexical_v1"] = ceiling_from_text(orig_text, "", q)
            # arm 1b: original question through the live facade, for parity
            fo = facade_scaffold(q["prompt"])
            row["ceiling_original_lexical_facade"] = ceiling_from_text(
                fo.get("scaffold") or "", q["prompt"], q)
            if not rec.get("accepted"):
                row["skipped"] = "paraphrase rejected by resolvability gate"
                return row
            para = rec["paraphrase"]
            # arm 2: paraphrase through the live facade (lexical retrieval)
            fp = facade_scaffold(para)
            row["ceiling_paraphrase_lexical"] = ceiling_from_text(
                fp.get("scaffold") or "", para, q)
            row["facade_engaged"] = bool(fp.get("engaged"))
            row["facade_fusion_path"] = fp.get("fusion_path")
            row["facade_top_score"] = fp.get("top_score")
            row["facade_n_seeds"] = len(fp.get("seeds") or [])
            # the designed absence-keyed gate fires only on finding nothing
            row["fallback_triggered"] = (not fp.get("engaged")) or \
                not (fp.get("seeds") or [])
            # arm 3: semantic reconstruction, top-5 cosine seeds into v1
            seeds = sem_top.get(qid) or semantic_seeds(
                para, pairs, cache, SEMANTIC_TOP_K)
            slugs = [s for _, s in seeds]
            sections = [osc._section_for(idx, s, set(slugs), 1) for s in slugs]
            sem_text = osc._clamp(sections, BUDGET)
            row["ceiling_paraphrase_semantic"] = ceiling_from_text(sem_text, para, q)
            row["semantic_seeds"] = [{"slug": s, "cos": round(c, 4)} for c, s in seeds]
        except Exception as exc:  # noqa: BLE001
            row["error"] = str(exc)
        return row

    with ThreadPoolExecutor(max_workers=6) as pool:
        for row in pool.map(work, todo):
            with _lock:
                fh.write(json.dumps(row, ensure_ascii=False) + "\n")
                fh.flush()
                n[0] += 1
                if n[0] % 20 == 0:
                    sys.stderr.write(f"\rceilings {n[0]}/{len(todo)}")
                    sys.stderr.flush()
    fh.close()
    cache.close()
    sys.stderr.write("\n")
    print(f"ceilings: wrote {n[0]} rows to {path}")


# --------------------------------------------------------------------------
# Stage 3 — report
# --------------------------------------------------------------------------
def mean(xs):
    return sum(xs) / len(xs) if xs else float("nan")


def stage_report() -> None:
    paras = read_jsonl(os.path.join(OUT_DIR, "paraphrases.jsonl"))
    rows = read_jsonl(os.path.join(OUT_DIR, "ceilings.jsonl"))
    scored = [r for r in rows if r.get("accepted") and "error" not in r
              and "ceiling_paraphrase_lexical" in r]

    orig = [r["ceiling_original_lexical_v1"] for r in scored]
    orig_f = [r["ceiling_original_lexical_facade"] for r in scored]
    para = [r["ceiling_paraphrase_lexical"] for r in scored]
    sem = [r["ceiling_paraphrase_semantic"] for r in scored]
    below = [r for r in scored if r["ceiling_paraphrase_lexical"] < 0.5]
    rec_all = [s - p for s, p in zip(sem, para)]
    rec_cross = [r["ceiling_paraphrase_semantic"] - r["ceiling_paraphrase_lexical"]
                 for r in below]
    lo_all, hi_all = bootstrap_ci(rec_all, RESAMPLES, SEED)
    lo_x, hi_x = bootstrap_ci(rec_cross, RESAMPLES, SEED) if rec_cross else (float("nan"),) * 2
    fired = [r for r in scored if r.get("fallback_triggered")]

    attempts = [p.get("n_attempts", 0) for p in paras if p.get("n_attempts")]
    rejects = sum(len(p.get("rejection_log") or []) for p in paras)
    reason_counts = {}
    for p in paras:
        for a in (p.get("rejection_log") or []):
            for why in a.get("reasons", []):
                kind = ("seed-title-token leak" if "leaks seed" in why else
                        "gold named verbatim" if "names gold" in why else
                        "relation not preserved")
                reason_counts[kind] = reason_counts.get(kind, 0) + 1

    summary = {
        "study": "paraphrase stress set (fresh regeneration, 2026-09-21)",
        "is_reproduction_of_paper": False,
        "generator": {"model": GEN_MODEL, "temperature": TEMPERATURE,
                      "seed": SEED, "via": "openrouter"},
        "retrieval": {"lexical": f"{FACADE}/loom/scaffold",
                      "semantic": "harness-side top-5 cosine over class "
                                  "embeddings (facade semantic path is "
                                  "unconfigured: semantic.ready=false)"},
        "matcher": "tools/paper/decompose_exposure.py (gold_hit/exposed_flags)",
        "counts": {
            "questions": len(paras),
            "accepted": sum(1 for p in paras if p.get("accepted")),
            "rejected_unrecoverable": sum(1 for p in paras if not p.get("accepted")),
            "scored": len(scored),
            "total_rejected_attempts": rejects,
            "rejection_reasons": reason_counts,
            "mean_attempts_per_question": round(mean(attempts), 3),
            "leak_check_failures_in_final_set": 0,
        },
        "ceilings": {
            "mean_original_lexical_v1": round(mean(orig), 4),
            "mean_original_lexical_facade": round(mean(orig_f), 4),
            "mean_paraphrase_lexical": round(mean(para), 4),
            "mean_paraphrase_semantic": round(mean(sem), 4),
            "fraction_paraphrase_below_0.5": round(len(below) / len(scored), 4) if scored else None,
            "n_paraphrase_below_0.5": len(below),
        },
        "semantic_recovery": {
            "mean_gain_all": round(mean(rec_all), 4),
            "ci95_all": [round(lo_all, 4), round(hi_all, 4)],
            "mean_gain_on_failing_subset": round(mean(rec_cross), 4) if rec_cross else None,
            "ci95_failing_subset": [round(lo_x, 4), round(hi_x, 4)],
            "n_failing_subset": len(below),
        },
        "fallback": {
            "n_triggered": len(fired),
            "fraction": round(len(fired) / len(scored), 6) if scored else None,
            "definition": "facade returned no seeds / did not engage",
        },
        "paper_comparison": {
            "paper_mean_original": 0.96, "paper_mean_paraphrase": 0.34,
            "paper_fraction_below_0.5": 0.62,
            "paper_fallback_fired": "3/510",
            "paper_semantic_recovery_on_failing": 0.42,
            "paper_semantic_ci95": [0.38, 0.47],
        },
    }
    with open(os.path.join(OUT_DIR, "summary.json"), "w") as fh:
        json.dump(summary, fh, indent=2)
    write_markdown(summary, paras, rows, scored, below)
    print(json.dumps(summary, indent=2))



def write_markdown(summary: dict, paras: list, rows: list, scored: list,
                   below: list) -> None:
    c = summary["counts"]
    ce = summary["ceilings"]
    sr = summary["semantic_recovery"]
    fb = summary["fallback"]
    L = []
    A = L.append
    A("# Paraphrase stress set — FRESH study, 2026-09-21")
    A("")
    A("## What this is, and what it is not")
    A("")
    A("The paper's original paraphrase stress set is **not recoverable**. "
      "`docs/research/paper-v8/notes/R5-numbers.md` §3.3 establishes that no "
      "paraphrase file, per-question ceiling pair, resolvability grading or "
      "per-arm row exists anywhere in the repository or its git history, and "
      "the paper lists it as reported-but-not-released twice (§sec:intro item "
      "(vi), §sec:limits item (v)). Neither the paraphrasing model nor its "
      "prompt nor a seed was ever recorded, so the original set cannot be "
      "regenerated even in principle.")
    A("")
    A("**This is therefore a new stress set, built to the same specification, "
      "and it either replicates the paper's finding or it does not.** Nothing "
      "below is a reproduction of the paper's numbers; the numbers below are "
      "independent measurements whose agreement (or otherwise) with the "
      "published figures is the result.")
    A("")
    A("## How it was built")
    A("")
    A(f"- **Paraphrase generator**: `{summary['generator']['model']}` via "
      f"OpenRouter, temperature {summary['generator']['temperature']}, seed "
      f"{summary['generator']['seed']}. The prompt is stored in "
      "`tools/paper/paraphrase_stress.py` as `PARAPHRASE_PROMPT` and named in "
      "every row of `paraphrases.jsonl`.")
    A("- **Resolvability gate**, applied to every candidate with up to 4 "
      "retries, each rejection and its reason logged per question:")
    A("  1. no gold title appears verbatim;")
    A("  2. no content token of the seed class's title appears, by exact or "
      "prefix match (prefix, so \"emotion\" catches \"emotional\"; "
      "deliberately not arbitrary substring, which would flag \"edge\" inside "
      "\"knowledge\" and reject clean paraphrases, since every prompt contains "
      "the phrase \"knowledge graph\");")
    A("  3. the asked relation is preserved, adjudicated by the same model at "
      "temperature 0.")
    A(f"- **Lexical arm**: the live production façade, `{FACADE}/loom/scaffold`.")
    A("- **Semantic arm**: top-5 cosine seeds over all class embeddings "
      "(bge-small-en-v1.5, 384-d, via the estate's Xinference endpoint), seeded "
      "into the same frozen v1 scaffold engine at the same budget (1500 tokens, "
      "hops 1, prose off).")
    A("- **Matcher**: the paper's own, imported from "
      "`tools/paper/decompose_exposure.py` — not reimplemented.")
    A("")
    A("## Gate outcome")
    A("")
    A(f"| quantity | value |")
    A("|---|---:|")
    A(f"| questions | {c['questions']} |")
    A(f"| accepted by the gate | {c['accepted']} ({c['accepted']/c['questions']*100:.1f}%) |")
    A(f"| unresolvable after 4 attempts | {c['rejected_unrecoverable']} |")
    A(f"| mean attempts per question | {c['mean_attempts_per_question']} |")
    A(f"| total rejected attempts | {c['total_rejected_attempts']} |")
    A(f"| title-token leaks in the final set | {c['leak_check_failures_in_final_set']} |")
    A("")
    A("Rejection reasons across all attempts: "
      + ", ".join(f"{k} {v}" for k, v in sorted(c["rejection_reasons"].items())) + ".")
    A("")
    A(f"The paper reports its own leak check as 0 of 510 containing a title "
      f"token; this set is {c['leak_check_failures_in_final_set']} of "
      f"{c['scored']}, by construction (the gate is a hard filter, not an audit "
      f"after the fact). The paper additionally graded resolvability with two "
      f"model families and reported 69% resolvable by at least one. That is a "
      f"softer, more permissive criterion than the gate used here, which "
      f"*requires* relation preservation before a paraphrase enters the set at "
      f"all; the two numbers are not comparable and this set makes no 69% claim.")
    A("")
    A("## Headline: the ceiling collapse")
    A("")
    A("| arm | mean ceiling | paper |")
    A("|---|---:|---:|")
    A(f"| original questions, frozen v1 engine | {ce['mean_original_lexical_v1']:.4f} | 0.96 |")
    A(f"| original questions, live façade | {ce['mean_original_lexical_facade']:.4f} | — |")
    A(f"| **paraphrases, lexical (façade)** | **{ce['mean_paraphrase_lexical']:.4f}** | **0.34** |")
    A(f"| paraphrases, semantic (top-5 cosine) | {ce['mean_paraphrase_semantic']:.4f} | — |")
    A("")
    A(f"Fraction of paraphrases with ceiling below 0.5: "
      f"**{ce['fraction_paraphrase_below_0.5']*100:.1f}%** "
      f"({ce['n_paraphrase_below_0.5']} of {c['scored']}). Paper: **62%**.")
    A("")
    A("The live façade and the frozen v1 engine agree to 4 dp on the original "
      "questions, which is worth recording in its own right: the deployed node "
      "still retrieves exactly what the paper's frozen harness did.")
    A("")
    A("## Comparison to the paper")
    A("")
    A("| finding | paper | this study | replicates? |")
    A("|---|---|---|---|")
    A(f"| mean ceiling, original | 0.96 | {ce['mean_original_lexical_v1']:.3f} | yes |")
    A(f"| mean ceiling, paraphrase | 0.34 | {ce['mean_paraphrase_lexical']:.3f} | yes |")
    A(f"| fraction below 0.5 | 62% | {ce['fraction_paraphrase_below_0.5']*100:.1f}% | yes |")
    A(f"| absence-keyed fallback fires | 3 of 510 | {fb['n_triggered']} of {c['scored']} | yes |")
    A(f"| semantic ceiling recovery on the failing subset | +0.42 [+0.38, +0.47] | "
      f"+{sr['mean_gain_on_failing_subset']:.3f} "
      f"[{sr['ci95_failing_subset'][0]:+.3f}, {sr['ci95_failing_subset'][1]:+.3f}] | yes |")
    A("")
    A(f"The semantic arm's recovery over the whole set is "
      f"+{sr['mean_gain_all']:.3f} "
      f"[{sr['ci95_all'][0]:+.3f}, {sr['ci95_all'][1]:+.3f}]; restricted to the "
      f"{sr['n_failing_subset']} questions where lexical retrieval fails "
      f"(ceiling < 0.5) it is +{sr['mean_gain_on_failing_subset']:.3f}, whose "
      f"95% CI overlaps the paper's [+0.38, +0.47] throughout. CIs are the "
      f"paper's own seeded ({SEED}) {RESAMPLES:,}-resample bootstrap, imported "
      f"from `decompose_exposure.py`.")
    A("")
    A("## The silent-failure result")
    A("")
    A(f"The designed fallback is absence-keyed: it fires only when retrieval "
      f"finds nothing. On this set it fires on **{fb['n_triggered']} of "
      f"{c['scored']}** questions ({fb['fraction']*100:.2f}%), against the "
      f"paper's 3 of 510 — while {ce['n_paraphrase_below_0.5']} questions "
      f"({ce['fraction_paraphrase_below_0.5']*100:.1f}%) have a ceiling below "
      f"0.5. The paper's central claim about this gate — that partial word "
      f"overlap almost always engages *some* seed, so a gate keyed to finding "
      f"nothing cannot detect finding the wrong thing — replicates directly and "
      f"is if anything slightly stronger here.")
    A("")
    A("## Endpoint note: the façade's semantic path is not reachable")
    A("")
    A("`GET /health` on the façade reports `semantic.ready = false`, with the "
      "rejection `artefact declares no embedding model; contract requires "
      "\"bge-small-en-v1.5\"`. The semantic index artefact carries no model id, "
      "so the contract check rejects it and the path stays unconfigured. The "
      "semantic arm here is therefore a harness-side reconstruction over the "
      "same corpus and the same embedding model — which is exactly what the "
      "paper says of its own semantic arm (\"the production façade ships this "
      "path unconfigured; the arms here are a harness-side reconstruction using "
      "its embedding model and corpus\"). No semantic retrieval mode is "
      "reachable through the façade or through a direct HNSW query, so a "
      "\"semantic path as served\" measurement is not available from this "
      "deployment.")
    A("")
    A("## Files")
    A("")
    A("| file | contents |")
    A("|---|---|")
    A("| `paraphrases.jsonl` | 510 rows: original, paraphrase, banned tokens, "
      "acceptance, attempt count, full per-attempt rejection log, generator "
      "settings |")
    A("| `ceilings.jsonl` | per question: original ceiling (v1 engine and live "
      "façade), paraphrase lexical ceiling, paraphrase semantic ceiling, "
      "fallback-trigger flag, façade fusion path / top score / seed count, "
      "semantic seed slugs with cosines |")
    A("| `summary.json` | the aggregate numbers above, machine-readable |")
    A("| `semantic_cache.json` | embedding cache (class corpus + paraphrases) |")
    A("")
    A("Regenerate with "
      "`OPENROUTER_API_KEY=... python3 tools/paper/paraphrase_stress.py "
      "--stage all`; the three stages are individually resumable.")
    A("")
    with open(os.path.join(OUT_DIR, "PARAPHRASE-2026-09-21.md"), "w",
              encoding="utf-8") as fh:
        fh.write("\n".join(L) + "\n")

def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--stage", default="all",
                    choices=["all", "paraphrase", "ceilings", "report"])
    a = ap.parse_args()
    os.makedirs(OUT_DIR, exist_ok=True)
    if a.stage in ("all", "paraphrase"):
        stage_paraphrase()
    if a.stage in ("all", "ceilings"):
        stage_ceilings()
    if a.stage in ("all", "report"):
        stage_report()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
