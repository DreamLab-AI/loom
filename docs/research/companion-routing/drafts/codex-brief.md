# Adversarial methodology and statistics review — arXiv preprint section + raw data

You are reviewing a new section of a preprint and the per-item data behind it. Read the
files in this directory yourself. **Do not write, move or delete any file.** Do not call
MCP tools (approval policy is `never` in this mode and they will fail). Everything you
need is here.

## Read in this order

1. `method-copy-ceiling.tex` — the paper's existing instrument, defined in an earlier
   section: the **copy ceiling** (the recall a verbatim copy of the shown context would
   achieve) and the **signed gain over copy**. This is the method the new section applies
   to a second domain. It is the context for everything else.
2. `abstract.tex` — the paper's abstract, so you can see what the section is claiming
   into.
3. `section-routing.tex` — **the section under review.** New material.
4. `routing-cases.json` — the labelled corpus: 86 turns, each `{prompt, expected_skill,
   rationale, class, provenance, late_discriminative}`. Note `_provenance` at the top
   level, which records a re-adjudication.
5. `skill-rubrics.json` — the 115 candidate rubrics shown to the judge as choice options.
   The gold label is one of these keys. This is the *exposure*.
6. `openjev-run-v2.json` — the judge's per-turn results on the corrected corpus
   (`cases[]`: index, expected, choice, correct, soft_correct, confidence, ms,
   input_tokens, class).
7. `copy-ceiling-v2.txt` — the control's output: naive and fair ceilings for two
   judge-free rankers, signed per-item gain, power line, subgroup breakdown.
8. `sweep-v2.txt` — the judge's decline-threshold sweep, one full corpus pass per row.
9. `mined-rows.json` — per-turn joined records from an earlier analysis pass (pre
   re-adjudication, so its `expected` for index 14 is the OLD label; treat it as the
   source for BM25 scores/margins per turn, not for labels).

## What the section claims

A discriminative choice task where the gold label is one of 115 rubrics *shown to the
judge*, so exposure is total. A 4B NLI cross-encoder judge scores each (turn, rubric)
pair independently and answers `none` below a threshold. The control is a judge-free
surface match over the same exposed options (BM25, and cosine under a 384-d embedding),
each given a decline rule whose threshold is tuned on the evaluated corpus so that the
ceiling is flattered and the judge's gain is a floor. Headline: judge 88.4% top-1, fair
lexical ceiling 76.7%, signed per-item gain +0.116, 95% CI [+0.012, +0.221].

## Your task

Find the **strongest defensible finding this evidence supports**, and say where the
current section is wrong, weak, overclaiming, or *underclaiming*. I am explicitly not
asking you to make the number bigger. I am asking what the correct analysis is and what
the data actually licenses. If the honest answer is that the headline should be weaker,
say that. If the honest answer is that the paper is burying a stronger claim, say that
too — I suspect it may be.

Address at least these, and anything else you find:

1. **Statistics.** The signed per-item gain uses a normal-approximation CI on a paired
   *binary* difference (judge correct minus ranker correct, per turn, values in
   {-1,0,+1}), n=86, with 16 discordant turns favouring the judge and 6 favouring the
   ranker. Is that the right instrument? Compute what you consider correct (McNemar,
   exact binomial on discordant pairs, bootstrap — your call, state why) and report the
   numbers. The paper's earlier sections use domain-clustered block bootstrap and Holm
   correction, so the house standard is high. Also: the subgroup table reports seven
   populations with no multiplicity correction — what should be done about that, and does
   it change which subgroup claims survive?

2. **The adjudication.** One label was corrected (turn 14, `browser` →
   `browser-automation`) after the disagreement audit, and that single turn is what moves
   the interval clear of zero. The section discloses this. Is the disclosure adequate?
   Is the adjudication defensible on its stated criterion (the `browser` rubric's own
   text says browser *tasks* go to `browser-automation`)? Read the other six disputed
   turns in the data and say whether any of *them* should also have been corrected — if
   the audit was asymmetric in the judge's favour, that is the most damaging thing you
   could find, and I want it found.

3. **The oracle-tuned ceiling.** The copy baseline's decline threshold is tuned to
   maximise its own accuracy on the same corpus it is scored on, which the section argues
   makes the judge's gain a floor. Is that argument sound? Is there a *stronger* ceiling
   the section should have used and did not — a better judge-free procedure over the same
   exposure that would shrink the gain? Propose the strongest one you can and estimate
   what it would score.

4. **What the finding actually is.** The section leads with the gain number and treats
   the subgroup decomposition, the label-audit use of disagreement, and the
   ranker/judge complementarity as secondary. Is that the right ordering? Which of these
   is the most novel and most defensible contribution given n=86 and a self-authored
   corpus? Say plainly which claim you would lead with.

5. **Threats a hostile reviewer would raise.** The corpus was written by the same authors
   as the rubrics it is scored against. Turn distribution differs from production (16/86
   `none` here versus roughly 65% in the live system). Single seed. One engine. Name the
   critique that would be hardest to answer and say whether it is answerable with the
   data present or needs new measurement.

6. **Re-tuning that would be legitimate versus illegitimate.** Be explicit about which
   proposed changes are principled improvements to the analysis and which would be
   searching the garden of forking paths. I want that line drawn for me, not blurred.

## Output

Write your final message as a structured review with a short verdict at the top ("the
strongest defensible claim is X"), then the numbered sections above, then:

- **Numbers you computed**, with the command or formula, so I can reproduce them.
- **Specific edits**, quoting the sentence in `section-routing.tex` you would change and
  giving the replacement.
- **Context I still need** — anything you could not find in these files that would change
  your assessment.

Be direct. Terse is fine. Where you are uncertain, say so and say what would settle it.
