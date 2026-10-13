# ADR-140 P1 pilot: preregistration

Written 2026-10-13, **before the first scored run**. This is a directional pilot of 40 questions. It is
not PRD-028's 120-question phase-1 pilot, it is not powered, and its intervals will be wide.

## Question

ADR-140 §5, P1: how does arm 3 (the scaffold injected into the prompt, Loom today) compare with arm 4
(the same corpus, gate and generation, driven by the model through `/mcp`) on answer quality, tokens,
turns and wall-clock? ADR-140 D8 states the prediction in advance so that it can be wrong:
**injection wins single-turn closed-corpus lookup; tools win multi-turn research over wide or
heterogeneous sources.**

## Arms (one door, one model, one generation)

Both arms go through `POST :8084/v1/chat/completions`, `temperature 0`, `max_tokens 2048`, the model
`qwen3.8-27b-heretic-q8_0`. The user prompt is byte-identical.

| Arm | `loom_options` | What the model sees |
|---|---|---|
| arm 3 | `{"verbatim": false}` | the façade-injected scaffold; the verbatim short-circuit is declined so a model actually reads it |
| arm 4 | `{"scaffold": false}` | no scaffold; the six `loom.*` tools from the live `/mcp` `tools/list`, plus a one-sentence system prompt naming them |

Arm 4's system sentence is the only text arm 3 lacks; it is needed to use tools at all. The production
default (a verbatim dump with no model call whenever the top lexical score is at least 8.0) is **not**
an arm. It served both lookup questions in a probe with zero tokens, so it is reported as a side note
only.

**Arm 4 stop rule:** at most 8 tool calls. A run that hits the cap scores whatever answer text it holds
(possibly empty) and is counted as `capped`; it is **not excluded** (intention to treat). A transport
error scores as a miss, is recorded, and is not retried.

## Questions (`questions.jsonl`, committed with this file, seed 42)

40 questions, 16 domains, ids and gold fixed here. Gold is generated mechanically from the live graph
at generation `visionGraph@45976a8c7ca4305dd38bf66c7c6128732f6de4d7`.

| Stratum | n | Source | Gold |
|---|---|---|---|
| lookup | 20 | seeded sample of `bench/questions.jsonl` (T-REL, T-TAX single-hop) | the question's recorded gold titles |
| cross | 10 | pairs of salient classes from different domains | their shared ancestors, minus ancestors common to more than 25% of the 200 salient classes; 1 to 5 titles |
| path | 10 | pairs from different domains with a `loom.paths` result of 2 or 3 hops | the intermediate classes on that path |

**Known bias, stated now.** Cross and path gold come from the same `/mcp` tools that arm 4 calls, so
arm 4 can reproduce the derivation and arm 3 cannot. A cross or path advantage for arm 4 is therefore
partly construction and must not be read as a general claim about research tasks. The lookup stratum
has no such bias. The cross and path prompts do not reveal the derivation.

## Metric and analysis

- **Primary:** recall, the fraction of gold titles that appear as whole words (case-insensitive,
  punctuation-insensitive) in the final answer. No judge model.
- **Paired difference** per question, arm 4 minus arm 3. Mean per stratum and overall, with a
  **domain-clustered** 95% bootstrap CI as primary (10,000 resamples, seed 7, whole domains resampled
  then questions within) and the naive per-question CI beside it.
- **Cost axes** (ADR-140 D8), per task: prompt tokens, completion tokens, total tokens, turns,
  wall-clock. Reported as medians per arm and stratum. Arm 3's `prompt_tokens` already includes the
  scaffold (checked on a dry run); the façade's `injected_tokens` is an estimate shown separately.
- **Execution:** serial; arm order randomised per question (seeded, so a resumed run keeps the
  schedule); `/loom/generation` checked before every call, and the run **aborts** if it changes.
- **Reported alongside:** arm 3 calls served verbatim (should be 0), arm 4 capped runs, errors.

## What would count as what

This pilot can say which way each stratum leans and how large the token and latency gap is. With 40
questions and wide intervals it cannot establish an effect; a stratum whose clustered CI spans zero is
reported as undetermined, not as a tie. A dry run of 3 questions before this was written showed arm 4
using 3 to 15 times arm 3's tokens; those 6 records are discarded and are not part of the result.
