#!/usr/bin/env python3
"""parametric_suppression — does injection suppress parametric recall the bare
model demonstrably has?

For every gold item that the deterministic scaffold does NOT expose (exposure
flag False, model-independent), compare:
  * raw-arm recovery   (bare model, no scaffold)   -> parametric recall
  * scaffold-arm recovery (same item, with scaffold) -> n01 behaviour
Pooled and per-model, over the exact 510-question sweep, using the paper's own
byte-identical matcher (imported from decompose_exposure).

Dependency-free; run from repo root:
  PYTHONPATH=tools/paper python3 tools/paper/parametric_suppression.py
"""
from __future__ import annotations
import json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, HERE)

import ontology_scaffold_v1 as osc  # noqa: E402
from decompose_exposure import (  # noqa: E402
    read_jsonl, exposed_flags, recovered_flags, MODELS,
    INDEX_PATH, QUESTIONS, SWEEP, BUDGET,
)


def main() -> int:
    questions = {q["id"]: q for q in read_jsonl(QUESTIONS)}
    idx = osc.ScaffoldIndex.load(INDEX_PATH)

    # model-independent per-item exposure vector
    exposure = {}
    for qid, q in questions.items():
        gold = q.get("gold") or []
        msgs = [{"role": "user", "content": q["prompt"]}]
        new = osc.scaffold_messages(msgs, budget_tokens=BUDGET, index=idx, prose=False)
        exposure[qid] = exposed_flags(new, gold)

    per_model = {}
    pool_unexp = 0
    pool_raw_rec = 0
    pool_scaf_rec = 0
    for label, disp in MODELS:
        scaf = {r["id"]: r for r in read_jsonl(os.path.join(SWEEP, f"results-{label}-scaffold.jsonl"))}
        raw = {r["id"]: r for r in read_jsonl(os.path.join(SWEEP, f"results-{label}-raw.jsonl"))}

        n_unexp = 0          # unexposed gold items with BOTH arms scored
        raw_rec = 0          # of those, recovered by the bare (raw) model
        scaf_rec = 0         # of those, recovered by the scaffold arm
        for qid, q in questions.items():
            exp = exposure[qid]
            r_row = raw.get(qid)
            s_row = scaf.get(qid)
            if not r_row or not s_row or "error" in r_row or "error" in s_row:
                continue
            rraw = recovered_flags(q, r_row.get("answer") or "")
            rscaf = recovered_flags(q, s_row.get("answer") or "")
            for e, vr, vs in zip(exp, rraw, rscaf):
                if e:
                    continue  # only UNEXPOSED gold items
                n_unexp += 1
                if vr:
                    raw_rec += 1
                if vs:
                    scaf_rec += 1
        per_model[label] = {
            "display": disp,
            "n_unexposed_items": n_unexp,
            "raw_recovered": raw_rec,
            "scaffold_recovered": scaf_rec,
            "raw_recall_on_unexposed": raw_rec / n_unexp if n_unexp else None,
            "scaffold_recall_on_unexposed": scaf_rec / n_unexp if n_unexp else None,
        }
        pool_unexp += n_unexp
        pool_raw_rec += raw_rec
        pool_scaf_rec += scaf_rec

    result = {
        "pooled": {
            "n_unexposed_items": pool_unexp,
            "raw_recovered": pool_raw_rec,
            "scaffold_recovered": pool_scaf_rec,
            "raw_recall_on_unexposed": pool_raw_rec / pool_unexp,
            "scaffold_recall_on_unexposed": pool_scaf_rec / pool_unexp,
            "suppression_ratio": (pool_raw_rec / pool_unexp) /
                                 ((pool_scaf_rec / pool_unexp) or float("nan")),
            "n_models_scaffold_zero": sum(
                1 for m in per_model if per_model[m]["scaffold_recovered"] == 0),
        },
        "per_model": per_model,
    }
    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
