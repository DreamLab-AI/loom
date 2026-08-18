#!/usr/bin/env python3
"""Paragraph-by-paragraph "v4" rewrite driver for the Copy Ceiling paper.

Re-expresses the paper's prose in the local Qwen model's own voice, one paragraph
at a time, with mechanically enforced content invariants. Protected LaTeX
(preamble, environments, tables/figures, math, headings, \\input, bibliography)
passes through byte-identical; only contiguous prose paragraphs (and the abstract's
inner prose, and \\paragraph{...} bodies) are rewritten.

Every rewrite is validated against its source (citation/ref/label/url sets,
inline-math multiset, numeric multiset, banned words, em-dashes, length ratio).
Failures retry with a targeted FEEDBACK line; after MAX_ATTEMPTS the original
paragraph is kept. Accepted rewrites are cached (resumable) keyed by
(block_index, sha256(source)).

Style-matched to tools/paper/live_harness.py and control_harness.py (stdlib only).

Endpoint: http://127.0.0.1:18085/v1/chat/completions  (SSH tunnel to llama-server
on the HP; model alias "qwen3.8-27b-heretic-q8_0", a reasoning model).

Usage:
  # smoke test (first 3 rewriteable paragraphs, output to scratchpad, NOT paper-v4):
  python3 tools/paper/rewrite_v4.py --limit 3 --outdir /path/to/scratch/v4-smoke
  # full run (gated on upstream task; do not start without instruction):
  python3 tools/paper/rewrite_v4.py
"""
from __future__ import annotations
import argparse, hashlib, json, re, shutil, sys, time, urllib.request, urllib.error
from collections import Counter
from difflib import SequenceMatcher
from pathlib import Path

RAW = "http://127.0.0.1:18085/v1/chat/completions"
MODEL = "qwen3.8-27b-heretic-q8_0"
MAX_TOKENS = 4096
TEMPERATURE = 0.7
TOP_P = 0.95
MAX_ATTEMPTS = 3
TIMEOUT = 600

# Set from --no-think in main(); when True, sends chat_template_kwargs
# {"enable_thinking": false} so the model skips the reasoning pass. Default None
# leaves the model's default (thinking on). reasoning_content, if returned, is
# ignored: we validate choices[0].message.content only.
DISABLE_THINKING = False

DEFAULT_INPUT = Path("docs/research/paper-v2/main.tex")
DEFAULT_OUTDIR = Path("docs/research/paper-v4")

PROTECTED_ENVS = {
    "table", "table*", "figure", "figure*", "tikzpicture", "axis",
    "equation", "equation*", "align", "align*", "itemize", "enumerate",
    "description", "tabular", "tabularx",
}
# Lone formatting-command lines that carry no prose and must pass through verbatim.
STANDALONE_CMDS = {
    r"\noindent", r"\centering", r"\small", r"\footnotesize", r"\bigskip",
    r"\medskip", r"\smallskip", r"\clearpage", r"\newpage", r"\par",
}
HEADING_CMDS = (r"\section", r"\subsection", r"\subsubsection")

# ---------------------------------------------------------------------------
# The prompt (verbatim; do not edit).
# ---------------------------------------------------------------------------
SYSTEM_PROMPT = """You are rewriting a technical report's prose in your own voice, paragraph by paragraph. The paper is "The Copy Ceiling: An Input-Exposure Control for Ontology-Grounded Generation over Private Corpora", an arXiv technical report about a deployed ontology-serving system (the Loom) and a measurement instrument.

THE ARGUMENT YOU MUST NEVER DISTORT:
1. When gold answers derive from the same corpus injected as context, "grounding uplift" conflates faithful delivery of exposed facts with reasoning over injected structure.
2. The copy ceiling is the recall a verbatim copy of the shown context would already achieve; gain over copy is model recall minus that ceiling. It is deterministic, judge-free and per-item, and it is paired with a placebo verified inert by seed-disjointness.
3. Across ten models the gain over copy is uniformly negative (-0.067 to -0.022). A direct decomposition of 11,360 gold items found only 3 unexposed items recovered: the uplift is delivery, measured rather than inferred.
4. A production paired study (same model, only the serving path varied) lifts judged quality +0.27 pooled and +0.79 where curation is deepest; the four control arms bind the lift to scaffold content (served block +0.59, verified placebo +0.04); the out-of-domain delta +0.05 sits inside a +-0.25 equivalence margin.
5. Register: honest engineering narrative. Claims exactly as strong as the evidence. No marketing.

YOUR TASK, one request at a time: re-express the given LaTeX paragraph as you would naturally write it. Same content, same claims, same strength, your sentences, your rhythm. Do not imitate the source cadence.

ABSOLUTE RULES:
R1. Copy every number, interval, percentage and count exactly. Never round, drop or add a number.
R2. Copy every LaTeX token verbatim and keep it attached to the same content: \\cite{...}, \\ref{...}, \\S\\ref{...}, \\label{...}, \\footnote{...}, \\url{...}, \\texttt{...}, and every inline math span $...$. Do not translate math into words. Do not invent commands.
R3. Never write an em-dash (---). Use a comma, colon, semicolon, parentheses or a new sentence.
R4. Do not strengthen or weaken any claim. Hedges ("consistent with", "suggests") survive at equal force; so do confidence statements.
R5. Add no facts, examples or transitions that import meaning. Drop no caveats.
R6. UK English. Plain verbs. Never use: leverage, robust, seamless, comprehensive, delve, utilise, paradigm, harness as a verb.
R7. Stay within +-25% of the original length.
R8. Output ONLY the rewritten LaTeX paragraph. No explanation, no code fences, no surrounding quotes.
If a paragraph cannot be rewritten without breaking a rule (mostly math, or a single citation sentence), return it unchanged."""

USER_TEMPLATE = """Section: {section_title}
Preceding paragraph (already rewritten, for flow only; do not modify): {prev}
Rewrite this paragraph:
<<<
{paragraph}
>>>"""

# ---------------------------------------------------------------------------
# Segmentation
# ---------------------------------------------------------------------------

def balanced_span(text: str, open_idx: int) -> int:
    """Return index just past the '}' matching the '{' at open_idx, or -1."""
    depth = 0
    for i in range(open_idx, len(text)):
        c = text[i]
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return i + 1
    return -1


def env_name(stripped: str) -> str | None:
    m = re.match(r"\\begin\{([^}]*)\}", stripped)
    return m.group(1) if m else None


def is_standalone_cmd(stripped: str) -> bool:
    return stripped in STANDALONE_CMDS


def heading_title(stripped: str) -> str:
    """Extract the {title} text of a section/subsection heading line."""
    b = stripped.find("{")
    if b < 0:
        return stripped
    end = balanced_span(stripped, b)
    return stripped[b + 1:end - 1] if end > 0 else stripped[b + 1:]


class Block:
    __slots__ = ("kind", "lines", "prefix", "section", "note", "index", "_result")

    def __init__(self, kind, lines, prefix="", section="", note="", index=-1):
        self.kind = kind          # "keep" | "rewrite"
        self.lines = lines        # list[str]; for rewrite this is the source body
        self.prefix = prefix      # verbatim prefix for rewrite blocks (\paragraph{..})
        self.section = section
        self.note = note
        self.index = index        # rewriteable-paragraph index (rewrite blocks only)


def segment(text: str):
    """Split into ordered Blocks. Prose paragraphs -> rewrite; all else -> keep."""
    lines = text.split("\n")
    # preamble: everything up to and including \begin{document}
    doc_i = next((i for i, ln in enumerate(lines)
                  if ln.strip().startswith(r"\begin{document}")), None)
    if doc_i is None:
        raise SystemExit("no \\begin{document} found")
    blocks = [Block("keep", lines[:doc_i + 1])]
    body = lines[doc_i + 1:]

    out: list[Block] = []
    keep_buf: list[str] = []
    section = ""
    i = 0
    n = len(body)

    def flush_keep():
        nonlocal keep_buf
        if keep_buf:
            out.append(Block("keep", keep_buf))
            keep_buf = []

    def prose_stops(stripped: str) -> bool:
        """A stripped line that terminates prose accumulation (not consumed here).

        Note: display math (\\[) is NOT a stop here; when it appears mid-prose
        (no blank line separating it) it is pulled into the block and the whole
        run is protected, because the surrounding sentence continues around it.
        """
        if stripped == "" or stripped.startswith("%"):
            return True
        if stripped.startswith((r"\begin{", r"\end{", r"\input",
                                r"\printbibliography", r"\maketitle")):
            return True
        if stripped.startswith(HEADING_CMDS) or stripped.startswith((r"\paragraph", r"\subparagraph")):
            return True
        if is_standalone_cmd(stripped):
            return True
        return False

    HAZARDS = (r"\begin{", r"\end{", r"\[", r"\]")

    def accumulate_prose(start_first_line: str, start_i: int):
        """Collect a prose paragraph. Returns (body_lines, next_i, hazard_bool).

        An adjacent display-math block (\\[ ... \\]) is consumed into the buffer
        and flags a hazard, so a prose/math/prose run protects as one block.
        """
        buf = [start_first_line]
        j = start_i
        hazard = any(h in start_first_line for h in HAZARDS)
        while j < n:
            ln = body[j]
            st = ln.strip()
            if st.startswith(r"\["):
                # pull the whole display-math block in; keep accumulating after \]
                while j < n:
                    buf.append(body[j])
                    if r"\]" in body[j]:
                        j += 1
                        break
                    j += 1
                hazard = True
                continue
            if prose_stops(st):
                break
            buf.append(ln)
            if any(h in ln for h in HAZARDS):
                hazard = True
            j += 1
        return buf, j, hazard

    rw_index = 0
    while i < n:
        line = body[i]
        st = line.strip()

        # blank / comment
        if st == "" or st.startswith("%"):
            keep_buf.append(line)
            i += 1
            continue

        # protected environment (consume to matching \end{env}); abstract special
        if st.startswith(r"\begin{"):
            env = env_name(st)
            if env == "abstract":
                section = "Abstract"    # tag abstract-inner prose blocks
                keep_buf.append(line)   # protect the \begin{abstract} line only
                i += 1
                continue                # inner prose flows through normal walk
            # consume whole environment (protected set OR unknown -> protect anyway)
            depth = 0
            j = i
            while j < n:
                sj = body[j].strip()
                if sj.startswith(r"\begin{" + (env or "")):
                    depth += 1
                if sj.startswith(r"\end{" + (env or "")):
                    depth -= 1
                    if depth == 0:
                        break
                j += 1
            for k in range(i, min(j + 1, n)):
                keep_buf.append(body[k])
            i = j + 1
            continue

        # lone \end{...} (e.g. \end{abstract}) or standalone cmd or \input etc.
        if (st.startswith(r"\end{") or st.startswith(r"\input")
                or st.startswith(r"\printbibliography") or st.startswith(r"\maketitle")
                or is_standalone_cmd(st)):
            keep_buf.append(line)
            if st.startswith(r"\end{abstract}"):
                section = ""            # leaving the abstract; next heading resets it
            i += 1
            continue

        # display math block \[ ... \]
        if st.startswith(r"\["):
            j = i
            while j < n and r"\]" not in body[j]:
                j += 1
            for k in range(i, min(j + 1, n)):
                keep_buf.append(body[k])
            i = j + 1
            continue

        # section / subsection heading line -> protected, update section context,
        # reset the "preceding paragraph" flow.
        if st.startswith(HEADING_CMDS):
            section = heading_title(st)
            keep_buf.append(line)
            i += 1
            continue

        # \paragraph{Title.} [body...]
        if st.startswith((r"\paragraph", r"\subparagraph")):
            # find balanced {title} possibly spanning lines
            joined = line
            jj = i
            b = joined.find("{")
            while b >= 0 and balanced_span(joined, b) < 0 and jj + 1 < n:
                jj += 1
                joined = joined + "\n" + body[jj]
            end = balanced_span(joined, b) if b >= 0 else -1
            if end < 0:
                keep_buf.append(line)     # malformed; protect
                i += 1
                continue
            prefix = joined[:end]
            remainder = joined[end:]
            if remainder.strip() == "":
                # standalone heading (body lives in a following env/enumerate)
                for k in range(i, jj + 1):
                    keep_buf.append(body[k])
                i = jj + 1
                continue
            # body starts on this line after the title
            buf, next_i, hazard = accumulate_prose(remainder.lstrip(), jj + 1)
            flush_keep()
            if hazard:
                out.append(Block("keep", [prefix + remainder]
                                 + [body[k] for k in range(jj + 1, next_i)]))
                out[-1].note = "protected: inline environment/display-math in \\paragraph body"
            else:
                out.append(Block("rewrite", buf, prefix=prefix + " ",
                                 section=section, index=rw_index))
                rw_index += 1
            i = next_i
            continue

        # ordinary prose paragraph
        buf, next_i, hazard = accumulate_prose(line, i + 1)
        flush_keep()
        if hazard:
            out.append(Block("keep", buf))
            out[-1].note = "protected: inline environment/display-math in prose"
        else:
            out.append(Block("rewrite", buf, section=section, index=rw_index))
            rw_index += 1
        i = next_i

    flush_keep()
    return blocks + out


# ---------------------------------------------------------------------------
# Validation
# ---------------------------------------------------------------------------
CITE_RE = re.compile(r"\\[a-zA-Z]*cite[a-zA-Z]*\{([^}]*)\}")
REF_RE = re.compile(r"\\ref\{([^}]*)\}")
LABEL_RE = re.compile(r"\\label\{([^}]*)\}")
URL_RE = re.compile(r"\\url\{([^}]*)\}")
FOOTNOTE_RE = re.compile(r"\\footnote\{")
MATH_RE = re.compile(r"\$[^$]*\$")
NUM_RE = re.compile(r"\d[\d,]*(?:\.\d+)?")

BANNED_HARD = ("leverage", "seamless", "delve", "utilise", "paradigm")
BANNED_COND = ("robust", "comprehensive", "harness", "harnessed")


def cite_keys(t: str) -> Counter:
    keys: list[str] = []
    for m in CITE_RE.finditer(t):
        keys += [k.strip() for k in m.group(1).split(",") if k.strip()]
    return Counter(keys)


def _set(regex: re.Pattern, t: str) -> Counter:
    return Counter(m.group(1).strip() for m in regex.finditer(t))


def math_spans(t: str) -> Counter:
    return Counter(MATH_RE.findall(t))


def texttt_args(t: str) -> Counter:
    """Multiset of \\texttt{...} argument strings (balanced braces)."""
    args, i = [], 0
    needle = r"\texttt{"
    while True:
        p = t.find(needle, i)
        if p < 0:
            break
        brace = p + len(needle) - 1
        end = balanced_span(t, brace)
        if end < 0:
            break
        args.append(t[brace + 1:end - 1])
        i = end
    return Counter(args)


def numbers_outside_math(t: str) -> Counter:
    t = MATH_RE.sub(" ", t)
    t = t.replace("{,}", "")
    return Counter(tok.replace(",", "") for tok in NUM_RE.findall(t))


def word_present(word: str, t: str) -> bool:
    return re.search(r"\b" + re.escape(word) + r"\b", t, re.IGNORECASE) is not None


def clean_output(raw: str) -> str:
    """Strip reasoning tags, code fences, 'Here is...' preambles, wrapping quotes."""
    t = raw.strip()
    t = re.sub(r"<think>.*?</think>", "", t, flags=re.DOTALL | re.IGNORECASE).strip()
    # drop any stray unclosed reasoning tag
    t = re.sub(r"^.*?</think>", "", t, flags=re.DOTALL).strip() if "</think>" in t else t
    if t.startswith("```"):
        t = re.sub(r"^```[a-zA-Z]*\n?", "", t)
        t = re.sub(r"\n?```$", "", t).strip()
    # leading conversational preamble line
    t = re.sub(r"^(here(?:'s| is)|sure|certainly|below is|rewritten paragraph)[^\n]*\n",
               "", t, flags=re.IGNORECASE).strip()
    if len(t) >= 2 and t[0] in "\"'" and t[-1] in "\"'":
        t = t[1:-1].strip()
    return t


def validate(src: str, out: str) -> list[str]:
    """Return list of violation strings; empty == pass."""
    v: list[str] = []
    if not out.strip():
        return ["empty output"]

    # a. cite / ref / label / url sets; footnote count
    for name, fn in (("\\cite", cite_keys), ("\\ref", lambda t: _set(REF_RE, t)),
                     ("\\label", lambda t: _set(LABEL_RE, t)),
                     ("\\url", lambda t: _set(URL_RE, t))):
        cs, co = fn(src), fn(out)
        missing = list((cs - co).elements())
        added = list((co - cs).elements())
        if missing:
            v.append(f"missing {name}{{{','.join(sorted(set(missing)))}}}")
        if added:
            v.append(f"invented {name}{{{','.join(sorted(set(added)))}}}")
    fs, fo = len(FOOTNOTE_RE.findall(src)), len(FOOTNOTE_RE.findall(out))
    if fs != fo:
        v.append(f"\\footnote count {fo} != source {fs}")

    # \texttt{...} argument multiset must match (source vs output)
    ts, to = texttt_args(src), texttt_args(out)
    if ts != to:
        miss = list((ts - to).elements())
        add = list((to - ts).elements())
        if miss:
            v.append("missing \\texttt{" + "},{".join(sorted(set(miss))) + "}")
        if add:
            v.append("invented/altered \\texttt{" + "},{".join(sorted(set(add))) + "}")

    # b. inline math multiset
    ms, mo = math_spans(src), math_spans(out)
    if ms != mo:
        miss = list((ms - mo).elements())
        add = list((mo - ms).elements())
        if miss:
            v.append("missing math span(s): " + " ".join(sorted(set(miss))))
        if add:
            v.append("altered/invented math span(s): " + " ".join(sorted(set(add))))

    # c. numeric multiset outside math
    ns, no = numbers_outside_math(src), numbers_outside_math(out)
    if ns != no:
        miss = list((ns - no).elements())
        add = list((no - ns).elements())
        if miss:
            v.append("numbers missing: " + ",".join(sorted(set(miss))))
        if add:
            v.append("numbers added/changed: " + ",".join(sorted(set(add))))

    # d. em-dash + banned words
    if "---" in out:
        v.append("em-dash '---' present")
    for w in BANNED_HARD:
        if word_present(w, out):
            v.append(f"banned word '{w}'")
    for w in BANNED_COND:
        if word_present(w, out) and not word_present(w, src):
            v.append(f"banned word '{w}' (absent from source)")

    # e. length ratio
    ratio = len(out) / max(1, len(src))
    if not (0.75 <= ratio <= 1.25):
        v.append(f"length ratio {ratio:.2f} outside [0.75,1.25]")
    return v


# ---------------------------------------------------------------------------
# Model call
# ---------------------------------------------------------------------------

def call_model(system: str, user: str, timeout: int = TIMEOUT) -> tuple[str, float, int]:
    body = {
        "model": MODEL,
        "messages": [{"role": "system", "content": system},
                     {"role": "user", "content": user}],
        "max_tokens": MAX_TOKENS,
        "temperature": TEMPERATURE,
        "top_p": TOP_P,
    }
    if DISABLE_THINKING:
        body["chat_template_kwargs"] = {"enable_thinking": False}
    req = urllib.request.Request(RAW, data=json.dumps(body).encode(),
                                 headers={"Content-Type": "application/json"}, method="POST")
    t0 = time.time()
    with urllib.request.urlopen(req, timeout=timeout) as r:
        d = json.load(r)
    latency = time.time() - t0
    content = (d.get("choices") or [{}])[0].get("message", {}).get("content", "") or ""
    toks = (d.get("usage") or {}).get("completion_tokens") or 0
    return content, latency, toks


DIVERSITY_MAX = 0.9        # difflib ratio above which a rewrite is "too close to source"
DIVERSITY_RETRY_CAP = 2    # soft-fail retries before accepting the best (lowest-ratio) attempt


def rewrite_paragraph(src: str, section: str, prev: str) -> dict:
    """Rewrite one paragraph with hard-validation retries (MAX_ATTEMPTS) and, on top,
    diversity soft-fail retries (DIVERSITY_RETRY_CAP): a hard-valid rewrite whose
    difflib ratio > DIVERSITY_MAX is retried asking for more re-voicing; after the
    cap the best (lowest-ratio) hard-valid attempt is ACCEPTED and flagged, because
    some dense definitional paragraphs are legitimately rigid (do not force to
    fallback). The prompt stays frozen; only a FEEDBACK line is appended."""
    base_user = USER_TEMPLATE.format(section_title=section or "(untitled)",
                                     prev=prev or "(start of section)", paragraph=src)
    feedback = ""
    last_out = ""
    latencies: list[float] = []
    hard_fails = 0
    div_retries = 0
    best_soft: dict | None = None    # best hard-valid-but-too-similar attempt so far

    def accept(out, div, warn, rigid=False):
        return {"status": "accepted", "output": out, "attempts": len(latencies),
                "latencies": latencies, "ratio": round(len(out) / max(1, len(src)), 3),
                "diversity": round(div, 3), "diversity_warn": warn, "rigid": rigid}

    while len(latencies) < MAX_ATTEMPTS + DIVERSITY_RETRY_CAP:
        user = base_user + (("\n\nFEEDBACK: your previous attempt failed. "
                             + feedback) if feedback else "")
        try:
            raw, lat, _ = call_model(SYSTEM_PROMPT, user)
        except Exception as e:  # noqa: BLE001 — transport error, retry
            feedback = f"VIOLATION: request error ({e}); return ONLY the rewritten LaTeX paragraph."
            latencies.append(0.0)
            hard_fails += 1
            if hard_fails >= MAX_ATTEMPTS:
                break
            time.sleep(3 * hard_fails)
            continue
        latencies.append(round(lat, 2))
        out = clean_output(raw)
        if not out:
            feedback = ("VIOLATION: empty output (reasoning likely exhausted the token "
                        "budget); return ONLY the final rewritten LaTeX paragraph, no reasoning.")
            hard_fails += 1
            if hard_fails >= MAX_ATTEMPTS:
                break
            continue
        last_out = out
        viol = validate(src, out)
        if viol:
            feedback = "VIOLATION: " + "; ".join(viol) + ". Return ONLY the corrected LaTeX paragraph."
            hard_fails += 1
            if hard_fails >= MAX_ATTEMPTS:
                break
            continue
        # hard-valid: check diversity
        div = SequenceMatcher(None, src, out).ratio()
        if div <= DIVERSITY_MAX:
            return accept(out, div, warn=False)
        if best_soft is None or div < best_soft["div"]:
            best_soft = {"out": out, "div": div}
        if div_retries < DIVERSITY_RETRY_CAP:
            div_retries += 1
            feedback = ("VIOLATION: too close to source, restructure sentence order and "
                        "connectives")
            continue
        # cap reached: accept the best (lowest-ratio) hard-valid attempt, flagged
        return accept(best_soft["out"], best_soft["div"], warn=True, rigid=True)

    # hard budget exhausted. Prefer a hard-valid-but-similar attempt over fallback.
    if best_soft is not None:
        return accept(best_soft["out"], best_soft["div"], warn=True, rigid=True)
    return {"status": "fallback", "output": src, "attempts": len(latencies),
            "latencies": latencies, "ratio": 1.0,
            "diversity": 1.0, "diversity_warn": False,
            "last_output": last_out, "last_violations": feedback}


# ---------------------------------------------------------------------------
# Cache
# ---------------------------------------------------------------------------

def load_cache(path: Path) -> dict:
    cache = {}
    if path.exists():
        for ln in path.read_text().splitlines():
            if not ln.strip():
                continue
            r = json.loads(ln)
            cache[(r["block_index"], r["src_sha"])] = r
    return cache


def append_cache(path: Path, rec: dict):
    with open(path, "a") as f:
        f.write(json.dumps(rec, ensure_ascii=False) + "\n")


# ---------------------------------------------------------------------------
# Driver
# ---------------------------------------------------------------------------

def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--input", type=Path, default=DEFAULT_INPUT)
    ap.add_argument("--outdir", type=Path, default=DEFAULT_OUTDIR)
    ap.add_argument("--limit", type=int, default=0,
                    help="process only the first N rewriteable paragraphs (0 = all)")
    ap.add_argument("--dry-segment", action="store_true",
                    help="segment only; print counts and exit (no model calls)")
    ap.add_argument("--no-think", action="store_true",
                    help="send chat_template_kwargs {enable_thinking:false} to skip the reasoning pass")
    args = ap.parse_args(argv)

    global DISABLE_THINKING
    DISABLE_THINKING = args.no_think

    text = args.input.read_text()
    had_final_nl = text.endswith("\n")
    blocks = segment(text)

    rewriteable = [b for b in blocks if b.kind == "rewrite"]
    protected = [b for b in blocks if b.kind == "keep"]
    protected_notes = [b for b in blocks if b.kind == "keep" and b.note]
    print(f"segmentation: {len(rewriteable)} rewriteable paragraphs, "
          f"{len(protected)} protected blocks "
          f"({len(protected_notes)} protected-with-note)", file=sys.stderr)
    for b in protected_notes:
        preview = (b.lines[0][:70] + "...") if b.lines else ""
        print(f"  [protected note] {b.note}: {preview}", file=sys.stderr)

    if args.dry_segment:
        return 0

    args.outdir.mkdir(parents=True, exist_ok=True)
    cache_path = args.outdir / "rewrite-cache.jsonl"
    cache = load_cache(cache_path)
    print(f"cache: {len(cache)} accepted rewrites on disk", file=sys.stderr)

    limit = args.limit if args.limit > 0 else len(rewriteable)
    report_rows = []
    prev_by_output = {}   # section -> last emitted paragraph text (flow context)

    for b in blocks:
        if b.kind != "rewrite":
            continue
        src = "\n".join(b.lines).strip("\n")
        sha = hashlib.sha256(src.encode()).hexdigest()
        section = b.section
        prev = prev_by_output.get(section, "")

        if b.index >= limit:
            # beyond the limit: leave source untouched, not called
            b.prefix = b.prefix  # noqa
            b._result = {"status": "unprocessed", "output": src, "attempts": 0,
                         "latencies": [], "ratio": 1.0, "diversity": 1.0}
            continue

        key = (b.index, sha)
        if key in cache:
            rec = cache[key]
            res = {"status": "accepted", "output": rec["output"],
                   "attempts": rec.get("attempts", 0), "latencies": rec.get("latencies", []),
                   "ratio": rec.get("ratio", 0.0), "diversity": rec.get("diversity", 0.0),
                   "diversity_warn": rec.get("diversity_warn", False), "cached": True}
        else:
            res = rewrite_paragraph(src, section, prev)
            if res["status"] == "accepted":
                append_cache(cache_path, {
                    "block_index": b.index, "src_sha": sha, "section": section,
                    "output": res["output"], "attempts": res["attempts"],
                    "latencies": res["latencies"], "ratio": res["ratio"],
                    "diversity": res["diversity"], "diversity_warn": res["diversity_warn"],
                })
            lat_str = ",".join(f"{x:.1f}" for x in res["latencies"])
            warn = " DIVERSITY!" if res.get("diversity_warn") else ""
            print(f"  [{b.index+1}/{limit}] {res['status']} "
                  f"attempts={res['attempts']} lat=[{lat_str}]s "
                  f"ratio={res['ratio']} div={res.get('diversity')}{warn}  ({section})",
                  file=sys.stderr)

        b._result = res
        prev_by_output[section] = res["output"]
        report_rows.append((b, res, sha))

    # ---- assemble output main.tex ----
    out_lines: list[str] = []
    for b in blocks:
        if b.kind == "keep":
            out_lines.extend(b.lines)
        else:
            res = getattr(b, "_result", None)
            if res and res["status"] in ("accepted",):
                out_lines.append((b.prefix + res["output"]).rstrip())
            elif res and res["status"] in ("fallback", "unprocessed"):
                # keep original body verbatim (with its prefix if any)
                body = "\n".join(b.lines)
                out_lines.append((b.prefix + body) if b.prefix else body)
            else:
                body = "\n".join(b.lines)
                out_lines.append((b.prefix + body) if b.prefix else body)

    out_text = "\n".join(out_lines)
    if had_final_nl and not out_text.endswith("\n"):
        out_text += "\n"
    (args.outdir / "main.tex").write_text(out_text)

    # ---- copy refs.bib and figures/*.tex (paths stay relative) ----
    src_dir = args.input.parent
    refs = src_dir / "refs.bib"
    if refs.exists():
        shutil.copy2(refs, args.outdir / "refs.bib")
    figs = src_dir / "figures"
    if figs.is_dir():
        (args.outdir / "figures").mkdir(exist_ok=True)
        for f in figs.glob("*.tex"):
            shutil.copy2(f, args.outdir / "figures" / f.name)

    # ---- report ----
    write_report(args.outdir / "REWRITE-REPORT.md", blocks, rewriteable, protected,
                 protected_notes, report_rows, limit, args.input)
    print(f"complete -> {args.outdir/'main.tex'}", file=sys.stderr)
    return 0


def write_report(path, blocks, rewriteable, protected, protected_notes, rows, limit, input_path):
    accepted = [r for _, r, _ in rows if r["status"] == "accepted"]
    fallback = [r for _, r, _ in rows if r["status"] == "fallback"]
    div_warns = [r for _, r, _ in rows if r.get("diversity_warn")]
    lines = [
        "# v4 Rewrite Report",
        "",
        f"- Source: `{input_path}`",
        f"- Rewriteable paragraphs (total): **{len(rewriteable)}**",
        f"- Protected blocks: **{len(protected)}** ({len(protected_notes)} protected-with-note)",
        f"- Processed this run (limit={limit}): **{len(rows)}**",
        f"- Accepted: **{len(accepted)}**  |  Fallback (kept original): **{len(fallback)}**",
        f"- Diversity warnings (>0.9 token-identical): **{len(div_warns)}**",
        "",
        "## Per-paragraph",
        "",
        "| idx | section | status | attempts | len-ratio | diversity |",
        "|----:|---------|--------|---------:|----------:|----------:|",
    ]
    for b, r, _sha in rows:
        cached = " (cached)" if r.get("cached") else ""
        rigid = " rigid" if r.get("rigid") else ""
        warn = " ⚠" if r.get("diversity_warn") else ""
        lines.append(f"| {b.index} | {(b.section or '')[:40]} | {r['status']}{cached}{rigid} | "
                     f"{r['attempts']} | {r['ratio']} | {r.get('diversity','')}{warn} |")

    # footnote-bearing rewriteable paragraphs: footnote prose is not validated, so
    # flag them for a human diff.
    fn_rows = [(b, r) for b, r, _sha in rows if FOOTNOTE_RE.search("\n".join(b.lines))]
    if fn_rows:
        lines += ["", "## Footnote-bearing paragraphs (footnote prose unchecked; eyeball these)", ""]
        for b, r in fn_rows:
            lines.append(f"- idx {b.index} ({b.section}): status {r['status']}")

    if protected_notes:
        lines += ["", "## Protected-with-note blocks", ""]
        for b in protected_notes:
            preview = (b.lines[0][:80] + "...") if b.lines else ""
            lines.append(f"- {b.note}: `{preview}`")
    if fallback:
        lines += ["", "## Fallbacks (last violation)", ""]
        for b, r, _sha in rows:
            if r["status"] == "fallback":
                lines.append(f"- idx {b.index} ({b.section}): {r.get('last_violations','')}")
    Path(path).write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    raise SystemExit(main())
