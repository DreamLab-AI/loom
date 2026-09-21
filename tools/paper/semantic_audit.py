#!/usr/bin/env python3
"""semantic_audit — does the lexical title matcher agree with semantic answer correctness?

Reviewer request (docs/research/paper-v8/REVIEW-2026-09-21-external-2.md, section
"The paper now identifies the semantic-validation gap correctly, but identifying it
has not closed it"):

    * sample across templates and exposure/recovery categories;
    * distinguish correct relational answers from name-only matches;
    * separate required targets from acceptable alternatives;
    * report where lexical scoring and human assessment disagree.

This script implements the first three and prepares the fourth. **The adjudicator
here is a large language model, not a human.** Every output file, every table and
every field name says so. The human pass is prepared but not performed: the script
emits `human-audit-sample.csv` (120 units, blank verdict columns, blind to both the
matcher category and the model verdict) plus `ANNOTATION-GUIDE.md` so a human
annotator can later be scored against the same rubric and the agreement measured.

Unit of analysis
----------------
A unit is (question, model, gold item, arm=scaffold) — one gold item of one question
as answered by one of the ten released sweep models in the scaffold arm. Exactly the
unit the paper's 2x2 exposure/recovery decomposition counts.

Strata (matcher category), regenerated here, not read from decomposition.json:
    n11  exposed & credited       (the matcher says the model delivered the item)
    n10  exposed & omitted        (the matcher says the model dropped an exposed item)
    n01  unexposed & credited     (only 3 exist pooled; all 3 are taken)
    n00  unexposed & not credited (the negative control for the judge)

Exposure and recovery flags come from `decompose_exposure.exposed_flags` /
`recovered_flags`, imported rather than re-implemented, so the stratification uses
byte-identically the matcher the paper reports. Exposure is model-independent
(regenerated from the frozen question set + app/data/scaffold-index.json with the
vendored v1 scaffold engine), hence every model shares the same 1060 exposed / 76
unexposed items and the n00 stratum is 76 items repeated ten times.

Commands
--------
    python3 tools/paper/semantic_audit.py sample
    python3 tools/paper/semantic_audit.py judge
    python3 tools/paper/semantic_audit.py summarise

Outputs (uplift-results/semantic-audit/):
    frame.json                 sampling frame counts (the full population, by stratum)
    sample.jsonl               the sampled units, with everything the judge sees
    judge-cache.jsonl          every judge call, keyed by sha256(judge_model||prompt)
    verdicts.jsonl             one parsed verdict per unit
    summary.json               all numbers, machine-readable
    SUMMARY.md                 the tables
    human-audit-sample.csv     120 units for a human annotator (blind, blank columns)
    ANNOTATION-GUIDE.md        the rubric, written for a human

Deterministic: seed 42 everywhere; judging is temperature 0 and cached, so re-running
`judge` costs nothing and returns the identical verdicts.
"""
from __future__ import annotations

import argparse
import concurrent.futures as cf
import csv
import hashlib
import json
import math
import os
import random
import re
import sys
import threading
import time
import urllib.error
import urllib.request
from collections import Counter, defaultdict
from datetime import datetime, timezone

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, HERE)

import decompose_exposure as dx  # noqa: E402  (the paper's own matcher + 2x2 logic)
import ontology_scaffold_v1 as osc  # noqa: E402  (vendored v1 scaffold engine)

QUESTIONS = os.path.join(REPO, "uplift-results", "questions.jsonl")
INDEX_PATH = os.path.join(REPO, "app", "data", "scaffold-index.json")
SWEEP = os.path.join(REPO, "uplift-results", "sweep")
OUT_DIR = os.path.join(REPO, "uplift-results", "semantic-audit")

SEED = 42
BUDGET = dx.BUDGET            # 1500, the sweep's scaffold budget
MODELS = dx.MODELS            # the ten released sweep models, (label, display)

# Target sample sizes per matcher category (the reviewer's "focused" audit).
QUOTAS = {"n11": 240, "n10": 120, "n00": 60}   # n01 is a census: all 3 units
ANSWER_CHARS = 4000           # answer truncation shown to the judge (as judge_v2)
HUMAN_N = 120                 # nested human-annotation subsample

VERDICTS = ["correct_relation", "name_only", "wrong_relation",
            "reversed", "negated", "absent"]
REQ_ALT = ["required", "alternative", "n/a"]

# --------------------------------------------------------------------------
# The rubric. Printed verbatim into SUMMARY.md, ANNOTATION-GUIDE.md and
# summary.json, and hashed into the cache key so a rubric edit invalidates the
# cached verdicts rather than silently mixing two rubrics.
# --------------------------------------------------------------------------
RUBRIC = """\
You are auditing whether a model's free-text answer actually ASSERTS a specific \
fact from a knowledge graph, or merely mentions a name.

You are given: a QUESTION about the DreamLab knowledge graph; the GOLD EDGE, which \
is the single graph fact being audited (subject, relation, target); the GOLD ITEM, \
which is that edge's target; the FULL GOLD SET for the question and whether the \
question requires ALL of them or ANY ONE of them; and the model's ANSWER.

Decide what the ANSWER asserts about the GOLD ITEM, with respect to the relation the \
QUESTION asks for. Judge the answer's claims only; do not reward or punish style, \
length, hedging or extra correct material.

Field 1 - "asserted", exactly one of:
  "correct_relation" - the answer states that the GOLD ITEM stands in the relation \
the question asks for, with the correct direction and no negation. Paraphrase, \
synonym and inflection count: "relies on X", "X is a prerequisite" and "depends on \
X" are the same assertion. A bare list offered as the answer to the question counts \
as asserting the relation for every item in it.
  "name_only" - the GOLD ITEM's name (or an unambiguous paraphrase of it) appears in \
the answer, but the answer does not assert that it stands in the asked-for relation: \
e.g. it appears in a caveat, a restatement of the question, an aside, a different \
relation's list, an example of something else, or a list the answer explicitly \
declines to endorse.
  "wrong_relation" - the answer places the GOLD ITEM in a DIFFERENT relation to the \
subject than the one asked for (e.g. the question asks what the subject enables and \
the answer says the subject depends on it, or calls it a sibling/synonym/part).
  "reversed" - the answer asserts the asked-for relation but with subject and target \
swapped (e.g. asked "what does A enable?", the answer says the GOLD ITEM enables A).
  "negated" - the answer explicitly denies the relation (e.g. "A does not depend on \
the GOLD ITEM", "the GOLD ITEM is not among them").
  "absent" - the GOLD ITEM is not referred to at all, under any name.

Choose the first label that fits in this order of precedence: negated, reversed, \
wrong_relation, correct_relation, name_only, absent.

Field 2 - "required_or_alternative", exactly one of:
  "required" - a complete answer to the QUESTION must include the GOLD ITEM \
(the question requires ALL of the gold set, or the gold set has only this member).
  "alternative" - the question accepts ANY ONE of several gold items, and the ANSWER \
satisfies the question by supplying a DIFFERENT member of the gold set instead. Use \
this only when the answer really does supply an accepted alternative.
  "n/a" - the question accepts any one of several gold items, but the answer supplies \
none of them (so nothing was substituted).

Field 3 - "confidence", a number between 0 and 1: how sure you are of field 1.

Field 4 - "quote": the shortest verbatim span of the ANSWER your field-1 label rests \
on, copied exactly. Use the empty string "" if and only if field 1 is "absent".

Respond with ONLY a JSON object and nothing else:
{"asserted": "...", "required_or_alternative": "...", "confidence": 0.0, "quote": "..."}
"""

RUBRIC_HASH = hashlib.sha256(RUBRIC.encode()).hexdigest()[:16]

RELATION_PHRASE = {
    "enables": "enables",
    "requires": "requires",
    "dependsOn": "depends on",
    "partOf": "is part of",
    "relatedTo": "is related to",
    "usedBy": "is used by",
    "produces": "produces",
    "influences": "influences",
}


# --------------------------------------------------------------------------
# frame construction
# --------------------------------------------------------------------------

def load_questions() -> dict:
    return {q["id"]: q for q in dx.read_jsonl(QUESTIONS)}


def build_exposure(questions: dict, cache_path: str) -> dict:
    """Per-question per-gold-item exposure flags, regenerated with the v1 engine.

    Cached to disk because regeneration is the slow part (510 scaffold builds)
    and it is model-independent and fully deterministic."""
    if os.path.exists(cache_path):
        cached = json.load(open(cache_path))
        if cached.get("budget") == BUDGET and len(cached.get("exposure", {})) == len(questions):
            return cached["exposure"]
    idx = osc.ScaffoldIndex.load(INDEX_PATH)
    exposure = {}
    for qid, q in questions.items():
        msgs = [{"role": "user", "content": q["prompt"]}]
        new = osc.scaffold_messages(msgs, budget_tokens=BUDGET, index=idx, prose=False)
        exposure[qid] = dx.exposed_flags(new, q.get("gold") or [])
    os.makedirs(os.path.dirname(cache_path), exist_ok=True)
    json.dump({"budget": BUDGET, "engine": "ontology_scaffold_v1.py (git blob c7b8fb1)",
               "exposure": exposure}, open(cache_path, "w"))
    return exposure


def load_index_titles() -> dict:
    idx = json.load(open(INDEX_PATH))
    return {slug: rec.get("t", slug) for slug, rec in idx["classes"].items()}


def subject_titles(q: dict, titles: dict) -> list:
    return [titles.get(s, s.replace("-", " ").title()) for s in q.get("class_slugs") or []]


def gold_edge(q: dict, gold: dict, titles: dict) -> tuple:
    """(relation_asked, human-readable gold graph edge) for this question/item."""
    subs = subject_titles(q, titles)
    gt = gold.get("title", gold.get("slug", ""))
    tpl = q["template"]
    if tpl == "T-REL":
        pred = q["key"].split(":")[-1]
        phrase = RELATION_PHRASE.get(pred, re.sub(r"(?<!^)(?=[A-Z])", " ", pred).lower())
        return (f"{pred} ({phrase})", f"{subs[0]} --{pred}--> {gt}")
    if tpl == "T-TAX":
        return ("subClassOf (immediate parent concept)",
                f"{subs[0]} --subClassOf--> {gt}")
    if tpl == "T-COMMON":
        a, b = (subs + ["?", "?"])[:2]
        return ("common ancestor (shared broader concept)",
                f"{a} --ancestorOf^-1--> {gt} <--ancestorOf^-1-- {b}")
    return ("unknown", f"? --?--> {gt}")


_CAMEL_RE = re.compile(r"[a-z][A-Z]")


def mechanism_flags(gold_title: str, subjects: list, answer_norm: str,
                    answer_words: set, credited: bool) -> dict:
    """Deterministic, judge-free flags for the known failure mechanisms of the
    matcher. Computed over the WHOLE frame, so they say *why* the matcher errs
    exhaustively, where the judged sample only estimates *how often*.

    gold_substring_of_subject - the gold title is a substring of one of the
        question's own subject names, so the matcher can credit the item purely
        because the answer restates the question (e.g. gold "Customer
        Experience" inside subject "Customer Experience Management").
    matched_by_word_bag_only - credit came from the loose >=0.8 long-word overlap
        path rather than an outright substring hit.
    gold_unspaced_camel - the gold title runs words together ("Sustainability
        Reporting" stored as "SustainabilityReporting"); `normalise` strips
        punctuation but does not split camel case, so no naturally spaced answer
        can ever match it. A defect in the frozen gold, not in the model.
    gold_single_token - the gold title normalises to a single word, the weakest
        possible substring test.
    """
    nt = dx.normalise(gold_title)
    subs = [dx.normalise(s) for s in subjects]
    substring_hit = bool(nt) and nt in answer_norm
    return {
        "gold_substring_of_subject": bool(nt) and any(
            nt in s and nt != s for s in subs),
        "matched_by_word_bag_only": bool(credited and not substring_hit),
        "gold_unspaced_camel": bool(_CAMEL_RE.search(gold_title or "")
                                    and " " not in (gold_title or "").strip()),
        "gold_single_token": len(nt.split()) == 1,
    }


def build_frame(questions: dict, exposure: dict, titles: dict) -> list:
    """Every (question, model, gold item) unit in the scaffold arm, with its
    matcher category. This IS the sampling frame: 11,360 units."""
    frame = []
    for label, disp in MODELS:
        rows = dx.read_jsonl(os.path.join(SWEEP, f"results-{label}-scaffold.jsonl"))
        for r in rows:
            if "error" in r:
                continue
            qid = r["id"]
            q = questions[qid]
            gold = q.get("gold") or []
            answer = r.get("answer") or ""
            exp = exposure[qid]
            rec = dx.recovered_flags(q, answer)
            q_recall = dx.score_answer_recall(q, answer)
            other = [g["title"] for g, v in zip(gold, rec) if v]
            a_norm = dx.normalise(answer)
            a_words = set(a_norm.split())
            subs = subject_titles(q, titles)
            for i, (g, e, v) in enumerate(zip(gold, exp, rec)):
                cat = ("n11" if (e and v) else "n10" if (e and not v)
                       else "n01" if ((not e) and v) else "n00")
                rel, edge = gold_edge(q, g, titles)
                mech = mechanism_flags(g.get("title", ""), subs, a_norm, a_words, v)
                frame.append({
                    "mechanisms": mech,
                    "unit_id": f"{label}|{qid}|{i}",
                    "model": label, "model_display": disp, "arm": "scaffold",
                    "question_id": qid, "item_index": i,
                    "template": q["template"], "gold_type": q["gold_type"],
                    "domain": q.get("domain"), "question_key": q["key"],
                    "category": cat, "exposed": bool(e), "credited": bool(v),
                    "question": q["prompt"],
                    "subjects": subs,
                    "relation_asked": rel, "gold_edge": edge,
                    "gold_slug": g.get("slug"), "gold_title": g.get("title"),
                    "gold_set": [x.get("title") for x in gold],
                    "n_gold": len(gold),
                    "question_recall_matcher": q_recall,
                    "other_gold_credited": [t for t in other if t != g.get("title")],
                    "answer": answer,
                })
    return frame


# --------------------------------------------------------------------------
# stratified allocation (largest remainder, deterministic)
# --------------------------------------------------------------------------

def largest_remainder(quota: int, weights: dict) -> dict:
    """Allocate `quota` over keys proportionally to `weights`, ties broken by
    sorted key order so the allocation is deterministic. Never allocates more
    than a stratum holds; the surplus spills to strata with room left."""
    total = sum(weights.values())
    if total == 0 or quota <= 0:
        return {k: 0 for k in weights}
    quota = min(quota, total)
    exact = {k: quota * w / total for k, w in weights.items()}
    alloc = {k: int(math.floor(v)) for k, v in exact.items()}
    left = quota - sum(alloc.values())
    order = sorted(weights, key=lambda k: (-(exact[k] - alloc[k]), str(k)))
    for k in order[:left]:
        alloc[k] += 1
    spill = 0
    for k in list(alloc):
        if alloc[k] > weights[k]:
            spill += alloc[k] - weights[k]
            alloc[k] = weights[k]
    while spill > 0:
        room = [k for k in weights if alloc[k] < weights[k]]
        if not room:
            break
        for k in sorted(room, key=lambda k: (-(weights[k] - alloc[k]), str(k))):
            if spill == 0:
                break
            alloc[k] += 1
            spill -= 1
    return alloc


def stratified_draw(units: list, quota: int, rng: random.Random) -> list:
    """Two-level proportional allocation: (template, gold_type) cell, then model
    within the cell; simple random sample without replacement inside each
    (cell, model) bucket."""
    by_cell = defaultdict(list)
    for u in units:
        by_cell[(u["template"], u["gold_type"])].append(u)
    cell_alloc = largest_remainder(quota, {k: len(v) for k, v in by_cell.items()})
    out = []
    for cell in sorted(by_cell, key=lambda c: (c[0], c[1])):
        pool = by_cell[cell]
        by_model = defaultdict(list)
        for u in pool:
            by_model[u["model"]].append(u)
        m_alloc = largest_remainder(cell_alloc[cell],
                                    {k: len(v) for k, v in by_model.items()})
        for m in sorted(by_model):
            bucket = sorted(by_model[m], key=lambda u: u["unit_id"])
            k = m_alloc[m]
            if k:
                out.extend(rng.sample(bucket, k))
    return sorted(out, key=lambda u: u["unit_id"])


# --------------------------------------------------------------------------
# sample
# --------------------------------------------------------------------------

def cmd_sample(args) -> int:
    os.makedirs(OUT_DIR, exist_ok=True)
    questions = load_questions()
    titles = load_index_titles()
    exposure = build_exposure(questions, os.path.join(OUT_DIR, "exposure-cache.json"))
    frame = build_frame(questions, exposure, titles)

    by_cat = defaultdict(list)
    for u in frame:
        by_cat[u["category"]].append(u)

    # sanity gate: the regenerated frame must reproduce decomposition.json's 2x2
    pooled = {c: len(by_cat[c]) for c in ("n11", "n10", "n01", "n00")}
    dec_path = os.path.join(REPO, "uplift-results", "paper-v2", "decomposition.json")
    gate = {"checked": False}
    if os.path.exists(dec_path):
        d = json.load(open(dec_path))["pooled_2x2"]
        expect = {"n11": d["n11_exposed_recovered"], "n10": d["n10_exposed_omitted"],
                  "n01": d["n01_unexposed_recovered"], "n00": d["n00_unexposed_omitted"]}
        gate = {"checked": True, "expected": expect, "regenerated": pooled,
                "match": expect == pooled}
        if not gate["match"]:
            print(f"FATAL: regenerated 2x2 {pooled} != decomposition.json {expect}",
                  file=sys.stderr)
            return 2

    rng = random.Random(SEED)
    sample = []
    for cat in ("n11", "n10", "n00"):
        sample.extend(stratified_draw(by_cat[cat], QUOTAS[cat], rng))
    sample.extend(sorted(by_cat["n01"], key=lambda u: u["unit_id"]))  # census
    sample.sort(key=lambda u: (u["category"], u["unit_id"]))

    # nested human subsample, drawn from the judged sample with the same design
    hrng = random.Random(SEED + 1)
    human = []
    hq = largest_remainder(HUMAN_N, {c: len([u for u in sample if u["category"] == c])
                                     for c in ("n11", "n10", "n00", "n01")})
    for cat in ("n11", "n10", "n00", "n01"):
        human.extend(stratified_draw([u for u in sample if u["category"] == cat],
                                     hq[cat], hrng))
    human_ids = {u["unit_id"] for u in human}
    for u in sample:
        u["in_human_sample"] = u["unit_id"] in human_ids

    frame_counts = {
        "n_units_total": len(frame),
        "pooled_by_category": pooled,
        "by_category_template_gold_type": {
            c: {f"{t}|{gt}": n for (t, gt), n in sorted(
                Counter((u["template"], u["gold_type"]) for u in by_cat[c]).items())}
            for c in ("n11", "n10", "n01", "n00")},
        "by_category_model": {
            c: dict(sorted(Counter(u["model"] for u in by_cat[c]).items()))
            for c in ("n11", "n10", "n01", "n00")},
        "population_note": (
            "Exposure is model-independent (regenerated from the frozen question set "
            "and app/data/scaffold-index.json), so all ten models share the same 1060 "
            "exposed and 76 unexposed gold items; the 760 n00 units are 76 distinct "
            "items repeated across ten models and are therefore not independent."),
        "decomposition_gate": gate,
        "mechanism_census": {
            "note": (
                "Deterministic, judge-free flags over all 11,360 frame units, by "
                "matcher category. These identify structural failure modes of the "
                "matcher exhaustively; the judged sample estimates how often each "
                "one actually changes the verdict."),
            "counts": {
                c: {m: sum(1 for u in by_cat[c] if u["mechanisms"][m])
                    for m in ("gold_substring_of_subject", "matched_by_word_bag_only",
                              "gold_unspaced_camel", "gold_single_token")}
                for c in ("n11", "n10", "n01", "n00")},
            "denominators": pooled,
            "unspaced_camel_gold_titles": sorted({
                u["gold_title"] for u in frame if u["mechanisms"]["gold_unspaced_camel"]}),
        },
        "deterministic_compliant_omissions": {
            "note": (
                "The paper's existing sensitivity analysis: an n10 item on an "
                "`any`-type question whose answer nonetheless satisfies the question "
                "via a different accepted alternative, so the any-collapse scorer "
                "already counts the question as fully correct. Counted here over the "
                "whole frame, to be compared with the judge's sampled "
                "`n10_compliant_omission_rate`."),
            "n10_total": pooled["n10"],
            "n10_from_any_questions": sum(
                1 for u in by_cat["n10"] if u["gold_type"] == "any"),
            "compliant_omissions": sum(
                1 for u in by_cat["n10"]
                if u["gold_type"] == "any" and u["question_recall_matcher"] == 1.0),
        },
        "quotas": {**QUOTAS, "n01": len(by_cat["n01"])},
        "sample_size": len(sample),
        "sampled_by_category": dict(sorted(Counter(u["category"] for u in sample).items())),
        "sampled_by_category_template_gold_type": {
            c: {f"{t}|{gt}": n for (t, gt), n in sorted(Counter(
                (u["template"], u["gold_type"]) for u in sample
                if u["category"] == c).items())}
            for c in ("n11", "n10", "n01", "n00")},
        "sampled_by_model": dict(sorted(Counter(u["model"] for u in sample).items())),
        "human_sample_size": len(human),
        "human_by_category": dict(sorted(Counter(u["category"] for u in human).items())),
        "seed": SEED,
        "rubric_sha256_16": RUBRIC_HASH,
    }

    with open(os.path.join(OUT_DIR, "sample.jsonl"), "w") as fh:
        for u in sample:
            fh.write(json.dumps(u) + "\n")
    json.dump(frame_counts, open(os.path.join(OUT_DIR, "frame.json"), "w"), indent=2)
    write_human_csv(sample, human_ids)

    print(f"frame: {len(frame)} units  {pooled}")
    print(f"sample: {len(sample)} units  {frame_counts['sampled_by_category']}")
    print(f"human subsample: {len(human)} units  {frame_counts['human_by_category']}")
    print(f"wrote {OUT_DIR}/sample.jsonl, frame.json, human-audit-sample.csv")
    return 0


def write_human_csv(sample: list, human_ids: set) -> None:
    """Blind human-annotation sheet: no matcher category, no model verdict."""
    cols = ["unit_id", "model", "question_id", "template", "gold_type",
            "question", "gold_item", "relation_asked", "gold_edge",
            "full_gold_set", "model_answer",
            "human_asserted", "human_required_or_alternative",
            "human_confidence", "human_quote", "human_notes"]
    rows = [u for u in sample if u["unit_id"] in human_ids]
    rows.sort(key=lambda u: hashlib.sha256((u["unit_id"] + "blind").encode()).hexdigest())
    path = os.path.join(OUT_DIR, "human-audit-sample.csv")
    with open(path, "w", newline="", encoding="utf-8") as fh:
        w = csv.writer(fh)
        w.writerow(cols)
        for u in rows:
            w.writerow([u["unit_id"], u["model_display"], u["question_id"],
                        u["template"], u["gold_type"], u["question"],
                        u["gold_title"], u["relation_asked"], u["gold_edge"],
                        " | ".join(u["gold_set"]), u["answer"][:ANSWER_CHARS],
                        "", "", "", "", ""])
    with open(os.path.join(OUT_DIR, "ANNOTATION-GUIDE.md"), "w") as fh:
        fh.write(annotation_guide())


def annotation_guide() -> str:
    verdict_list = "`, `".join(VERDICTS)
    reqalt_list = "`, `".join(REQ_ALT)
    return f"""\
# Annotation guide — semantic audit of the lexical title matcher

**Status: the verdicts in `verdicts.jsonl` were produced by a large language model,
not by a human.** This sheet exists so that a human pass can be added and the two
compared. Nothing in this directory should be described as human-validated until
`human-audit-sample.csv` is filled in and re-summarised.

## What you are annotating

`human-audit-sample.csv` holds {HUMAN_N} rows. Each row is one **unit**: one gold
knowledge-graph item, for one question, as answered by one model in the scaffold
(grounded) arm of the released ten-model sweep.

The sheet is **blind**: it deliberately does not tell you what the lexical matcher
decided about the item, nor what the model judge decided. Rows are shuffled by a hash
of the unit id so the categories are interleaved. Do not consult `sample.jsonl` or
`verdicts.jsonl` before annotating — they both carry the matcher category.

## Columns to fill

| Column | Values |
|---|---|
| `human_asserted` | one of `{verdict_list}` |
| `human_required_or_alternative` | one of `{reqalt_list}` |
| `human_confidence` | 0.0-1.0 |
| `human_quote` | shortest verbatim span of `model_answer` your label rests on; empty iff `absent` |
| `human_notes` | free text, optional |

## The rubric (identical to the one given to the model judge)

```
{RUBRIC}```

## After annotating

Save as `human-audit-sample-filled.csv` in this directory and re-run:

    python3 tools/paper/semantic_audit.py summarise --human human-audit-sample-filled.csv

`summarise` will then add a human-vs-model-judge agreement block (per-field
agreement, Cohen's kappa on `asserted`, and the human-only versions of the headline
rates) to `summary.json` and `SUMMARY.md`.
"""


# --------------------------------------------------------------------------
# judge
# --------------------------------------------------------------------------

def render_prompt(u: dict) -> str:
    gold_set = u["gold_set"]
    req = ("ALL of the gold set are required for a complete answer."
           if u["gold_type"] == "all" else
           "ANY ONE of the gold set is an acceptable answer.")
    others = (", ".join(u["other_gold_credited"]) or "(none)")
    return (
        f"QUESTION:\n{u['question']}\n\n"
        f"RELATION THE QUESTION ASKS FOR:\n{u['relation_asked']}\n\n"
        f"GOLD EDGE UNDER AUDIT:\n{u['gold_edge']}\n\n"
        f"GOLD ITEM (the edge's target, the item you are judging):\n{u['gold_title']}\n\n"
        f"FULL GOLD SET FOR THIS QUESTION ({u['gold_type']}): "
        f"{'; '.join(gold_set)}\n{req}\n"
        f"OTHER GOLD-SET MEMBERS THE ANSWER APPEARS TO NAME: {others}\n\n"
        f"ANSWER:\n{u['answer'][:ANSWER_CHARS]}\n\n"
        f"Judge now."
    )


def cache_key(judge_model: str, prompt: str) -> str:
    return hashlib.sha256(
        f"{judge_model}\x00{RUBRIC_HASH}\x00{prompt}".encode()).hexdigest()


def _post_json(url: str, headers: dict, body: bytes, timeout: int):
    req = urllib.request.Request(url, data=body, headers=headers, method="POST")
    return json.load(urllib.request.urlopen(req, timeout=timeout))


BACKENDS = [
    # (name, base url, model id, env var holding the key, family)
    ("openai", "https://api.openai.com/v1", "gpt-4.1", "OPENAI_API_KEY", "openai/gpt"),
    ("openrouter", "https://openrouter.ai/api/v1", "openai/gpt-4.1",
     "OPENROUTER_API_KEY", "openai/gpt"),
    ("anthropic", "https://api.anthropic.com/v1", "claude-opus-4-6",
     "ANTHROPIC_API_KEY", "anthropic/claude"),
]


def probe_backend(name: str) -> tuple:
    """Return (ok, detail) for a backend, with one cheap live call."""
    spec = next(b for b in BACKENDS if b[0] == name)
    _, base, model, envvar, _ = spec
    key = os.environ.get(envvar) or ""
    if not key:
        return False, f"{envvar} empty or unset"
    try:
        if name == "anthropic":
            _post_json(base + "/messages",
                       {"x-api-key": key, "anthropic-version": "2023-06-01",
                        "Content-Type": "application/json"},
                       json.dumps({"model": model, "max_tokens": 8,
                                   "messages": [{"role": "user",
                                                 "content": "ok"}]}).encode(),
                       40)
        else:
            _post_json(base + "/chat/completions",
                       {"Authorization": f"Bearer {key}",
                        "Content-Type": "application/json"},
                       json.dumps({"model": model, "temperature": 0, "max_tokens": 8,
                                   "messages": [{"role": "user",
                                                 "content": "ok"}]}).encode(),
                       40)
        return True, "live probe ok"
    except urllib.error.HTTPError as e:
        return False, f"HTTP {e.code}: {e.read().decode()[:160]}"
    except Exception as e:  # noqa: BLE001
        return False, f"{type(e).__name__}: {str(e)[:160]}"


def call_judge(backend: str, prompt: str, retries: int = 5, timeout: int = 120):
    spec = next(b for b in BACKENDS if b[0] == backend)
    _, base, model, envvar, _ = spec
    key = os.environ[envvar]
    if backend == "anthropic":
        url = base + "/messages"
        headers = {"x-api-key": key, "anthropic-version": "2023-06-01",
                   "Content-Type": "application/json"}
        body = json.dumps({"model": model, "max_tokens": 400, "temperature": 0,
                           "system": RUBRIC,
                           "messages": [{"role": "user", "content": prompt}]}).encode()
    else:
        url = base + "/chat/completions"
        headers = {"Authorization": f"Bearer {key}", "Content-Type": "application/json"}
        body = json.dumps({"model": model, "temperature": 0, "max_tokens": 400,
                           "messages": [{"role": "system", "content": RUBRIC},
                                        {"role": "user", "content": prompt}]}).encode()
    delay, last = 4.0, None
    for attempt in range(retries + 1):
        try:
            r = _post_json(url, headers, body, timeout)
            if backend == "anthropic":
                txt = "".join(c.get("text", "") for c in r.get("content", []))
                u = r.get("usage", {})
                usage = {"prompt_tokens": u.get("input_tokens"),
                         "completion_tokens": u.get("output_tokens")}
            else:
                txt = r["choices"][0]["message"].get("content") or ""
                usage = r.get("usage", {})
            return txt, usage, None
        except urllib.error.HTTPError as e:
            code = e.code
            last = f"HTTP {code}: {e.read().decode()[:160]}"
            if code in (408, 409, 429, 500, 502, 503, 529) and attempt < retries:
                time.sleep(delay)
                delay = min(delay * 2, 60)
                continue
        except Exception as e:  # noqa: BLE001
            last = f"{type(e).__name__}: {str(e)[:160]}"
        if attempt < retries:
            time.sleep(delay)
            delay = min(delay * 2, 60)
    return None, {}, last


_JSON_RE = re.compile(r"\{.*\}", re.DOTALL)

# Appended to the prompt on the single repair re-ask, for the rare response whose
# field 1 does not carry one of the six rubric labels. Temperature is 0, so a plain
# retry would return the identical malformed reply; the re-ask has to differ. Repair
# calls are cached under their own key and flagged `repair_pass` in the verdict, so
# every repaired unit stays auditable.
REPAIR_SUFFIX = (
    "\n\nIMPORTANT: your previous reply put a value in \"asserted\" that is not one "
    "of the six permitted labels. \"asserted\" must be exactly one of "
    + ", ".join(f'"{v}"' for v in VERDICTS)
    + ". \"required_or_alternative\" is a separate field and must be exactly one of "
    + ", ".join(f'"{v}"' for v in REQ_ALT)
    + ". Escape any double quote inside \"quote\" as \\\". Reply with only the JSON "
      "object."
)


def _salvage_fields(raw: str):
    """Recover the four fields from a JSON object the judge failed to escape
    properly (in practice: an unescaped double quote inside `quote`). Purely
    mechanical extraction of what the judge actually wrote — no inference."""
    a = re.search(r'"asserted"\s*:\s*"([^"]*)"', raw)
    ra = re.search(r'"required_or_alternative"\s*:\s*"([^"]*)"', raw)
    cf_ = re.search(r'"confidence"\s*:\s*([0-9.]+)', raw)
    qt = re.search(r'"quote"\s*:\s*(.*?)\s*\}\s*$', raw, re.DOTALL)
    if not a:
        return None
    quote = (qt.group(1) if qt else "").strip().strip('"')
    return {"asserted": a.group(1).strip(),
            "required_or_alternative": (ra.group(1).strip() if ra else "n/a"),
            "confidence": float(cf_.group(1)) if cf_ else 0.0,
            "quote": quote}


def parse_verdict(txt: str) -> tuple:
    """-> (verdict dict, None) or (None, reason). The dict carries `parse_repair`
    when the reply had to be salvaged rather than parsed as valid JSON."""
    if not txt:
        return None, "empty response"
    clean = re.sub(r"```(?:json)?|```", "", txt)
    m = _JSON_RE.search(clean)
    if not m:
        return None, f"no JSON object: {txt[:120]}"
    raw = m.group(0)
    repair = None
    try:
        obj = json.loads(raw)
    except json.JSONDecodeError:
        obj = None
        depth, start = 0, None
        for i, ch in enumerate(raw):
            if ch == "{":
                if depth == 0:
                    start = i
                depth += 1
            elif ch == "}":
                depth -= 1
                if depth == 0 and start is not None:
                    try:
                        obj = json.loads(raw[start:i + 1])
                        break
                    except json.JSONDecodeError:
                        pass
        if obj is None:
            obj = _salvage_fields(raw)
            repair = "regex_field_extraction"
        if obj is None:
            return None, f"bad JSON: {txt[:120]}"
    a = str(obj.get("asserted", "")).strip()
    ra = str(obj.get("required_or_alternative", "")).strip()
    if a not in VERDICTS:
        return None, f"asserted not in rubric: {a!r}"
    if ra not in REQ_ALT:
        ra = "n/a"
    try:
        conf = float(obj.get("confidence", 0.0))
    except (TypeError, ValueError):
        conf = 0.0
    v = {"asserted": a, "required_or_alternative": ra,
         "confidence": max(0.0, min(1.0, conf)),
         "quote": str(obj.get("quote", ""))[:600]}
    if repair:
        v["parse_repair"] = repair
    return v, None


def cache_cost_totals(cache_path: str) -> dict:
    """Token and cost totals over every call ever made, from the on-disk cache —
    the real cost of the audit, not just of the most recent (mostly cached) run."""
    ti = to = calls = 0
    if os.path.exists(cache_path):
        for rec in dx.read_jsonl(cache_path):
            u = rec.get("usage") or {}
            ti += u.get("prompt_tokens") or 0
            to += u.get("completion_tokens") or 0
            calls += 1
    return {"total_judge_calls": calls,
            "total_prompt_tokens": ti, "total_completion_tokens": to,
            "cost_estimate_usd_total": round(ti / 1e6 * 2.0 + to / 1e6 * 8.0, 4)}


def cmd_judge(args) -> int:
    sample_path = os.path.join(OUT_DIR, "sample.jsonl")
    if not os.path.exists(sample_path):
        print("run `sample` first", file=sys.stderr)
        return 2
    sample = dx.read_jsonl(sample_path)

    order = [args.backend] if args.backend != "auto" else [b[0] for b in BACKENDS]
    chosen, probes = None, {}
    for name in order:
        ok, detail = probe_backend(name)
        probes[name] = detail
        print(f"backend probe {name}: {'OK' if ok else 'FAIL'} - {detail}")
        if ok:
            chosen = name
            break
    if chosen is None:
        print("FATAL: no judge backend reachable. Probes: "
              + json.dumps(probes, indent=2), file=sys.stderr)
        json.dump({"backend_probes": probes,
                   "when": datetime.now(timezone.utc).isoformat()},
                  open(os.path.join(OUT_DIR, "judge-backend-failure.json"), "w"),
                  indent=2)
        return 3
    spec = next(b for b in BACKENDS if b[0] == chosen)
    judge_model = spec[2]
    print(f"judge backend = {chosen}, model = {judge_model} (family {spec[4]})")

    cache_path = os.path.join(OUT_DIR, "judge-cache.jsonl")
    cache = {}
    if os.path.exists(cache_path):
        for rec in dx.read_jsonl(cache_path):
            if rec.get("response"):
                cache[rec["key"]] = rec
    lock = threading.Lock()
    cache_fh = open(cache_path, "a")

    counters = {"tok_in": 0, "tok_out": 0, "hits": 0, "repairs": 0}

    def ask(u, prompt):
        """One cached judge call for `prompt`; returns the cache record."""
        k = cache_key(judge_model, prompt)
        with lock:
            hit = cache.get(k)
        if hit:
            with lock:
                counters["hits"] += 1
            return hit, True
        txt, usage, err = call_judge(chosen, prompt, retries=args.retries)
        rec = {"key": k, "unit_id": u["unit_id"], "backend": chosen,
               "judge_model": judge_model, "rubric": RUBRIC_HASH,
               "prompt_sha256": hashlib.sha256(prompt.encode()).hexdigest(),
               "response": txt, "usage": usage, "error": err,
               "when": datetime.now(timezone.utc).isoformat()}
        with lock:
            if txt:
                cache[k] = rec
            cache_fh.write(json.dumps(rec) + "\n")
            cache_fh.flush()
            counters["tok_in"] += (usage or {}).get("prompt_tokens") or 0
            counters["tok_out"] += (usage or {}).get("completion_tokens") or 0
        return rec, False

    def work(u):
        prompt = render_prompt(u)
        rec, was_hit = ask(u, prompt)
        v, perr = parse_verdict(rec.get("response") or "")
        if v is not None:
            return u, rec, v, None, False
        # one recorded repair re-ask: the reply was unparseable or field 1 did not
        # carry a rubric label. Temperature is 0, so the re-ask must differ.
        rec2, _ = ask(u, prompt + REPAIR_SUFFIX)
        v2, perr2 = parse_verdict(rec2.get("response") or "")
        if v2 is not None:
            with lock:
                counters["repairs"] += 1
            return u, rec2, v2, None, True
        return u, rec2, None, (rec2.get("error") or perr2 or perr), True

    verdicts, failures = [], []
    with cf.ThreadPoolExecutor(max_workers=args.workers) as ex:
        for n, (u, rec, v, perr, repaired) in enumerate(ex.map(work, sample), 1):
            if v is None:
                failures.append({"unit_id": u["unit_id"], "reason": perr})
                continue
            verdicts.append({
                "unit_id": u["unit_id"], "model": u["model"],
                "model_display": u["model_display"], "question_id": u["question_id"],
                "template": u["template"], "gold_type": u["gold_type"],
                "category": u["category"], "gold_title": u["gold_title"],
                "judge_backend": chosen, "judge_model": judge_model,
                "judge_is_model_not_human": True, "repair_pass": repaired, **v,
            })
            if n % 50 == 0:
                print(f"  {n}/{len(sample)} ({counters['hits']} from cache)")
    cache_fh.close()
    tok_in, tok_out, hits = counters["tok_in"], counters["tok_out"], counters["hits"]

    with open(os.path.join(OUT_DIR, "verdicts.jsonl"), "w") as fh:
        for v in sorted(verdicts, key=lambda v: (v["category"], v["unit_id"])):
            fh.write(json.dumps(v) + "\n")
    meta = {"backend": chosen, "judge_model": judge_model,
            "judge_family": spec[4], "judge_is_model_not_human": True,
            "backend_probes": probes, "temperature": 0,
            "rubric_sha256_16": RUBRIC_HASH,
            "n_units": len(sample), "n_verdicts": len(verdicts),
            "n_failures": len(failures), "failures": failures[:50],
            "n_repair_reasks": counters["repairs"],
            "repair_policy": (
                "A reply whose field 1 is not one of the six rubric labels, or which "
                "is not valid JSON, gets exactly one re-ask with an appended "
                "formatting correction (the rubric and the item are unchanged); "
                "malformed JSON whose fields are nonetheless unambiguous is salvaged "
                "by mechanical field extraction. Both paths are cached under their "
                "own key and flagged (`repair_pass`, `parse_repair`) on the verdict."),
            "cache_hits": hits,
            "prompt_tokens": tok_in, "completion_tokens": tok_out,
            "cost_estimate_usd": round(tok_in / 1e6 * 2.0 + tok_out / 1e6 * 8.0, 4),
            **cache_cost_totals(cache_path),
            "cost_basis": "gpt-4.1 list price $2.00/M input, $8.00/M output. "
                          "`cost_estimate_usd` covers the uncached calls of the last "
                          "run only; `cost_estimate_usd_total` sums every call ever "
                          "written to judge-cache.jsonl, which is what the audit "
                          "actually cost to produce.",
            "when": datetime.now(timezone.utc).isoformat()}
    json.dump(meta, open(os.path.join(OUT_DIR, "judge-meta.json"), "w"), indent=2)
    print(f"verdicts: {len(verdicts)}  failures: {len(failures)}  "
          f"from cache: {hits}  est cost this run ${meta['cost_estimate_usd']}")
    return 0


# --------------------------------------------------------------------------
# summarise
# --------------------------------------------------------------------------

def wilson(k: int, n: int, z: float = 1.959963985) -> tuple:
    if n == 0:
        return (float("nan"), float("nan"), float("nan"))
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return (round(p, 4), round(max(0.0, c - h), 4), round(min(1.0, c + h), 4))


def prop(k: int, n: int) -> dict:
    p, lo, hi = wilson(k, n)
    return {"k": k, "n": n, "p": p, "ci95": [lo, hi]}


def cohen_kappa(pairs: list, labels: list) -> float:
    n = len(pairs)
    if n == 0:
        return float("nan")
    agree = sum(1 for a, b in pairs if a == b) / n
    ca = Counter(a for a, _ in pairs)
    cb = Counter(b for _, b in pairs)
    exp = sum((ca[l] / n) * (cb[l] / n) for l in labels)
    return round((agree - exp) / (1 - exp), 4) if exp < 1 else 1.0


def quote_verifies(verdict: dict, answer: str) -> bool:
    """True when the judge's cited span really occurs in the answer.

    An `absent` verdict cites nothing, so it verifies vacuously. Otherwise both
    sides are put through the matcher's own `normalise` (lowercase, strip
    punctuation, collapse whitespace) and the span — or every fragment of an
    elided span — must occur in the answer. A verdict failing this is a judge
    error: the judge cannot have read there what it says it read."""
    if verdict.get("asserted") == "absent":
        return True
    raw = verdict.get("quote") or ""
    q = dx.normalise(raw)
    if not q:
        return False
    a = dx.normalise(answer)
    if q in a:
        return True
    frags = [f for f in (dx.normalise(p)
                         for p in re.split(r"\.\.\.|…", raw)) if f]
    return bool(frags) and all(f in a for f in frags)


def build_headline(n11: list, n10: list, n00: list, n01: list, rate) -> dict:
    """The headline rates, given the four strata's verdicts. Factored out so the
    same definitions serve both the raw reading and the conservative,
    quote-verified reading."""
    return {
        "matcher_credit_precision_correct_relation": rate(
            n11, lambda v: v["asserted"] == "correct_relation"),
        "matcher_credit_name_only": rate(n11, lambda v: v["asserted"] == "name_only"),
        "matcher_credit_relationally_wrong": rate(
            n11, lambda v: v["asserted"] in ("wrong_relation", "reversed", "negated")),
        "matcher_credit_absent_judge_disagrees": rate(
            n11, lambda v: v["asserted"] == "absent"),
        "n10_compliant_omission_rate": rate(
            n10, lambda v: v["required_or_alternative"] == "alternative"),
        "n10_judge_finds_item_asserted_anyway": rate(
            n10, lambda v: v["asserted"] == "correct_relation"),
        "n10_matcher_false_negative_name_present": rate(
            n10, lambda v: v["asserted"] in ("correct_relation", "name_only")),
        "n10_negated": rate(n10, lambda v: v["asserted"] == "negated"),
        "n10_true_omission_rate": rate(
            n10, lambda v: v["required_or_alternative"] != "alternative"
            and v["asserted"] == "absent"),
        "n00_absent_as_expected": rate(n00, lambda v: v["asserted"] == "absent"),
        "n00_matcher_false_negative": rate(
            n00, lambda v: v["asserted"] == "correct_relation"),
        "n00_name_present_matcher_missed": rate(
            n00, lambda v: v["asserted"] != "absent"),
        "n01_verdicts": dict(sorted(Counter(v["asserted"] for v in n01).items())),
    }


def corrected_estimate(headline: dict, pooled: dict, reading: str) -> dict:
    p11 = headline["matcher_credit_precision_correct_relation"]
    p10 = headline["n10_judge_finds_item_asserted_anyway"]
    N11, N10 = pooled["n11"], pooled["n10"]
    pt = (N11 * p11["p"] + N10 * p10["p"]) / (N11 + N10)
    return {
        "reading": reading,
        "definition": (
            "Matcher credit rate (context utilisation) is n11/(n11+n10) = "
            f"{N11}/{N11 + N10} = {round(N11 / (N11 + N10), 4)}. The model judge's "
            "corrected relational-correctness rate reweights each matcher stratum by "
            "the judged probability that the answer actually asserts the gold edge: "
            "(n11 * P(correct_relation | n11) + n10 * P(correct_relation | n10)) / "
            "(n11 + n10). Both strata are sampled, so this is an estimate; the "
            "interval propagates the two Wilson intervals through the same weights. "
            "The adjudicator is a model, not a human."),
        "matcher_credit_rate_context_utilisation": round(N11 / (N11 + N10), 4),
        "judged_correct_relation_given_credited": p11["p"],
        "judged_correct_relation_given_omitted": p10["p"],
        "corrected_relational_correctness_of_exposed_items": round(pt, 4),
        "corrected_ci95_propagated": [
            round((N11 * p11["ci95"][0] + N10 * p10["ci95"][0]) / (N11 + N10), 4),
            round((N11 * p11["ci95"][1] + N10 * p10["ci95"][1]) / (N11 + N10), 4)],
        "overstatement_pp": round((N11 / (N11 + N10) - pt) * 100, 2),
    }


def breakdown(verdicts: list, keyfn, cats=("n11", "n10", "n00", "n01")) -> dict:
    out = {}
    for k in sorted({keyfn(v) for v in verdicts}):
        sub = [v for v in verdicts if keyfn(v) == k]
        entry = {"n": len(sub)}
        for c in cats:
            cs = [v for v in sub if v["category"] == c]
            if not cs:
                continue
            entry[c] = {"n": len(cs),
                        "verdicts": dict(sorted(
                            Counter(v["asserted"] for v in cs).items()))}
            if c == "n11":
                entry[c]["correct_relation_rate"] = prop(
                    sum(1 for v in cs if v["asserted"] == "correct_relation"), len(cs))
            if c == "n10":
                entry[c]["acceptable_alternative_rate"] = prop(
                    sum(1 for v in cs if v["required_or_alternative"] == "alternative"),
                    len(cs))
        out[k] = entry
    return out


def cmd_summarise(args) -> int:
    frame = json.load(open(os.path.join(OUT_DIR, "frame.json")))
    meta = json.load(open(os.path.join(OUT_DIR, "judge-meta.json")))
    verdicts = dx.read_jsonl(os.path.join(OUT_DIR, "verdicts.jsonl"))
    sample = {u["unit_id"]: u
              for u in dx.read_jsonl(os.path.join(OUT_DIR, "sample.jsonl"))}

    confusion = {c: dict(sorted(Counter(
        v["asserted"] for v in verdicts if v["category"] == c).items()))
        for c in ("n11", "n10", "n01", "n00")}
    req_alt = {c: dict(sorted(Counter(
        v["required_or_alternative"] for v in verdicts if v["category"] == c).items()))
        for c in ("n11", "n10", "n01", "n00")}

    # Attach the quote-verification flag to every verdict before any rate is
    # computed: a non-`absent` verdict that cites a span not present in the answer
    # is a judge error, and the conservative reading discounts it.
    for v in verdicts:
        v["quote_verified"] = quote_verifies(v, sample[v["unit_id"]]["answer"])

    def rate(sub, pred):
        return prop(sum(1 for v in sub if pred(v)), len(sub))

    def headline_for(vs: list) -> dict:
        n11 = [v for v in vs if v["category"] == "n11"]
        n10 = [v for v in vs if v["category"] == "n10"]
        n00 = [v for v in vs if v["category"] == "n00"]
        n01 = [v for v in vs if v["category"] == "n01"]
        return build_headline(n11, n10, n00, n01, rate)

    headline = headline_for(verdicts)

    # Conservative variant. A failed quote check means two different things
    # depending on the stratum, and only one of them impeaches the verdict:
    #
    #   credited strata (n11, n01) - the matcher has independently established
    #       that the gold title occurs in the answer. Inspection shows these
    #       failures are abridged list reconstructions (the judge quotes the list
    #       but drops the per-item gloss between entries). The verdict stands;
    #       the citation is merely imprecise.
    #   uncredited strata (n10, n00) - the matcher says the title does NOT occur.
    #       The judge's quote is then the entire evidence for the contrary claim,
    #       and if that span is not in the answer the claim is unsupported. These
    #       are demoted to `absent`, and any substitution claim is dropped.
    conservative = []
    for v in verdicts:
        credited = sample[v["unit_id"]]["credited"]
        demote = (v["asserted"] != "absent" and not v["quote_verified"]
                  and not credited)
        conservative.append(
            dict(v, asserted="absent", required_or_alternative="n/a",
                 demoted_unsupported_quote=True) if demote
            else dict(v, demoted_unsupported_quote=False))
    headline_cons = headline_for(conservative)

    n11 = [v for v in verdicts if v["category"] == "n11"]
    n10 = [v for v in verdicts if v["category"] == "n10"]
    n00 = [v for v in verdicts if v["category"] == "n00"]
    n01 = [v for v in verdicts if v["category"] == "n01"]

    pooled = frame["pooled_by_category"]
    corrected = corrected_estimate(headline, pooled, "as judged")
    corrected_cons = corrected_estimate(
        headline_cons, pooled,
        "conservative: non-`absent` verdicts with an unverifiable cited span demoted")

    # -- judge reliability: does the judge's quote actually occur in the answer? --
    quoted = [v for v in verdicts if v["asserted"] != "absent"]
    unverifiable = [
        {"unit_id": v["unit_id"], "category": v["category"],
         "asserted": v["asserted"], "quote": v.get("quote"),
         "kind": ("abridged_citation" if sample[v["unit_id"]]["credited"]
                  else "unsupported_citation")}
        for v in quoted if not v["quote_verified"]]
    unsupported = [e for e in unverifiable if e["kind"] == "unsupported_citation"]
    judge_reliability = {
        "definition": (
            "Every non-`absent` verdict must cite a verbatim span of the answer. "
            "This check normalises both (lowercase, strip punctuation, collapse "
            "whitespace — the matcher's own `normalise`) and asks whether the cited "
            "span, or every fragment of an elided span, actually occurs in the "
            "answer. A failure means one of two things. In a CREDITED stratum "
            "(n11/n01) the matcher has already established that the title occurs, "
            "and inspection shows these are abridged list reconstructions — the "
            "judge quotes a list but drops the gloss between entries; the verdict "
            "stands and only the citation is loose (`abridged_citation`). In an "
            "UNCREDITED stratum (n10/n00) the matcher says the title does not "
            "occur, so the quote is the whole of the judge's evidence for the "
            "contrary claim; if that span is not in the answer the claim is "
            "unsupported (`unsupported_citation`) and the conservative reading "
            "demotes it to `absent`."),
        "n_verdicts_with_quote_required": len(quoted),
        "quote_verified": prop(len(quoted) - len(unverifiable), len(quoted)),
        "n_unverifiable": len(unverifiable),
        "n_unsupported_demoted": len(unsupported),
        "n_abridged_kept": len(unverifiable) - len(unsupported),
        "unverifiable_by_category": dict(sorted(
            Counter(e["category"] for e in unverifiable).items())),
        "unverifiable_by_kind": dict(sorted(
            Counter(e["kind"] for e in unverifiable).items())),
        "unverifiable": unverifiable,
    }

    # -- judge verdict cross-tabulated against the deterministic mechanisms -----
    mech_names = ("gold_substring_of_subject", "matched_by_word_bag_only",
                  "gold_unspaced_camel", "gold_single_token")
    mechanism_vs_verdict = {}
    for m in mech_names:
        hits = [v for v in verdicts if sample[v["unit_id"]]["mechanisms"][m]]
        if not hits:
            mechanism_vs_verdict[m] = {"n_in_sample": 0}
            continue
        n11m = [v for v in hits if v["category"] == "n11"]
        mechanism_vs_verdict[m] = {
            "n_in_sample": len(hits),
            "verdicts": dict(sorted(Counter(v["asserted"] for v in hits).items())),
            "n11_correct_relation_rate": prop(
                sum(1 for v in n11m if v["asserted"] == "correct_relation"),
                len(n11m)) if n11m else None,
        }

    examples = []
    for v in verdicts:
        bad = ((v["category"] == "n11" and v["asserted"] != "correct_relation")
               or (v["category"] == "n00" and v["asserted"] != "absent")
               or (v["category"] == "n10" and v["asserted"] == "correct_relation"))
        if not bad:
            continue
        u = sample[v["unit_id"]]
        examples.append({
            "unit_id": v["unit_id"], "category": v["category"],
            "model": v["model_display"], "question_id": v["question_id"],
            "template": v["template"], "question": u["question"],
            "gold_edge": u["gold_edge"], "gold_item": v["gold_title"],
            "asserted": v["asserted"],
            "required_or_alternative": v["required_or_alternative"],
            "confidence": v["confidence"], "quote": v["quote"],
            "answer_excerpt": u["answer"][:600],
        })
    examples.sort(key=lambda e: (-e["confidence"], e["unit_id"]))

    summary = {
        "what_this_is": (
            "A focused, stratified audit of when the paper's lexical title matcher "
            "agrees with semantic answer correctness, and how it fails. "
            "THE ADJUDICATOR IS A LARGE LANGUAGE MODEL, NOT A HUMAN. No human has "
            "annotated any unit in this release; human-audit-sample.csv is prepared "
            "for that pass and is blind to both the matcher category and the model "
            "judge's verdict."),
        "generated": datetime.now(timezone.utc).isoformat(),
        "judge": {k: meta[k] for k in (
            "backend", "judge_model", "judge_family", "judge_is_model_not_human",
            "temperature", "rubric_sha256_16", "n_units", "n_verdicts",
            "n_failures", "n_repair_reasks", "repair_policy",
            "prompt_tokens", "completion_tokens", "cost_estimate_usd",
            "total_judge_calls", "total_prompt_tokens", "total_completion_tokens",
            "cost_estimate_usd_total", "cost_basis", "when")},
        "judge_family_caveat": (
            "The judge is GPT-4.1, cross-family to the local Qwen model the Loom "
            "serves and to eight of the ten sweep models. It is the SAME family as "
            "gpt-4.1-mini, one of the ten, so that model's per-model row carries a "
            "self-preference risk the others do not; the per-model breakdown is "
            "reported so a reader can check it."),
        "rubric": RUBRIC,
        "seed": SEED,
        "sampling_frame": frame,
        "confusion_matcher_category_vs_judge_verdict": confusion,
        "required_or_alternative_by_category": req_alt,
        "headline_rates": headline,
        "headline_rates_conservative": headline_cons,
        "corrected_estimate": corrected,
        "corrected_estimate_conservative": corrected_cons,
        "judge_reliability": judge_reliability,
        "mechanism_vs_verdict": mechanism_vs_verdict,
        "per_template": breakdown(verdicts, lambda v: v["template"]),
        "per_gold_type": breakdown(verdicts, lambda v: v["gold_type"]),
        "per_model": breakdown(verdicts, lambda v: v["model_display"]),
        "notable_disagreements": examples,
    }

    if args.human:
        summary["human_pass"] = human_block(args.human, verdicts, sample)

    json.dump(summary, open(os.path.join(OUT_DIR, "summary.json"), "w"), indent=2)
    with open(os.path.join(OUT_DIR, "SUMMARY.md"), "w") as fh:
        fh.write(render_markdown(summary))
    print(f"wrote {OUT_DIR}/summary.json and SUMMARY.md")
    print(json.dumps(headline, indent=2))
    print(json.dumps(corrected, indent=2))
    return 0


def human_block(path: str, verdicts: list, sample: dict) -> dict:
    p = path if os.path.isabs(path) else os.path.join(OUT_DIR, path)
    if not os.path.exists(p):
        return {"error": f"human file not found: {p}"}
    by_unit = {v["unit_id"]: v for v in verdicts}
    rows, pairs, ra_pairs = [], [], []
    for r in csv.DictReader(open(p, encoding="utf-8")):
        h = (r.get("human_asserted") or "").strip()
        if h not in VERDICTS or r.get("unit_id") not in sample:
            continue
        rows.append(r)
        j = by_unit.get(r["unit_id"])
        if j:
            pairs.append((h, j["asserted"]))
            ra_pairs.append(((r.get("human_required_or_alternative") or "n/a").strip(),
                             j["required_or_alternative"]))
    hv = [{"unit_id": r["unit_id"],
           "category": sample[r["unit_id"]]["category"],
           "asserted": r["human_asserted"].strip(),
           "required_or_alternative":
               (r.get("human_required_or_alternative") or "n/a").strip()}
          for r in rows]
    n11h = [v for v in hv if v["category"] == "n11"]
    n10h = [v for v in hv if v["category"] == "n10"]
    return {
        "n_annotated": len(rows),
        "agreement_asserted": round(
            sum(1 for a, b in pairs if a == b) / len(pairs), 4) if pairs else None,
        "cohens_kappa_asserted": cohen_kappa(pairs, VERDICTS),
        "agreement_required_or_alternative": round(
            sum(1 for a, b in ra_pairs if a == b) / len(ra_pairs), 4)
        if ra_pairs else None,
        "cohens_kappa_required_or_alternative": cohen_kappa(ra_pairs, REQ_ALT),
        "human_confusion": {c: dict(sorted(Counter(
            v["asserted"] for v in hv if v["category"] == c).items()))
            for c in ("n11", "n10", "n01", "n00")},
        "human_matcher_credit_precision": prop(
            sum(1 for v in n11h if v["asserted"] == "correct_relation"), len(n11h)),
        "human_n10_compliant_omission_rate": prop(
            sum(1 for v in n10h if v["required_or_alternative"] == "alternative"),
            len(n10h)),
    }


def _fmt(pr: dict) -> str:
    return (f"{pr['p']:.3f} [{pr['ci95'][0]:.3f}, {pr['ci95'][1]:.3f}] "
            f"({pr['k']}/{pr['n']})")


def render_markdown(s: dict) -> str:
    f, h, c, j = (s["sampling_frame"], s["headline_rates"],
                  s["corrected_estimate"], s["judge"])
    L = []
    A = L.append
    means = {"n11": "exposed & credited", "n10": "exposed & omitted",
             "n01": "unexposed & credited", "n00": "unexposed & not credited"}
    A("# Semantic audit of the lexical title matcher\n")
    A("> **The adjudicator in this audit is a large language model, not a human.**  ")
    A(f"> Every verdict below was produced by `{j['judge_model']}` at temperature 0 "
      f"via the `{j['backend']}` backend on {j['when'][:10]}. No human has annotated "
      "any unit. `human-audit-sample.csv` and `ANNOTATION-GUIDE.md` in this directory "
      "prepare that pass; until it is done and re-summarised, nothing here is "
      "human-validated.\n")
    A(f"{s['what_this_is']}\n")
    A(f"**Judge-family caveat.** {s['judge_family_caveat']}\n")

    A("## 1. Sampling frame\n")
    A("Unit = (question, model, gold item, arm = scaffold). The frame is every such "
      f"unit in the released ten-model sweep: **{f['n_units_total']:,}** units "
      "(510 questions x 10 models x their gold items).\n")
    A("| matcher category | meaning | frame | sampled |")
    A("|---|---|---:|---:|")
    for k in ("n11", "n10", "n01", "n00"):
        A(f"| {k} | {means[k]} | {f['pooled_by_category'][k]:,} | "
          f"{f['sampled_by_category'].get(k, 0)} |")
    A(f"| **total** | | **{f['n_units_total']:,}** | **{f['sample_size']}** |\n")
    A(f"{f['population_note']}\n")
    g = f.get("decomposition_gate", {})
    if g.get("checked"):
        A(f"Gate: the regenerated 2x2 "
          f"{'matches' if g['match'] else '**DOES NOT MATCH**'} "
          f"`uplift-results/paper-v2/decomposition.json` exactly "
          f"({g['regenerated']}).\n")
    A("Sampling is stratified: within each matcher category the quota is allocated "
      "over (template x gold_type) cells proportionally to cell size (largest "
      "remainder), then over models within each cell proportionally to their counts, "
      "then simple random sampling without replacement inside each bucket. "
      f"Seed {s['seed']}. All {f['pooled_by_category']['n01']} n01 units are a "
      "census, not a sample.\n")
    cells = sorted({k for cat in ("n11", "n10", "n01", "n00")
                    for k in f["sampled_by_category_template_gold_type"].get(cat, {})})
    A("Sampled units by (template / gold_type):\n")
    A("| category | " + " | ".join(c.replace("|", " / ") for c in cells) + " |")
    A("|---" * (len(cells) + 1) + "|")
    for cat in ("n11", "n10", "n01", "n00"):
        row = f["sampled_by_category_template_gold_type"].get(cat, {})
        A(f"| {cat} | " + " | ".join(str(row.get(k, 0)) for k in cells) + " |")
    A("")
    A("Sampled units by model: "
      + ", ".join(f"{k} {v}" for k, v in f["sampled_by_model"].items()) + "\n")

    A("## 2. Confusion: matcher category vs model-judge verdict\n")
    A("| matcher category | " + " | ".join(VERDICTS) + " | n |")
    A("|---" * (len(VERDICTS) + 2) + "|")
    for cat in ("n11", "n10", "n01", "n00"):
        row = s["confusion_matcher_category_vs_judge_verdict"][cat]
        A(f"| {cat} ({means[cat]}) | "
          + " | ".join(str(row.get(v, 0)) for v in VERDICTS)
          + f" | {sum(row.values())} |")
    A("")
    A("`required_or_alternative`, by category:\n")
    A("| matcher category | " + " | ".join(REQ_ALT) + " |")
    A("|---" * (len(REQ_ALT) + 1) + "|")
    for cat in ("n11", "n10", "n01", "n00"):
        row = s["required_or_alternative_by_category"][cat]
        A(f"| {cat} | " + " | ".join(str(row.get(v, 0)) for v in REQ_ALT) + " |")
    A("")

    A("## 3. Headline rates (95% Wilson intervals)\n")
    A("Two readings of every rate. *As judged* takes the judge at its word. "
      "*Quote-verified* demotes to `absent` those verdicts in the **uncredited** "
      "strata (n10, n00) whose cited span cannot be found in the answer: there the "
      "quote is the judge's entire evidence for contradicting the matcher, so an "
      "absent span means an unsupported claim. Verdicts in the credited strata are "
      "left alone, because the matcher has independently established that the title "
      "occurs — see section 9. Where the two columns differ, the quote-verified one "
      "is the defensible figure.\n")
    hc = s["headline_rates_conservative"]
    A("| quantity | as judged | quote-verified |")
    A("|---|---|---|")
    for k, lab in [
        ("matcher_credit_precision_correct_relation",
         "**Precision of the matcher's credit**: of credited (n11) items, share the "
         "judge calls `correct_relation`"),
        ("matcher_credit_name_only", "of credited items, share that are `name_only`"),
        ("matcher_credit_relationally_wrong",
         "of credited items, share `wrong_relation` / `reversed` / `negated`"),
        ("matcher_credit_absent_judge_disagrees",
         "of credited items, share the judge cannot find at all"),
        ("n10_compliant_omission_rate",
         "**Compliant omissions**: of omitted (n10) items, share where the answer "
         "supplied an accepted alternative"),
        ("n10_judge_finds_item_asserted_anyway",
         "of omitted items, share the judge says the answer *does* assert the edge "
         "(outright matcher false negative)"),
        ("n10_matcher_false_negative_name_present",
         "**Matcher false negatives**: of omitted items, share where the judge finds "
         "the item named at all (`correct_relation` or `name_only`)"),
        ("n10_negated",
         "of omitted items, share the answer explicitly *denies* the edge"),
        ("n10_true_omission_rate",
         "of omitted items, share that are true omissions (absent and not substituted)"),
        ("n00_absent_as_expected",
         "**n00 sanity**: of unexposed & uncredited items, share the judge calls "
         "`absent`"),
        ("n00_matcher_false_negative",
         "of unexposed & uncredited items, share the judge calls `correct_relation`"),
        ("n00_name_present_matcher_missed",
         "of unexposed & uncredited items, share where the judge finds the item "
         "referred to at all"),
    ]:
        A(f"| {lab} | {_fmt(h[k])} | {_fmt(hc[k])} |")
    A(f"\nn01 (unexposed & credited; census of "
      f"{f['pooled_by_category']['n01']} units): {h['n01_verdicts']}\n")

    A("## 4. Model-judged corrected estimate\n")
    A(f"{c['definition']}\n")
    cc = s["corrected_estimate_conservative"]
    A("| quantity | as judged | quote-verified |")
    A("|---|---|---|")
    A("| matcher credit rate (context utilisation), released sweep | "
      f"{c['matcher_credit_rate_context_utilisation']:.4f} | "
      f"{cc['matcher_credit_rate_context_utilisation']:.4f} |")
    A("| P(correct_relation given credited), judged | "
      f"{c['judged_correct_relation_given_credited']:.4f} | "
      f"{cc['judged_correct_relation_given_credited']:.4f} |")
    A("| P(correct_relation given omitted), judged | "
      f"{c['judged_correct_relation_given_omitted']:.4f} | "
      f"{cc['judged_correct_relation_given_omitted']:.4f} |")
    A("| **corrected relational-correctness rate of exposed items** | "
      f"**{c['corrected_relational_correctness_of_exposed_items']:.4f}** "
      f"[{c['corrected_ci95_propagated'][0]:.4f}, "
      f"{c['corrected_ci95_propagated'][1]:.4f}] | "
      f"**{cc['corrected_relational_correctness_of_exposed_items']:.4f}** "
      f"[{cc['corrected_ci95_propagated'][0]:.4f}, "
      f"{cc['corrected_ci95_propagated'][1]:.4f}] |")
    A("| overstatement by the lexical matcher | "
      f"{c['overstatement_pp']:.2f} pp | {cc['overstatement_pp']:.2f} pp |")
    A("")

    for title, key in (("5. Per template", "per_template"),
                       ("6. Per gold_type", "per_gold_type"),
                       ("7. Per model", "per_model")):
        A(f"## {title}\n")
        A("| stratum | n | n11 `correct_relation` | n10 acceptable alternative |")
        A("|---|---:|---|---|")
        for k, e in s[key].items():
            a = _fmt(e["n11"]["correct_relation_rate"]) if "n11" in e else "-"
            b = _fmt(e["n10"]["acceptable_alternative_rate"]) if "n10" in e else "-"
            A(f"| {k} | {e['n']} | {a} | {b} |")
        A("")

    A("## 8. Why the matcher errs: deterministic mechanism census\n")
    mc = f["mechanism_census"]
    A(f"{mc['note']}\n")
    A("| mechanism | n11 | n10 | n01 | n00 |")
    A("|---|---:|---:|---:|---:|")
    labels = {
        "gold_substring_of_subject":
            "gold title is a substring of the question's own subject name",
        "matched_by_word_bag_only":
            "credit came from the loose >=0.8 word-overlap path, not a substring hit",
        "gold_unspaced_camel":
            "gold title runs words together, so no spaced answer can match it",
        "gold_single_token": "gold title is a single word",
    }
    for m, lab in labels.items():
        A(f"| {lab} | " + " | ".join(
            str(mc["counts"][c][m]) for c in ("n11", "n10", "n01", "n00")) + " |")
    A(f"| *denominator* | {mc['denominators']['n11']:,} | "
      f"{mc['denominators']['n10']:,} | {mc['denominators']['n01']} | "
      f"{mc['denominators']['n00']} |\n")
    if mc["unspaced_camel_gold_titles"]:
        A("Gold titles that run words together (a defect in the frozen question set, "
          "not in any model — each is unmatchable for every model at once): "
          + ", ".join(f"`{t}`" for t in mc["unspaced_camel_gold_titles"]) + "\n")
    dc = f.get("deterministic_compliant_omissions")
    if dc:
        det = prop(dc["compliant_omissions"], dc["n10_total"])
        A(f"**Cross-check against the paper's existing optional-answer sensitivity "
          f"analysis.** {dc['note']} Over the whole frame that rule flags "
          f"{dc['compliant_omissions']}/{dc['n10_total']} = {det['p']:.4f} of "
          f"omissions as compliant ({dc['n10_from_any_questions']} n10 items come "
          "from `any`-type questions at all). The judge, on the stratified sample, "
          f"puts it at {_fmt(h['n10_compliant_omission_rate'])} as judged and "
          f"{_fmt(hc['n10_compliant_omission_rate'])} quote-verified — that is, the "
          "judge is *stricter* than the mechanical any-collapse rule, because it "
          "requires the substituted item to be genuinely asserted rather than merely "
          "matched. The deterministic figure should therefore be read as the upper "
          "end of the compliant-omission correction.\n")
    A("How the judge scored the sampled units carrying each mechanism:\n")
    A("| mechanism | units in sample | judge verdicts | n11 `correct_relation` |")
    A("|---|---:|---|---|")
    for m, lab in labels.items():
        e = s["mechanism_vs_verdict"][m]
        if not e.get("n_in_sample"):
            A(f"| {lab} | 0 | - | - |")
            continue
        r = e.get("n11_correct_relation_rate")
        A(f"| {lab} | {e['n_in_sample']} | "
          + ", ".join(f"{k} {v}" for k, v in e["verdicts"].items())
          + f" | {_fmt(r) if r else '-'} |")
    A("")

    A("## 9. Judge reliability\n")
    jr = s["judge_reliability"]
    A(f"{jr['definition']}\n")
    A(f"- verdicts required to cite a span: {jr['n_verdicts_with_quote_required']}")
    A(f"- span verified verbatim in the answer: {_fmt(jr['quote_verified'])}")
    A(f"- citations that do not verify: {jr['n_unverifiable']} "
      f"({jr['unverifiable_by_kind']}), by matcher category "
      f"{jr['unverifiable_by_category']}")
    A(f"- of those, **{jr['n_unsupported_demoted']} unsupported** (uncredited "
      "stratum: the judge's only evidence is a span that is not in the answer) — "
      "demoted to `absent` in the quote-verified column of sections 3 and 4")
    A(f"- and {jr['n_abridged_kept']} merely abridged (credited stratum: the matcher "
      "independently confirms the title is present) — verdict kept\n")
    A("Unsupported citations, in full:\n")
    for e in jr["unverifiable"]:
        if e["kind"] != "unsupported_citation":
            continue
        A(f"  - `{e['unit_id']}` ({e['category']}) claimed `{e['asserted']}` citing "
          f"\"{e['quote']}\", which does not occur in the answer")
    A("")

    A("## 10. Notable disagreements\n")
    A("Cases where the matcher and the model judge part company, highest judge "
      f"confidence first ({len(s['notable_disagreements'])} in total; up to 8 "
      "shown).\n")
    for e in s["notable_disagreements"][:8]:
        A(f"- **{e['unit_id']}** ({e['category']}, {e['template']}, {e['model']}) — "
          f"judge: `{e['asserted']}` (confidence {e['confidence']:.2f})  ")
        A(f"  Q: {e['question']}  ")
        A(f"  Gold edge: `{e['gold_edge']}`  ")
        A(f"  Judge quote: \"{e['quote']}\"")
    A("")

    A("## 11. Human pass\n")
    if "human_pass" in s:
        hp = s["human_pass"]
        if "error" in hp:
            A(f"Not available: {hp['error']}\n")
        else:
            A(f"- units annotated by a human: {hp['n_annotated']}")
            A(f"- human / model-judge agreement on `asserted`: "
              f"{hp['agreement_asserted']} (Cohen's kappa "
              f"{hp['cohens_kappa_asserted']})")
            A(f"- agreement on `required_or_alternative`: "
              f"{hp['agreement_required_or_alternative']} (kappa "
              f"{hp['cohens_kappa_required_or_alternative']})")
            A(f"- human precision of matcher credit: "
              f"{_fmt(hp['human_matcher_credit_precision'])}")
            A(f"- human compliant-omission rate: "
              f"{_fmt(hp['human_n10_compliant_omission_rate'])}\n")
    else:
        A("**Not performed.** `human-audit-sample.csv` (120 units, blind, blank "
          "verdict columns) and `ANNOTATION-GUIDE.md` are in this directory. After "
          "annotating, re-run `summarise --human human-audit-sample-filled.csv` to "
          "add agreement and kappa against the model judge.\n")

    A("## Reproduce\n")
    A("```\npython3 tools/paper/semantic_audit.py sample\n"
      "python3 tools/paper/semantic_audit.py judge\n"
      "python3 tools/paper/semantic_audit.py summarise\n```\n")
    A(f"Judge model `{j['judge_model']}` via `{j['backend']}`, temperature "
      f"{j['temperature']}, rubric sha256[:16] `{j['rubric_sha256_16']}`, seed "
      f"{s['seed']}. {j['n_verdicts']}/{j['n_units']} units returned a parseable "
      f"verdict ({j['n_failures']} failures, {j['n_repair_reasks']} repair re-asks). "
      f"Total cost of the audit: **${j['cost_estimate_usd_total']}** over "
      f"{j['total_judge_calls']} judge calls "
      f"({j['total_prompt_tokens']:,} prompt + {j['total_completion_tokens']:,} "
      f"completion tokens). {j['cost_basis']} Every call is cached in "
      "`judge-cache.jsonl` keyed by sha256(judge_model || rubric_hash || prompt), so "
      "re-running `judge` is free and returns identical verdicts.\n")
    A(f"Repair policy: {j['repair_policy']}\n")
    A("## Rubric given to the judge\n")
    A("```\n" + s["rubric"] + "```\n")
    return "\n".join(L)


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(
        description="Focused, stratified audit of the lexical title matcher against "
                    "a cross-family LLM judge (not a human).",
        formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("sample", help="build the frame and draw the stratified sample")
    jp = sub.add_parser("judge", help="run the model judge over the sample")
    jp.add_argument("--backend", default="auto",
                    choices=["auto", "openai", "openrouter", "anthropic"])
    jp.add_argument("--workers", type=int, default=8)
    jp.add_argument("--retries", type=int, default=5)
    sp = sub.add_parser("summarise", help="compute the tables")
    sp.add_argument("--human", default=None,
                    help="filled human-annotation CSV, for the agreement block")
    args = ap.parse_args(argv)
    return {"sample": cmd_sample, "judge": cmd_judge,
            "summarise": cmd_summarise}[args.cmd](args)


if __name__ == "__main__":
    raise SystemExit(main())
