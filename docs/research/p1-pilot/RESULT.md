# ADR-140 P1 pilot: result

Run 2026-10-13 on generation `visionGraph@45976a8c7ca4305dd38bf66c7c6128732f6de4d7` (unchanged for all 80
calls), following `bench/p1-pilot/PREREGISTRATION.md` (committed as `148b017` before the run). 40
questions, 2 arms, serial. Raw records: `runs.jsonl`; scored table: `score.md`
(`loom-bench score --questions bench/p1-pilot/questions.jsonl --runs docs/research/p1-pilot/runs.jsonl`).

| stratum | n | arm 3 recall | arm 4 recall | delta | clustered 95% CI | tokens a3 / a4 (median) | wall s a3 / a4 (median) |
|---|---|---|---|---|---|---|---|
| lookup | 20 | 0.93 | 0.94 | +0.01 | [-0.14, +0.12] | 1,130 / 4,952 | 17 / 51 |
| cross | 10 | 1.00 | 1.00 | 0.00 | [0.00, 0.00] | 1,254 / 8,958 | 27 / 89 |
| path | 10 | 0.50 | 0.90 | +0.40 | [+0.06, +0.67] | 3,110 / 7,797 | 133 / 84 |
| all | 40 | 0.84 | 0.94 | +0.11 | [-0.03, +0.24] | 1,242 / 6,986 | 28 / 68 |

Arm 4 used a median of 3 turns (4 on cross and path). One arm 4 run hit the 8-call cap (q018, path) and
scored as it stood. No errors; no arm 3 call was served verbatim.

## What it supports

- **Cost.** Arm 4 used about 4 to 7 times arm 3's tokens on lookup and cross, and roughly twice on path.
  Each tool turn resends a growing context. This is the clearest result and the opposite of a token
  saving. It holds with wide margins; no interval is needed to see it.
- **Lookup.** A tie on quality (0.93 against 0.94, CI spanning zero) at about 4 times the tokens and
  3 times the wall-clock. Injection did not win on quality, as the ADR predicted, but it is much cheaper.
- **Path.** Arm 4 leads by 0.40 and the clustered CI excludes zero.

## What it does not support

- **Cross is a ceiling.** Both arms scored 1.00, so the stratum separates nothing. Its gold (mostly one
  shared ancestor) was too easy; this is a defect of the question set.
- **The path lead is partly construction.** Gold is derived from `loom.paths`, which arm 4 calls (the
  bias the preregistration stated). It is also partly truncation: 6 of arm 3's 10 path answers ran into
  the 2,048-token limit while reasoning about the chain without being able to look it up, which depresses
  its recall and inflates its 133 s median wall-clock. Read the path delta as "a model with a path tool
  beats one without", not as a measure of research tasks in general.
- **Overall.** The pooled delta (+0.11) has a clustered CI that includes zero. 13 of 40 questions
  differed between arms: 9 favoured arm 4, 2 favoured arm 3 (q017 lookup, q018 path), 2 lookups were
  mixed at partial scores. With n=40 this is directional only.

## Against the ADR's prediction

"Injection wins single-turn closed-corpus lookup; tools win multi-turn research over wide or
heterogeneous sources." Lookup: not a quality win for injection, a large cost win. Multi-turn research:
the only stratum that separated favoured tools, with the caveats above. The routing rule the ADR wants
would be "inject by default; offer tools for chain or path questions", but this pilot does not establish
it.

## Before anyone relies on this

A real P1 needs a cross stratum with enough gold to separate arms, gold that does not come from the tool
under test (for example, human-checked), a larger `max_tokens` or a stop rule for arm 3's path
rambling, and more than 40 questions. PRD-028's powered design is the route to that.
