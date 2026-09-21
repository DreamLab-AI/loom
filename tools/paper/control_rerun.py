#!/usr/bin/env python3
"""Five-arm negative-control rerun under the common retry policy (paper-v9).

The paper reports a five-arm control rerun (the original four plus a
fluent-noise floor) run under a common retry policy, but those rows were never
written to disk. This regenerates them, and writes down everything the original
harness threw away.

It imports `control_harness` rather than forking it, so the scaffold fetch, the
sentence shuffle, the entity mask and the seed-disjoint donor search are the
SAME code that produced `uplift-results/paper-v2/control-results.jsonl` - the
two cohorts stay comparable. What is new:

  * `fluent_noise` arm. The reviewer's objection to `irrelevant` is that a
    donor scaffold is still a coherent answer to *some* question, so a model
    might key on its coherence. `fluent_noise` removes that: sections are drawn
    at RANDOM from the same on-corpus scaffold index, from classes that are
    seed-disjoint from the recipient (disjoint from its seed slugs AND from
    their 1-hop relation targets and named ancestors), rendered by the SAME
    `ontology_scaffold_v1._section_for` renderer, and accumulated until the
    block matches the recipient's TRUE block token length, then clamped to it
    by `_clamp`. So it is length-matched, well-formed, fluent, on-corpus
    ontology prose about entities that have nothing to do with the question.
    Selection is seeded per question id, so the arm is reproducible.

  * Common retry policy. Every arm gets `max_tokens=4096`; any arm whose answer
    comes back empty (the 1536-token cohort's failure mode - a reasoning block
    eating the whole budget) is retried once at 8192. Both attempts are kept in
    the row's `attempts` ledger with their budgets and finish reasons.

  * Full provenance per row: the exact system and user messages sent, the
    injected block, the donor pairing and its verified IRI disjointness, the
    finish reason, prompt/completion tokens, and the final budget.

  * Placebo-exposure measurement. Each row records how much of the question's
    gold target set the INJECTED BLOCK actually exposes, using the paper's own
    matcher (`decompose_exposure.gold_hit`, byte-identical to the bench
    scorer). Gold targets for a control question are the titles of its true
    scaffold's seed classes plus the question set's `topic` field - i.e. the
    entities a correct answer has to name. This is the measurement the reviewer
    asked for: it turns "we believe the placebo arms carry no gold content"
    into a number per row.

Endpoint is the loom facade, called with `loom_options.scaffold=false` so the
facade delegates to the model without injecting a scaffold of its own; the
block this harness injects as the system message is the only one in play.
Note (carried over from the original harness, deliberately, for comparability):
the control system message is the bare `[ONTOLOGY CONTEXT]` block WITHOUT the
`SYSTEM_PREAMBLE` authority sentence the live serving path prepends.

Usage:
  python3 tools/paper/control_rerun.py --sets arcane,thin \
      --out uplift-results/control-rerun-2026-09-21 --concurrency 2
"""
from __future__ import annotations
import argparse, json, os, random, sys, time, urllib.request
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
from pathlib import Path
from threading import Lock

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
REPO = HERE.parent.parent
os.environ.setdefault("ONTOLOGY_INDEX", str(REPO / "app" / "data" / "scaffold-index.json"))

import control_harness as ch                      # noqa: E402  (same code as the original cohort)
import ontology_scaffold_v1 as osc                # noqa: E402
from decompose_exposure import gold_hit, normalise  # noqa: E402

FACADE = "http://192.168.2.132:8084"
CHAT = FACADE + "/v1/chat/completions"
SCAFFOLD = FACADE + "/loom/scaffold"
ARMS = ("true", "shuffled", "masked", "irrelevant", "fluent_noise")
BUDGET = 4096
RETRY_BUDGET = 8192


# --- fluent-noise construction ---------------------------------------------

def _forbidden_slugs(idx: osc.ScaffoldIndex, seed_slugs: set) -> set:
    """Seed slugs plus their 1-hop relation targets and named ancestors - the
    neighbourhood a random donor must avoid to be genuinely off-topic."""
    bad = set(seed_slugs)
    for s in seed_slugs:
        e = idx.classes.get(s) or {}
        for ref in (e.get("sup") or []) + (e.get("isup") or []):
            bad.add(osc._ref_to_slug(ref))
        for _rt, targets in osc._rel_items(e):
            for t in targets:
                bad.add(osc._ref_to_slug(t))
    return bad


def fluent_noise_block(idx: osc.ScaffoldIndex, true_block: str, seed_slugs: set,
                       rng: random.Random, tolerance: float = 0.05,
                       max_tries: int = 600) -> tuple:
    """Length-matched random on-corpus block from seed-disjoint classes.

    Greedy best-fit rather than fill-then-clamp: a candidate section is kept
    only if it still fits the true block's token budget, so the block is packed
    up to that budget instead of overshooting and then losing a whole section
    to `_clamp`. Stops once within `tolerance` of the target length or after
    `max_tries` candidates.
    """
    target = osc._est_tokens(true_block)
    wrapper = osc._est_tokens(osc.HEADER + "\n" + "\n" + osc.FOOTER)
    bad = _forbidden_slugs(idx, seed_slugs)
    pool = [s for s in idx.classes
            if s not in bad and (idx.classes[s].get("d") or "").strip()]
    rng.shuffle(pool)
    sections, chosen = [], []
    used = wrapper
    for slug in pool[:max_tries]:
        sec = osc._section_for(idx, slug, set(), hops=1)
        if not sec.strip():
            continue
        cost = osc._est_tokens(sec) + 1
        if used + cost > target:
            continue                               # would overshoot: try another
        sections.append(sec)
        chosen.append(slug)
        used += cost
        if used >= target * (1 - tolerance):
            break
    if not sections:                               # every section alone overshoots
        slug = pool[0]
        sections, chosen = [osc._section_for(idx, slug, set(), hops=1)], [pool[0]]
    block = osc.HEADER + "\n" + "\n\n".join(sections) + "\n" + osc.FOOTER
    return block, chosen


# --- placebo exposure --------------------------------------------------------

def gold_titles_for(idx: osc.ScaffoldIndex, q: dict, seed_slugs: list) -> list:
    """The entities a correct answer must name: the true scaffold's seed-class
    titles plus the question's declared topic."""
    titles = [idx.title_of(s) for s in seed_slugs if s in idx.classes]
    topic = (q.get("topic") or "").strip()
    if topic:
        titles.append(topic)
    out, seen = [], set()
    for t in titles:
        k = normalise(t)
        if k and k not in seen:
            seen.add(k)
            out.append(t)
    return out


def exposure_of(block: str, gold: list) -> dict:
    norm = normalise(block)
    words = set(norm.split())
    flags = [bool(gold_hit(t, norm, words)) for t in gold]
    return {"gold_titles": gold,
            "exposed_flags": flags,
            "n_gold": len(gold),
            "n_exposed": sum(flags),
            "exposure_fraction": round(sum(flags) / len(gold), 4) if gold else None}


# --- completion --------------------------------------------------------------

def complete(system: str, question: str, max_tokens: int, timeout: int = 1800) -> dict:
    body = {"model": "loom",
            "messages": [{"role": "system", "content": system},
                         {"role": "user", "content": question}],
            "max_tokens": max_tokens, "temperature": 0.0,
            "loom_options": {"scaffold": False}}
    t0 = time.time()
    req = urllib.request.Request(CHAT, data=json.dumps(body).encode(),
                                 headers={"Content-Type": "application/json"},
                                 method="POST")
    with urllib.request.urlopen(req, timeout=timeout) as r:
        d = json.load(r)
    choice = (d.get("choices") or [{}])[0]
    usage = d.get("usage") or {}
    return {"budget": max_tokens,
            "content": (choice.get("message") or {}).get("content") or "",
            "finish_reason": choice.get("finish_reason"),
            "prompt_tokens": usage.get("prompt_tokens"),
            "completion_tokens": usage.get("completion_tokens"),
            "total_tokens": usage.get("total_tokens"),
            "model": d.get("model"),
            "latency_s": round(time.time() - t0, 2)}


def run_arm(job: dict, retries: int = 3) -> dict:
    """One (question, arm) cell: 4096, retried once at 8192 if empty."""
    row = {k: job[k] for k in ("set", "id", "arm", "question")}
    row["system_message"] = job["block"]
    row["user_message"] = job["question"]
    row["injected_block"] = job["block"]
    row["injected_block_chars"] = len(job["block"])
    row["injected_block_est_tokens"] = osc._est_tokens(job["block"])
    row["system_preamble_included"] = False
    row["donor"] = job.get("donor")
    row["fluent_noise_slugs"] = job.get("noise_slugs")
    row["exposure"] = exposure_of(job["block"], job["gold"])
    row["true_block_exposure"] = job["true_exposure"]
    attempts = []
    for budget in (BUDGET, RETRY_BUDGET):
        last = None
        for a in range(retries):
            try:
                r = complete(job["block"], job["question"], budget)
                r["attempt"] = a
                attempts.append(r)
                last = None
                break
            except Exception as e:                  # noqa: BLE001
                last = str(e)[:200]
                attempts.append({"budget": budget, "attempt": a, "error": last})
                time.sleep(5 * (a + 1))
        if last is None and (attempts[-1].get("content") or "").strip():
            break                                    # non-empty: done
    row["attempts"] = attempts
    final = attempts[-1] if attempts else {}
    row.update({"content": final.get("content", ""),
                "finish_reason": final.get("finish_reason"),
                "prompt_tokens": final.get("prompt_tokens"),
                "completion_tokens": final.get("completion_tokens"),
                "final_budget": final.get("budget"),
                "latency_s": final.get("latency_s"),
                "model": final.get("model"),
                "error": final.get("error")})
    row["empty"] = not (row["content"] or "").strip()
    row["run_at"] = datetime.now(timezone.utc).isoformat()
    return row


def main(argv=None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--sets", default="arcane,thin")
    ap.add_argument("--out", type=Path,
                    default=REPO / "uplift-results" / "control-rerun-2026-09-21")
    ap.add_argument("--concurrency", type=int, default=2)
    ap.add_argument("--limit", type=int, default=0, help="first N questions (smoke test)")
    ap.add_argument("--arms", default=",".join(ARMS))
    args = ap.parse_args(argv)
    arms = tuple(a for a in ARMS if a in args.arms.split(","))
    args.out.mkdir(parents=True, exist_ok=True)
    idx = osc.get_index()

    # Phase 1 - scaffolds (LLM-free), same fetch the original harness used.
    questions, blocks, seedmap = [], {}, {}
    for name in args.sets.split(","):
        qs = json.load(open(REPO / ch.SETS[name]))
        if args.limit:
            qs = qs[:args.limit]
        for q in qs:
            questions.append((name, q))
            try:
                sc = ch.get_scaffold(q["question"])
                blocks[(name, q["id"])] = sc.get("scaffold") or ""
                seedmap[(name, q["id"])] = sc.get("seeds") or []
            except Exception as e:                   # noqa: BLE001
                print(f"scaffold fetch failed {q['id']}: {e}", file=sys.stderr)
                blocks[(name, q["id"])], seedmap[(name, q["id"])] = "", []

    # Phase 2 - donor pairing for the irrelevant arm (verified seed-IRI disjoint,
    # cross-set preferred), identical rule to control_harness.main.
    engaged = [(n, q) for (n, q) in questions if blocks[(n, q["id"])]]
    iris = {(n, q["id"]): {s.get("iri") for s in seedmap[(n, q["id"])]} for (n, q) in engaged}
    donors = {}
    for (n, q) in engaged:
        me = iris[(n, q["id"])]
        donor = None
        for cross_set in (True, False):
            for (m, p) in engaged:
                if (m, p["id"]) == (n, q["id"]) or (m != n) != cross_set:
                    continue
                if not (me & iris[(m, p["id"])]):
                    donor = (m, p["id"])
                    break
            if donor:
                break
        donors[(n, q["id"])] = donor
    print(f"engaged {len(engaged)}/{len(questions)}; donors "
          f"{sum(1 for v in donors.values() if v)}", file=sys.stderr)

    # Phase 3 - build every arm's block, plus its gold set and exposure.
    jobs = []
    manifest = []
    for (n, q) in questions:
        key = (n, q["id"])
        block, seeds = blocks[key], seedmap[key]
        if not block:
            for a in arms:
                manifest.append({"set": n, "id": q["id"], "arm": a,
                                 "skipped": "no-scaffold"})
            continue
        seed_slugs = [osc._ref_to_slug(s.get("iri", "")) for s in seeds]
        gold = gold_titles_for(idx, q, seed_slugs)
        true_exposure = exposure_of(block, gold)
        rng = random.Random(f"fluent-noise::{n}::{q['id']}")
        noise, noise_slugs = fluent_noise_block(idx, block, set(seed_slugs), rng)
        donor = donors.get(key)
        variants = {
            "true": (block, None, None),
            "shuffled": (ch.shuffle_block(block, seed=hash(q["id"]) & 0xFFFF), None, None),
            "masked": (ch.mask_block(block, seeds), None, None),
            "irrelevant": (blocks.get(donor, "") if donor else "",
                           {"set": donor[0], "id": donor[1],
                            "donor_seed_iris": sorted(x for x in iris[donor] if x),
                            "recipient_seed_iris": sorted(x for x in iris[key] if x),
                            "iri_disjoint": not (iris[key] & iris[donor])} if donor else None,
                           None),
            "fluent_noise": (noise, None, noise_slugs),
        }
        for arm in arms:
            blk, dn, ns = variants[arm]
            if not blk:
                manifest.append({"set": n, "id": q["id"], "arm": arm,
                                 "skipped": "no-block"})
                continue
            jobs.append({"set": n, "id": q["id"], "arm": arm,
                         "question": q["question"], "block": blk,
                         "donor": dn, "noise_slugs": ns, "gold": gold,
                         "true_exposure": true_exposure})

    outpath = args.out / "rows.jsonl"
    done = set()
    if outpath.exists():
        for line in open(outpath):
            r = json.loads(line)
            if not r.get("error") and not r.get("empty"):
                done.add((r["set"], r["id"], r["arm"]))
    jobs = [j for j in jobs if (j["set"], j["id"], j["arm"]) not in done]
    print(f"{len(jobs)} cells to run ({len(done)} already complete), "
          f"concurrency {args.concurrency}", file=sys.stderr, flush=True)

    with open(args.out / "manifest-skipped.json", "w") as f:
        json.dump(manifest, f, indent=1)

    lock, n_done, t_start = Lock(), [0], time.time()
    with open(outpath, "a") as f, ThreadPoolExecutor(max_workers=args.concurrency) as ex:
        for row in ex.map(run_arm, jobs):
            with lock:
                f.write(json.dumps(row, ensure_ascii=False) + "\n")
                f.flush()
                n_done[0] += 1
                if n_done[0] % 5 == 0 or n_done[0] == len(jobs):
                    el = time.time() - t_start
                    rate = el / n_done[0]
                    print(f"  {n_done[0]}/{len(jobs)} cells, {el/60:.1f} min elapsed, "
                          f"{rate:.1f} s/cell, ETA {(len(jobs)-n_done[0])*rate/60:.0f} min",
                          file=sys.stderr, flush=True)
    print(f"complete -> {outpath}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
