# Annotation guide — semantic audit of the lexical title matcher

**Status: the verdicts in `verdicts.jsonl` were produced by a large language model,
not by a human.** This sheet exists so that a human pass can be added and the two
compared. Nothing in this directory should be described as human-validated until
`human-audit-sample.csv` is filled in and re-summarised.

## What you are annotating

`human-audit-sample.csv` holds 120 rows. Each row is one **unit**: one gold
knowledge-graph item, for one question, as answered by one model in the scaffold
(grounded) arm of the released ten-model sweep.

The sheet is **blind**: it deliberately does not tell you what the lexical matcher
decided about the item, nor what the model judge decided. Rows are shuffled by a hash
of the unit id so the categories are interleaved. Do not consult `sample.jsonl` or
`verdicts.jsonl` before annotating — they both carry the matcher category.

## Columns to fill

| Column | Values |
|---|---|
| `human_asserted` | one of `correct_relation`, `name_only`, `wrong_relation`, `reversed`, `negated`, `absent` |
| `human_required_or_alternative` | one of `required`, `alternative`, `n/a` |
| `human_confidence` | 0.0-1.0 |
| `human_quote` | shortest verbatim span of `model_answer` your label rests on; empty iff `absent` |
| `human_notes` | free text, optional |

## The rubric (identical to the one given to the model judge)

```
You are auditing whether a model's free-text answer actually ASSERTS a specific fact from a knowledge graph, or merely mentions a name.

You are given: a QUESTION about the DreamLab knowledge graph; the GOLD EDGE, which is the single graph fact being audited (subject, relation, target); the GOLD ITEM, which is that edge's target; the FULL GOLD SET for the question and whether the question requires ALL of them or ANY ONE of them; and the model's ANSWER.

Decide what the ANSWER asserts about the GOLD ITEM, with respect to the relation the QUESTION asks for. Judge the answer's claims only; do not reward or punish style, length, hedging or extra correct material.

Field 1 - "asserted", exactly one of:
  "correct_relation" - the answer states that the GOLD ITEM stands in the relation the question asks for, with the correct direction and no negation. Paraphrase, synonym and inflection count: "relies on X", "X is a prerequisite" and "depends on X" are the same assertion. A bare list offered as the answer to the question counts as asserting the relation for every item in it.
  "name_only" - the GOLD ITEM's name (or an unambiguous paraphrase of it) appears in the answer, but the answer does not assert that it stands in the asked-for relation: e.g. it appears in a caveat, a restatement of the question, an aside, a different relation's list, an example of something else, or a list the answer explicitly declines to endorse.
  "wrong_relation" - the answer places the GOLD ITEM in a DIFFERENT relation to the subject than the one asked for (e.g. the question asks what the subject enables and the answer says the subject depends on it, or calls it a sibling/synonym/part).
  "reversed" - the answer asserts the asked-for relation but with subject and target swapped (e.g. asked "what does A enable?", the answer says the GOLD ITEM enables A).
  "negated" - the answer explicitly denies the relation (e.g. "A does not depend on the GOLD ITEM", "the GOLD ITEM is not among them").
  "absent" - the GOLD ITEM is not referred to at all, under any name.

Choose the first label that fits in this order of precedence: negated, reversed, wrong_relation, correct_relation, name_only, absent.

Field 2 - "required_or_alternative", exactly one of:
  "required" - a complete answer to the QUESTION must include the GOLD ITEM (the question requires ALL of the gold set, or the gold set has only this member).
  "alternative" - the question accepts ANY ONE of several gold items, and the ANSWER satisfies the question by supplying a DIFFERENT member of the gold set instead. Use this only when the answer really does supply an accepted alternative.
  "n/a" - the question accepts any one of several gold items, but the answer supplies none of them (so nothing was substituted).

Field 3 - "confidence", a number between 0 and 1: how sure you are of field 1.

Field 4 - "quote": the shortest verbatim span of the ANSWER your field-1 label rests on, copied exactly. Use the empty string "" if and only if field 1 is "absent".

Respond with ONLY a JSON object and nothing else:
{"asserted": "...", "required_or_alternative": "...", "confidence": 0.0, "quote": "..."}
```

## After annotating

Save as `human-audit-sample-filled.csv` in this directory and re-run:

    python3 tools/paper/semantic_audit.py summarise --human human-audit-sample-filled.csv

`summarise` will then add a human-vs-model-judge agreement block (per-field
agreement, Cohen's kappa on `asserted`, and the human-only versions of the headline
rates) to `summary.json` and `SUMMARY.md`.
