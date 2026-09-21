#!/usr/bin/env python3
"""Regenerate copy_contexts.json for rescore.py.

rescore.py reads the copy-arm ("copy ceiling") text per question from
``copy_contexts.json``. That file was not released either, so it is rebuilt
here by exactly the procedure decompose_exposure.py uses: the git-recovered v1
scaffold engine (ontology_scaffold_v1.py), index app/data/scaffold-index.json,
budget 1500 tokens, max_seeds 4, hops 1, prose off. The concatenated visible
input (system scaffold + user prompt) is what decompose_exposure.py calls
``copy_text``.

Hard gate: the regenerated per-question exposed-gold count must equal the
stored ``n_gold_exposed`` in every sweep scaffold row, and the mean per-question
lexical ceiling must round to 0.964. Aborts otherwise.
"""
from __future__ import annotations
import json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, os.path.join(REPO, "tools", "paper"))
import ontology_scaffold_v1 as osc  # noqa: E402
from decompose_exposure import exposed_flags, read_jsonl  # noqa: E402

INDEX_PATH = os.path.join(REPO, "app", "data", "scaffold-index.json")
QUESTIONS = os.path.join(REPO, "uplift-results", "questions.jsonl")
SWEEP = os.path.join(REPO, "uplift-results", "sweep")
BUDGET = 1500


def main() -> int:
    questions = {q["id"]: q for q in read_jsonl(QUESTIONS)}
    idx = osc.ScaffoldIndex.load(INDEX_PATH)
    contexts, ceilings, exposed = {}, {}, {}
    for qid, q in questions.items():
        gold = q.get("gold") or []
        msgs = [{"role": "user", "content": q["prompt"]}]
        new = osc.scaffold_messages(msgs, budget_tokens=BUDGET, index=idx, prose=False)
        flags = exposed_flags(new, gold)
        exposed[qid] = sum(flags)
        ceilings[qid] = (sum(flags) / len(gold)) if gold else 0.0
        contexts[qid] = " ".join(m.get("content") for m in new
                                 if isinstance(m.get("content"), str))

    mean_ceiling = sum(ceilings.values()) / len(ceilings)
    if round(mean_ceiling, 3) != 0.964:
        sys.stderr.write(f"GATE FAIL: mean ceiling {mean_ceiling:.4f} != 0.964\n")
        return 1
    mismatch = 0
    for label in ("gemini-3.7-flash-t0", "claude-haiku-4.5", "qwen-2.5-72b"):
        for r in read_jsonl(os.path.join(SWEEP, f"results-{label}-scaffold.jsonl")):
            if "error" in r:
                continue
            if exposed[r["id"]] != r.get("n_gold_exposed"):
                mismatch += 1
    if mismatch:
        sys.stderr.write(f"GATE FAIL: {mismatch} exposure mismatches vs stored n_gold_exposed\n")
        return 1

    out = os.path.join(HERE, "copy_contexts.json")
    with open(out, "w", encoding="utf-8") as fh:
        json.dump(contexts, fh, ensure_ascii=False, indent=0)
    print(json.dumps({"questions": len(contexts),
                      "mean_lexical_ceiling": round(mean_ceiling, 6),
                      "exposure_mismatches": mismatch,
                      "mean_context_chars": round(sum(len(v) for v in contexts.values()) / len(contexts), 1),
                      "out": out}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
