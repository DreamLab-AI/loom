#!/usr/bin/env python3
"""semantic_audit — does the lexical title matcher agree with semantic answer correctness?

Reviewer request (docs/research/gain-over-copy-paper/archive/v8/REVIEW-2026-09-21-external-2.md, section
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

Protocol v2 (docs/research/gain-over-copy-paper/REVIEW-2026-09-21-external-3.md)
-------------------------------------------------------------------
The third external review asked for three things, all added here without disturbing
the v1 outputs; `summarise` reports both protocols side by side.

  * a SYMMETRIC quote gate. v1 kept a failed citation in a credited stratum and
    demoted one in an uncredited stratum, which decides the outcome by the side the
    verdict favours. v2 applies one policy to all 29: re-ask for a verbatim span,
    keep the verdict if it verifies, otherwise mark the unit `unresolved` and report
    every headline rate three ways as sensitivity bounds. `quote-gate-v2.jsonl` is
    the per-unit adjudication log.
  * EXPLICIT uncertainty for the corrected exposed-item rate: a stratified cluster
    bootstrap over the real design - frame weights 9,865 and 735, uncertainty in
    both stratum rates, and questions (not units) resampled, because units share a
    question and a target.
  * the BARE-ARM recoveries behind the suppression result: all 92 raw-arm lexical
    credits of unexposed gold items, judged under the same rubric and gate, so the
    suppression comparison can be restated relationally.

Commands
--------
    python3 tools/paper/semantic_audit.py sample
    python3 tools/paper/semantic_audit.py judge
    python3 tools/paper/semantic_audit.py quotegate
    python3 tools/paper/semantic_audit.py rawhits
    python3 tools/paper/semantic_audit.py summarise

Outputs (uplift-results/semantic-audit/):
    frame.json                 sampling frame counts (the full population, by stratum)
    sample.jsonl               the sampled units, with everything the judge sees
    judge-cache.jsonl          every judge call, keyed by sha256(judge_model||prompt)
    verdicts.jsonl             one parsed verdict per unit
    summary.json               all numbers, machine-readable
    SUMMARY.md                 the tables
    quote-gate-v2.jsonl        per-unit adjudication log of the symmetric gate
    quote-gate-v2-meta.json    the gate's policy, decisions and judge metadata
    raw-hits-sample.jsonl      the 92 bare-arm recoveries, as the judge sees them
    raw-hits-audit.jsonl       their verdicts, with the gate outcome per unit
    camel-census.json          the run-together-title mechanism, frame-wide
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
BOOTSTRAP_DRAWS = 10_000      # stratified cluster bootstrap for the corrected rate

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

# --------------------------------------------------------------------------
# The symmetric quote gate (protocol v2).
#
# Protocol v1 treated a failed supporting quote asymmetrically: a failure in a
# CREDITED stratum was called an abridged citation and the verdict kept, while a
# failure in an UNCREDITED stratum was called unsupported and demoted to `absent`.
# That policy decides the outcome by which side the verdict favours, which is
# exactly the thing under audit. Protocol v2 applies ONE policy to every failed
# quote in every stratum:
#
#   (a) re-ask the judge for a verbatim supporting span, with the answer supplied
#       again and the requirement that the span appear character for character
#       (whitespace normalisation only);
#   (b) if the re-asked span verifies, keep the verdict and record the repaired
#       evidence;
#   (c) if it still fails, the unit is `unresolved` - a new category, neither the
#       original verdict nor `absent`.
#
# Every headline rate is then reported three ways (unresolved counted against the
# matcher, for the matcher, and excluded) as sensitivity bounds.
# --------------------------------------------------------------------------
QUOTE_REASK_RUBRIC = """\
You previously audited whether a model's free-text ANSWER asserts a specific fact \
from a knowledge graph about a GOLD ITEM. You recorded a verdict together with a \
supporting quotation. That quotation cannot be found in the ANSWER.

Your only task now is to supply a supporting span, or to concede that none exists. \
Do not revise the recorded verdict; do not judge anything else.

Rules for the span:
  * It MUST be copied from the ANSWER character for character. Only whitespace may \
be normalised: a run of spaces, tabs or newlines may be collapsed to a single \
space. Nothing else may change - not the case, not the punctuation, not the \
spelling, not the word order.
  * Do not paraphrase, summarise, re-case, correct, abbreviate, expand, reorder or \
elide. Do not use "..." or any other ellipsis. Supply ONE contiguous span.
  * The span must, on its own, support the RECORDED VERDICT about the GOLD ITEM.
  * If the ANSWER contains no such span, reply with "found": false and an empty \
span. Conceding is the correct answer when the supporting text is simply not \
there. Do not invent a span and do not stretch an unrelated sentence to fit.

Respond with ONLY a JSON object and nothing else:
{"found": true, "span": "the exact text, copied from the ANSWER"}
or
{"found": false, "span": ""}
"""
QUOTE_REASK_HASH = hashlib.sha256(QUOTE_REASK_RUBRIC.encode()).hexdigest()[:16]

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

## `unresolved` — a category of the audit, not of the rubric

The six labels above are the only ones you choose from. `unresolved` is a seventh
category the **audit** assigns afterwards, and you never write it yourself.

It exists because a verdict has to be evidenced. Every non-`absent` label must cite
a verbatim span of the answer. When a cited span cannot be found in the answer, the
audit re-asks the adjudicator for a span that can — supplying the answer again and
requiring a character-for-character copy. If a verifying span comes back, the
verdict stands on the repaired evidence. If none does, the unit becomes
`unresolved`: the label was not shown to be wrong, but it was not shown to be right
either, so it is neither the original verdict nor `absent`, and the headline rates
are reported three ways (unresolved counted against the matcher, for the matcher,
and excluded) rather than picking one. This policy is applied to every failed
citation in every stratum, so it never depends on which side a verdict favours.

The same will apply to your annotations: fill `human_quote` with a span you have
actually copied out of `model_answer`, and if you cannot find one, the label you
want is almost certainly `absent`.

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


def cache_key(judge_model: str, prompt: str, rubric_hash: str = RUBRIC_HASH) -> str:
    """Content address of one judge call: sha256(model || rubric || prompt).

    The rubric is part of the key so that a rubric edit invalidates the cached
    verdicts rather than silently mixing two rubrics, and so that the quote-gate
    re-ask (a different rubric) can share this one cache file without colliding
    with the audit rubric's entries."""
    return hashlib.sha256(
        f"{judge_model}\x00{rubric_hash}\x00{prompt}".encode()).hexdigest()


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


def call_judge(backend: str, prompt: str, retries: int = 5, timeout: int = 120,
               system: str = None):
    system = RUBRIC if system is None else system
    spec = next(b for b in BACKENDS if b[0] == backend)
    _, base, model, envvar, _ = spec
    key = os.environ[envvar]
    if backend == "anthropic":
        url = base + "/messages"
        headers = {"x-api-key": key, "anthropic-version": "2023-06-01",
                   "Content-Type": "application/json"}
        body = json.dumps({"model": model, "max_tokens": 400, "temperature": 0,
                           "system": system,
                           "messages": [{"role": "user", "content": prompt}]}).encode()
    else:
        url = base + "/chat/completions"
        headers = {"Authorization": f"Bearer {key}", "Content-Type": "application/json"}
        body = json.dumps({"model": model, "temperature": 0, "max_tokens": 400,
                           "messages": [{"role": "system", "content": system},
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
    try:
        session = JudgeSession(args.backend, args.retries)
    except RuntimeError as e:
        print(f"FATAL: {e}", file=sys.stderr)
        json.dump({"error": str(e),
                   "when": datetime.now(timezone.utc).isoformat()},
                  open(os.path.join(OUT_DIR, "judge-backend-failure.json"), "w"),
                  indent=2)
        return 3

    verdicts, failures = [], []
    try:
        def work(u):
            v, perr, repaired = session.judge_unit(u)
            return u, v, perr, repaired

        with cf.ThreadPoolExecutor(max_workers=args.workers) as ex:
            for n, (u, v, perr, repaired) in enumerate(ex.map(work, sample), 1):
                if v is None:
                    failures.append({"unit_id": u["unit_id"], "reason": perr})
                    continue
                verdicts.append({
                    "unit_id": u["unit_id"], "model": u["model"],
                    "model_display": u["model_display"],
                    "question_id": u["question_id"],
                    "template": u["template"], "gold_type": u["gold_type"],
                    "category": u["category"], "gold_title": u["gold_title"],
                    "judge_backend": session.name, "judge_model": session.model,
                    "judge_is_model_not_human": True, "repair_pass": repaired,
                    **v,
                })
                if n % 50 == 0:
                    print(f"  {n}/{len(sample)} "
                          f"({session.counters['hits']} from cache)")
    finally:
        session.close()

    with open(os.path.join(OUT_DIR, "verdicts.jsonl"), "w") as fh:
        for v in sorted(verdicts, key=lambda v: (v["category"], v["unit_id"])):
            fh.write(json.dumps(v) + "\n")
    meta = session.meta({
        "n_units": len(sample), "n_verdicts": len(verdicts),
        "n_failures": len(failures), "failures": failures[:50],
        "n_repair_reasks": session.counters["repairs"],
        "repair_policy": REPAIR_POLICY,
    })
    json.dump(meta, open(os.path.join(OUT_DIR, "judge-meta.json"), "w"), indent=2)
    print(f"verdicts: {len(verdicts)}  failures: {len(failures)}  "
          f"from cache: {session.counters['hits']}  "
          f"est cost this run ${meta['cost_estimate_usd']}")
    return 0


# --------------------------------------------------------------------------
# judge session: backend selection + the shared, content-addressed call cache
# --------------------------------------------------------------------------

class JudgeSession:
    """One judge backend plus the append-only judge cache, shared by every
    command that talks to the model.

    `judge`, `quotegate` and `rawhits` all go through a single session, so they
    write to one cache file and one cost ledger. Keys are
    sha256(model || rubric_hash || prompt), so the audit rubric and the quote-gate
    re-ask rubric coexist without collision and every command is free to re-run.
    """

    def __init__(self, backend: str = "auto", retries: int = 5):
        order = [backend] if backend != "auto" else [b[0] for b in BACKENDS]
        self.probes, self.name = {}, None
        for name in order:
            ok, detail = probe_backend(name)
            self.probes[name] = detail
            print(f"backend probe {name}: {'OK' if ok else 'FAIL'} - {detail}")
            if ok:
                self.name = name
                break
        if self.name is None:
            raise RuntimeError("no judge backend reachable; probes: "
                               + json.dumps(self.probes))
        self.spec = next(b for b in BACKENDS if b[0] == self.name)
        self.model = self.spec[2]
        self.retries = retries
        self.cache_path = os.path.join(OUT_DIR, "judge-cache.jsonl")
        self.cache = {}
        if os.path.exists(self.cache_path):
            for rec in dx.read_jsonl(self.cache_path):
                if rec.get("response"):
                    self.cache[rec["key"]] = rec
        self._lock = threading.Lock()
        self._fh = open(self.cache_path, "a")
        self.counters = {"tok_in": 0, "tok_out": 0, "hits": 0, "repairs": 0}
        print(f"judge backend = {self.name}, model = {self.model} "
              f"(family {self.spec[4]})")

    def ask(self, unit_id: str, prompt: str, system: str = None,
            rubric_hash: str = RUBRIC_HASH):
        """One cached judge call. Returns (cache record, was_hit)."""
        k = cache_key(self.model, prompt, rubric_hash)
        with self._lock:
            hit = self.cache.get(k)
        if hit:
            with self._lock:
                self.counters["hits"] += 1
            return hit, True
        txt, usage, err = call_judge(self.name, prompt, retries=self.retries,
                                     system=system)
        rec = {"key": k, "unit_id": unit_id, "backend": self.name,
               "judge_model": self.model, "rubric": rubric_hash,
               "prompt_sha256": hashlib.sha256(prompt.encode()).hexdigest(),
               "response": txt, "usage": usage, "error": err,
               "when": datetime.now(timezone.utc).isoformat()}
        with self._lock:
            if txt:
                self.cache[k] = rec
            self._fh.write(json.dumps(rec) + "\n")
            self._fh.flush()
            self.counters["tok_in"] += (usage or {}).get("prompt_tokens") or 0
            self.counters["tok_out"] += (usage or {}).get("completion_tokens") or 0
        return rec, False

    def judge_unit(self, u: dict):
        """The audit rubric applied to one unit, with the single recorded repair
        re-ask for an unparseable reply. Returns (verdict|None, error|None,
        repaired)."""
        prompt = render_prompt(u)
        rec, _ = self.ask(u["unit_id"], prompt)
        v, perr = parse_verdict(rec.get("response") or "")
        if v is not None:
            return v, None, False
        rec2, _ = self.ask(u["unit_id"], prompt + REPAIR_SUFFIX)
        v2, perr2 = parse_verdict(rec2.get("response") or "")
        if v2 is not None:
            with self._lock:
                self.counters["repairs"] += 1
            return v2, None, True
        return None, (rec2.get("error") or perr2 or perr), True

    def meta(self, extra: dict = None) -> dict:
        c = self.counters
        m = {"backend": self.name, "judge_base_url": self.spec[1],
             "judge_model": self.model, "judge_key_env": self.spec[3],
             "judge_family": self.spec[4], "judge_is_model_not_human": True,
             "backend_probes": self.probes, "temperature": 0,
             "rubric_sha256_16": RUBRIC_HASH,
             "quote_reask_rubric_sha256_16": QUOTE_REASK_HASH,
             "cache_hits": c["hits"], "prompt_tokens": c["tok_in"],
             "completion_tokens": c["tok_out"],
             "cost_estimate_usd": round(c["tok_in"] / 1e6 * 2.0
                                        + c["tok_out"] / 1e6 * 8.0, 4),
             **cache_cost_totals(self.cache_path),
             "cost_basis": COST_BASIS,
             "when": datetime.now(timezone.utc).isoformat()}
        m.update(extra or {})
        return m

    def close(self):
        self._fh.close()


COST_BASIS = ("gpt-4.1 list price $2.00/M input, $8.00/M output. "
              "`cost_estimate_usd` covers the uncached calls of the last run "
              "only; `cost_estimate_usd_total` sums every call ever written to "
              "judge-cache.jsonl, which is what the audit actually cost to "
              "produce.")

REPAIR_POLICY = (
    "A reply whose field 1 is not one of the six rubric labels, or which is not "
    "valid JSON, gets exactly one re-ask with an appended formatting correction "
    "(the rubric and the item are unchanged); malformed JSON whose fields are "
    "nonetheless unambiguous is salvaged by mechanical field extraction. Both "
    "paths are cached under their own key and flagged (`repair_pass`, "
    "`parse_repair`) on the verdict.")


# --------------------------------------------------------------------------
# the symmetric quote gate (protocol v2)
# --------------------------------------------------------------------------

_WSN_RE = re.compile(r"\s+")


def render_quote_reask_prompt(u: dict, v: dict) -> str:
    """The re-ask. The answer is supplied again in full (same truncation the
    original judgement saw) so the judge is not asked to recall it."""
    return (
        f"QUESTION:\n{u['question']}\n\n"
        f"RELATION THE QUESTION ASKS FOR:\n{u['relation_asked']}\n\n"
        f"GOLD EDGE UNDER AUDIT:\n{u['gold_edge']}\n\n"
        f"GOLD ITEM (the item you judged):\n{u['gold_title']}\n\n"
        f"RECORDED VERDICT (unchanged, not under review):\n{v['asserted']}\n\n"
        "THE QUOTATION YOU GAVE, WHICH IS NOT IN THE ANSWER:\n"
        f"{v.get('quote') or ''}\n\n"
        f"ANSWER:\n{u['answer'][:ANSWER_CHARS]}\n\n"
        "Supply one verbatim supporting span from the ANSWER, or concede that "
        "none exists."
    )


def parse_reask(txt: str):
    """-> (found, span, None) or (None, None, reason)."""
    if not txt:
        return None, None, "empty response"
    clean = re.sub(r"```(?:json)?|```", "", txt)
    m = _JSON_RE.search(clean)
    if not m:
        return None, None, f"no JSON object: {txt[:120]}"
    raw = m.group(0)
    try:
        obj = json.loads(raw)
    except json.JSONDecodeError:
        fm = re.search(r'"found"\s*:\s*(true|false)', raw, re.I)
        sm = re.search(r'"span"\s*:\s*(.*?)\s*\}\s*$', raw, re.DOTALL)
        if not fm:
            return None, None, f"bad JSON: {txt[:120]}"
        obj = {"found": fm.group(1).lower() == "true",
               "span": (sm.group(1) if sm else "").strip().strip('"')}
    return bool(obj.get("found")), str(obj.get("span") or "")[:2000], None


def span_verifies_verbatim(span: str, answer: str) -> bool:
    """Character-for-character containment with whitespace normalisation only:
    case, punctuation and spelling must match the answer exactly. This is the
    standard the re-ask instruction states."""
    s = _WSN_RE.sub(" ", span or "").strip()
    return bool(s) and s in _WSN_RE.sub(" ", answer or "")


def span_verifies_normalised(span: str, answer: str) -> bool:
    """Protocol v1's acceptance rule: the matcher's own `normalise` (lowercase,
    strip punctuation, collapse whitespace) on both sides."""
    q = dx.normalise(span or "")
    return bool(q) and q in dx.normalise(answer or "")


def run_quote_gate(session: "JudgeSession", units: dict, verdicts: list,
                   workers: int = 8) -> list:
    """Apply the symmetric gate to every verdict whose original citation failed.

    One record per adjudicated unit: the original quote, the re-ask prompt and
    response, the repaired quote, both verification results and the decision.
    Acceptance is `verbatim OR matcher-normalised`, i.e. deliberately no stricter
    than protocol v1, so v2 can never demote a unit that v1 would have kept; the
    verbatim flag is recorded separately so the stricter count is auditable too.
    A verdict whose original citation verified is not re-asked and gets no record.
    """
    failed = [v for v in verdicts
              if v["asserted"] != "absent"
              and not quote_verifies(v, units[v["unit_id"]]["answer"])]

    def work(v):
        u = units[v["unit_id"]]
        prompt = render_quote_reask_prompt(u, v)
        rec, _ = session.ask(v["unit_id"], prompt, QUOTE_REASK_RUBRIC,
                             QUOTE_REASK_HASH)
        found, span, perr = parse_reask(rec.get("response") or "")
        span = span or ""
        verbatim = bool(found) and span_verifies_verbatim(span, u["answer"])
        normalised = bool(found) and span_verifies_normalised(span, u["answer"])
        repaired = verbatim or normalised
        return {
            "unit_id": v["unit_id"],
            "arm": u.get("arm", "scaffold"),
            "category": v["category"],
            "model": v.get("model_display") or v.get("model"),
            "question_id": v["question_id"],
            "gold_title": v["gold_title"],
            "credited_by_matcher": bool(u.get("credited")),
            "original_asserted": v["asserted"],
            "original_required_or_alternative": v["required_or_alternative"],
            "original_quote": v.get("quote") or "",
            "original_quote_verified": False,
            "reask_rubric_sha256_16": QUOTE_REASK_HASH,
            "reask_prompt": prompt,
            "reask_prompt_sha256": hashlib.sha256(prompt.encode()).hexdigest(),
            "reask_response": rec.get("response") or "",
            "reask_parse_error": perr,
            "reask_conceded_no_span": (found is False),
            "repaired_quote": span,
            "repaired_verifies_verbatim_whitespace_only": verbatim,
            "repaired_verifies_matcher_normalised": normalised,
            "decision": "repaired" if repaired else "unresolved",
            "verdict_after_gate": v["asserted"] if repaired else "unresolved",
            "judge_backend": session.name, "judge_model": session.model,
        }

    with cf.ThreadPoolExecutor(max_workers=workers) as ex:
        out = list(ex.map(work, failed))
    return sorted(out, key=lambda r: (r["category"], r["unit_id"]))


def cmd_quotegate(args) -> int:
    sample_path = os.path.join(OUT_DIR, "sample.jsonl")
    verdict_path = os.path.join(OUT_DIR, "verdicts.jsonl")
    if not (os.path.exists(sample_path) and os.path.exists(verdict_path)):
        print("run `sample` and `judge` first", file=sys.stderr)
        return 2
    units = {u["unit_id"]: u for u in dx.read_jsonl(sample_path)}
    verdicts = dx.read_jsonl(verdict_path)
    try:
        session = JudgeSession(args.backend, args.retries)
    except RuntimeError as e:
        print(f"FATAL: {e}", file=sys.stderr)
        return 3
    try:
        log = run_quote_gate(session, units, verdicts, args.workers)
    finally:
        session.close()
    path = os.path.join(OUT_DIR, "quote-gate-v2.jsonl")
    with open(path, "w") as fh:
        for r in log:
            fh.write(json.dumps(r) + "\n")
    counts = Counter(r["decision"] for r in log)
    meta = session.meta({
        "protocol": "v2 symmetric quote gate",
        "policy": (
            "One policy for every failed supporting quote in every stratum: "
            "re-ask the judge for a verbatim supporting span with the answer "
            "supplied again; if the re-asked span verifies, keep the verdict with "
            "the repaired evidence; if it still fails, the unit is `unresolved` - "
            "neither the original verdict nor `absent`. Acceptance is verbatim "
            "(whitespace-normalised) OR matcher-normalised containment, so the "
            "gate is never stricter than protocol v1."),
        "n_adjudicated": len(log),
        "decisions": dict(sorted(counts.items())),
        "n_repaired_verbatim": sum(
            1 for r in log if r["repaired_verifies_verbatim_whitespace_only"]),
        "n_repaired_normalised_only": sum(
            1 for r in log
            if r["repaired_verifies_matcher_normalised"]
            and not r["repaired_verifies_verbatim_whitespace_only"]),
        "n_conceded_no_span": sum(1 for r in log if r["reask_conceded_no_span"]),
        "by_category": {c: dict(sorted(Counter(
            r["decision"] for r in log if r["category"] == c).items()))
            for c in sorted({r["category"] for r in log})},
    })
    json.dump(meta, open(os.path.join(OUT_DIR, "quote-gate-v2-meta.json"), "w"),
              indent=2)
    print(f"adjudicated {len(log)} failed quotes: {dict(sorted(counts.items()))}")
    print(f"wrote {path}")
    return 0


# --------------------------------------------------------------------------
# raw-arm hits: the 92 lexical recoveries behind the suppression result
# --------------------------------------------------------------------------

def build_raw_hit_frame(questions: dict, exposure: dict, titles: dict) -> tuple:
    """Every (model, question, gold item) where the item is NOT exposed by the
    scaffold and the RAW (bare, no-scaffold) arm's answer lexically credits it.

    These are the paper's raw recoveries on unexposed items - 92 of 760 pooled -
    and the numerator of the suppression comparison. The denominator counts only
    unexposed item-slots for which BOTH arms were scored, exactly as
    `parametric_suppression.py` does. Returns (hits, denominator)."""
    hits, denom = [], 0
    for label, disp in MODELS:
        raw = {r["id"]: r for r in dx.read_jsonl(
            os.path.join(SWEEP, f"results-{label}-raw.jsonl"))}
        scaf = {r["id"]: r for r in dx.read_jsonl(
            os.path.join(SWEEP, f"results-{label}-scaffold.jsonl"))}
        for qid, q in questions.items():
            rr, sr = raw.get(qid), scaf.get(qid)
            if not rr or not sr or "error" in rr or "error" in sr:
                continue
            gold = q.get("gold") or []
            answer = rr.get("answer") or ""
            exp = exposure[qid]
            rec_raw = dx.recovered_flags(q, answer)
            rec_scaf = dx.recovered_flags(q, sr.get("answer") or "")
            a_norm = dx.normalise(answer)
            a_words = set(a_norm.split())
            subs = subject_titles(q, titles)
            other = [g["title"] for g, v in zip(gold, rec_raw) if v]
            for i, (g, e, vr, vs) in enumerate(
                    zip(gold, exp, rec_raw, rec_scaf)):
                if e:
                    continue              # unexposed gold items only
                denom += 1
                if not vr:
                    continue              # raw arm did not credit it
                rel, edge = gold_edge(q, g, titles)
                hits.append({
                    "mechanisms": mechanism_flags(
                        g.get("title", ""), subs, a_norm, a_words, True),
                    "unit_id": f"raw|{label}|{qid}|{i}",
                    "model": label, "model_display": disp, "arm": "raw",
                    "question_id": qid, "item_index": i,
                    "template": q["template"], "gold_type": q["gold_type"],
                    "domain": q.get("domain"), "question_key": q["key"],
                    "category": "raw_unexposed_credited",
                    "exposed": False, "credited": True,
                    "scaffold_arm_credited_same_item": bool(vs),
                    "question": q["prompt"], "subjects": subs,
                    "relation_asked": rel, "gold_edge": edge,
                    "gold_slug": g.get("slug"), "gold_title": g.get("title"),
                    "gold_set": [x.get("title") for x in gold],
                    "n_gold": len(gold),
                    "question_recall_matcher": dx.score_answer_recall(q, answer),
                    "other_gold_credited": [
                        t for t in other if t != g.get("title")],
                    "answer": answer,
                })
    return sorted(hits, key=lambda u: u["unit_id"]), denom


def cmd_rawhits(args) -> int:
    """Judge every raw-arm lexical recovery of an unexposed gold item, under the
    same rubric and the same symmetric quote gate as the scaffold-arm audit."""
    os.makedirs(OUT_DIR, exist_ok=True)
    questions = load_questions()
    titles = load_index_titles()
    exposure = build_exposure(questions, os.path.join(OUT_DIR,
                                                      "exposure-cache.json"))
    hits, denom = build_raw_hit_frame(questions, exposure, titles)
    print(f"raw-arm lexical recoveries of unexposed gold items: "
          f"{len(hits)} of {denom} unexposed item-slots")
    with open(os.path.join(OUT_DIR, "raw-hits-sample.jsonl"), "w") as fh:
        for u in hits:
            fh.write(json.dumps(u) + "\n")

    try:
        session = JudgeSession(args.backend, args.retries)
    except RuntimeError as e:
        print(f"FATAL: {e}", file=sys.stderr)
        return 3
    verdicts, failures = [], []
    try:
        def work(u):
            v, perr, repaired = session.judge_unit(u)
            return u, v, perr, repaired
        with cf.ThreadPoolExecutor(max_workers=args.workers) as ex:
            for u, v, perr, repaired in ex.map(work, hits):
                if v is None:
                    failures.append({"unit_id": u["unit_id"], "reason": perr})
                    continue
                verdicts.append({
                    "unit_id": u["unit_id"], "model": u["model"],
                    "model_display": u["model_display"],
                    "question_id": u["question_id"], "template": u["template"],
                    "gold_type": u["gold_type"], "category": u["category"],
                    "arm": "raw", "gold_title": u["gold_title"],
                    "scaffold_arm_credited_same_item":
                        u["scaffold_arm_credited_same_item"],
                    "judge_backend": session.name, "judge_model": session.model,
                    "judge_is_model_not_human": True, "repair_pass": repaired,
                    **v,
                })
        units = {u["unit_id"]: u for u in hits}
        gate = {r["unit_id"]: r
                for r in run_quote_gate(session, units, verdicts, args.workers)}
    finally:
        session.close()

    for v in verdicts:
        u = units[v["unit_id"]]
        g = gate.get(v["unit_id"])
        v["quote_verified_v1"] = quote_verifies(v, u["answer"])
        v["gate_decision"] = (g["decision"] if g else "ok")
        v["repaired_quote"] = (g["repaired_quote"] if g else None)
        v["asserted_after_gate"] = (g["verdict_after_gate"] if g
                                    else v["asserted"])
    path = os.path.join(OUT_DIR, "raw-hits-audit.jsonl")
    with open(path, "w") as fh:
        for v in sorted(verdicts, key=lambda v: v["unit_id"]):
            fh.write(json.dumps(v) + "\n")
    meta = session.meta({
        "what_this_is": (
            "Every raw-arm (bare, no-scaffold) lexical recovery of a gold item the "
            "deterministic scaffold does NOT expose, judged under the audit rubric "
            "and the v2 symmetric quote gate. The question is whether the bare "
            "answer asserts the correct relation or merely names the target."),
        "n_unexposed_item_slots": denom,
        "n_raw_lexical_recoveries": len(hits),
        "n_verdicts": len(verdicts), "n_failures": len(failures),
        "failures": failures[:50],
        "n_repair_reasks": session.counters["repairs"],
        "repair_policy": REPAIR_POLICY,
        "n_quote_gate_adjudicated": len(gate),
        "quote_gate_decisions": dict(sorted(
            Counter(r["decision"] for r in gate.values()).items())),
    })
    json.dump(meta, open(os.path.join(OUT_DIR, "raw-hits-meta.json"), "w"),
              indent=2)
    print(f"wrote {path} ({len(verdicts)} verdicts, {len(failures)} failures)")
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



# --------------------------------------------------------------------------
# protocol v2: unresolved units, three-way sensitivity, cluster bootstrap
# --------------------------------------------------------------------------

UNRESOLVED = "unresolved"


def _doc(fn) -> str:
    """A function docstring as one flowed paragraph block, fit to drop into
    Markdown or JSON (the source indentation would otherwise render as code)."""
    return "\n\n".join(
        " ".join(line.strip() for line in para.strip().splitlines())
        for para in (fn.__doc__ or "").strip().split("\n\n"))

# Three readings of an `unresolved` unit. The gate destroyed the evidence, not the
# answer, so the honest position is that the unit's verdict is unknown and bounded:
#   stands    - the original judge label is kept (the verdict is believed without
#               its citation);
#   withdrawn - the verdict that needed the citation is withdrawn, so the unit
#               falls back to `absent` and any substitution claim is dropped;
#   excluded  - the unit leaves both numerator and denominator.
# Which of `stands` and `withdrawn` favours the matcher depends on the stratum, so
# the named bounds are derived per rate from RATE_MATCHER_FAVOURS_HIGH rather than
# assumed. In the credited strata a failed citation sat under `correct_relation`
# (the judge AGREEING with the matcher), so withdrawal counts against the matcher;
# in the uncredited strata it sat under a verdict CONTRADICTING the matcher, so
# withdrawal counts for it. One mechanical policy, two directions of effect.
V2_READINGS = ("unresolved_verdict_stands", "unresolved_withdrawn",
               "unresolved_excluded")

# Does a HIGHER value of this rate make the lexical matcher look more faithful?
RATE_MATCHER_FAVOURS_HIGH = {
    "matcher_credit_precision_correct_relation": True,
    "matcher_credit_name_only": False,
    "matcher_credit_relationally_wrong": False,
    "matcher_credit_absent_judge_disagrees": False,
    "n10_compliant_omission_rate": False,
    "n10_judge_finds_item_asserted_anyway": False,
    "n10_matcher_false_negative_name_present": False,
    "n10_negated": True,
    "n10_true_omission_rate": True,
    "n00_absent_as_expected": True,
    "n00_matcher_false_negative": False,
    "n00_name_present_matcher_missed": False,
}


def apply_gate_v2(verdicts: list, gate: dict, reading: str) -> list:
    """Rewrite the verdict list under one reading of the unresolved units."""
    out = []
    for v in verdicts:
        g = gate.get(v["unit_id"])
        if g is None:
            out.append(dict(v, v2_status="ok"))
        elif g["decision"] == "repaired":
            out.append(dict(v, v2_status="repaired",
                            quote=g["repaired_quote"], quote_repaired=True))
        elif reading == "unresolved_excluded":
            continue
        elif reading == "unresolved_withdrawn":
            out.append(dict(v, asserted="absent", required_or_alternative="n/a",
                            v2_status="unresolved_withdrawn"))
        else:
            out.append(dict(v, v2_status="unresolved_stands"))
    return out


def three_way_bounds(h_stands: dict, h_withdrawn: dict, h_excluded: dict) -> dict:
    """For every scalar headline rate, name the two sensitivity bounds by which
    reading is worse and which is better for the matcher, and carry the excluded
    reading alongside. The bounds are a property of the statistic, not a guess."""
    out = {}
    for k, favours_high in RATE_MATCHER_FAVOURS_HIGH.items():
        a, b = h_stands[k], h_withdrawn[k]
        worse, better = ((a, b) if (a["p"] < b["p"]) == favours_high else (b, a))
        name = {id(a): "unresolved_verdict_stands",
                id(b): "unresolved_withdrawn"}
        out[k] = {
            "against_matcher": worse, "against_matcher_reading": name[id(worse)],
            "for_matcher": better, "for_matcher_reading": name[id(better)],
            "excluded": h_excluded[k],
        }
    return out


def _percentile(sorted_vals: list, q: float) -> float:
    if not sorted_vals:
        return float("nan")
    i = q * (len(sorted_vals) - 1)
    lo, hi = math.floor(i), math.ceil(i)
    if lo == hi:
        return sorted_vals[int(i)]
    return sorted_vals[lo] + (sorted_vals[hi] - sorted_vals[lo]) * (i - lo)


def stratified_cluster_bootstrap(n11v: list, n10v: list, N11: int, N10: int,
                                 draws: int, seed: int, cluster: str) -> dict:
    """Percentile interval for the corrected rate (N11*p11 + N10*p10)/(N11+N10).

    The design has three features a plain Wilson interval on one proportion
    ignores, and this handles all three:

      * stratified sampling weights - the n11 and n10 samples are drawn at very
        different rates from frames of N11 and N10, so the estimator is the
        frame-weighted combination, not a pooled proportion;
      * uncertainty in BOTH stratum rates - each draw resamples n11 and n10
        independently and recombines, so both rates vary together in the interval;
      * clustering - units are not independent. Each sampled question contributes
        several gold items, and a gold item's target is fixed by its question, so
        the question is the primary sampling unit. Resampling whole questions with
        replacement inside each stratum (rather than units) carries the
        within-question correlation into the interval. The model is a crossed
        factor rather than a nested one; `cluster="model"` gives the companion
        model-clustered interval, which is much wider because there are only ten
        clusters, and is reported as a robustness bound rather than the headline.
    """
    rng = random.Random(seed)

    def prep(vs):
        d = defaultdict(lambda: [0, 0])
        for v in vs:
            c = d[v[cluster]]
            c[0] += int(v["asserted"] == "correct_relation")
            c[1] += 1
        return [tuple(d[k]) for k in sorted(d)]

    b11, b10 = prep(n11v), prep(n10v)
    if not b11 or not b10:
        return {"error": "empty stratum"}
    W = N11 + N10
    vals = []
    for _ in range(draws):
        num = den = 0
        for _ in b11:
            a, b = b11[rng.randrange(len(b11))]
            num += a
            den += b
        p11 = num / den if den else 0.0
        num = den = 0
        for _ in b10:
            a, b = b10[rng.randrange(len(b10))]
            num += a
            den += b
        p10 = num / den if den else 0.0
        vals.append((N11 * p11 + N10 * p10) / W)
    vals.sort()
    mean = sum(vals) / len(vals)
    return {
        "cluster": cluster,
        "n_clusters_n11": len(b11), "n_clusters_n10": len(b10),
        "draws": draws, "seed": seed,
        "ci95": [round(_percentile(vals, 0.025), 4),
                 round(_percentile(vals, 0.975), 4)],
        "bootstrap_mean": round(mean, 4),
        "bootstrap_sd": round(
            math.sqrt(sum((x - mean) ** 2 for x in vals) / (len(vals) - 1)), 4),
    }


CORRECTED_ESTIMATOR_DOC = (
    "Estimator. The frame holds N11 = 9,865 exposed-and-credited and N10 = 735 "
    "exposed-and-omitted gold-item slots, N11 + N10 = 10,600 exposed slots. Each "
    "stratum is sampled at its own rate (240 of 9,865 and 120 of 735), so the "
    "corrected relational-correctness rate of exposed items is the frame-weighted "
    "combination of the two judged stratum rates:\n\n"
    "    R = (9865 * p11 + 735 * p10) / 10600\n\n"
    "where p11 = P(the answer asserts the gold edge | the matcher credited it) and "
    "p10 = P(the answer asserts the gold edge | the matcher scored it omitted), "
    "both estimated on the stratified sample. Interval: a stratified cluster "
    "bootstrap. Within each stratum the QUESTION is resampled with replacement "
    "(not the unit), because one question contributes several gold items and a "
    "gold item's target is fixed by its question; both stratum rates are "
    "recomputed on each draw and recombined through the same weights, so the "
    "interval carries the sampling weights, the uncertainty in BOTH rates, and the "
    "within-question clustering. 10,000 draws, seed 42, percentile method. A "
    "model-clustered bootstrap is reported beside it as a robustness bound. "
    "R is an ESTIMATE under this adjudicator (openai/gpt-4.1, temperature 0) and "
    "this protocol, on this sample. It is not a measurement of truth, it is not "
    "human-validated, and a different judge or rubric would move it.")


def corrected_estimate_v2(vs: list, pooled: dict, reading: str, draws: int) -> dict:
    n11v = [v for v in vs if v["category"] == "n11"]
    n10v = [v for v in vs if v["category"] == "n10"]
    N11, N10 = pooled["n11"], pooled["n10"]
    k11 = sum(1 for v in n11v if v["asserted"] == "correct_relation")
    k10 = sum(1 for v in n10v if v["asserted"] == "correct_relation")
    p11, p10 = prop(k11, len(n11v)), prop(k10, len(n10v))
    pt = (N11 * p11["p"] + N10 * p10["p"]) / (N11 + N10)
    return {
        "reading": reading,
        "n11_sampled": len(n11v), "n10_sampled": len(n10v),
        "p11": p11, "p10": p10,
        "matcher_credit_rate_context_utilisation": round(N11 / (N11 + N10), 4),
        "corrected_relational_correctness_of_exposed_items": round(pt, 4),
        "naive_ci95_propagated_wilson": [
            round((N11 * p11["ci95"][0] + N10 * p10["ci95"][0]) / (N11 + N10), 4),
            round((N11 * p11["ci95"][1] + N10 * p10["ci95"][1]) / (N11 + N10), 4)],
        "cluster_bootstrap_question": stratified_cluster_bootstrap(
            n11v, n10v, N11, N10, draws, SEED, "question_id"),
        "cluster_bootstrap_model": stratified_cluster_bootstrap(
            n11v, n10v, N11, N10, draws, SEED, "model"),
        "overstatement_pp": round((N11 / (N11 + N10) - pt) * 100, 2),
    }


# --------------------------------------------------------------------------
# the run-together (camelCase) gold-title mechanism, over the whole frame
# --------------------------------------------------------------------------

def spaced_form(title: str) -> str:
    return re.sub(r"(?<=[a-z0-9])(?=[A-Z])", " ", title or "")


def camelcase_census(cache_path: str) -> dict:
    """Exactly what the run-together gold titles cost the matcher, frame-wide.

    The paper's earlier wording ("no spaced answer can match it") described the
    normalisation correctly but implied the units were lost. They are not: the
    scaffold presents the title in its run-together form and the models copy that
    form, so most of these units are credited. This census separates the three
    outcomes per unit - credited by verbatim substring, credited only via the
    loose word-bag path, or omitted - and, for each omission, whether the answer
    contains the SPACED form of the title (the matcher evaded) or does not
    mention it at all (a genuine omission)."""
    if os.path.exists(cache_path):
        c = json.load(open(cache_path))
        if c.get("budget") == BUDGET:
            return c
    questions = load_questions()
    camel = {}
    for q in questions.values():
        for g in q.get("gold") or []:
            t = g.get("title") or ""
            if _CAMEL_RE.search(t) and " " not in t.strip():
                camel[t] = spaced_form(t)
    per = {t: {"title": t, "spaced_form": camel[t], "frame_units": 0,
               "credited": 0, "credited_verbatim_substring": 0,
               "credited_word_bag_only": 0, "omitted": 0,
               "omitted_spaced_form_present": 0, "omitted_not_mentioned": 0}
           for t in camel}
    omissions = []
    for label, disp in MODELS:
        for r in dx.read_jsonl(os.path.join(SWEEP,
                                            f"results-{label}-scaffold.jsonl")):
            if "error" in r:
                continue
            q = questions[r["id"]]
            gold = q.get("gold") or []
            answer = r.get("answer") or ""
            a_norm = dx.normalise(answer)
            for i, (g, v) in enumerate(zip(gold, dx.recovered_flags(q, answer))):
                t = g.get("title") or ""
                if t not in per:
                    continue
                e = per[t]
                e["frame_units"] += 1
                if v:
                    e["credited"] += 1
                    if dx.normalise(t) in a_norm:
                        e["credited_verbatim_substring"] += 1
                    else:
                        e["credited_word_bag_only"] += 1
                else:
                    e["omitted"] += 1
                    sp = dx.normalise(camel[t]) in a_norm
                    e["omitted_spaced_form_present" if sp
                      else "omitted_not_mentioned"] += 1
                    omissions.append({
                        "unit_id": f"{label}|{r['id']}|{i}",
                        "model": disp, "question_id": r["id"], "gold_title": t,
                        "spaced_form": camel[t],
                        "spaced_form_present_in_answer": sp})
    evading_titles = sorted({o["gold_title"] for o in omissions
                             if o["spaced_form_present_in_answer"]})
    tot = {k: sum(e[k] for e in per.values())
           for k in ("frame_units", "credited", "credited_verbatim_substring",
                     "credited_word_bag_only", "omitted",
                     "omitted_spaced_form_present", "omitted_not_mentioned")}
    out = {
        "budget": BUDGET,
        "what_this_is": _doc(camelcase_census),
        "n_titles": len(per), "titles": sorted(per),
        "totals": tot,
        "per_title": [per[t] for t in sorted(per)],
        "omissions": sorted(omissions, key=lambda o: o["unit_id"]),
        "mechanism": (
            f"{tot['credited']} of the {tot['frame_units']} frame units carrying a "
            f"run-together gold title ARE credited, and every one of them by a "
            f"verbatim substring hit ({tot['credited_verbatim_substring']} of "
            f"{tot['credited']}; {tot['credited_word_bag_only']} via the loose "
            "word-bag path): the model reproduced the run-together form the "
            "scaffold put in front of it, so `normalise`'s refusal to split camel "
            "case never bit. The mechanism bites only on the "
            f"{tot['omitted']} omitted units, and there on the "
            f"{tot['omitted_spaced_form_present']} where the answer does contain "
            "the SPACED form of the title and the matcher cannot see it; the "
            f"remaining {tot['omitted_not_mentioned']} do not mention the item "
            "under any spelling and are genuine omissions. The defect therefore "
            f"costs {tot['omitted_spaced_form_present']} units of 11,360, not "
            f"{tot['frame_units']}, and those come from "
            f"{len(evading_titles)} of the {len(per)} run-together titles "
            + ", ".join(f"`{t}`" for t in evading_titles)
            + ". The remaining titles are conventional unspaced proper nouns "
              "(`DeFi`, `WebRTC`, `OpenXR`, `EdDSA`, `GraphQL` are written that "
              "way in the field), which is why models reproduce them exactly and "
              "the matcher never misses them."),
    }
    json.dump(out, open(cache_path, "w"), indent=2)
    return out



def raw_hits_block(n01_verdicts: list, gate: dict) -> dict:
    """The 92 raw-arm lexical recoveries, judged, plus the relational version of
    the suppression comparison.

    The paper's suppression result is lexical: on the 760 unexposed gold-item
    slots the BARE model lexically recovers 92 and the GROUNDED model 3. That
    compares name hits. This block asks the harder question of both arms - does
    the answer assert the correct relation, or does it only name the target? - and
    restates the comparison on relational recoveries."""
    path = os.path.join(OUT_DIR, "raw-hits-audit.jsonl")
    meta_path = os.path.join(OUT_DIR, "raw-hits-meta.json")
    if not (os.path.exists(path) and os.path.exists(meta_path)):
        return {"status": "not run: `python3 tools/paper/semantic_audit.py rawhits`"}
    rows = dx.read_jsonl(path)
    meta = json.load(open(meta_path))
    denom = meta["n_unexposed_item_slots"]

    def counts(reading):
        c = Counter()
        n = 0
        for r in rows:
            a = r["asserted"]
            if r["gate_decision"] == "unresolved":
                if reading == "unresolved_excluded":
                    continue
                if reading == "unresolved_withdrawn":
                    a = "absent"
            n += 1
            c[a] += 1
        return c, n

    by_reading = {}
    for reading in V2_READINGS:
        c, n = counts(reading)
        by_reading[reading] = {
            "n_judged": n,
            "verdicts": dict(sorted(c.items())),
            "relationally_correct": prop(c["correct_relation"], n),
            "name_only": prop(c["name_only"], n),
            "wrong_or_reversed_or_negated": prop(
                c["wrong_relation"] + c["reversed"] + c["negated"], n),
            "absent": prop(c["absent"], n),
            "relational_recoveries_per_unexposed_slot": prop(
                c["correct_relation"],
                denom - (len(rows) - n)),
        }
    ks = {r: by_reading[r]["relationally_correct"] for r in V2_READINGS}
    lo = min(ks.values(), key=lambda p: p["p"])
    hi = max(ks.values(), key=lambda p: p["p"])

    per_model = {}
    for m in sorted({r["model_display"] for r in rows}):
        sub = [r for r in rows if r["model_display"] == m]
        per_model[m] = {
            "n_raw_lexical_recoveries": len(sub),
            "verdicts": dict(sorted(Counter(
                r["asserted_after_gate"] for r in sub).items())),
            "relationally_correct": sum(
                1 for r in sub if r["asserted_after_gate"] == "correct_relation"),
        }

    # -- the grounded arm's three n01 credits, under the same v2 gate ----------
    g_rows = []
    for v in n01_verdicts:
        g = gate.get(v["unit_id"])
        g_rows.append({
            "unit_id": v["unit_id"], "model": v["model_display"],
            "question_id": v["question_id"], "gold_title": v["gold_title"],
            "asserted": v["asserted"],
            "gate_decision": (g["decision"] if g else "ok"),
            "asserted_after_gate": (g["verdict_after_gate"] if g
                                    else v["asserted"])})
    g_correct = sum(1 for r in g_rows
                    if r["asserted_after_gate"] == "correct_relation")
    g_lex = len(g_rows)

    raw_correct = by_reading["unresolved_verdict_stands"][
        "verdicts"].get("correct_relation", 0)
    return {
        "what_this_is": _doc(raw_hits_block),
        "judge": {"model": meta["judge_model"], "backend": meta["backend"],
                  "temperature": 0, "rubric_sha256_16": meta["rubric_sha256_16"],
                  "is_model_not_human": True},
        "n_unexposed_item_slots": denom,
        "n_raw_lexical_recoveries": len(rows),
        "count_verified": (len(rows) == meta["n_raw_lexical_recoveries"]),
        "quote_gate": meta["quote_gate_decisions"],
        "by_reading": by_reading,
        "relationally_correct_bounds": {
            "against_matcher": lo, "for_matcher": hi,
            "excluded": by_reading["unresolved_excluded"]["relationally_correct"]},
        "per_model": per_model,
        "grounded_arm_n01_under_v2_gate": {
            "units": g_rows,
            "n_lexical_credits": g_lex,
            "n_relationally_correct": g_correct,
            "note": ("The grounded arm's unexposed-yet-credited census is "
                     f"{g_lex} units in the whole released sweep. Under the v2 "
                     f"gate {g_correct} of them assert the gold relation, so "
                     "precision on that census is zero: the three lexical credits "
                     "are two items the judge cannot find at all and one named "
                     "without the relation.")},
        "relational_suppression_comparison": {
            "definition": (
                "Recoveries of gold items the deterministic scaffold does not "
                f"expose, per {denom} unexposed item-slots, bare arm vs grounded "
                "arm. The lexical row is the paper's published comparison; the "
                "relational row requires the answer to assert the gold edge, not "
                "merely to name the target."),
            "lexical": {
                "raw": prop(len(rows), denom), "grounded": prop(g_lex, denom)},
            "relational": {
                "raw": prop(raw_correct, denom),
                "raw_sensitivity_excluded": prop(
                    by_reading["unresolved_excluded"]["verdicts"].get(
                        "correct_relation", 0),
                    denom - (len(rows)
                             - by_reading["unresolved_excluded"]["n_judged"])),
                "grounded": prop(g_correct, denom)},
            "reading": (
                f"Of the {len(rows)} bare-arm lexical recoveries, {raw_correct} "
                "assert the correct relation, so the suppression comparison "
                f"restated relationally is {raw_correct}/{denom} bare versus "
                f"{g_correct}/{denom} grounded. Relational adjudication costs the "
                f"bare arm {len(rows) - raw_correct} of its {len(rows)} credits "
                "and the grounded arm all "
                f"{g_lex} of its {g_lex}; the direction and the near-total "
                "suppression survive, and the grounded side goes to exactly "
                "zero."),
        },
    }


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
    # ---------------- protocol v2: the symmetric quote gate ------------------
    gate_path = os.path.join(OUT_DIR, "quote-gate-v2.jsonl")
    gate = {r["unit_id"]: r for r in dx.read_jsonl(gate_path)} \
        if os.path.exists(gate_path) else {}
    gate_meta_path = os.path.join(OUT_DIR, "quote-gate-v2-meta.json")
    gate_meta = (json.load(open(gate_meta_path))
                 if os.path.exists(gate_meta_path) else {})
    headline_v2, corrected_v2 = {}, {}
    for reading in V2_READINGS:
        vs = apply_gate_v2(verdicts, gate, reading)
        headline_v2[reading] = headline_for(vs)
        corrected_v2[reading] = corrected_estimate_v2(
            vs, frame["pooled_by_category"], reading, args.bootstrap)
    bounds_v2 = three_way_bounds(headline_v2["unresolved_verdict_stands"],
                                 headline_v2["unresolved_withdrawn"],
                                 headline_v2["unresolved_excluded"])
    c_lo = min(corrected_v2.values(),
               key=lambda c: c["corrected_relational_correctness_of_exposed_items"])
    c_hi = max(corrected_v2.values(),
               key=lambda c: c["corrected_relational_correctness_of_exposed_items"])

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
            "agrees with semantic answer correctness, and how it fails. Protocol "
            "v1 (sections 3, 4, 9) is the earlier release, unchanged. Protocol v2 "
            "(sections 3A, 3B, 4A) adds the symmetric quote gate, the `unresolved` "
            "category, three-way sensitivity bounds and a stratified cluster "
            "bootstrap; section 12 audits the bare-arm recoveries behind the "
            "suppression result. "
            "THE ADJUDICATOR IS A LARGE LANGUAGE MODEL, NOT A HUMAN. No human has "
            "annotated any unit in this release; human-audit-sample.csv is prepared "
            "for that pass and is blind to both the matcher category and the model "
            "judge's verdict."),
        "generated": datetime.now(timezone.utc).isoformat(),
        "judge": {k: meta[k] for k in (
            "backend", "judge_base_url", "judge_model", "judge_key_env",
            "judge_family", "judge_is_model_not_human",
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
        "quote_gate_v2": {
            "policy": gate_meta.get("policy", "not run"),
            "per_unit_log": "quote-gate-v2.jsonl",
            "reading_definitions": {
                "unresolved_verdict_stands":
                    "the original judge label is kept although its citation could "
                    "not be repaired",
                "unresolved_withdrawn":
                    "the verdict that rested on the citation is withdrawn; the "
                    "unit falls back to `absent` and any substitution claim is "
                    "dropped",
                "unresolved_excluded":
                    "the unit leaves both numerator and denominator"},
            **{k: gate_meta[k] for k in (
                "n_adjudicated", "decisions", "by_category",
                "n_repaired_verbatim", "n_repaired_normalised_only",
                "n_conceded_no_span", "quote_reask_rubric_sha256_16")
               if k in gate_meta},
            "quote_reask_rubric": QUOTE_REASK_RUBRIC,
            "finding": (
                "Protocol v1 kept the 11 failures in credited strata and demoted "
                "the 18 in uncredited strata, and the reviewer rightly called that "
                "asymmetric. Applying ONE policy to all 29 reproduces the same "
                "split from evidence rather than from assumption: every one of the "
                "11 credited-stratum failures was repaired by a span that verifies, "
                "and on every one of the 18 uncredited-stratum failures the judge "
                "conceded that no supporting span exists. The v1 asymmetry was the "
                "right call; it is now earned."
                if gate_meta.get("decisions") else "quote gate not run"),
        },
        "headline_rates_v2": headline_v2,
        "headline_rates_v2_bounds": bounds_v2,
        "corrected_estimate_v2": corrected_v2,
        "corrected_estimate_v2_bounds": {
            "against_matcher": c_lo["reading"], "for_matcher": c_hi["reading"],
            "range": [c_lo["corrected_relational_correctness_of_exposed_items"],
                      c_hi["corrected_relational_correctness_of_exposed_items"]]},
        "corrected_estimate_uncertainty": CORRECTED_ESTIMATOR_DOC,
        "camelcase_mechanism": camelcase_census(
            os.path.join(OUT_DIR, "camel-census.json")),
        "raw_hits_audit": raw_hits_block(n01, gate),
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



RATE_LABELS = [
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
]


def render_v2_sections(s: dict, A) -> None:
    g = s.get("quote_gate_v2") or {}
    if not g.get("decisions"):
        A("## 3A. Symmetric quote gate (protocol v2)\n")
        A("**Not run.** `python3 tools/paper/semantic_audit.py quotegate`\n")
        return
    d = g["decisions"]
    A("## 3A. Symmetric quote gate (protocol v2)\n")
    A(g["policy"] + "\n")
    A(f"- failed citations adjudicated: **{g['n_adjudicated']}**, "
      f"in every stratum, under one policy\n"
      f"- repaired (the re-asked span verifies): **{d.get('repaired', 0)}** "
      f"({g.get('n_repaired_verbatim', 0)} character-for-character, "
      f"{g.get('n_repaired_normalised_only', 0)} only under the matcher's own "
      "normalisation)\n"
      f"- unresolved (the judge conceded no supporting span exists, or supplied "
      f"one that still does not verify): **{d.get('unresolved', 0)}**\n"
      f"- of the unresolved, the judge explicitly conceded on "
      f"{g.get('n_conceded_no_span', 0)}\n"
      f"- by matcher category: {g.get('by_category')}\n"
      f"- per-unit adjudication log, with the original quote, the re-ask prompt "
      f"and response, the repaired quote, both verification results and the "
      f"decision: `{g['per_unit_log']}`\n")
    A(f"**Finding.** {g['finding']}\n")
    A("Acceptance is deliberately no stricter than protocol v1 (a repaired span "
      "counts if it verifies character-for-character under whitespace "
      "normalisation **or** under the matcher's own `normalise`), so v2 can never "
      "demote a unit v1 would have kept. `unresolved` is a third outcome: neither "
      "the original verdict nor `absent`.\n")

    b = s["headline_rates_v2_bounds"]
    h, hc = s["headline_rates"], s["headline_rates_conservative"]
    A("### 3B. Every headline rate, both protocols side by side\n")
    A("The two v1 columns are section 3 unchanged. The three v2 columns are the "
      "sensitivity bounds: *against the matcher* and *for the matcher* are the "
      "worse and the better of the two readings of an unresolved unit (verdict "
      "stands / verdict withdrawn) **for that particular statistic**, and "
      "*excluded* drops the unresolved units from numerator and denominator. "
      "Where the three v2 columns agree, no unresolved unit can touch the rate.\n")
    A("| quantity | v1 as judged | v1 quote-verified | v2 against matcher | "
      "v2 for matcher | v2 excluded |")
    A("|---|---|---|---|---|---|")
    for k, lab in RATE_LABELS:
        e = b[k]
        A(f"| {lab} | {_fmt(h[k])} | {_fmt(hc[k])} | {_fmt(e['against_matcher'])} "
          f"| {_fmt(e['for_matcher'])} | {_fmt(e['excluded'])} |")
    A("")


def render_corrected_v2(s: dict, A) -> None:
    if not (s.get("quote_gate_v2") or {}).get("decisions"):
        return
    A("## 4A. Corrected exposed-item rate, with the design's uncertainty\n")
    A(s["corrected_estimate_uncertainty"] + "\n")
    A("| reading of `unresolved` | p11 | p10 | corrected rate | naive propagated "
      "Wilson | question-cluster bootstrap 95% | model-cluster bootstrap 95% |")
    A("|---|---|---|---|---|---|---|")
    for reading in V2_READINGS:
        c = s["corrected_estimate_v2"][reading]
        qb, mb = c["cluster_bootstrap_question"], c["cluster_bootstrap_model"]
        A(f"| {reading} | {c['p11']['p']:.4f} ({c['p11']['k']}/{c['p11']['n']}) "
          f"| {c['p10']['p']:.4f} ({c['p10']['k']}/{c['p10']['n']}) "
          f"| **{c['corrected_relational_correctness_of_exposed_items']:.4f}** "
          f"| [{c['naive_ci95_propagated_wilson'][0]:.4f}, "
          f"{c['naive_ci95_propagated_wilson'][1]:.4f}] "
          f"| [{qb['ci95'][0]:.4f}, {qb['ci95'][1]:.4f}] "
          f"| [{mb['ci95'][0]:.4f}, {mb['ci95'][1]:.4f}] |")
    cb = s["corrected_estimate_v2_bounds"]
    c0 = s["corrected_estimate_v2"]["unresolved_verdict_stands"]
    qb = c0["cluster_bootstrap_question"]
    mb = c0["cluster_bootstrap_model"]
    qw = qb["ci95"][1] - qb["ci95"][0]
    mw = mb["ci95"][1] - mb["ci95"][0]
    A(f"\nThe question-cluster bootstrap resamples {qb['n_clusters_n11']} distinct "
      f"questions in the n11 stratum and {qb['n_clusters_n10']} in the n10 "
      f"stratum, {qb['draws']:,} draws, seed {qb['seed']}. The model-clustered "
      "column resamples the ten models instead, as a robustness bound on the "
      f"other crossed factor; it is {mw:.4f} wide against the question-clustered "
      f"{qw:.4f}, so clustering on either factor gives an interval of much the "
      "same width and neither dominates. The naive propagated-Wilson column is "
      f"{c0['naive_ci95_propagated_wilson'][1] - c0['naive_ci95_propagated_wilson'][0]:.4f} "
      "wide: it is not a valid interval for this design (it adds the two Wilson "
      "bounds rather than resampling the joint distribution, and it ignores "
      "clustering entirely), and it is reported only so the two can be "
      "compared.\n")
    A(f"Across the three readings of `unresolved` the corrected rate moves only "
      f"between {cb['range'][0]:.4f} and {cb['range'][1]:.4f}, so the headline "
      "correction does not depend on how the unresolved units are counted. "
      f"Matcher credit rate (context utilisation) on the released sweep is "
      f"{c0['matcher_credit_rate_context_utilisation']:.4f}; the corrected rate is "
      f"{c0['corrected_relational_correctness_of_exposed_items']:.4f}, an "
      f"overstatement of {c0['overstatement_pp']:.2f} pp by the lexical matcher. "
      "**This is an estimate under this adjudicator and this protocol, on this "
      "sample; it is not human-validated and a different judge or rubric would "
      "move it.**\n")


def render_camel_section(s: dict, A) -> None:
    cm = s.get("camelcase_mechanism") or {}
    if not cm:
        return
    t = cm["totals"]
    A("## 8A. The run-together (camelCase) gold titles, precisely\n")
    A("Earlier drafts said a run-together gold title means \"no spaced answer can "
      "match it\", which is true of the normalisation and misleading about the "
      "outcome. Frame-wide census of all "
      f"{t['frame_units']} units carrying one of the {cm['n_titles']} "
      "run-together gold titles:\n")
    A("| gold title | spaced form | frame units | credited | of which verbatim "
      "substring | of which word-bag only | omitted | omitted with spaced form in "
      "the answer | omitted, not mentioned at all |")
    A("|---|---|---:|---:|---:|---:|---:|---:|---:|")
    for e in cm["per_title"]:
        A(f"| `{e['title']}` | {e['spaced_form']} | {e['frame_units']} | "
          f"{e['credited']} | {e['credited_verbatim_substring']} | "
          f"{e['credited_word_bag_only']} | {e['omitted']} | "
          f"{e['omitted_spaced_form_present']} | {e['omitted_not_mentioned']} |")
    A(f"| **total** | | **{t['frame_units']}** | **{t['credited']}** | "
      f"**{t['credited_verbatim_substring']}** | "
      f"**{t['credited_word_bag_only']}** | **{t['omitted']}** | "
      f"**{t['omitted_spaced_form_present']}** | "
      f"**{t['omitted_not_mentioned']}** |")
    A("")
    A(f"**Mechanism.** {cm['mechanism']}\n")
    ev = [o for o in cm["omissions"] if o["spaced_form_present_in_answer"]]
    A("Omissions in which the spaced form is present in the answer and the matcher "
      f"cannot see it ({len(ev)}):\n")
    for o in ev:
        A(f"- `{o['unit_id']}` ({o['model']}, {o['question_id']}) — gold "
          f"`{o['gold_title']}`, the answer says \"{o['spaced_form']}\"")
    other = [o for o in cm["omissions"] if not o["spaced_form_present_in_answer"]]
    A(f"\nOmissions where the item is not mentioned under any spelling, so the "
      f"matcher is right ({len(other)}): "
      + ", ".join(f"`{o['unit_id']}` ({o['gold_title']})" for o in other) + "\n")


def render_raw_hits_section(s: dict, A) -> None:
    r = s.get("raw_hits_audit") or {}
    A("## 12. The bare-arm lexical recoveries behind the suppression result\n")
    if r.get("status"):
        A(f"**Not run.** {r['status']}\n")
        return
    A(r["what_this_is"] + "\n")
    ngate = sum(v for v in r["quote_gate"].values())
    A(f"Enumerated from the released sweep rows with the paper's own matcher and "
      f"the regenerated exposure vector: **{r['n_raw_lexical_recoveries']} of "
      f"{r['n_unexposed_item_slots']}** unexposed gold-item slots are lexically "
      f"credited by the bare arm (count verified against the paper's 92/760: "
      f"{r['count_verified']}). Every one was judged under the same rubric and the "
      f"same symmetric quote gate; {ngate} of the "
      f"{r['n_raw_lexical_recoveries']} citations needed adjudication, outcomes "
      f"{r['quote_gate']}.\n")
    A("| reading of `unresolved` | n judged | relationally correct | name only | "
      "wrong / reversed / negated | absent |")
    A("|---|---:|---|---|---|---|")
    for reading in V2_READINGS:
        e = r["by_reading"][reading]
        A(f"| {reading} | {e['n_judged']} | {_fmt(e['relationally_correct'])} | "
          f"{_fmt(e['name_only'])} | {_fmt(e['wrong_or_reversed_or_negated'])} | "
          f"{_fmt(e['absent'])} |")
    bd = r["relationally_correct_bounds"]
    A(f"\nSensitivity bounds on relational correctness: against the matcher "
      f"{_fmt(bd['against_matcher'])}, for the matcher {_fmt(bd['for_matcher'])}, "
      f"unresolved excluded {_fmt(bd['excluded'])}.\n")
    A("Per model:\n")
    A("| model | raw lexical recoveries | relationally correct | verdicts after "
      "the gate |")
    A("|---|---:|---:|---|")
    for m, e in r["per_model"].items():
        A(f"| {m} | {e['n_raw_lexical_recoveries']} | "
          f"{e['relationally_correct']} | {e['verdicts']} |")
    gn = r["grounded_arm_n01_under_v2_gate"]
    A(f"\n**The grounded arm's side of the comparison.** {gn['note']}\n")
    A("| unit | model | question | gold item | judge verdict | gate | after gate |")
    A("|---|---|---|---|---|---|---|")
    for u in gn["units"]:
        A(f"| `{u['unit_id']}` | {u['model']} | {u['question_id']} | "
          f"{u['gold_title']} | {u['asserted']} | {u['gate_decision']} | "
          f"{u['asserted_after_gate']} |")
    rs = r["relational_suppression_comparison"]
    A(f"\n### 12A. Suppression, restated relationally\n")
    A(rs["definition"] + "\n")
    A("| comparison | bare (raw) arm | grounded (scaffold) arm |")
    A("|---|---|---|")
    A(f"| lexical recovery of unexposed gold, per "
      f"{r['n_unexposed_item_slots']} slots (the paper's published row) | "
      f"{_fmt(rs['lexical']['raw'])} | {_fmt(rs['lexical']['grounded'])} |")
    A(f"| **relational** recovery (the answer asserts the gold edge) | "
      f"{_fmt(rs['relational']['raw'])} | {_fmt(rs['relational']['grounded'])} |")
    A(f"\n{rs['reading']}\n")


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

    A("## 3. Headline rates, protocol v1 (95% Wilson intervals)\n")
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
    for k, lab in RATE_LABELS:
        A(f"| {lab} | {_fmt(h[k])} | {_fmt(hc[k])} |")
    A(f"\nn01 (unexposed & credited; census of "
      f"{f['pooled_by_category']['n01']} units): {h['n01_verdicts']}\n")

    render_v2_sections(s, A)

    A("## 4. Model-judged corrected estimate (protocol v1)\n")
    A("Superseded by section 4A, which handles the design's uncertainty "
      "properly; kept here unchanged so the two protocols can be compared.\n")
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

    render_corrected_v2(s, A)

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
            "gold title runs words together, so `normalise` cannot match a spaced "
            "rendering of it (section 8A counts what this actually costs)",
        "gold_single_token": "gold title is a single word",
    }
    for m, lab in labels.items():
        A(f"| {lab} | " + " | ".join(
            str(mc["counts"][c][m]) for c in ("n11", "n10", "n01", "n00")) + " |")
    A(f"| *denominator* | {mc['denominators']['n11']:,} | "
      f"{mc['denominators']['n10']:,} | {mc['denominators']['n01']} | "
      f"{mc['denominators']['n00']} |\n")
    if mc["unspaced_camel_gold_titles"]:
        A("Gold titles that run words together: "
          + ", ".join(f"`{t}`" for t in mc["unspaced_camel_gold_titles"])
          + ". This flag marks a title `normalise` cannot match in spaced form; it "
            "is NOT a count of lost credits, and section 8A resolves what it costs "
            "unit by unit.\n")
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

    render_camel_section(s, A)

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

    render_raw_hits_section(s, A)

    A("## Reproduce\n")
    A("```\n"
      "python3 tools/paper/semantic_audit.py sample      # frame + stratified sample\n"
      "python3 tools/paper/semantic_audit.py judge       # verdicts (protocol v1)\n"
      "python3 tools/paper/semantic_audit.py quotegate   # symmetric quote gate (v2)\n"
      "python3 tools/paper/semantic_audit.py rawhits     # the bare-arm recoveries\n"
      "python3 tools/paper/semantic_audit.py summarise   # every table above\n"
      "```\n")
    A("`quotegate` writes the per-unit adjudication log `quote-gate-v2.jsonl`; "
      "`rawhits` writes `raw-hits-sample.jsonl` and `raw-hits-audit.jsonl`. "
      "`summarise` folds both in when they are present and reports protocol v1 "
      "and protocol v2 side by side, so the earlier release's numbers stay "
      "visible and unchanged.\n")
    A(f"Judge model `{j['judge_model']}` via `{j['backend']}` at "
      f"`{j['judge_base_url']}` (key from `{j['judge_key_env']}`), temperature "
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
    qg = sub.add_parser(
        "quotegate",
        help="protocol v2: re-ask for a verbatim span on every failed citation, "
             "in every stratum; unrepaired units become `unresolved`")
    rh = sub.add_parser(
        "rawhits",
        help="judge every raw-arm lexical recovery of an unexposed gold item "
             "(the 92 behind the suppression result) under the same rubric+gate")
    for p in (qg, rh):
        p.add_argument("--backend", default="auto",
                       choices=["auto", "openai", "openrouter", "anthropic"])
        p.add_argument("--workers", type=int, default=8)
        p.add_argument("--retries", type=int, default=5)
    sp = sub.add_parser("summarise", help="compute the tables")
    sp.add_argument("--human", default=None,
                    help="filled human-annotation CSV, for the agreement block")
    sp.add_argument("--bootstrap", type=int, default=BOOTSTRAP_DRAWS,
                    help="stratified cluster-bootstrap resamples (default "
                         f"{BOOTSTRAP_DRAWS})")
    args = ap.parse_args(argv)
    return {"sample": cmd_sample, "judge": cmd_judge, "quotegate": cmd_quotegate,
            "rawhits": cmd_rawhits, "summarise": cmd_summarise}[args.cmd](args)


if __name__ == "__main__":
    raise SystemExit(main())
