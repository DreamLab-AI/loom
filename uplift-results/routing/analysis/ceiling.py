"""Stronger judge-free ceilings over the same exposed rubrics.

Reproduces system-one-eval/src/copy.rs exactly (baseline), then strengthens it.
No trained judge, no label used except the single oracle-tuned decline threshold
the paper already permits (field weights are declared separately where tuned).
"""
import json, math, re, os, sys
from collections import Counter, defaultdict

HERE = os.path.dirname(os.path.abspath(__file__))
NONE = "none"
K1, B, SWEEP = 1.5, 0.75, 41
EXHAUSTIVE = True  # exhaustive midpoint thresholds; set False to reproduce the 41-point grid
STOP = set("""a an the of to for and or in on with without is are be this that it its as at by from
into over under when use used using not never only your you we our their they them there here what
which who how why do does did can could should would may might will shall must if then than else
also more most less least very""".split())

def tok(s):
    return [t for t in re.split(r"[^a-z0-9]+", s.lower()) if len(t) > 2 and t not in STOP]

# ---------- light stemmer (deterministic, rule-based, no training) ----------
def stem(t):
    for suf in ("ationally","ization","isation","ations","ising","izing","ement","ingly","ility",
                "ables","ibles","ances","ences","ments","ional","ising","ized","ised","ing","ers",
                "ies","ive","ion","ial","ely","est","ers","ed","es","al","er","ly","s"):
        if t.endswith(suf) and len(t) - len(suf) >= 4:
            base = t[:-len(suf)]
            if suf == "ies": base += "y"
            if suf in ("ing","ed") and len(base) > 2 and base[-1] == base[-2] and base[-1] not in "lsz":
                base = base[:-1]
            return base
    return t

def tok_s(s):
    return [stem(t) for t in tok(s)]

# ---------- BM25 ----------
def bm25_index(docs):
    n = len(docs)
    lens = [len(d) for d in docs]
    avgdl = sum(lens) / n if n else 0.0
    df = Counter()
    for d in docs: df.update(set(d))
    tfs = [Counter(d) for d in docs]
    idf = {t: math.log(1.0 + (n - c + 0.5) / (c + 0.5)) for t, c in df.items()}
    return tfs, lens, avgdl, idf

def bm25_scores(qterms, idx, k1=K1, b=B, qweights=None):
    tfs, lens, avgdl, idf = idx
    qs = set(qterms) if qweights is None else set(qweights)
    out = []
    for i, tf in enumerate(tfs):
        s = 0.0
        norm = k1 * (1 - b + b * (lens[i] / avgdl if avgdl else 0.0))
        for t in qs:
            f = tf.get(t)
            if not f: continue
            w = 1.0 if qweights is None else qweights[t]
            s += w * idf.get(t, 0.0) * (f * (k1 + 1)) / (f + norm)
        out.append(s)
    return out

# ---------- negative-clause segmentation ----------
NEG = re.compile(
    r"\b(not for|not to|not a |not the |not when|never for|never use|skip (?:for|this|it)|"
    r"do not use|don'?t use|avoid (?:this|using)|rather than|instead of|use \S+ instead|"
    r"prefer \S+|not (?:for )?executing|superseded|not this|choose \S+, not this)\b", re.I)
SPLIT = re.compile(r"(?<=[.;—])\s+|\s+—\s+")

def segment(rubric):
    """Return (positive_text, negative_text). A clause naming an exclusion goes negative."""
    parts = [p for p in SPLIT.split(rubric) if p.strip()]
    pos, neg = [], []
    for p in parts:
        m = NEG.search(p)
        if m:
            # everything from the marker onward is the exclusion; the lead-in stays positive
            pos.append(p[:m.start()]); neg.append(p[m.start():])
        else:
            pos.append(p)
    return " ".join(pos), " ".join(neg)

# ---------- data ----------
cands = json.load(open("/tmp/candidates.json"))
rows  = json.load(open("/tmp/mined-rows.json"))
KEYS  = list(cands.keys())
# ADR: paper's re-adjudication 2026-09-21 — turn 14 browser -> browser-automation.
rows[14]["expected"] = "browser-automation"
rows[14]["judge_ok"] = (rows[14]["judge_pick"] == "browser-automation")
EXPECTED = [r["expected"] for r in rows]
PROMPTS  = [r["prompt"] for r in rows]
JUDGE_OK = [bool(r["judge_ok"]) for r in rows]
JUDGE_PICK = [r["judge_pick"] for r in rows]
CLS   = [r["class"] for r in rows]
_run = json.load(open("/tmp/openjev-run-v2.json"))["cases"]
assert len(_run) == len(rows)
LATE  = [bool(c["late_discriminator"]) for c in _run]   # the run's flag, as the paper reports
ISNONE= [bool(r["is_none"]) for r in rows]
N = len(rows)

# ---------- scoring machinery ----------
def rank_from_scores(scores):
    order = sorted(range(len(KEYS)), key=lambda i: -scores[i])  # stable: ties -> lower index
    top3 = [KEYS[i] for i in order[:3]]
    best = scores[order[0]]
    second = scores[order[1]] if len(order) > 1 else 0.0
    return top3, best, best - second

def evaluate(per_case, expected, decline="abs"):
    """per_case: list of (top3, best_score, margin). Sweep the decline threshold
    on a fixed 41-point grid between observed extremes; return the oracle point."""
    sig = [pc[1] if decline == "abs" else pc[2] for pc in per_case]
    if EXHAUSTIVE:
        # The optimum of a step function of the threshold lies between ADJACENT
        # OBSERVED scores. Enumerate every distinct interval; no grid can miss it.
        obs = sorted(set(sig))
        grid = [obs[0] - 1.0] + [(obs[i] + obs[i+1]) / 2 for i in range(len(obs)-1)] + [obs[-1] + 1.0]
    else:
        lo, hi = min(sig), max(sig)
        grid = [lo + (hi - lo) * i / (SWEEP - 1) for i in range(SWEEP)] if hi > lo else [lo]
    best = None
    for th in grid:
        items = []
        for k, (top3, bs, mg) in enumerate(per_case):
            s = bs if decline == "abs" else mg
            want = expected[k]
            if s < th:
                t3 = [NONE] + top3[:2]
                items.append((want == NONE, want in t3))
            else:
                items.append((top3[0] == want, want in top3))
        t1 = sum(a for a, _ in items) / len(items)
        t3a = sum(b for _, b in items) / len(items)
        fired = sum(1 for (_, bs, mg) in per_case if (bs if decline == "abs" else mg) < th)
        if best is None or t1 > best["top1"] + 1e-12:
            best = dict(threshold=th, top1=t1, top3=t3a, fired=fired, items=items, decline=decline)
    return best

def signed(items, judge_ok):
    g = [ (1 if j else 0) - (1 if c else 0) for (c, _), j in zip(items, judge_ok) ]
    m = sum(g) / len(g)
    var = sum((x - m) ** 2 for x in g) / (len(g) - 1)
    se = math.sqrt(var / len(g))
    jo = sum(1 for x in g if x > 0); co = sum(1 for x in g if x < 0)
    return m, m - 1.96 * se, m + 1.96 * se, jo, co

POPS = [
    ("all items",           lambda i: True),
    ("skill items",         lambda i: not ISNONE[i]),
    ("`none` items",        lambda i: ISNONE[i]),
    ("late discriminator",  lambda i: LATE[i]),
    ("early discriminator", lambda i: not LATE[i]),
    ("class boundary",      lambda i: CLS[i] == "boundary"),
    ("class near-neighbour",lambda i: CLS[i] == "near-neighbour"),
    ("class none",          lambda i: CLS[i] == "none"),
    ("class single",        lambda i: CLS[i] == "single"),
]

# ============================ rankers ============================
def baseline_bm25():
    """EXACT reproduction of copy.rs: doc = '<key>: <rubric>', one bag, plain BM25."""
    docs = [tok(f"{k}: {cands[k]}") for k in KEYS]
    idx = bm25_index(docs)
    return [rank_from_scores(bm25_scores(tok(p), idx)) for p in PROMPTS]

# pre-segment once
SEG = {k: segment(cands[k]) for k in KEYS}
NAMEFIELD = {k: k.replace("-", " ") for k in KEYS}

def fielded(w_name=1.0, w_pos=1.0, w_neg=0.0, stemming=False, bigrams=False, k1=K1, b=B):
    T = tok_s if stemming else tok
    def bigr(ts): return ts + ["%s_%s" % (ts[i], ts[i+1]) for i in range(len(ts)-1)] if bigrams else ts
    name_docs = [bigr(T(NAMEFIELD[k])) for k in KEYS]
    pos_docs  = [bigr(T(f"{NAMEFIELD[k]} {SEG[k][0]}")) for k in KEYS]
    neg_docs  = [bigr(T(SEG[k][1])) for k in KEYS]
    in_, ip, ineg = bm25_index(name_docs), bm25_index(pos_docs), bm25_index(neg_docs)
    out = []
    for p in PROMPTS:
        q = bigr(T(p))
        sn = bm25_scores(q, in_, k1, b); sp = bm25_scores(q, ip, k1, b); sg = bm25_scores(q, ineg, k1, b)
        sc = [w_name*sn[i] + w_pos*sp[i] - w_neg*sg[i] for i in range(len(KEYS))]
        out.append(rank_from_scores(sc))
    return out

# ---------- embeddings (cached, one text per request) ----------
import urllib.request
CACHE = os.path.join(HERE, "emb-cache.json")
def embed_all(texts):
    cache = json.load(open(CACHE)) if os.path.exists(CACHE) else {}
    todo = [t for t in texts if t[:2000] not in cache]
    for i, t in enumerate(todo):
        body = json.dumps({"model": "bge-small-en-v1.5", "input": [t[:2000]]}).encode()
        req = urllib.request.Request("http://192.168.2.132:9997/v1/embeddings", data=body,
                                     headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=60) as r:
            cache[t[:2000]] = json.loads(r.read())["data"][0]["embedding"]
        if (i+1) % 25 == 0:
            print(f"  embedded {i+1}/{len(todo)}", file=sys.stderr)
            json.dump(cache, open(CACHE, "w"))
    json.dump(cache, open(CACHE, "w"))
    return [cache[t[:2000]] for t in texts]

def cos(a, b):
    d = sum(x*y for x, y in zip(a, b))
    na = math.sqrt(sum(x*x for x in a)); nb = math.sqrt(sum(x*x for x in b))
    return d/(na*nb) if na and nb else 0.0

_EMB = {}
def emb_scores():
    if "s" in _EMB: return _EMB["s"]
    rv = embed_all([f"{k}: {cands[k]}" for k in KEYS])
    pv = embed_all(PROMPTS)
    _EMB["s"] = [[cos(p, r) for r in rv] for p in pv]
    return _EMB["s"]

def baseline_emb():
    return [rank_from_scores(s) for s in emb_scores()]

def rrf(rank_lists, k=60):
    """Reciprocal-rank fusion of several score vectors -> fused score vectors."""
    out = []
    for case in zip(*rank_lists):
        fused = [0.0]*len(KEYS)
        for scores in case:
            order = sorted(range(len(KEYS)), key=lambda i: -scores[i])
            for rank, i in enumerate(order):
                fused[i] += 1.0/(k + rank + 1)
        out.append(fused)
    return out
