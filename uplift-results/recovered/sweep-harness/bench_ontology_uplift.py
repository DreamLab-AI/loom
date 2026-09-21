#!/usr/bin/env python3
"""bench_ontology_uplift — measure LLM ontology uplift with graph-derived gold.

STDLIB ONLY. Python 3.10+ (targets 3.14 on HP-Desktop). Sits next to
``ontology_scaffold.py`` (import integration point) and ``ontology_proxy.py``.

Design
------
The gold answers are OBJECTIVE and GRAPH-DERIVED: every question is generated
from the scaffold index (title/definition/sup/isup/rel), and the gold is the
set of {slug, title} targets the graph itself asserts. Scoring is lexical
(substring / word-overlap), so no LLM is needed to score. An optional
LLM-judge pass adds a 0-5 groundedness grade; the judge model must NEVER be
the model under test (use the other model on the box, or a third endpoint) —
judge failures degrade to objective-only scoring.

Honesty: in scaffold mode the injected ontology context contains gold-adjacent
facts BY DESIGN. The benchmark therefore measures grounded-answer capability
and retrieval quality; raw mode measures parametric knowledge. The paired
delta is "uplift available from grounding", not "model quality". Substring
scoring undercounts paraphrases: absolute numbers are lower bounds, deltas are
the signal.

Subcommands
-----------
  generate  build questions.jsonl from the scaffold index (deterministic --seed)
  run       ask one model (raw | scaffold mode) -> results-<model>-<mode>.jsonl
  score     objective scoring (+ optional LLM judge) -> scores/summary files
  report    markdown report over many score files, paired bootstrap uplift
  all       generate -> run (per --endpoint, both modes) -> score -> report

  --selftest  end-to-end test against an inline fixture + stub server; no
              network, no index file, no live model needed.

Examples
--------
  python3 bench_ontology_uplift.py generate --index data/scaffold-index.json
  python3 bench_ontology_uplift.py run --questions questions.jsonl \
      --base-url http://127.0.0.1:8085/v1 --model-name muse-glimmer --mode scaffold
  python3 bench_ontology_uplift.py score --questions questions.jsonl \
      --results results-muse-glimmer-scaffold.jsonl
  python3 bench_ontology_uplift.py report \
      --scores muse-glimmer/raw=scores-muse-glimmer-raw.jsonl \
      --scores muse-glimmer/scaffold=scores-muse-glimmer-scaffold.jsonl \
      --out report.md
  python3 bench_ontology_uplift.py all --index data/scaffold-index.json \
      --endpoint muse-glimmer=http://127.0.0.1:8085/v1 \
      --endpoint gemma=http://127.0.0.1:8084/v1 --outdir uplift-results
"""

from __future__ import annotations

import argparse
import json
import os
import random
import re
import sys
import time
import urllib.error
import urllib.request
from typing import Any, Optional

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import ontology_scaffold  # noqa: E402  (sibling module, stdlib-only)

DEFAULT_SEED = 42

# T-REL phrasing per relation type. Only these five types generate questions.
REL_TEMPLATES: dict[str, str] = {
    "requires": "According to the DreamLab knowledge graph, what does {t} require?",
    "hasPart": "According to the DreamLab knowledge graph, what are the parts of {t}?",
    "enables": "According to the DreamLab knowledge graph, what does {t} enable?",
    "dependsOn": "According to the DreamLab knowledge graph, what does {t} depend on?",
    "uses": "According to the DreamLab knowledge graph, what does {t} use?",
}

TAX_TEMPLATE = (
    "In the DreamLab knowledge graph, what is the immediate parent concept of "
    "{t}? Also name up to three broader ancestor concepts."
)

COMMON_TEMPLATE = (
    "What broader concept do {a} and {b} both fall under in the DreamLab "
    "knowledge graph?"
)

HONEST_NOTES = """\
## Honest notes (read before quoting numbers)

1. **The scaffold contains gold-adjacent facts BY DESIGN.** Questions and gold
   are both derived from the knowledge graph, and scaffold mode injects a
   budget-clamped extract of that same graph. Scaffolded scores therefore
   measure *grounded-answer capability and retrieval quality* (can the model
   find, trust and restate the injected facts); raw scores measure *parametric
   knowledge*. The paired delta is "uplift available from grounding", not
   "model quality".
2. **Substring scoring undercounts paraphrases.** A gold item only counts as a
   hit when its title (or >=80% of its length-4+ words) appears in the answer.
   Treat absolute recall numbers as lower bounds; the paired deltas — same
   questions, same scorer, same model — are the signal.
3. Questions where the scaffold never engaged are excluded from the paired
   delta: both arms saw the identical prompt, so they measure nothing about
   uplift. They are counted separately above; the intention-to-treat delta
   (which keeps them) is reported alongside.
4. **The primary endpoint is *lexical gold-title recall*, not "grounding".**
   It counts whether expected class titles appear in the answer; it does not
   verify relation direction, negation, correctness of explanation, or absence
   of contradiction. Read the *copy ceiling* column: much of a scaffold arm's
   recall is gold that was present verbatim in the injected context, so a no-op
   extractor would score near it. The honest quantity is *gain over copy*.
5. **Independence and sampling.** Questions cluster by class and domain, so the
   naive per-question bootstrap CI is optimistic; a domain-clustered CI is
   reported beside it. At temperature > 0 a single completion per arm leaves
   run-to-run variance unmeasured — treat a lone run's point estimate as one
   draw, and prefer several replicates before claiming convergence.
"""


# ---------------------------------------------------------------------------
# small helpers
# ---------------------------------------------------------------------------

def _fsafe(name: str) -> str:
    return re.sub(r"[^A-Za-z0-9._-]+", "-", name).strip("-") or "model"


def _read_jsonl(path: str) -> list[dict]:
    rows: list[dict] = []
    with open(path, "r", encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if line:
                rows.append(json.loads(line))
    return rows


def _write_jsonl(path: str, rows: list[dict]) -> None:
    with open(path, "w", encoding="utf-8") as fh:
        for r in rows:
            fh.write(json.dumps(r, ensure_ascii=False) + "\n")


def _mean(vals: list[float]) -> Optional[float]:
    return sum(vals) / len(vals) if vals else None


def _fmt(v: Optional[float], nd: int = 3) -> str:
    return "-" if v is None else f"{v:.{nd}f}"


def _chat_url(base_url: str) -> str:
    """Resolve an OpenAI-compatible chat/completions URL.

    Handles three shapes so the harness works for local llama.cpp *and*
    cloud OpenAI-compat providers:
      - already complete (…/chat/completions)          -> used as-is
      - OpenAI-compat base (…/v1beta/openai, …/openai) -> + /chat/completions
        (Google Gemini's shim lives at /v1beta/openai/, NOT /v1)
      - bare host or …/v1                              -> ensure /v1 then append
    """
    b = base_url.rstrip("/")
    if b.endswith("/chat/completions"):
        return b
    if b.endswith("/v1") or b.endswith("/openai"):
        return b + "/chat/completions"
    return b + "/v1/chat/completions"


def chat_request(base_url: str, payload: dict, timeout: float,
                 retries: int, auth_bearer: Optional[str] = None,
                 stats: Optional[dict] = None) -> dict:
    """POST an OpenAI chat payload; retry transport/HTTP failures.

    ``auth_bearer`` (if given) is sent as ``Authorization: Bearer <token>`` —
    required by cloud providers (e.g. Gemini's OpenAI-compat endpoint). Local
    llama.cpp endpoints ignore it, so it is safe to always thread through.

    ``stats`` (if given) is populated with ``{"attempts": <n>}`` so callers can
    record retry counts per row — a successful retry otherwise hides transport
    flakiness and confounds latency comparisons (adversarial pass 2026-08-16).
    """
    url = _chat_url(base_url)
    data = json.dumps(payload).encode("utf-8")
    headers = {"Content-Type": "application/json"}
    if auth_bearer:
        headers["Authorization"] = f"Bearer {auth_bearer}"
    last: Optional[Exception] = None
    for attempt in range(retries + 1):
        try:
            req = urllib.request.Request(
                url, data=data, method="POST", headers=headers,
            )
            with urllib.request.urlopen(req, timeout=timeout) as resp:
                raw = resp.read()
            obj = json.loads(raw.decode("utf-8"))
            if not isinstance(obj, dict):
                raise ValueError("upstream returned non-object JSON")
            if stats is not None:
                stats["attempts"] = attempt + 1
            return obj
        except urllib.error.HTTPError as exc:
            try:
                detail = exc.read().decode("utf-8", "replace")[:300]
            except Exception:
                detail = ""
            last = RuntimeError(f"HTTP {exc.code} from {url}: {detail}")
        except Exception as exc:
            last = exc
        if attempt < retries:
            time.sleep(min(2.0, 0.5 * (attempt + 1)))
    if stats is not None:
        stats["attempts"] = retries + 1
    raise RuntimeError(f"chat request failed after {retries + 1} attempts: {last}")


def _usage_normalise(usage: Any) -> dict:
    """Pull a flat, provider-agnostic token breakdown out of an OpenAI-style
    usage block. Reasoning/thinking tokens (Gemini 3.x, o-series) live in
    completion_tokens_details.reasoning_tokens — surfacing them is what lets a
    reader see whether a small max_tokens was eaten by hidden thinking."""
    if not isinstance(usage, dict):
        return {}
    out: dict[str, Any] = {}
    for k in ("prompt_tokens", "completion_tokens", "total_tokens"):
        if isinstance(usage.get(k), (int, float)):
            out[k] = usage[k]
    det = usage.get("completion_tokens_details")
    if isinstance(det, dict) and isinstance(det.get("reasoning_tokens"), (int, float)):
        out["reasoning_tokens"] = det["reasoning_tokens"]
    return out


def _gold_exposed(messages: list, gold: list) -> int:
    """How many gold titles are literally present in what the model was shown.

    This is the deterministic-copy ceiling (adversarial pass 2026-08-16, findings
    1/2): a no-op extractor that echoed the injected context would score roughly
    this recall. Reporting it next to headline recall makes the "scaffold contains
    gold by design" honesty note MEASURABLE rather than merely asserted — if
    exposed-recall ≈ scaffold-recall, most of the lift is copy, not reasoning."""
    text = " ".join(
        m.get("content") for m in messages if isinstance(m.get("content"), str))
    norm = normalise(text)
    words = set(norm.split())
    return sum(1 for g in (gold or []) if gold_hit(g.get("title", ""), norm, words))


# ---------------------------------------------------------------------------
# GENERATE — objective, graph-derived gold
# ---------------------------------------------------------------------------

def generate_questions(
    index_path: Optional[str],
    seed: int = DEFAULT_SEED,
    per_domain: int = 12,
    min_domain_classes: int = 50,
    min_quality: float = 0.6,
    null_quality: float = 0.65,
    min_rel_types: int = 2,
    min_def_len: int = 120,
    rel_min: int = 2,
    rel_max: int = 5,
) -> list[dict]:
    idx = ontology_scaffold.ScaffoldIndex.load(index_path)
    classes = idx.classes

    def resolve(refs: Any, cap: Optional[int] = None) -> list[dict]:
        """Resolve refs (slug or urn) to in-index [{slug, title}], deduped."""
        out: list[dict] = []
        seen: set[str] = set()
        for r in refs or []:
            s = ontology_scaffold._ref_to_slug(str(r))
            if s in seen or s not in classes:
                continue
            seen.add(s)
            out.append({"slug": s, "title": idx.title_of(s)})
            if cap is not None and len(out) >= cap:
                break
        return out

    def is_eligible(e: dict) -> bool:
        q = e.get("q")
        q = null_quality if q is None else q
        if q < min_quality:
            return False
        rel = e.get("rel") or {}
        if sum(1 for v in rel.values() if v) < min_rel_types:
            return False
        return len(e.get("d") or "") >= min_def_len

    eligible_by_domain: dict[str, list[str]] = {}
    for slug in sorted(classes):
        e = classes[slug]
        dom = e.get("dom") or ""
        if dom and is_eligible(e):
            eligible_by_domain.setdefault(dom, []).append(slug)
    # Only domains with at least min_domain_classes ELIGIBLE classes.
    domains = sorted(d for d, slugs in eligible_by_domain.items()
                     if len(slugs) >= min_domain_classes)

    rng = random.Random(seed)
    questions: list[dict] = []

    for dom in domains:
        eligible = eligible_by_domain[dom]
        sample = sorted(rng.sample(eligible, min(per_domain, len(eligible))))

        for slug in sample:
            e = classes[slug]
            title = idx.title_of(slug)
            rel = e.get("rel") or {}

            # T-REL: two largest templated rel types with 2-5 in-index targets.
            cands: list[tuple[str, list[dict]]] = []
            for rt in REL_TEMPLATES:
                targets = resolve(rel.get(rt))
                if rel_min <= len(targets) <= rel_max:
                    cands.append((rt, targets))
            cands.sort(key=lambda kv: (-len(kv[1]), kv[0]))
            for rt, targets in cands[:2]:
                questions.append({
                    "key": f"rel:{slug}:{rt}",
                    "domain": dom,
                    "template": "T-REL",
                    "class_slugs": [slug],
                    "prompt": REL_TEMPLATES[rt].format(t=title),
                    "gold": targets,
                    "gold_type": "all",
                })

            # T-TAX: gold = direct parents; isup ancestors (capped 5) go to
            # gold_extra and are credited separately at score time.
            sup = resolve(e.get("sup"))
            if sup:
                sup_slugs = {g["slug"] for g in sup}
                extra = [g for g in resolve(e.get("isup"))
                         if g["slug"] not in sup_slugs][:5]
                qrec: dict[str, Any] = {
                    "key": f"tax:{slug}",
                    "domain": dom,
                    "template": "T-TAX",
                    "class_slugs": [slug],
                    "prompt": TAX_TEMPLATE.format(t=title),
                    "gold": sup,
                    "gold_type": "all",
                }
                if extra:
                    qrec["gold_extra"] = extra
                questions.append(qrec)

        # T-COMMON: same-domain sampled pairs sharing >=1 isup ancestor.
        anc = {s: {g["slug"] for g in resolve(classes[s].get("isup"))}
               for s in sample}
        pairs: list[tuple[str, str, list[str]]] = []
        for i in range(len(sample)):
            for j in range(i + 1, len(sample)):
                common = sorted(anc[sample[i]] & anc[sample[j]])
                if common:
                    pairs.append((sample[i], sample[j], common))
        take = min(len(pairs), per_domain // 4)
        chosen = sorted(rng.sample(pairs, take)) if take else []
        for a, b, common in chosen:
            questions.append({
                "key": f"common:{a}+{b}",
                "domain": dom,
                "template": "T-COMMON",
                "class_slugs": [a, b],
                "prompt": COMMON_TEMPLATE.format(
                    a=idx.title_of(a), b=idx.title_of(b)),
                "gold": [{"slug": s, "title": idx.title_of(s)}
                         for s in common[:3]],
                "gold_type": "any",
            })

    # Sequential q0001-style ids (order is deterministic for a given seed);
    # the graph-derived key is kept for traceability.
    return [{"id": f"q{i:04d}", **q} for i, q in enumerate(questions, 1)]


def cmd_generate(args: argparse.Namespace) -> int:
    questions = generate_questions(
        args.index, seed=args.seed, per_domain=args.per_domain,
        min_domain_classes=args.min_domain_classes,
        min_quality=args.min_quality, min_def_len=args.min_def_len,
    )
    _write_jsonl(args.out, questions)
    by_dom: dict[str, int] = {}
    by_tmpl: dict[str, int] = {}
    for q in questions:
        by_dom[q["domain"]] = by_dom.get(q["domain"], 0) + 1
        by_tmpl[q["template"]] = by_tmpl.get(q["template"], 0) + 1
    print(f"generate: {len(questions)} questions -> {args.out} (seed={args.seed})")
    print("  by template: " + ", ".join(
        f"{t}={n}" for t, n in sorted(by_tmpl.items())))
    print("  by domain:")
    for d, n in sorted(by_dom.items(), key=lambda kv: (-kv[1], kv[0])):
        print(f"    {d}: {n}")
    return 0


# ---------------------------------------------------------------------------
# RUN
# ---------------------------------------------------------------------------

def _content_chars(messages: list) -> int:
    total = 0
    for m in messages:
        c = m.get("content")
        if isinstance(c, str):
            total += len(c)
        elif isinstance(c, list):
            for part in c:
                if isinstance(part, dict) and part.get("type") == "text":
                    total += len(str(part.get("text", "")))
    return total


def run_bench(
    questions_path: str,
    base_url: str,
    model_name: str,
    mode: str,
    outdir: str = ".",
    mode_label: Optional[str] = None,
    budget: int = 1500,
    index_path: Optional[str] = None,
    temp: float = 0.0,
    max_tokens: int = 400,
    timeout: float = 120.0,
    retries: int = 2,
    sleep: float = 0.0,
    out: Optional[str] = None,
    reasoning_effort: Optional[str] = None,
    auth_bearer: Optional[str] = None,
) -> str:
    questions = _read_jsonl(questions_path)
    label = mode_label or mode
    scaf_idx = None
    prose_mode = mode == "scaffold-prose"
    if mode in ("scaffold", "scaffold-prose"):
        scaf_idx = ontology_scaffold.ScaffoldIndex.load(index_path)

    os.makedirs(outdir, exist_ok=True)
    out_path = out or os.path.join(
        outdir, f"results-{_fsafe(model_name)}-{_fsafe(label)}.jsonl")

    n_err = 0
    with open(out_path, "w", encoding="utf-8") as fh:
        for i, q in enumerate(questions, 1):
            row: dict[str, Any] = {"id": q["id"], "model": model_name,
                                   "mode": label}
            engaged = False
            injected = 0
            try:
                messages: list = [{"role": "user", "content": q["prompt"]}]
                if scaf_idx is not None:
                    before = _content_chars(messages)
                    new = ontology_scaffold.scaffold_messages(
                        messages, budget_tokens=budget, index=scaf_idx,
                        prose=prose_mode)
                    engaged = new != messages
                    if engaged:
                        injected = max(
                            0, (_content_chars(new) - before + 3) // 4)
                    messages = new
                # Deterministic-copy ceiling: how much gold was in what the model
                # saw. Cheap to compute here where we still hold the messages.
                row["n_gold"] = len(q.get("gold") or [])
                row["n_gold_exposed"] = _gold_exposed(messages, q.get("gold") or [])
                payload = {
                    "model": model_name,
                    "messages": messages,
                    "temperature": temp,
                    "max_tokens": max_tokens,
                }
                # reasoning_effort is an OpenAI-compat knob for thinking models
                # (Gemini 3.x maps it to thinking_level). Sending "low" keeps
                # mandatory thinking from eating the max_tokens answer budget.
                if reasoning_effort:
                    payload["reasoning_effort"] = reasoning_effort
                stats: dict = {}
                t0 = time.perf_counter()
                resp = chat_request(base_url, payload, timeout, retries,
                                    auth_bearer=auth_bearer, stats=stats)
                latency_ms = round((time.perf_counter() - t0) * 1000.0, 1)
                try:
                    choice0 = resp["choices"][0]
                    answer = choice0["message"].get("content") or ""
                except (KeyError, IndexError, TypeError):
                    raise RuntimeError(
                        f"malformed upstream response: {str(resp)[:200]}")
                # If we ran through ontology_proxy, adopt its accounting.
                onto = resp.get("ontology")
                if isinstance(onto, dict):
                    row["proxy_ontology"] = onto
                    pi = onto.get("injected_tokens")
                    if isinstance(pi, int) and pi > 0 and injected == 0:
                        injected, engaged = pi, True
                    tc = onto.get("tool_calls")
                    if isinstance(tc, int) and tc > 0:
                        engaged = True
                row.update(answer=answer, latency_ms=latency_ms,
                           scaffold_engaged=engaged,
                           injected_tokens=injected)
                # Per-row observability (adversarial pass 2026-08-16): finish_reason
                # exposes truncation (thinking eating the budget); token breakdown,
                # attempts and provider model-version make the run auditable and
                # reproducible instead of a black box.
                row["finish_reason"] = choice0.get("finish_reason")
                row["answer_chars"] = len(answer)
                row["attempts"] = stats.get("attempts")
                if resp.get("model"):
                    row["response_model"] = resp.get("model")
                usage = resp.get("usage")
                if isinstance(usage, dict):
                    row["usage"] = usage
                    norm_usage = _usage_normalise(usage)
                    if norm_usage:
                        row["tokens"] = norm_usage
            except Exception as exc:
                n_err += 1
                row.update(error=str(exc), scaffold_engaged=engaged,
                           injected_tokens=injected)
            fh.write(json.dumps(row, ensure_ascii=False) + "\n")
            fh.flush()
            if i % 10 == 0 or i == len(questions):
                print(f"run [{model_name}/{label}]: {i}/{len(questions)} "
                      f"({n_err} errors)", file=sys.stderr)
            if sleep > 0:
                time.sleep(sleep)
    print(f"run: wrote {out_path} ({len(questions)} rows, {n_err} errors)")
    return out_path


def cmd_run(args: argparse.Namespace) -> int:
    run_bench(
        args.questions, args.base_url, args.model_name, args.mode,
        outdir=args.outdir, mode_label=args.mode_label, budget=args.budget,
        index_path=args.index, temp=args.temp, max_tokens=args.max_tokens,
        timeout=args.timeout, retries=args.retries, sleep=args.sleep,
        out=args.out, reasoning_effort=args.reasoning_effort,
        auth_bearer=_resolve_bearer(args.auth_bearer_env),
    )
    return 0


# ---------------------------------------------------------------------------
# SCORE — objective, no LLM needed (optional judge)
# ---------------------------------------------------------------------------

_PUNCT_RE = re.compile(r"[^a-z0-9\s]+")
_WS_RE = re.compile(r"\s+")


def normalise(s: str) -> str:
    return _WS_RE.sub(" ", _PUNCT_RE.sub(" ", s.lower())).strip()


def gold_hit(title: str, norm_answer: str, answer_words: set[str]) -> bool:
    """HIT when the normalised title is a substring of the answer, or >=80%
    of its words of length >= 4 appear in the answer."""
    nt = normalise(title)
    if not nt:
        return False
    if nt in norm_answer:
        return True
    words = [w for w in nt.split() if len(w) >= 4]
    if not words:
        return False
    return sum(1 for w in words if w in answer_words) / len(words) >= 0.8


def score_answer(question: dict, answer: str) -> dict:
    norm = normalise(answer)
    words = set(norm.split())
    gold = question.get("gold") or []
    hits = [g["title"] for g in gold if gold_hit(g["title"], norm, words)]
    if question.get("gold_type") == "any":
        recall = 1.0 if hits else 0.0
    else:
        recall = len(hits) / len(gold) if gold else 0.0
    out = {"recall": round(recall, 4), "hits": hits,
           "n_gold": len(gold), "n_hits": len(hits)}
    # T-TAX gold_extra (inferred ancestors) is credited separately: it never
    # touches the headline recall.
    extra = question.get("gold_extra")
    if extra:
        ehits = [g["title"] for g in extra
                 if gold_hit(g["title"], norm, words)]
        out["extra_recall"] = round(len(ehits) / len(extra), 4)
        out["extra_hits"] = ehits
    return out


JUDGE_SYSTEM = (
    "You are a strict benchmark judge. Grade the candidate answer for "
    "groundedness and correctness against the reference facts from a curated "
    "knowledge graph. Respond with ONLY a single integer from 0 to 5."
)

JUDGE_RUBRIC = (
    "Rubric: 5 = names essentially all reference facts, correct, no "
    "fabrication; 4 = most facts, minor omissions; 3 = some facts, no major "
    "fabrication; 2 = one fact or heavy vagueness; 1 = topical but no "
    "reference facts; 0 = wrong, irrelevant, or fabricated. "
    "Respond with only the integer."
)


def judge_answer(judge_base_url: str, judge_model: str, question: dict,
                 answer: str, timeout: float, retries: int) -> Optional[int]:
    gold_lines = "\n".join("- " + g["title"] for g in question.get("gold", []))
    user = (
        f"Question:\n{question['prompt']}\n\n"
        f"Reference facts (gold, from the knowledge graph):\n{gold_lines}\n\n"
        f"Candidate answer:\n{answer[:4000]}\n\n{JUDGE_RUBRIC}"
    )
    payload = {
        "model": judge_model,
        "messages": [{"role": "system", "content": JUDGE_SYSTEM},
                     {"role": "user", "content": user}],
        "temperature": 0.0,
        "max_tokens": 8,
    }
    resp = chat_request(judge_base_url, payload, timeout, retries)
    content = resp["choices"][0]["message"].get("content") or ""
    m = re.search(r"[0-5]", content)
    if not m:
        raise ValueError(f"judge returned no 0-5 grade: {content!r}")
    return int(m.group(0))


def score_results(
    questions_path: str,
    results_path: str,
    outdir: str = ".",
    judge_base_url: Optional[str] = None,
    judge_model: str = "judge",
    timeout: float = 120.0,
    retries: int = 1,
    out: Optional[str] = None,
) -> tuple[str, str]:
    questions = {q["id"]: q for q in _read_jsonl(questions_path)}
    results = _read_jsonl(results_path)
    if not results:
        raise SystemExit(f"score: empty results file {results_path}")
    model = results[0].get("model") or "model"
    mode = results[0].get("mode") or "mode"

    # Guard against duplicate ids (adversarial pass 2026-08-16, finding 3):
    # duplicates would silently double-count in summaries and collapse in the
    # id-keyed pairing, desyncing n across stages.
    seen_ids: set = set()
    dupes = [r["id"] for r in results
             if r.get("id") in seen_ids or seen_ids.add(r.get("id"))]
    if dupes:
        print(f"score: WARNING {len(dupes)} duplicate result id(s), "
              f"e.g. {dupes[:5]} — expected one row per question",
              file=sys.stderr)

    score_rows: list[dict] = []
    judge_fail_streak = 0
    judge_disabled = judge_base_url is None
    n_judge_fail = 0

    for r in results:
        q = questions.get(r.get("id"))
        row: dict[str, Any] = {
            "id": r.get("id"), "model": model, "mode": mode,
            "domain": (q or {}).get("domain", ""),
            "template": (q or {}).get("template", ""),
            "scaffold_engaged": bool(r.get("scaffold_engaged")),
            "injected_tokens": r.get("injected_tokens", 0),
            "latency_ms": r.get("latency_ms"),
            # observability carried through for the summary (adversarial pass)
            "n_gold": r.get("n_gold"),
            "n_gold_exposed": r.get("n_gold_exposed"),
            "finish_reason": r.get("finish_reason"),
            "attempts": r.get("attempts"),
        }
        if q is None:
            row["error"] = "question id not found in questions file"
            row["recall"] = None
        elif "error" in r:
            row["error"] = r["error"]
            row["recall"] = None
        else:
            row.update(score_answer(q, r.get("answer") or ""))
            if not judge_disabled:
                try:
                    row["judge"] = judge_answer(
                        judge_base_url, judge_model, q,
                        r.get("answer") or "", timeout, retries)
                    judge_fail_streak = 0
                except Exception as exc:
                    n_judge_fail += 1
                    judge_fail_streak += 1
                    row["judge"] = None
                    row["judge_error"] = str(exc)
                    if judge_fail_streak >= 3:
                        judge_disabled = True
                        print("score: judge disabled after 3 consecutive "
                              "failures — degrading to objective-only",
                              file=sys.stderr)
        score_rows.append(row)

    os.makedirs(outdir, exist_ok=True)
    base = f"{_fsafe(model)}-{_fsafe(mode)}"
    if out:
        scores_path = out
        root = out[:-6] if out.endswith(".jsonl") else out
        summary_path = root + "-summary.json"
    else:
        scores_path = os.path.join(outdir, f"scores-{base}.jsonl")
        summary_path = os.path.join(outdir, f"summary-{base}.json")
    _write_jsonl(scores_path, score_rows)

    scored = [r for r in score_rows if r.get("recall") is not None]
    recalls = [r["recall"] for r in scored]
    engaged = [r["recall"] for r in scored if r["scaffold_engaged"]]
    extra = [r["extra_recall"] for r in scored if "extra_recall" in r]
    judges = [r["judge"] for r in scored
              if isinstance(r.get("judge"), int)]
    per_dom: dict[str, list[float]] = {}
    per_tmpl: dict[str, list[float]] = {}
    for r in scored:
        per_dom.setdefault(r["domain"], []).append(r["recall"])
        per_tmpl.setdefault(r["template"], []).append(r["recall"])
    # Deterministic-copy ceiling: per-row fraction of gold already present in
    # what the model was shown. If this ~= mean_recall, most of the "recall" is
    # copy-from-context, not reasoning (adversarial pass 2026-08-16, findings 1/2).
    exp = [r["n_gold_exposed"] / r["n_gold"] for r in scored
           if isinstance(r.get("n_gold_exposed"), int) and (r.get("n_gold") or 0) > 0]
    n_trunc = sum(1 for r in score_rows if r.get("finish_reason") == "length")
    retries_used = [r["attempts"] for r in score_rows
                    if isinstance(r.get("attempts"), int)]
    summary = {
        "model": model,
        "mode": mode,
        "n": len(score_rows),
        "n_scored": len(scored),
        "n_errors": len(score_rows) - len(scored),
        "mean_recall": _mean(recalls),
        "mean_recall_engaged_only": _mean(engaged),
        "n_engaged": len(engaged),
        "engagement_rate": (len(engaged) / len(scored)) if scored else None,
        "mean_extra_recall": _mean(extra),
        "n_extra_scored": len(extra),
        # copy-baseline: the recall a no-op extractor of the injected context
        # would get, and the headline's genuine gain over it.
        "mean_gold_exposed_recall": _mean(exp),
        "recall_gain_over_exposure": (
            None if _mean(recalls) is None or _mean(exp) is None
            else round(_mean(recalls) - _mean(exp), 4)),
        # truncation + transport auditability
        "n_truncated_finish_length": n_trunc,
        "n_with_retries_gt1": sum(1 for a in retries_used if a > 1),
        "judge_mean": _mean([float(j) for j in judges]),
        "n_judged": len(judges),
        "n_judge_failures": n_judge_fail,
        "mean_injected_tokens": _mean(
            [float(r.get("injected_tokens") or 0) for r in scored]),
        "mean_latency_ms": _mean(
            [float(r["latency_ms"]) for r in scored
             if isinstance(r.get("latency_ms"), (int, float))]),
        "per_domain_mean_recall": {d: _mean(v) for d, v in sorted(per_dom.items())},
        "per_template_mean_recall": {t: _mean(v) for t, v in sorted(per_tmpl.items())},
    }
    with open(summary_path, "w", encoding="utf-8") as fh:
        json.dump(summary, fh, indent=2)
    print(f"score: {model}/{mode} -> {scores_path}")
    print(json.dumps(summary, indent=2))
    return scores_path, summary_path


def cmd_score(args: argparse.Namespace) -> int:
    score_results(
        args.questions, args.results, outdir=args.outdir,
        judge_base_url=args.judge_base_url, judge_model=args.judge_model,
        timeout=args.timeout, retries=args.retries, out=args.out,
    )
    return 0


# ---------------------------------------------------------------------------
# REPORT
# ---------------------------------------------------------------------------

def bootstrap_ci(deltas: list[float], resamples: int = 10000,
                 seed: int = DEFAULT_SEED) -> tuple[float, float]:
    rng = random.Random(seed)
    n = len(deltas)
    means = sorted(
        sum(deltas[rng.randrange(n)] for _ in range(n)) / n
        for _ in range(resamples)
    )
    lo = means[max(0, int(0.025 * resamples))]
    hi = means[min(resamples - 1, int(0.975 * resamples))]
    return lo, hi


def bootstrap_ci_clustered(clusters: list[list[float]], resamples: int = 10000,
                           seed: int = DEFAULT_SEED) -> tuple[float, float]:
    """Cluster (block) bootstrap: resample whole clusters with replacement,
    then pool their deltas. Questions cluster by domain (and by seed class),
    so the naive per-question bootstrap treats correlated observations as
    independent and reports too-narrow an interval (adversarial pass
    2026-08-16, finding 4). Resampling at the domain level restores the
    domain-to-domain uncertainty the naive interval hides."""
    clusters = [c for c in clusters if c]
    k = len(clusters)
    if k < 2:
        return (float("nan"), float("nan"))
    rng = random.Random(seed)
    means: list[float] = []
    for _ in range(resamples):
        pooled: list[float] = []
        for _ in range(k):
            pooled.extend(clusters[rng.randrange(k)])
        if pooled:
            means.append(sum(pooled) / len(pooled))
    means.sort()
    m = len(means)
    return (means[max(0, int(0.025 * m))], means[min(m - 1, int(0.975 * m))])


def build_report(score_specs: list[str], resamples: int = 10000,
                 seed: int = DEFAULT_SEED) -> str:
    """Build the markdown report.

    ``score_specs`` entries are either bare paths (model/mode taken from the
    rows) or ``model/mode=path`` labels that override the rows' own labels.
    """
    groups: dict[tuple[str, str], list[dict]] = {}
    paths: list[str] = []
    for spec in score_specs:
        label: Optional[tuple[str, str]] = None
        path = spec
        if "=" in spec and not os.path.exists(spec):
            lab, path = spec.split("=", 1)
            if "/" not in lab:
                raise SystemExit(
                    f"report: --scores label must be model/mode=path, got {spec!r}")
            model_l, mode_l = lab.split("/", 1)
            label = (model_l, mode_l)
        paths.append(path)
        for r in _read_jsonl(path):
            if label is not None:
                r = {**r, "model": label[0], "mode": label[1]}
            groups.setdefault((r["model"], r["mode"]), []).append(r)
    if not groups:
        raise SystemExit("report: no score rows found")

    keys = sorted(groups)
    lines: list[str] = []
    lines.append("# Ontology Uplift Benchmark Report")
    lines.append("")
    lines.append(f"Score files: {', '.join(os.path.basename(p) for p in paths)}")
    lines.append("")

    # -- summary table -----------------------------------------------------
    lines.append("## Summary (model x mode)")
    lines.append("")
    lines.append("| model | mode | n | errors | mean recall | recall (engaged only) "
                 "| copy ceiling (gold exposed) | gain over copy "
                 "| extra recall (T-TAX ancestors) | judge 0-5 | mean injected tok | mean latency ms |")
    lines.append("|---|---|---|---|---|---|---|---|---|---|---|---|")
    for (model, mode) in keys:
        rows = groups[(model, mode)]
        scored = [r for r in rows if r.get("recall") is not None]
        recalls = [r["recall"] for r in scored]
        eng = [r["recall"] for r in scored if r.get("scaffold_engaged")]
        extra = [r["extra_recall"] for r in scored if "extra_recall" in r]
        judges = [float(r["judge"]) for r in scored
                  if isinstance(r.get("judge"), int)]
        inj = [float(r.get("injected_tokens") or 0) for r in scored]
        lat = [float(r["latency_ms"]) for r in scored
               if isinstance(r.get("latency_ms"), (int, float))]
        exp = [r["n_gold_exposed"] / r["n_gold"] for r in scored
               if isinstance(r.get("n_gold_exposed"), int) and (r.get("n_gold") or 0) > 0]
        gain = (None if _mean(recalls) is None or _mean(exp) is None
                else _mean(recalls) - _mean(exp))
        lines.append(
            f"| {model} | {mode} | {len(rows)} | {len(rows) - len(scored)} "
            f"| {_fmt(_mean(recalls))} | {_fmt(_mean(eng))} "
            f"| {_fmt(_mean(exp))} | {_fmt(gain)} "
            f"| {_fmt(_mean(extra))} | {_fmt(_mean(judges), 2)} "
            f"| {_fmt(_mean(inj), 0)} | {_fmt(_mean(lat), 0)} |")
    lines.append("")
    lines.append("*Copy ceiling* = mean fraction of gold titles already present in "
                 "what the model was shown (raw ≈ 0; a no-op extractor of the "
                 "injected context would score ~this). *Gain over copy* = mean recall "
                 "− copy ceiling: the recall not explained by verbatim exposure.")
    lines.append("")

    # -- per-domain / per-template ----------------------------------------
    def _breakdown(title: str, field: str) -> None:
        cats = sorted({r[field] for rows in groups.values() for r in rows
                       if r.get(field)})
        if not cats:
            return
        lines.append(f"## Mean recall by {title}")
        lines.append("")
        header = "| " + title + " | " + " | ".join(
            f"{m} ({mo})" for m, mo in keys) + " |"
        lines.append(header)
        lines.append("|" + "---|" * (len(keys) + 1))
        for cat in cats:
            cells = []
            for k in keys:
                vals = [r["recall"] for r in groups[k]
                        if r.get(field) == cat and r.get("recall") is not None]
                cells.append(_fmt(_mean(vals)))
            lines.append(f"| {cat} | " + " | ".join(cells) + " |")
        lines.append("")

    _breakdown("domain", "domain")
    _breakdown("template", "template")

    # -- paired uplift -----------------------------------------------------
    lines.append("## Paired uplift (per model, vs raw, intersection of question ids)")
    lines.append("")
    models = sorted({m for m, _ in keys})
    any_pair = False
    for model in models:
        raw_rows = {r["id"]: r for r in groups.get((model, "raw"), [])}
        if not raw_rows:
            continue
        for (m, mode) in keys:
            if m != model or mode == "raw":
                continue
            other = {r["id"]: r for r in groups[(m, mode)]}
            ids = sorted(set(raw_rows) & set(other))
            deltas: list[float] = []
            itt_deltas: list[float] = []          # intention-to-treat: keep non-engaged
            clusters: dict[str, list[float]] = {}  # engaged deltas grouped by domain
            n_not_engaged = 0
            n_errors = 0
            not_engaged_pairs: list[tuple[float, float]] = []
            not_engaged_ids: list[str] = []
            for qid in ids:
                a, b = raw_rows[qid], other[qid]
                if a.get("recall") is None or b.get("recall") is None:
                    n_errors += 1
                    continue
                d = b["recall"] - a["recall"]
                itt_deltas.append(d)  # ITT: both arms answered; keep the real delta
                if not b.get("scaffold_engaged"):
                    n_not_engaged += 1
                    not_engaged_pairs.append((a["recall"], b["recall"]))
                    not_engaged_ids.append(str(qid))
                    continue
                deltas.append(d)
                clusters.setdefault(b.get("domain") or "", []).append(d)
            any_pair = True
            if deltas:
                d_mean = sum(deltas) / len(deltas)
                lo, hi = bootstrap_ci(deltas, resamples=resamples, seed=seed)
                line = (f"PAIRED UPLIFT {model} ({mode} - raw): "
                        f"delta={d_mean:+.3f} recall, 95% CI "
                        f"[{lo:+.3f}, {hi:+.3f}] "
                        f"(bootstrap {resamples} resamples, seed {seed}), "
                        f"n={len(deltas)}, excluded_not_engaged={n_not_engaged}, "
                        f"excluded_errors={n_errors}")
            else:
                line = (f"PAIRED UPLIFT {model} ({mode} - raw): no engaged "
                        f"error-free pairs (excluded_not_engaged="
                        f"{n_not_engaged}, excluded_errors={n_errors})")
            lines.append("- " + line)
            # Domain-clustered CI + intention-to-treat, per the adversarial pass
            # (findings 4 & 6): the naive CI above assumes independent questions;
            # ITT keeps the non-engaged pairs instead of dropping them.
            if deltas:
                clo, chi = bootstrap_ci_clustered(
                    list(clusters.values()), resamples=resamples, seed=seed)
                if clo == clo:  # not NaN
                    lines.append(
                        f"  - domain-clustered 95% CI [{clo:+.3f}, {chi:+.3f}] "
                        f"({len(clusters)} domain clusters) — wider than the naive "
                        f"interval because questions cluster within domain/class.")
                if itt_deltas:
                    itt_mean = sum(itt_deltas) / len(itt_deltas)
                    lines.append(
                        f"  - intention-to-treat delta={itt_mean:+.3f} over all "
                        f"n={len(itt_deltas)} answered pairs (non-engaged kept at "
                        f"their real, ~0, delta rather than excluded).")
            if not_engaged_pairs:
                ra = _mean([p[0] for p in not_engaged_pairs])
                rb = _mean([p[1] for p in not_engaged_pairs])
                lines.append(
                    f"  - not-engaged questions ({n_not_engaged}): both arms "
                    f"saw the identical prompt — they measure nothing about "
                    f"uplift and are excluded above. For the record: raw "
                    f"recall {_fmt(ra)}, {mode} recall {_fmt(rb)} on those.")
                shown = ", ".join(not_engaged_ids[:25])
                if len(not_engaged_ids) > 25:
                    shown += f", … (+{len(not_engaged_ids) - 25} more)"
                lines.append(f"  - not-engaged ids: {shown}")
    if not any_pair:
        lines.append("- no model has both a raw run and a non-raw run; "
                     "no paired uplift computable")
    lines.append("")
    lines.append(HONEST_NOTES)
    return "\n".join(lines)


def cmd_report(args: argparse.Namespace) -> int:
    text = build_report(args.scores, resamples=args.resamples, seed=args.seed)
    with open(args.out, "w", encoding="utf-8") as fh:
        fh.write(text)
    print(f"report: wrote {args.out}")
    for line in text.splitlines():
        if line.lstrip("- ").startswith("PAIRED UPLIFT"):
            print(line.lstrip("- "))
    return 0


# ---------------------------------------------------------------------------
# ALL
# ---------------------------------------------------------------------------

def cmd_all(args: argparse.Namespace) -> int:
    endpoints: list[tuple[str, str]] = []
    for spec in args.endpoint:
        if "=" not in spec:
            raise SystemExit(f"--endpoint must be name=url, got: {spec!r}")
        name, url = spec.split("=", 1)
        endpoints.append((name.strip(), url.strip()))
    if not endpoints:
        raise SystemExit("all: at least one --endpoint name=url is required")

    os.makedirs(args.outdir, exist_ok=True)
    questions_path = os.path.join(args.outdir, "questions.jsonl")
    gen_args = argparse.Namespace(
        index=args.index, out=questions_path, seed=args.seed,
        per_domain=args.per_domain,
        min_domain_classes=args.min_domain_classes,
        min_quality=args.min_quality, min_def_len=args.min_def_len)
    cmd_generate(gen_args)

    auth_bearer = _resolve_bearer(getattr(args, "auth_bearer_env", None))
    score_files: list[str] = []
    for name, url in endpoints:
        for mode in ("raw", "scaffold"):
            results = run_bench(
                questions_path, url, name, mode, outdir=args.outdir,
                budget=args.budget, index_path=args.index, temp=args.temp,
                max_tokens=args.max_tokens, timeout=args.timeout,
                retries=args.retries, sleep=args.sleep,
                reasoning_effort=getattr(args, "reasoning_effort", None),
                auth_bearer=auth_bearer)
            scores, _summary = score_results(
                questions_path, results, outdir=args.outdir,
                judge_base_url=args.judge_base_url,
                judge_model=args.judge_model, timeout=args.timeout)
            score_files.append(scores)

    report_path = os.path.join(args.outdir, "report.md")
    rep_args = argparse.Namespace(scores=score_files, out=report_path,
                                  resamples=args.resamples, seed=args.seed)
    cmd_report(rep_args)
    return 0


# ---------------------------------------------------------------------------
# SELFTEST — inline fixture + stub OpenAI server, full pipeline in a tempdir
# ---------------------------------------------------------------------------

def _long(base: str) -> str:
    """Pad a definition past the 120-char eligibility threshold."""
    pad = (" It is exercised only by the bench selftest fixture and exists "
           "to satisfy the definition-length eligibility gate.")
    while len(base) < 120:
        base += pad
    return base


_BENCH_FIXTURE: dict[str, Any] = {
    "version": 1,
    "generated": "2026-08-11T00:00:00Z",
    "counts": {"classes": 16},
    "classes": {
        "core-system": {
            "t": "Core System",
            "d": "The root concept of the selftest fixture taxonomy.",
            "dom": "testdom", "q": 0.9, "m": "mature",
            "sup": [], "isup": [], "bl": [],
        },
        "base-platform": {
            "t": "Base Platform",
            "d": "An intermediate ancestor concept in the fixture taxonomy.",
            "dom": "testdom", "q": 0.9, "m": "mature",
            "sup": ["core-system"], "isup": ["core-system"], "bl": [],
        },
        "signal-router": {
            "t": "Signal Router",
            "d": "A fixture component that routes signals.",
            "dom": "testdom", "q": 0.8, "m": "mature",
            "sup": ["core-system"], "isup": ["core-system"], "bl": [],
        },
        "data-fabric": {
            "t": "Data Fabric",
            "d": "A fixture component that moves data.",
            "dom": "testdom", "q": 0.8, "m": "mature",
            "sup": ["core-system"], "isup": ["core-system"], "bl": [],
        },
        "policy-engine": {
            "t": "Policy Engine",
            "d": "A fixture component that evaluates policies.",
            "dom": "testdom", "q": 0.8, "m": "mature",
            "sup": ["core-system"], "isup": ["core-system"], "bl": [],
        },
        "event-mesh": {
            "t": "Event Mesh",
            "d": "A fixture component that distributes events.",
            "dom": "testdom", "q": 0.8, "m": "mature",
            "sup": ["core-system"], "isup": ["core-system"], "bl": [],
        },
        "trust-anchor": {
            "t": "Trust Anchor",
            "d": "A fixture component that anchors trust decisions.",
            "dom": "testdom", "q": 0.8, "m": "mature",
            "sup": ["core-system"], "isup": ["core-system"], "bl": [],
        },
        "alpha-orchestrator": {
            "t": "Alpha Orchestrator",
            "d": _long("The Alpha Orchestrator coordinates fixture workloads "
                       "across the platform, scheduling components and "
                       "reconciling their state."),
            "dom": "testdom", "q": 0.9, "m": "mature",
            "sup": ["base-platform"], "isup": ["base-platform", "core-system"],
            "rel": {"requires": ["signal-router", "data-fabric"],
                    "hasPart": ["policy-engine", "event-mesh", "trust-anchor"]},
            "bl": [],
        },
        "beta-scheduler": {
            "t": "Beta Scheduler",
            "d": _long("The Beta Scheduler assigns fixture tasks to available "
                       "components and balances load across the platform over "
                       "time."),
            "dom": "testdom", "q": 0.9, "m": "mature",
            "sup": ["base-platform"], "isup": ["base-platform", "core-system"],
            "rel": {"uses": ["signal-router", "policy-engine"],
                    "dependsOn": ["data-fabric", "event-mesh"]},
            "bl": [],
        },
        "gamma-gateway": {
            "t": "Gamma Gateway",
            "d": _long("The Gamma Gateway exposes fixture services to "
                       "external callers, enforcing policy on every request "
                       "that crosses the boundary."),
            "dom": "testdom", "q": 0.85, "m": "mature",
            "sup": ["core-system"], "isup": ["core-system"],
            "rel": {"enables": ["event-mesh", "trust-anchor"],
                    "requires": ["signal-router", "policy-engine",
                                 "data-fabric"]},
            "bl": [],
        },
        # -- second domain, so per-domain tables and stratification are real --
        "meta-root": {
            "t": "Meta Root",
            "d": "The root concept of the second fixture domain.",
            "dom": "otherdom", "q": 0.9, "m": "mature",
            "sup": [], "isup": [], "bl": [],
        },
        "meta-layer": {
            "t": "Meta Layer",
            "d": "An intermediate ancestor in the second fixture domain.",
            "dom": "otherdom", "q": 0.9, "m": "mature",
            "sup": ["meta-root"], "isup": ["meta-root"], "bl": [],
        },
        "flux-buffer": {
            "t": "Flux Buffer",
            "d": "A second-domain component that buffers flux.",
            "dom": "otherdom", "q": 0.8, "m": "mature",
            "sup": ["meta-root"], "isup": ["meta-root"], "bl": [],
        },
        "sync-relay": {
            "t": "Sync Relay",
            "d": "A second-domain component that relays sync pulses.",
            "dom": "otherdom", "q": 0.8, "m": "mature",
            "sup": ["meta-root"], "isup": ["meta-root"], "bl": [],
        },
        "delta-analyzer": {
            "t": "Delta Analyzer",
            "d": _long("The Delta Analyzer inspects fixture deltas across "
                       "the second domain and reports drift between "
                       "components over time."),
            # q is null on purpose: exercises the null-counts-as-0.65 rule.
            "dom": "otherdom", "q": None, "m": "mature",
            "sup": ["meta-layer"], "isup": ["meta-layer", "meta-root"],
            "rel": {"requires": ["flux-buffer", "sync-relay"],
                    "uses": ["sync-relay", "flux-buffer"]},
            "bl": [],
        },
        "epsilon-monitor": {
            "t": "Epsilon Monitor",
            "d": _long("The Epsilon Monitor watches second-domain components "
                       "and raises alerts when their observed state diverges "
                       "from the declared state."),
            "dom": "otherdom", "q": 0.9, "m": "mature",
            "sup": ["meta-layer"], "isup": ["meta-layer", "meta-root"],
            "rel": {"dependsOn": ["flux-buffer", "sync-relay"],
                    "enables": ["sync-relay", "flux-buffer"]},
            "bl": [],
        },
    },
}


def _selftest() -> int:
    import tempfile
    import threading
    from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

    failures: list[str] = []

    def check(name: str, cond: bool, detail: str = "") -> None:
        status = "ok" if cond else "FAIL"
        print(f"  [{status}] {name}" + (f" — {detail}" if detail and not cond else ""))
        if not cond:
            failures.append(name)

    print("selftest: bench_ontology_uplift")

    # ontology_scaffold's inline _FIXTURE is too small/sparse for the generate
    # gates (definition length, >=2 rel types), so use the embedded bench
    # fixture, which follows the same v1 schema.
    fixture = _BENCH_FIXTURE
    fixture_titles: list[str] = [e["t"] for e in fixture["classes"].values()]
    # Weak parametric "knowledge": ~20% of the fixture titles.
    weak_titles = [t for i, t in enumerate(fixture_titles) if i % 5 == 0]

    class StubModel(BaseHTTPRequestHandler):
        """Stub OpenAI server whose answer QUALITY depends on the request:
        with [ONTOLOGY CONTEXT] present it echoes ~80% of the gold-ish titles
        found in that context; otherwise it gives a generic weak answer
        containing ~20% of the titles."""

        def log_message(self, *a):  # noqa: N802
            pass

        def do_POST(self):  # noqa: N802
            length = int(self.headers.get("Content-Length") or 0)
            body = json.loads(self.rfile.read(length).decode("utf-8"))
            msgs = body.get("messages") or []
            text = "\n".join(m.get("content") or "" for m in msgs
                             if isinstance(m.get("content"), str))
            if "[ONTOLOGY CONTEXT]" in text:
                found = [t for t in fixture_titles if t in text]
                kept = [t for i, t in enumerate(found) if i % 5 != 4]  # ~80%
                answer = ("Grounded in the provided graph context, the "
                          "relevant concepts are: " + ", ".join(kept) + ".")
            else:
                answer = ("I am not certain, but it may involve "
                          + ", ".join(weak_titles) + ".")
            resp = {
                "id": "chatcmpl-stub", "object": "chat.completion",
                "created": 0, "model": body.get("model", "stub"),
                "choices": [{"index": 0, "finish_reason": "stop",
                             "message": {"role": "assistant",
                                         "content": answer}}],
                "usage": {"prompt_tokens": len(text) // 4,
                          "completion_tokens": len(answer) // 4,
                          "total_tokens": (len(text) + len(answer)) // 4},
            }
            payload = json.dumps(resp).encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

    with tempfile.TemporaryDirectory(prefix="bench-uplift-selftest-") as tmp:
        index_path = os.path.join(tmp, "scaffold-index.json")
        with open(index_path, "w", encoding="utf-8") as fh:
            json.dump(fixture, fh)

        # -- generate: deterministic, all templates -----------------------
        gen_kwargs = dict(seed=DEFAULT_SEED, per_domain=8,
                          min_domain_classes=1)
        q1 = generate_questions(index_path, **gen_kwargs)
        q2 = generate_questions(index_path, **gen_kwargs)
        check("generate deterministic", json.dumps(q1) == json.dumps(q2))
        tmpls = {q["template"] for q in q1}
        check("all three templates generated",
              tmpls == {"T-REL", "T-TAX", "T-COMMON"}, repr(tmpls))
        check("some questions generated", len(q1) >= 8, str(len(q1)))
        check("ids are q0001-style and sequential",
              q1[0]["id"] == "q0001" and all(
                  re.fullmatch(r"q\d{4}", q["id"]) for q in q1))
        check("both fixture domains covered",
              {q["domain"] for q in q1} == {"testdom", "otherdom"})
        check("null-quality class treated as 0.65 (eligible)",
              any("delta-analyzer" in q["class_slugs"] for q in q1))
        check("T-REL gold sizes 2-5", all(
            2 <= len(q["gold"]) <= 5 for q in q1 if q["template"] == "T-REL"))
        check("T-COMMON is any-type", all(
            q["gold_type"] == "any" for q in q1 if q["template"] == "T-COMMON"))
        check("some T-TAX carry gold_extra (ancestors)", any(
            q.get("gold_extra") for q in q1 if q["template"] == "T-TAX"))
        questions_path = os.path.join(tmp, "questions.jsonl")
        _write_jsonl(questions_path, q1)

        # -- stub server ---------------------------------------------------
        httpd = ThreadingHTTPServer(("127.0.0.1", 0), StubModel)
        threading.Thread(target=httpd.serve_forever, daemon=True).start()
        base_url = "http://127.0.0.1:%d/v1" % httpd.server_address[1]

        try:
            # -- run both modes -------------------------------------------
            raw_path = run_bench(questions_path, base_url, "stub-model",
                                 "raw", outdir=tmp, index_path=index_path,
                                 timeout=15, retries=1)
            sc_path = run_bench(questions_path, base_url, "stub-model",
                                "scaffold", outdir=tmp,
                                index_path=index_path, timeout=15, retries=1)
            raw_rows = _read_jsonl(raw_path)
            sc_rows = _read_jsonl(sc_path)
            check("run produced all rows",
                  len(raw_rows) == len(q1) and len(sc_rows) == len(q1))
            check("no run errors", all("error" not in r
                                       for r in raw_rows + sc_rows))
            check("scaffold engaged on all questions",
                  all(r["scaffold_engaged"] for r in sc_rows))
            check("raw never engaged",
                  not any(r["scaffold_engaged"] for r in raw_rows))
            check("injected tokens recorded", all(
                r["injected_tokens"] > 0 for r in sc_rows))

            # -- score -----------------------------------------------------
            raw_scores, raw_summary = score_results(
                questions_path, raw_path, outdir=tmp)
            sc_scores, sc_summary = score_results(
                questions_path, sc_path, outdir=tmp)
            with open(raw_summary, encoding="utf-8") as fh:
                raw_sum = json.load(fh)
            with open(sc_summary, encoding="utf-8") as fh:
                sc_sum = json.load(fh)
            check("scaffold mean recall > raw mean recall",
                  sc_sum["mean_recall"] > raw_sum["mean_recall"],
                  f"scaffold={sc_sum['mean_recall']} raw={raw_sum['mean_recall']}")
            check("scaffold recall strong on stub (80% echo)",
                  sc_sum["mean_recall"] >= 0.7, str(sc_sum["mean_recall"]))
            check("raw recall weak on stub (20% titles)",
                  raw_sum["mean_recall"] <= 0.5, str(raw_sum["mean_recall"]))
            check("extra recall (T-TAX ancestors) reported for scaffold",
                  sc_sum["mean_extra_recall"] is not None
                  and sc_sum["mean_extra_recall"] > 0.5,
                  str(sc_sum["mean_extra_recall"]))

            # -- report ----------------------------------------------------
            report = build_report([raw_scores, sc_scores], resamples=2000,
                                  seed=DEFAULT_SEED)
            report_path = os.path.join(tmp, "report.md")
            with open(report_path, "w", encoding="utf-8") as fh:
                fh.write(report)
            check("report contains 'Paired'", "Paired" in report)
            check("report contains paired-delta line",
                  "PAIRED UPLIFT stub-model (scaffold - raw): delta=+" in report,
                  report[:400])
            check("report contains honest notes",
                  "Honest notes" in report and "BY DESIGN" in report)
            check("report has summary table",
                  "| stub-model | scaffold |" in report)
            m = re.search(r"delta=\+([0-9.]+) recall, 95% CI "
                          r"\[\+?(-?[0-9.]+), \+?(-?[0-9.]+)\]", report)
            check("bootstrap CI parses and is positive",
                  m is not None and float(m.group(2)) > 0.0,
                  m.group(0) if m else "no match")
        finally:
            httpd.shutdown()
            httpd.server_close()

    if failures:
        print(f"selftest: {len(failures)} FAILURE(S): {failures}")
        return 1
    print("selftest: PASS")
    return 0


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def _add_gen_args(ap: argparse.ArgumentParser) -> None:
    ap.add_argument("--index", default=None,
                    help="scaffold-index.json path (default: $ONTOLOGY_INDEX "
                         "or ~/githubs/loom/app/data/scaffold-index.json)")
    ap.add_argument("--seed", type=int, default=DEFAULT_SEED)
    ap.add_argument("--per-domain", type=int, default=12,
                    help="classes sampled per domain (default 12)")
    ap.add_argument("--min-domain-classes", type=int, default=50,
                    help="only domains with at least this many classes (default 50)")
    ap.add_argument("--min-quality", type=float, default=0.6)
    ap.add_argument("--min-def-len", type=int, default=120)


def _add_run_args(ap: argparse.ArgumentParser) -> None:
    ap.add_argument("--budget", type=int, default=1500,
                    help="scaffold token budget (default 1500)")
    ap.add_argument("--temp", type=float, default=0.0)
    ap.add_argument("--max-tokens", type=int, default=400)
    ap.add_argument("--timeout", type=float, default=120.0)
    ap.add_argument("--retries", type=int, default=2)
    ap.add_argument("--sleep", type=float, default=0.0,
                    help="seconds between calls (default 0)")
    ap.add_argument("--reasoning-effort", default=None,
                    choices=("low", "medium", "high"),
                    help="OpenAI-compat reasoning_effort for thinking models "
                         "(Gemini 3.x maps it to thinking_level). Use 'low' so "
                         "mandatory thinking does not consume the answer budget.")
    ap.add_argument("--auth-bearer-env", default=None, metavar="ENV_VAR",
                    help="name of an env var holding a bearer token, sent as "
                         "Authorization: Bearer <token> (e.g. GOOGLE_API_KEY for "
                         "Gemini). Passed by name so the secret never enters argv.")


def _resolve_bearer(env_name: Optional[str]) -> Optional[str]:
    if not env_name:
        return None
    val = os.environ.get(env_name)
    if not val:
        raise SystemExit(
            f"--auth-bearer-env {env_name}: environment variable is empty/unset")
    return val


def main(argv: Optional[list[str]] = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    if "--selftest" in argv:
        return _selftest()

    p = argparse.ArgumentParser(
        description="Measure LLM ontology uplift (graph-derived gold; "
                    "raw vs scaffold modes). Run with --selftest to verify "
                    "the whole pipeline offline.")
    sub = p.add_subparsers(dest="cmd", required=True)

    g = sub.add_parser("generate", help="build questions.jsonl from the index")
    _add_gen_args(g)
    g.add_argument("--out", default="questions.jsonl")

    r = sub.add_parser("run", help="run one model in one mode")
    r.add_argument("--questions", required=True)
    r.add_argument("--base-url", required=True,
                   help="OpenAI-compatible base URL, e.g. "
                        "http://127.0.0.1:8085/v1 — REQUIRED on purpose: "
                        "never defaults to a live port")
    r.add_argument("--model-name", required=True, help="label for this model")
    r.add_argument("--mode", choices=("raw", "scaffold", "scaffold-prose"),
                   required=True)
    r.add_argument("--mode-label", default=None,
                   help="override the recorded mode label (e.g. 'tools' when "
                        "running raw through the proxy in tools mode)")
    r.add_argument("--index", default=None,
                   help="scaffold index (scaffold mode; default env/standard path)")
    r.add_argument("--outdir", default=".")
    r.add_argument("--out", default=None,
                   help="results path (default results-<model>-<mode>.jsonl "
                        "in --outdir)")
    _add_run_args(r)

    s = sub.add_parser("score", help="objective scoring (+ optional judge)")
    s.add_argument("--questions", required=True)
    s.add_argument("--results", required=True)
    s.add_argument("--outdir", default=".")
    s.add_argument("--out", default=None,
                   help="scores path (default scores-<model>-<mode>.jsonl "
                        "in --outdir; summary json written alongside)")
    s.add_argument("--judge-base-url", default=None,
                   help="optional LLM judge endpoint. The judge model must "
                        "NEVER be the model under test — use the other model "
                        "on the box or a third endpoint.")
    s.add_argument("--judge-model", default="judge")
    s.add_argument("--timeout", type=float, default=120.0)
    s.add_argument("--retries", type=int, default=1)

    rep = sub.add_parser("report", help="markdown report over score files")
    rep.add_argument("--scores", action="append", required=True,
                     metavar="[MODEL/MODE=]PATH",
                     help="repeatable: a scores-*.jsonl file, optionally "
                          "labelled model/mode=path (label overrides the "
                          "rows' own model/mode)")
    rep.add_argument("--out", default="report.md")
    rep.add_argument("--resamples", type=int, default=10000)
    rep.add_argument("--seed", type=int, default=DEFAULT_SEED)

    a = sub.add_parser("all", help="generate -> run per endpoint (both modes) "
                                   "-> score -> report")
    _add_gen_args(a)
    a.add_argument("--endpoint", action="append", default=[],
                   metavar="NAME=URL",
                   help="repeatable: model label = OpenAI base URL")
    a.add_argument("--outdir", default="uplift-results")
    a.add_argument("--judge-base-url", default=None)
    a.add_argument("--judge-model", default="judge")
    a.add_argument("--resamples", type=int, default=10000)
    _add_run_args(a)

    args = p.parse_args(argv)
    if args.cmd == "generate":
        return cmd_generate(args)
    if args.cmd == "run":
        return cmd_run(args)
    if args.cmd == "score":
        return cmd_score(args)
    if args.cmd == "report":
        return cmd_report(args)
    if args.cmd == "all":
        return cmd_all(args)
    p.error("unknown command")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
