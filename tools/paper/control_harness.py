#!/usr/bin/env python3
"""Negative-control arms for the paper-v2 experiment (the lexical-artefact defence).

The singular claim (copy-ceiling / gain-over-copy as a standing control) dies to
one reviewer objection: "your gains are lexical overlap with the scaffold, not
knowledge delivery". These arms attribute the paired gain to the scaffold's
CONTENT, holding everything else fixed:

  true     : raw model + the loom's actual scaffold block injected as the system
             message (exactly the loom's merge semantics). Sanity: ≈ loom arm —
             proves the serving path adds nothing beyond the scaffold itself.
  shuffled : same block, sentences shuffled within the body (header/footer kept,
             seeded RNG). Same tokens, destroyed structure. If gains survive,
             they were lexical soup.
  masked   : same block with every seed-class title replaced by "Entity-<k>".
             Structure intact, entity names gone. If gains survive, the judge
             was matching names, not knowledge.
  irrelevant : the scaffold of a question whose SEED IRIS ARE DISJOINT from
             this one's (greedy cross-domain donor search; a plain shift-by-one
             derangement is invalid here — the arcane set clusters in two
             coherent domains, so a neighbour's scaffold plus 1-hop expansion
             frequently contains the right entities). Well-formed, on-corpus,
             wrong entities, verified disjoint. The classic wrong-context
             control: if gains survive, any ontology-shaped text would do.

Scaffolds come from the production loom's /loom/scaffold (LLM-free retrieval),
so every control uses the same block the loom arm actually served.

Usage (after live_harness.py completes; tunnel already up):
  python3 tools/paper/control_harness.py --sets arcane,thin \
      --out uplift-results/paper-v2
"""
from __future__ import annotations
import argparse, json, random, re, sys, time, urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

SCAFFOLD = "http://192.168.2.132:8084/loom/scaffold"
RAW = "http://127.0.0.1:18085/v1/chat/completions"
SETS = {
    "arcane": "uplift-results/general/arcane-questions.json",
    "thin": "uplift-results/general/thin-questions.json",
    "general": "uplift-results/general/general-questions.json",
}
MAX_TOKENS = 1536
HEADER, FOOTER = "[ONTOLOGY CONTEXT]", "[END ONTOLOGY CONTEXT]"


def post(url: str, body: dict, timeout: int = 300) -> dict:
    req = urllib.request.Request(url, data=json.dumps(body).encode(),
                                 headers={"Content-Type": "application/json"}, method="POST")
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.load(r)


def get_scaffold(question: str) -> dict:
    return post(SCAFFOLD, {"prompt": question})


def shuffle_block(block: str, seed: int) -> str:
    """Shuffle sentences of the body; keep wrapper lines in place."""
    body = block
    head = tail = ""
    if block.startswith(HEADER):
        head, body = block[:len(HEADER)], block[len(HEADER):]
    if body.rstrip().endswith(FOOTER):
        idx = body.rstrip().rfind(FOOTER)
        tail, body = body[idx:], body[:idx]
    sentences = re.split(r"(?<=[.;:])\s+|\n", body)
    sentences = [s for s in sentences if s.strip()]
    random.Random(seed).shuffle(sentences)
    return head + "\n" + " ".join(sentences) + "\n" + tail


def mask_block(block: str, seeds: list) -> str:
    """Replace each seed-class title (and slug form) with Entity-<k>."""
    out = block
    for k, s in enumerate(seeds, 1):
        iri = s.get("iri", "")
        slug = iri.rsplit(":", 1)[-1]
        title = slug.replace("-", " ")
        for form in (title, title.title(), slug):
            if form:
                out = re.sub(re.escape(form), f"Entity-{k}", out, flags=re.IGNORECASE)
    return out


def ask_with_system(system: str, question: str) -> dict:
    body = {
        "model": "loom",
        "messages": [{"role": "system", "content": system},
                     {"role": "user", "content": question}],
        "max_tokens": MAX_TOKENS,
        "temperature": 0.0,
    }
    t0 = time.time()
    d = post(RAW, body)
    return {
        "content": (d.get("choices") or [{}])[0].get("message", {}).get("content", ""),
        "latency_s": round(time.time() - t0, 2),
        "completion_tokens": d.get("usage", {}).get("completion_tokens"),
        "model": d.get("model"),
    }


ARMS = ("true", "shuffled", "masked", "irrelevant")


def run_one(setname: str, q: dict, block: str, seeds: list, other_block: str,
            arms: tuple = ARMS, retries: int = 3) -> list:
    rows = []
    if not block:
        # gate declined to inject — controls are identical to raw; record and skip
        return [{"set": setname, "id": q["id"], "arm": a, "skipped": "no-scaffold"}
                for a in arms]
    variants = {
        "true": block,
        "shuffled": shuffle_block(block, seed=hash(q["id"]) & 0xFFFF),
        "masked": mask_block(block, seeds),
        "irrelevant": other_block,
    }
    variants = {a: v for a, v in variants.items() if a in arms}
    if not other_block:
        variants.pop("irrelevant", None)
    for arm, sys_block in variants.items():
        last = None
        for attempt in range(retries):
            try:
                r = ask_with_system(sys_block, q["question"])
                rows.append({"set": setname, "id": q["id"], "arm": arm,
                             "n_seeds": len(seeds), "attempt": attempt, **r})
                break
            except Exception as e:  # noqa: BLE001
                last = str(e)
                time.sleep(5 * (attempt + 1))
        else:
            rows.append({"set": setname, "id": q["id"], "arm": arm, "error": last})
    return rows


def main(argv=None):
    ap = argparse.ArgumentParser()
    ap.add_argument("--sets", default="arcane,thin")
    ap.add_argument("--out", default="uplift-results/paper-v2", type=Path)
    ap.add_argument("--concurrency", type=int, default=2)
    args = ap.parse_args(argv)
    args.out.mkdir(parents=True, exist_ok=True)

    # Phase 1 — pre-fetch every scaffold (LLM-free, fast). The irrelevant arm is
    # a fixed shift-by-one derangement over the ENGAGED questions of each set,
    # so every wrong block is a real, well-formed block from the same corpus.
    questions, blocks, seedmap = [], {}, {}
    for name in args.sets.split(","):
        for q in json.load(open(SETS[name])):
            questions.append((name, q))
            try:
                sc = get_scaffold(q["question"])
                blocks[(name, q["id"])] = sc.get("scaffold") or ""
                seedmap[(name, q["id"])] = sc.get("seeds") or []
            except Exception as e:  # noqa: BLE001
                print(f"scaffold fetch failed for {q['id']}: {e}", file=sys.stderr)
                blocks[(name, q["id"])], seedmap[(name, q["id"])] = "", []
    # Irrelevant donor: any engaged question (either set) whose seed-IRI set is
    # DISJOINT from the recipient's — verified, not assumed. Prefer a donor from
    # a different set; fall back to minimal overlap only if no disjoint donor
    # exists (then record the overlap for honesty).
    engaged = [(n, q) for (n, q) in questions if blocks[(n, q["id"])]]
    iris = {(n, q["id"]): {s.get("iri") for s in seedmap[(n, q["id"])]}
            for (n, q) in engaged}
    other = {}
    for (n, q) in engaged:
        me = iris[(n, q["id"])]
        donor = None
        # pass 1: cross-set disjoint; pass 2: same-set disjoint
        for cross_set in (True, False):
            for (m, p) in engaged:
                if (m, p["id"]) == (n, q["id"]) or (m != n) != cross_set:
                    continue
                if not (me & iris[(m, p["id"])]):
                    donor = (m, p["id"])
                    break
            if donor:
                break
        other[(n, q["id"])] = blocks[donor] if donor else ""
        if not donor:
            print(f"  WARN no seed-disjoint donor for {q['id']} — irrelevant arm skipped",
                  file=sys.stderr)
    print(f"scaffolds fetched: {len(engaged)}/{len(blocks)} engaged", file=sys.stderr)

    outpath = args.out / "control-results.jsonl"
    done = set()
    if outpath.exists():
        for line in open(outpath):
            r = json.loads(line)
            if "error" not in r:
                done.add((r["set"], r["id"], r["arm"]))
    jobs = []
    for (n, q) in questions:
        pending = tuple(a for a in ARMS if (n, q["id"], a) not in done)
        if pending:
            jobs.append((n, q, blocks[(n, q["id"])], seedmap[(n, q["id"])],
                         other.get((n, q["id"]), ""), pending))
    print(f"{len(jobs)} questions with pending arms", file=sys.stderr)

    with open(outpath, "a") as f, ThreadPoolExecutor(max_workers=args.concurrency) as ex:
        n = 0
        for rows in ex.map(lambda j: run_one(*j), jobs):
            for r in rows:
                f.write(json.dumps(r, ensure_ascii=False) + "\n")
            f.flush()
            n += 1
            if n % 5 == 0:
                print(f"  {n}/{len(jobs)} questions done", file=sys.stderr)
    print(f"complete → {outpath}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
