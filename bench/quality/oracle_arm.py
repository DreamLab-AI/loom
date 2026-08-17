#!/usr/bin/env python3
"""Oracle-retrieval control: does grounding work when the RIGHT content is
retrieved? Separates 'RAG fails' (it doesn't) from 'the harness's retrieval
misses' (the real question).

For each question we bypass the Loom's fuzzy matcher and inject the corpus's
detailed prose (dfull) for the class(es) whose title best matches the question's
topic — an oracle retrieval. bare vs oracle, judged against independent gold.

  oracle >> bare  => RAG works given the right content; the harness RETRIEVAL is
                     the bottleneck (a fixable data-discovery problem).
  oracle ~ bare   => the corpus prose itself is insufficient for these questions.

Usage:
  PYTHONPATH=app python3 bench/quality/oracle_arm.py \
    --questions uplift-results/general/arcane-questions.json,uplift-results/general/thin-questions.json \
    --prose-index app/data/prose-index.json --scaffold-index app/data/scaffold-index.json \
    --outdir uplift-results/oracle --models gemini-2.5-flash-lite,gpt-4.1-mini,mistral-small-24b,claude-haiku-4.5,deepseek-chat
"""
from __future__ import annotations
import argparse, json, os, re, sys, time

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
import bench_ontology_uplift as bu

GEM = "https://generativelanguage.googleapis.com/v1beta/openai/"
OR = "https://openrouter.ai/api/v1"; DS = "https://api.deepseek.com/v1"
MODELS = {
    "gemini-2.5-flash-lite": (GEM, "gemini-2.5-flash-lite", "GOOGLE_API_KEY", None),
    "gpt-4.1-mini": (OR, "openai/gpt-4.1-mini", "OPENROUTER_API_KEY", None),
    "claude-haiku-4.5": (OR, "anthropic/claude-haiku-4.5", "OPENROUTER_API_KEY", None),
    "mistral-small-24b": (OR, "mistralai/mistral-small-24b-instruct-2501", "OPENROUTER_API_KEY", None),
    "deepseek-chat": (DS, "deepseek-chat", "DEEPSEEK_API_KEY", None),
    "qwen3.8-local": ("http://192.168.2.132:8085/v1", "qwen3.8-27B", None, None),
}
PREAMBLE = ("The following is curated reference material retrieved for this question. "
            "Use it as ground truth where relevant.\n\n[REFERENCE MATERIAL]\n")

_WS = re.compile(r"[^a-z0-9]+")
def norm(s): return _WS.sub(" ", (s or "").lower()).strip()
STOP = set("the a an of and or to for in on with what how does is are why explain given "
           "which describe difference between using use precise sense rather than".split())


def build_prose_lookup(prose_index, scaffold_index):
    """slug -> {title, dfull, toks}, plus the graph (si) for traversal."""
    pi = json.load(open(prose_index)).get("pages", {})
    si = json.load(open(scaffold_index)).get("classes", {})
    title_of = {slug: (e.get("t") or slug) for slug, e in si.items()}
    pages = {}
    for slug, pg in pi.items():
        df = (pg.get("dfull") or "") if isinstance(pg, dict) else ""
        title = title_of.get(slug, slug)
        if df:
            pages[slug] = {"title": title, "dfull": df, "toks": set(norm(title).split())}
    return pages, si


def _neighbours(seeds, si, hops=1):
    seen, frontier = set(seeds), set(seeds)
    for _ in range(hops):
        nxt = set()
        for s in frontier:
            e = si.get(s, {})
            for _rel, tgts in (e.get("rel") or {}).items():
                nxt.update(tgts)
            nxt.update(e.get("sup") or []); nxt.update(e.get("isup") or [])
        frontier = nxt - seen; seen |= nxt
    return seen


def oracle_context(topic, question, pages, si, hops=1, max_chars=24000):
    """TRAVERSAL PRELOAD: match seed concept(s) by title overlap with the
    topic/question, traverse their relation+taxonomy neighbourhood (hops), then
    preload the neighbourhood's dfull markdown ranked by relevance to the
    question, up to a large budget. This is the design under test: efficient
    ontology traversal feeding a large context window (no vector search yet)."""
    topic_toks = set(norm(topic).split()) - STOP
    want = topic_toks | (set(norm(question).split()) - STOP)
    # 1. seeds: PRECISE concept match — rank by Jaccard(title, topic), require a
    # strong overlap so generic classes ("AI Core") can't hitch on a shared word.
    # A proxy for the deferred vector-search layer's semantic concept-finding.
    def title_sim(s):
        tt = pages[s]["toks"]
        if not tt or not topic_toks:
            return 0.0
        inter = len(tt & topic_toks)
        jac = inter / len(tt | topic_toks)
        exact = 0.5 if norm(pages[s]["title"]) in norm(topic) else 0.0
        return jac + exact
    scored = sorted(pages, key=lambda s: -title_sim(s))
    seeds = [s for s in scored if title_sim(s) >= 0.34][:3]  # strong-match seeds only
    if not seeds:  # fall back to the single best title match
        seeds = scored[:1] if title_sim(scored[0]) > 0 else []
    if not seeds:
        return "", []
    # 2. traverse the neighbourhood
    hood = _neighbours(seeds, si, hops=hops)
    # 3. rank every neighbourhood page with prose by relevance to the question
    ranked = sorted((s for s in hood if s in pages),
                    key=lambda s: -(len(pages[s]["toks"] & want)
                                    + (3 if s in seeds else 0)))
    out, used, titles = [], 0, []
    for s in ranked:
        p = pages[s]
        block = f"## {p['title']}\n{p['dfull']}\n"
        if used + len(block) > max_chars:
            continue
        out.append(block); used += len(block); titles.append(p["title"])
    return "\n".join(out), titles


def run(label, question, oracle, max_tokens, timeout):
    base, model, keyenv, effort = MODELS[label]
    key = os.environ.get(keyenv) if keyenv else None
    res = {}
    for cond, ctx in (("bare", None), ("oracle", oracle)):
        content = (PREAMBLE + ctx + "\n\n[QUESTION]\n" + question) if ctx else question
        payload = {"model": model, "messages": [{"role": "user", "content": content}],
                   "temperature": 0, "max_tokens": max_tokens}
        if effort: payload["reasoning_effort"] = effort
        st = {}
        try:
            resp = bu.chat_request(base, payload, timeout, 3, auth_bearer=key, stats=st)
            res[cond] = {"answer": resp["choices"][0]["message"].get("content") or "",
                         "attempts": st.get("attempts")}
        except Exception as e:
            res[cond] = {"error": str(e)[:150]}
    return res


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--questions", required=True)
    ap.add_argument("--prose-index", default="app/data/prose-index.json")
    ap.add_argument("--scaffold-index", default="app/data/scaffold-index.json")
    ap.add_argument("--outdir", required=True)
    ap.add_argument("--models", required=True)
    ap.add_argument("--max-tokens", type=int, default=900)
    ap.add_argument("--hops", type=int, default=1, help="ontology-traversal hops from seed concepts")
    ap.add_argument("--max-chars", type=int, default=24000, help="preload budget (~chars); large-context models")
    ap.add_argument("--timeout", type=float, default=120)
    ap.add_argument("--sleep", type=float, default=0.2)
    args = ap.parse_args()
    os.makedirs(args.outdir, exist_ok=True)
    qs = []
    for f in args.questions.split(","):
        qs.extend(json.load(open(f.strip())))
    pages, si = build_prose_lookup(args.prose_index, args.scaffold_index)
    print(f"oracle: {len(pages)} prose pages; {len(qs)} questions; hops={args.hops} max_chars={args.max_chars}", file=sys.stderr)

    # precompute traversal-preload context + coverage per question
    octx = {}
    covered = 0
    for q in qs:
        ctx, titles = oracle_context(q.get("topic", ""), q["question"], pages, si,
                                     hops=args.hops, max_chars=args.max_chars)
        octx[q["id"]] = {"ctx": ctx, "titles": titles[:40], "chars": len(ctx), "n_classes": len(titles)}
        if len(ctx) > 500:
            covered += 1
    json.dump(octx, open(os.path.join(args.outdir, "oracle-context.json"), "w"), indent=2)
    print(f"oracle: {covered}/{len(qs)} questions got >200 chars of matched prose", file=sys.stderr)

    for label in [m.strip() for m in args.models.split(",") if m.strip() in MODELS]:
        out = {"bare": [], "oracle": []}
        for i, q in enumerate(qs, 1):
            ctx = octx[q["id"]]["ctx"]
            r = run(label, q["question"], ctx, args.max_tokens, args.timeout)
            for cond in ("bare", "oracle"):
                out[cond].append({"id": q["id"], "model": label,
                                  "category": q.get("category"),
                                  "oracle_chars": octx[q["id"]]["chars"], **r[cond]})
            if i % 15 == 0:
                print(f"  {label}: {i}/{len(qs)}", file=sys.stderr)
            if args.sleep: time.sleep(args.sleep)
        for cond in ("bare", "oracle"):
            with open(os.path.join(args.outdir, f"results-{label}-{cond}.jsonl"), "w") as fh:
                for row in out[cond]:
                    fh.write(json.dumps(row, ensure_ascii=False) + "\n")
        print(f"wrote {label} bare+oracle", file=sys.stderr)
    print("oracle done")


if __name__ == "__main__":
    raise SystemExit(main())
