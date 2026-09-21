---
title: Copy-ceiling harness notes — operating system-one-eval against a judge-free baseline
status: implemented-partial
last_updated: 2026-09-21
related: [ADR-2095, ADR-2094, ADR-2091, ADR-2090, ADR-2089]
---

# Copy-ceiling harness notes

Operational companion to ADR-2095, for whoever maintains `system-one-eval` and the skill
router. It carries what the loom research paper, its v7 change log, ADR-2095 and the paper's
first external review (2026-09-21) established that the harness has to act on. Anything that
is only about the paper stays in the paper.

The research write-up of the routing measurement now lives in its own note:
`loom/docs/research/companion-routing/`.

## 1. Name the quantity for the baseline it is measured against

The router's options are all shown to the judge, every turn. Under the paper's exposure
definition the exposed fraction is 1, so *gain over exposure* is accuracy minus 1 and is
negative for every engine we have measured (4B cross-encoder: 0.884 − 1 = −0.116). It is
vacuous here by construction, not a verdict on the engine.

What `copy-ceiling` actually computes on this seam is **gain over ranker** (`g_rank`):
accuracy minus the accuracy of the best judge-free ranker over the same exposed options,
with an oracle-tuned decline threshold. Report it under that name. Two consequences:

- Do not quote a `g_rank` figure as if it were the paper's exposure scalar, and do not let
  a report table label it "copy ceiling" without qualification. They are different
  instruments with different guarantees.
- The seven-point movement under the two repairs below was **baseline sensitivity in
  `g_rank`**. The exposure classification (which rubric a turn's gold names) never moved and
  could not have. Do not cite it as evidence that exposure counts are unstable.

## 2. The four measured-failure rules, with the review's corrections

From ADR-2095, each rule from a failure we actually hit. Corrections from the external review
are folded in.

1. **A threshold sweep enumerates breakpoints, never a fixed grid.** "Decline below `t`" is a
   step function whose only breakpoints are the observed scores. A 41-point even grid stepped
   over the maximising window (9.5715 < t ≤ 9.7420, narrower than one grid step) and
   understated the baseline by 2.4 points. **Correction:** tuning the threshold on the same
   corpus it is scored on is oracle tuning. It is favourable within the ranker family on that
   corpus and is *not* a deployment estimate or a general lower bound. Before `g_rank`
   justifies an engine in production, tune on a held-out split, or resample with the tuning
   repeated inside each resample, and report that number separately.
2. **Exclusion clauses are dropped before indexing.** 50 of 115 rubrics say what the skill is
   *not* for; indexed as ordinary text they are dense in the vocabulary they disclaim, so a
   turn scores highest on the rubric that exists to reject it. Cost the baseline a further
   4.6 points (79.1 % → 83.7 %). **Correction:** this is a *declared, pre-registered
   preprocessing choice*, not a neutral one. Using strictly less text does not make it
   supervision-free: a task-aware rule chosen after seeing results can adapt to the benchmark
   without adding a token. Declare the rule in the run config before scoring a new corpus,
   apply it uniformly to all rubrics, and state it in the report header. The claim that it
   "cannot introduce supervision" is withdrawn.
3. **A ranker that scores every option identically is flagged, not counted.** Where no option
   shares a content token with the state, the "pick" is candidate-map order and the baseline
   is chance. Long rubrics hide this; short option labels will not.
4. **The rig refuses a remote backend without a credential.** An unauthenticated remote run
   answers 403 on every case, and "0 of 86 answered" is indistinguishable from an outage. This
   cost hours and put a false availability conclusion into a draft paper.

Arithmetic, for anyone reconciling old drafts: the cascade is 11.6 → 9.3 → 4.7 points, a
total of **6.9** points manufactured by the two defects. Earlier text saying 8.1 is wrong.
In counts, the lexical baseline moved 66 → 68 → 72 correct of 86 while the engine stayed at
76.

## 3. "Every served path reports its ceiling" — what that means when there is no gold

A live request has no gold set, so the slogan needs an instrument named per seam. Four
distinct options, with different guarantees:

| Instrument | What it gives | Guarantee |
|---|---|---|
| Offline benchmark measurement | `g_rank` / exposure counts on a fixed labelled corpus, re-run on every engine swap | Strongest, but only about the corpus; says nothing about today's traffic |
| Executable graph-query gold | Gold derived at request time from a deterministic query over the corpus | Strong where the question is expressible as a query; narrow coverage |
| Model-proposed targets | A second model proposes the gold, then the same matcher scores it | Weakest; inherits the proposer's errors and its self-preference |
| Repeated-entity telemetry | Which retrieved entities were repeated in the output; no gold at all | Not a correctness measure — a utilisation signal only |

Recommendation:

- **Router seam:** offline benchmark, re-run on every engine swap, plus repeated-entity-style
  telemetry in production only as a drift alarm (agreement rate between the shipped ranker and
  the shipped judge). Never claim a live-request `g_rank`.
- **Keep-or-drop seam (transcripts):** executable gold where the keep criterion is expressible
  as a query over the corpus; otherwise offline benchmark. Do not use model-proposed targets
  here: the decline asymmetry is sharper and a proposer error biases the keep side.

Whichever is used, the report must say which one. Four different instruments under one slogan
is how the paper drew a review comment, and it is how a harness ends up comparing numbers that
are not comparable.

## 4. Label-audit queue

The ranker and the judge err on different turns; their union was right on 95.3 % against 88.4 %
for the judge alone. Only the movable set can change a paired comparison: the turns where the
two disagree, plus the turns both get wrong (26 of 86 on the routing corpus). A relabelling
inside the both-correct set is exactly neutral.

**Harness behaviour:** ranker/judge disagreements go to an audit queue, not silently into either
procedure's error count. Re-read the *complete* movable set against the rubrics' own text, not
only the rows that favour the ranker. On the routing corpus this found one genuine corpus error
(turn 14, `browser` → `browser-automation`, where the `browser` rubric itself says to choose
`browser-automation`), kept its mirror (turn 28) against the judge, and left a contestable row
(turn 18) standing. Correcting the one label moved `g_rank` by 2.3 points and changed no
conclusion, which is the state a single contested label should be in.

This is the finding that needs no statistical power and that a stronger baseline cannot take
away. It is worth wiring into the rig as an output artefact.

## 5. A promotion gate tests novelty and support, not positive gain

**Correction to ADR-2095's D1 lineage.** Do not gate a promotion on "recall exceeds what the
source exposes". For faithful extraction, recovering what the source already contains *is* the
objective; requiring positive gain penalises correct extraction and rewards unsupported
additions.

The gate must test two things jointly:

- **new to the destination corpus** (the fact is not already held), and
- **supported by the source** (every asserted claim traces to the source text).

A claim can be old in the source and valuable new information for the destination. That is the
normal case, and a gain-based gate rejects it.

## 6. A copy deficit is a diagnostic, not an optimisation objective

| System | Exposure | Answer recall | Gain over copy |
|---|---:|---:|---:|
| A | 0.50 | 0.50 | 0.00 |
| B | 1.00 | 0.95 | −0.05 |

A has the better gain and answers substantially less of the question. Do not rank retrieval
configurations, engines or prompts by gain alone, and do not put a gain term in any tuning
objective. Report exposure, recall and gain together; gain tells you where the work is being
done, not which system is better.

## 7. The suppression finding, scoped, and the prompt ladder to add

**Scope it exactly.** On the paper's corpus, items that were *absent from the retrieved
scaffold* were recovered at 0.121 bare and 0.004 grounded, under one authority instruction,
one retrieval policy and one matcher. "Absent from the retrieved scaffold" is not "outside the
corpus", and neither is "arbitrary out-of-domain knowledge". The "roughly thirty times worse at
anything outside the corpus" phrasing is withdrawn; anything the harness emits about
out-of-corpus routing must use the narrow statement.

**Experiment to add to the harness.** Compare the same omitted targets under four wrappers:

1. the existing authority instruction (current behaviour);
2. an explicit warning that the context may be incomplete;
3. permission to supplement the context with separately labelled knowledge;
4. a neutral context wrapper, no authority framing.

Score not only recall but *what replaced the answer*: omission, explicit contradiction, or
abstention. That turns a behavioural observation into a mechanism, and it decides whether the
right fix is a prompt change or an out-of-corpus routing rule.

One design note inherited from the paper's controls: keep message placement constant across
arms. The live path injects into a user message while the paper's controls used a system
message, which changes instruction priority as well as content.

## 8. Reproducibility items that actually bite

- `--concurrency 1`. The local 4B engine is serialised by a tokeniser lock; the default
  concurrency queues past the timeout. Use `--timeout-ms 120000`.
- Lock the tokeniser revision with the engine revision. A tokeniser change silently re-cuts the
  48-token option truncation and invalidates the late-discriminator partition.
- Request embeddings **one text per call**. The estate's endpoint returns global batch offsets
  in `data[].index` when concurrent requests merge, which misaligns every vector.
- The rig refuses a remote backend without a bearer token, deliberately. A 403-on-every-case run
  is an outage lookalike in the report.
- The cloud judge is non-deterministic across identical requests (confidence 0.95 / 0.92 / 0.93
  on repeats). Any cloud column is a single pass unless repeats are run; do not draw
  engine-to-engine comparisons from one pass.
- Record the corpus seed and authorship. The routing corpus is self-authored and single-seed;
  mean content-token overlap between a turn and its own gold rubric is 37.1 % against 2.7 % for
  a random rubric, which inflates any surface baseline.

## 9. Power: pre-register the effect, do not report observed-effect power

Exact power at n = 86 against the observed lexical effect is 0.50; about **157 turns** would be
needed for 80 % at that effect. Observed-effect power is a weak guide and should not carry
interpretive weight.

**Rule for the next corpus:** decide in advance the smallest difference that would change an
engine decision, and size the corpus to power for *that*. Record the chosen effect and the
resulting n in the run config, before the run. Note also that no subgroup contrast in the
routing decomposition survived Holm correction within its partition, and the two smallest
partitions carry four discordant pairs each, where the two-sided exact p floors at 0.125 and no
effect size can reach significance. Size subgroups, or report them as directions rather than
contrasts.

## 10. Deferred

- The rank-cheaply-escalate-on-boundary-and-late-clause cascade. Viable, but its figures were
  computed against the defective baseline and must be re-derived against the repaired one
  before it is built.
- Separating negative clauses from positive description in `SKILL.md` frontmatter. It is an
  authoring fix for every lexical retrieval over the skill corpus, including `/route` and the
  router's own shortlist, and it is not decided by ADR-2095.
