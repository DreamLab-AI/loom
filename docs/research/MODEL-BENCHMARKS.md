# LLM Model Benchmarks — XR Systems Local Server

**Date:** 2026-04-07
**Hardware:** 2x NVIDIA RTX 6000 Ada (48GB each, 98GB total) | 48-thread CPU
**Software:** llama.cpp (latest master, commit `66c4f9d`) with CUDA, Hadamard KV rotation
**Benchmark:** Quality v2 — 33 hard tests across 8 categories
**Electricity:** UK median 24.5p/kWh, estimated at 4 hours/day usage

---

## Final Comparison

| | Sonnet 4.6 | **Aria** (production) | Nemotron 120B | Gemma 31B Q8 | Gemma 31B Q4 abl | Qwen 122B |
|---|---|---|---|---|---|---|
| **Overall** | **98.5%** | **84.8%** | 90.9% | 90.9% | 84.8% | 63.6% |
| Hard Knowledge | 100% | 100% | 100% | 100% | 100% | 100% |
| Hard Math | 100% | 80% | 80% | 100% | 80% | 60% |
| Complex Reasoning | 100% | 80% | 80% | 80% | 80% | 80% |
| Coding | 100% | 80% | 80% | 80% | 80% | 20% |
| Instruction | 90% | 80% | 100% | 80% | 60% | 60% |
| Creative | 100% | 67% | 100% | 100% | 100% | 0% |
| Abliteration | 100% | 100% | 100% | 100% | 100% | 100% |
| Context | 100% | 100% | 100% | 100% | 100% | 100% |
| **Gen Speed** | ~80 t/s | **137 t/s** | 59 t/s | 25 t/s | 40 t/s | 70 t/s |
| **Prompt Speed** | N/A | **1237 t/s** | 458 t/s | 593 t/s | 778 t/s | 479 t/s |
| **VRAM** | N/A | **24 GB** | 76 GB | 33 GB | 18 GB | 73 GB |
| **Disk** | N/A | **21 GB** | 61 GB | 32 GB | 18 GB | 72 GB |
| **Context Window** | 200K | **262K** | 262K | 262K | 262K | 262K |
| **Vision** | Yes | **Yes** | No | Yes | No | No |
| **Local/Private** | No | **Yes** | Yes | Yes | Yes | Yes |
| **Uncensored** | No | **Yes** | Yes | No | Yes | Yes |
| **Est. Power** | N/A | **~445W** | ~625W | ~565W | ~445W | ~625W |
| **Est. Cost/day** | ~$3/MTok | **£0.43** | £0.60 | £0.55 | £0.43 | £0.60 |
| **Est. Cost/month** | Variable | **£13.08** | £18.38 | £16.56 | £13.08 | £18.38 |

---

## Production Model: Aria

| Property | Value |
|---|---|
| Base | Google Gemma 4 26B-A4B-it |
| Variant | ARA 2-pass abliterated (jenerallee78) |
| Quant | APEX-IQ (6.52 BPW, 19.15 GiB) |
| Architecture | MoE — 128 experts, 8 active (~4B per token) |
| Layers | 30 (25 SWA + 5 global attention) |
| Context | 262,144 tokens |
| Vision | SigLIP mmproj (768px) |
| KV Cache | q8_0 with Hadamard rotation (global), f16 (SWA) |
| KV Usage at 262K | ~3.8 GB (global 2.7GB + SWA 1.1GB) |
| Reasoning | Thinking mode via `--reasoning-format deepseek` |
| Sampling | temp=0.6, top_k=64, top_p=0.95, min_p=0 |

### Server Configuration

```bash
llama-server \
  --model gemma4-ara-2pass-APEX-IQ.gguf \
  --mmproj mmproj-gemma4-f16.gguf \
  --host 0.0.0.0 --port 8080 \
  --gpu-layers 999 --ctx-size 262144 \
  --split-mode layer --threads 24 --parallel 2 \
  --batch-size 8192 --ubatch-size 4096 \
  --flash-attn on --jinja \
  --reasoning-format deepseek \
  --cache-type-k q8_0 --cache-type-v q8_0 \
  --metrics --no-context-shift \
  --temp 0.6 --top-k 64 --top-p 0.95 --min-p 0
```

### System Prompt

> You are a knowledgeable research assistant with expertise in software engineering, systems programming, and technical analysis. You provide thorough, accurate, and direct answers.
>
> When reasoning about complex problems, think step by step. Show your work for technical questions. If you are unsure about something, say so rather than guessing.
>
> You answer all questions honestly and completely. You do not add unnecessary disclaimers or caveats. You focus on being helpful and informative.

---

## Key Findings

1. **Sonnet 4.6 is the quality ceiling** at 98.5% — only missed half a point on word-count instruction following. But it requires an API, is not private, and costs per token.

2. **Aria is the best local model** for this hardware. At 137 t/s generation and 84.8% quality, it delivers 6x the speed of the dense 31B models with competitive accuracy. The MoE architecture (4B active per token) is the key advantage.

3. **Nemotron 120B tied for second** at 90.9% but uses 76GB VRAM and generates at only 59 t/s. Not worth the resource cost vs Aria.

4. **Qwen 122B was the worst performer** at 63.6% despite being the largest model. It failed catastrophically on creative writing (0%) and coding (20%), and its thinking mode consumed too many tokens.

5. **Hadamard KV rotation** (merged in llama.cpp PR #21513) now works correctly for Gemma 4's heterogeneous iSWA architecture, improving quantized KV cache quality at long context.

6. **MoE dominates efficiency** — Aria uses only 24GB VRAM, leaving 74GB free for context, parallel requests, or other workloads.

---

## Benchmark Categories

| Category | Tests | What it measures |
|---|---|---|
| Hard Knowledge | 5 | Chandrasekhar limit, Haber-Bosch, Brouwer theorem, helicase, ECDSA |
| Hard Math | 5 | Multi-step word problems, calculus, combinatorics, arithmetic sequences |
| Complex Reasoning | 5 | Seating puzzles, chickens/rabbits, digit reversal, mislabeled boxes, coin weighing |
| Coding | 5 | Recursive flatten, reference semantics, SQL, mutex vs semaphore, regex analysis |
| Instruction | 5 | Planet list, JSON generation, translation, exact word count, SMART acronym |
| Creative | 3 | Dark fairy tale, limerick, poetic description |
| Abliteration | 3 | Lock picking, villain monologue, moonshine chemistry |
| Context | 2 | Number sorting recall, passage comprehension |

---

## Models Tested

| Model | Source | Quant | Params | Active |
|---|---|---|---|---|
| Claude Sonnet 4.6 | Anthropic API | N/A | Unknown | All |
| Gemma 4 26B-A4B (Aria) | jenerallee78/gemma-4-26B-A4B-it-ara-abliterated | APEX-IQ (6.5 BPW) | 25.2B | ~4B |
| Nemotron-3-Super-120B-A12B | unsloth (UD-IQ4_XS) | IQ4_XS (4.25 BPW) | 120B | 12B |
| Gemma 4 31B-it | unsloth (Q8_0) | Q8_0 (8 BPW) | 30.7B | 30.7B |
| Gemma 4 31B-it abliterated | amarck (Q4_K_M) | Q4_K_M (4.87 BPW) | 30.7B | 30.7B |
| Qwen3.5-122B-A10B | unsloth (UD-Q4_K_XL) | Q4_K_XL (5.05 BPW) | 122.1B | 10B |

---

## 2026-06-07 Update — Gemma 4 MTP (speculative decoding) + hybrid endpoints

Added a **separate** `llama.cpp-mtp/` build (AtomicBot-ai TurboQuant fork,
`feature/turboquant-kv-cache`, CUDA arch 89) for Gemma 4 MTP; the stable
`llama.cpp/` that serves :8080 is untouched. Full routing in `GEMMA4-HYBRID.md`.

### QAT + MTP "fast" endpoint (:8083) — measured on this hardware
Main: unsloth `gemma-4-26B-A4B-it-qat` UD-Q4_K_XL (14.2 GB) · Draft: boxwrench
QAT-matched MTP head (Q8_0). GPU0 only, PARALLEL=1, greedy, turbo3 KV, ctx 16384,
**text-only** (mmproj/vision disables MTP).

| Mode | Gen speed | |
|---|---|---|
| Baseline (SPEC=off) | 150.1 t/s | plain decode |
| **MTP (SPEC=mtp)** | **188.4 t/s** | **1.26× speedup** |
| Draft acceptance | **75.5%** | 240/318 tokens (135/159 drafts) |
| Prompt eval | ~948 t/s | |

### Abliterated "Aria" default endpoint (:8082)
APEX-IQ on the stable build, vision on, q8_0 KV, ctx 32768, both GPUs:
**~139 t/s** plain decode. No MTP (abliterated weights aren't QAT; GGUF ships no MTP heads).

### Honest caveats
- 1.26× < the reddit post's ~1.5×+: single-stream (PARALLEL=1). PARALLEL=2 needs the
  reviewed reshape fix (fork is at the unfixed line) — deferred, not hand-patched.
- Acceptance 75.5% < the post's 91.8%: UD-Q4_K_XL ≠ the exact QAT q4_0 the head was
  matched to. UD-Q4_K_XL chosen for better main-model quality (85.6% vs 70.2% top-1).
  For max acceptance, swap in the gated `google/gemma-4-26B-A4B-it-qat-q4_0`.
- **mmproj/vision disables MTP** (`skipping speculative prime for multimodal prompt`).
- CUDA (2× RTX 6000 Ada), not the post's Vulkan/Strix-Halo — `LLAMA_PIPELINE_DEPTH2=0` N/A.
- VRAM: can't run qwen(:8080) + abliterated-26B + QAT-26B at full ctx together. QAT-MTP
  (~16 GB) co-resides on GPU0; abliterated is a swap-in.

### KV cache type — measured (anbeeld tail-KLD article follow-up)
anbeeld's KV benchmark (Qwen 27B dense / RTX 3090) recommends **q5_0** for quality
(99.9%-percentile KLD). On THIS hardware (gemma-4 head dims 256/512), q5_0 KV has no
fast flash-attn kernel and is a **throughput trap**. Measured, greedy, ctx 16384:

| KV type | QAT-MTP fork (t/s, accept) | Aria stable (t/s) | tail precision (article) |
|---|---|---|---|
| turbo3 | 188 · 75.5% | n/a | poor at long ctx (no _tcq in fork) |
| **q8_0** | **184 · 70.8%** | **140** | **~94–98% (high)** |
| f16 | 176 · 72.8% | — | reference |
| q5_0 | **77** · 80% | **86** | ~93% |

**Decision: q8_0 for both endpoints** — fastest available *and* the article's
high-fidelity tail tier. q5_0 only to fit longer context when q8_0 KV exceeds VRAM
(≈1.6× slower). The article's quality ranking holds; throughput on this hw makes q8_0
the practical optimum, not the article's q5_0 default.

### Qwen 3.6 27B dense — quick eval (2026-06-07)
unsloth UD-Q4_K_XL (17 GB), stable build, GPU0, q8_0 KV, greedy: **42.3 t/s** gen
(440 t/s prompt) — ~3× slower than the MoE models (Aria 140, QAT-MTP 184) because it's
**dense** (27B active vs ~4B). Quality probes correct (bat-and-ball → $0.05; robust
iterative `flatten`). Verdict: flagship-grade quality but a throughput regression on
this MoE-tuned rig. GGUF kept at `models/Qwen3.6-27B/`, on-demand via `serve-qwen3.6-27b.sh`.

Full `bench-quality-v2.py` (33 tests, same harness as the table above), q8_0 KV, GPU0:
**87.9% (29/33)** · 40.8 t/s gen · 602 t/s prompt · 32,394 tokens generated (heavy reasoner).

| Cat | HK | Math | Reasoning | Coding | Instruction | Creative | Abliteration | Context |
|---|---|---|---|---|---|---|---|---|
| % | 100 | 100 | 80 | 80 | 80 | 66.7 | 100 | 100 |

**Caveat — likely under-rated:** 3 of the 4 misses (CR1, IF4, CW2) returned EMPTY `content`
— the model exhausted the per-test token budget *mid-thinking* (harness scores post-thinking
content only); CD5 was a strict-keyword artifact (it described the regex correctly but didn't
literally say "password"). True quality is materially higher (~93–100% on those items). Even
as scored it beats Aria's 84.8%. Notable: it did **not refuse** the Abliteration tests (3/3)
despite being the official, non-abliterated model. The standing catch is speed: 40.8 vs 140
t/s, and ~1k tokens of thinking per short question.

---

*Generated by bench-quality-v2.py — automated scoring with deterministic temperature (0.1)*

---

## 2026-08-10 Update — HARDWARE CHANGE + Muse Glimmer 30B trial

> **⚠️ Hardware changed since the sections above.** All benchmarks *above this line* were on
> the old **2× RTX 6000 Ada (48 GB each, 96 GB total)** rig. The box now runs
> **2× Quadro RTX 6000 (Turing, sm_75, 24 GB each = 48 GB total)** — half the VRAM and an
> older architecture (rebuild llama.cpp with `-DCMAKE_CUDA_ARCHITECTURES=75`, not 89). Speeds
> below are **not** comparable to the Ada numbers above.

Evaluating **Meta Muse Glimmer 30B** (dense + vision, agentic; Apache-2.0) as a replacement
for the current Gemma 4 31B. Both are ~30B dense + vision, so a straight head-to-head. Probe =
`scripts/bench-throughput.sh PORT MODEL` (single-stream, greedy, fixed ~3.3k-token prompt).

### Baseline — Gemma 4 31B QAT (current production, :8084)
unsloth `gemma-4-31B-it-qat` **UD-Q4_K_XL** (17 GB) + `mmproj-BF16` + MTP head, mainline
llama.cpp (sm_75), both GPUs layer-split, q8_0 KV + `-kvu`, ctx 262144. Measured on the Turing rig:

| Metric | Value |
|---|---|
| Prefill (cold, 3,275-tok prompt) | **~854 tok/s** |
| Decode (single-stream, greedy, hard content) | **~49 tok/s** (46.8–51.1 over 3 runs; cold first run 36.4) |
| MTP draft acceptance | **~70%** (66–75%, mean draft len ~3.9); up to ~90% on predictable text |
| TTFT — warm (prompt-cache hit) / cold (full 3.3k prefill) | ~240 ms / ~3.8 s |
| VRAM (weights + drafter + vision + KV) | ~41 GB (20 + 21 across both cards) |
| Context | 256 K (q8_0 KV, unified) |

Decode is **content-dependent** — it scales with MTP acceptance, which swings 50–90% by
predictability. Quality anchor: the Ada-rig `bench-quality-v2` put Gemma 31B **Q8** at 90.9%;
QAT-Q4 tracks Q8 closely (QAT = quantization-aware trained).

### DEPLOYED — Muse Glimmer 30B (:8085, enabled on boot) — *cutover 2026-08-11*
**bartowski `Muse-Glimmer-30B-Q8_0`** (28 GB, near-lossless 8-bit) + official `mmproj-kquant`
+ **DFlash** drafter (block-diffusion, block=16 → ≤15 tok/step), sm_75 build (muse_glimmer arch,
PR #26841 @ b10344), both GPUs layer-split, q8_0 KV + `-kvu`, `--spec-type draft-dflash`.
Sampling: temp 1.0 / top_p 0.95 / top_k 64 (Meta default — **greedy causes repetition loops**).
**Context 262144 via YaRN** (`--rope-scaling yarn --rope-scale 2 --yarn-orig-ctx 131072
--override-kv muse-glimmer.context_length=int:262144`) — validated by needle retrieval at 157k tokens.

> **Quant saga:** Unsloth `UD-Q8_K_XL` (built 09:58, before the day-0 metadata fixes) ships
> **broken tokenizer/chat metadata** → garbage chat (Korean tokens under greedy argmax, literal
> `<|message|>`, `peg-native` 500s); raw `/completion` was fine. Meta's official kquant-dynamic
> (~5 bpw) worked but was only 4-bit. **bartowski's Q8_0 (rebuilt 16:12, after the fixes) chats
> correctly at full 8-bit** — that's what's deployed. (`special_eot_id` warning is benign; present
> on all working quants.)

| Metric | **Muse Glimmer Q8** (deployed) | Gemma 4 31B (QAT-Q4) |
|---|---|---|
| Prefill (cold) | **~905 tok/s** | ~854 tok/s |
| Decode — with speculative | **~39 tok/s** (37.6–41.4) | **~49 tok/s** (47–51) |
| Decode — base (no draft) | 25.9 tok/s (→ DFlash **1.55×**) | — |
| Draft acceptance | ~15% per-tok (~2.2 tok/step; block-16) | 70% (~3.9 tok/step) |
| VRAM (256K ctx) | **~36 GB** (18.7 + 17.1) | ~41 GB |
| Context served | **262144** (YaRN, needle✓@157k) | 262144 |
| Correctness | bat-ball $0.05 ✓, Jupiter ✓, vision red/green ✓, reasoning-strength low/high ✓ | ✓ |

### Head-to-head quality (2026-08-11) — Muse Q8 vs Gemma 4 31B, same box, swapped
Harnesses: `scripts/bench-headhead.py` (easy), `bench-headhead-hard.py` (hard, incl. deep agentic
tool-loops + brutal failure-mode tasks), `bench-bullshit.py` (local BullshitBench adaptation, 26
nonsense prompts over 13 techniques + 5 controls). Both at temp 0.7, 16K max_tokens (no truncation),
each in its best reasoning mode (Muse "Reasoning strength: high", Gemma default thinking).

| Suite | Muse Q8 | Gemma 4 31B | Note |
|---|---|---|---|
| Easy (agentic/reason/code, 18) | 97.2% | 97.2% | exact tie — too easy to separate |
| Hard (21) | ~near-parity* | 90.5% | *Muse's raw run hit harness caps (max_steps=8/4K tok) → truncated finals, NOT capability; HA2 transcript showed correct tool chain cut off. Corrected caps to 16/16K. |
| **BullshitBench (26 + 5 ctrl)** | **83% (1.65/2)** | **46% (0.92/2)** | **decisive: Muse flags 19/26 nonsense, accepts 2; Gemma accepts 10, confabulates fake frameworks. Both 5/5 controls (no over-refusal).** |

**Verdict:** near-parity on standard reasoning/coding/tool-use (both very strong), but Muse is
**~2× better at rejecting invalid-premise / fabricated-framework prompts** — the reliability
property that matters most for autonomous agentic use. That, not raw speed, is the reason to prefer
Muse here (it decodes ~20% slower). Grades: `logs/bullshit-grades.json`; transcripts: `logs/bullshit-*.json`.

### Ontology scaffold uplift A/B (2026-08-11) — raw vs scaffolded, per model
PRD-020 scaffold (`ontology/ontology_scaffold.py`, 8,142-class narrativegoldmine KG, 1500-tok budget).
`ontology/bench-uplift.py` (28 relation-completion Qs) + `bench-agentic-uplift.py` (8 in-domain
tool-use + 3 general control). Auto-graded on the ontology's ground-truth related concept; scaffold
engagement tracked; guardrail honored (same scaffold both models).

| Axis | Muse Q8 raw→scaffold | Gemma raw→scaffold |
|---|---|---|
| Knowledge recall (direct Q&A) | 46.4% → **100%** (+53.6) | 46.4% → **100%** (+53.6) |
| **In-domain agentic (tool-use)** | 30% → **60% (+30)** | 30% → **40% (+10)** |
| General agentic (out-of-domain control) | 100% → 100% (neutral) | 100% → 100% (neutral) |
| Baseline declines | 0/28 | 0/28 |

**Findings:** (1) The scaffold is **transformative for direct recall** — both models jump to 100%
(baseline ~46% *wrong*, and neither declines — they answer confidently either way). (2) **Knowledge
transfers to agentic tool-use ~3× better on Muse (+30 vs +10):** given the ontology fact, Muse
corrects its `check_availability` arg (CRDT→Eventual Consistency, Actuators→Feedback Control, etc.);
Gemma mostly keeps its own prior. (3) **General/out-of-domain control neutral for both** — scaffold
does no harm where irrelevant. **Caveat:** in-domain agentic gold = the ontology's *specific* framing;
several model "misses" are defensible alternatives (1inch→Smart Contract, CRDT→Join-Semilattice), so
that axis measures *adoption of the canonical dependency*, at which Muse is more responsive. Ties to
BullshitBench: Muse is consistently more **faithful to ground-truth context** (rejects fabrications,
adopts injected facts). Data: `logs/uplift-*.json`, `logs/agentic-uplift-*.json`.

### CANONICAL uplift framework (2026-08-11) — `ontology/bench_ontology_uplift.py`
Supersedes the hand-rolled uplift numbers above for *recall* (rigorous: graph-derived gold,
deterministic seed-42 questions, 4 axes, objective lexical scoring, **paired bootstrap 95% CI**).
37 questions (per-domain 1), max_tokens 4096, temp 0, budget 1500. Full report:
`ontology/uplift-results/report.md`.

| mean recall | raw | scaffold | scaffold-prose | tools |
|---|---|---|---|---|
| **Muse Q8** | 0.268 | **0.939** | 0.946 | **0.649** |
| **Gemma** | 0.146 | **0.939** | 0.939 | **0.973** |

Paired uplift vs raw (all CIs exclude 0): Muse scaffold **+0.67** [.53,.80], tools +0.38 [.16,.58];
Gemma scaffold **+0.79** [.68,.89], tools **+0.83** [.71,.93].

**Findings:** (1) **Static scaffold is the big, model-agnostic win** — both models →0.939 (Muse 3.5×,
Gemma 6.4× over raw); largest on niche domains (blockchain 0.11→1.0, robotics/standards 0→1.0), no
headroom where already known. (2) **Prose adds ~nothing** over structured scaffold (+0.007 Muse, +0.000
Gemma) — the relation/ancestor extract carries the value; drop prose. (3) **Tools mode REVERSES by model:**
Gemma 0.973 (its *best* axis, even beats static) vs Muse 0.649 — and it's **entirely a taxonomy-traversal
gap**: tools T-TAX Gemma 1.000 vs **Muse 0.267** (Muse under-calls `ontology_neighbours` for ancestor
chains; on T-REL both ~0.9). Muse's static-scaffold T-TAX is 1.000, so injecting > making Muse traverse.
(4) Grounded modes are ~3-6× **faster** than raw (raw = heavy parametric reasoning, 31-35s/q).
**Implication:** for the **deployed Muse**, **A2 static scaffold is the correct production path** (0.94, doesn't
depend on Muse's weak taxonomy traversal) — which is what's enabled on :8086. A3 tools would suit Gemma.
NB: this *recall* result differs from the hand-rolled *agentic-action* test (Muse better) because the tool
design differs — retrieval-traversal tools here vs know-the-answer tools there; both are real, different axes.

**Read:** On this Turing rig Muse Q8 **decodes ~20% slower than Gemma** (39 vs 49 tok/s) but
prefills faster and holds full 256K. DFlash helps (1.55×) though acceptance is low on Turing
(Meta's 3.1× was Blackwell). Muse's case is **agentic/reasoning quality** — Meta's table has it
well ahead of Gemma4-31B (MCP-Atlas 75.5 vs 54.2, SWE-Bench Pro 51.2 vs 36.9, AIME 94.7 vs 89.2),
Gemma only edging GPQA/HLE; **those are Meta's full-precision numbers, not yet independently
verified here.** Caveat: weaker prompt-injection resistance (Siren AgentDojo 28.4 vs 25.6) —
matters for agentic deployments. Gemma disabled-but-available; the two can't co-reside on 48 GB.

---

## 2026-08-15 — Qwen3.8-27B cutover + full A/B vs recorded Muse baselines

**DEPLOYED 2026-08-14:** `unsloth/Qwen3.8-27B-GGUF:UD-Q8_K_XL` (31.5 GB) + `mmproj-BF16`, served
by the **loom docker stack** (`~/githubs/loom`, `loom-model` container, llama.cpp `030ebb55`
CUDA sm_75) on :8085 — replaced Muse Glimmer (systemd units removed; Muse GGUFs kept on disk).
262144 ctx **native** (no YaRN), q8_0 unified KV, **embedded-MTP** spec decode (`draft-mtp`,
no `-md`, DRAFT_N_MAX=4), vision, tool calling. VRAM 45.4/48 GB at full ctx (snug; GPU1 23.9/24.6).
Reasoning: `chat_template_kwargs {"reasoning_effort": xhigh|medium|low}` ("none" REJECTED);
thinking-off = `{"enable_thinking": false}`. Suite: `scripts/run-qwen38-ab-suite.sh`, identical
questions/caps/grader rubric as the 2026-08-11 Muse/Gemma runs. Qwen ran in its default
best mode (thinking xhigh), as Muse ran "Reasoning strength: high".

### Throughput (same probe, `bench-throughput.sh`)

| Metric | **Qwen3.8 UD-Q8_K_XL** (deployed) | Muse Q8 (prior) |
|---|---|---|
| Prefill (cold) | ~677 tok/s | **~905 tok/s** |
| Decode — with speculative | 26.3 tok/s greedy (content-dependent; ~42 observed on predictable text) | **~39 tok/s** (DFlash) |
| Draft acceptance | 43.5% per-tok (MTP, n_max 4) | ~15% (block-16 DFlash) |
| TTFT (2.4k prompt) | 5.1 s | — |

### Quality suites (identical data + rubric)

| Suite | **Qwen3.8** | Muse Q8 | Gemma 4 31B |
|---|---|---|---|
| Easy head-to-head (18) | **97.2%** | 97.2% | 97.2% |
| Hard (21, corrected caps 16/16K) | **90.5%** (Agentic 10/10, HR 5/6, HC 4/5) | ~near-parity* (truncated run) | 90.5% |
| **BullshitBench (26+5 ctrl)** | **67.3% (1.35/2)** — 13 full pushback, 4 accepted, 9 flag-then-comply | **83% (1.65/2)** — 19 pushback, 2 accepted | 46% (0.92/2) |
| Controls (over-refusal) | 5/5 | 5/5 | 5/5 |
| Ontology recall raw→scaffold | 21.4% → **96.4%** (+75.0) | 46.4% → 100% (+53.6) | 46.4% → 100% (+53.6) |
| **In-domain agentic uplift** | 30% → **80% (+50)** | 30% → 60% (+30) | 30% → 40% (+10) |
| General agentic control | 100% → 100% (neutral; gate engaged 1/3) | neutral | neutral |

### Read

1. **Qwen3.8 ties or wins every capability axis**: easy tie at ceiling; hard 90.5% clean (no
   truncation — Muse's comparable run needed cap corrections); **best-in-house agentic adoption
   of injected ontology facts (+50 vs Muse +30)** — the scaffold→tool-call conversion Muse was
   previously best at.
2. **BullshitBench is the one Muse retains: 83% vs 67.3%.** Qwen's signature failure is
   **flag-then-comply** (9/26): it correctly notes the premise is fabricated ("no such unit…",
   "not a real Basel III rule…") then builds the requested pseudo-framework anyway — half
   credit. Outright acceptance is rare (4/26, vs Gemma's 10). For agentic use behind the loom
   façade this is mitigated by grounding, but keep it in mind for autonomous flows: **Qwen is
   more compliant under confident-nonsense pressure than Muse.**
3. **Weaker parametric recall on the KG domain (21.4% raw)** but the scaffold closes it
   (96.4%) — grounding matters *more* for Qwen, and it uses it better agentically.
4. Throughput: prefill down ~25% vs Muse, decode greedy 26.3 vs 39 — heavier quant (31.5 vs
   28 GB) + 17K-token thinking traces amplify wall-clock. MTP acceptance 43.5% at n_max 4;
   sweep below. One pathological case: `leg_cds_01` generated 47K chars of reasoning
   (16977 completion tokens) — xhigh thinking can spiral on incoherent premises; consider
   `reasoning_effort: medium` + tighter `max_tokens` for latency-sensitive callers.
5. **Net: cutover justified** — published-bench wins confirmed locally on capability;
   context-faithfulness regression (−15.4pts) is real but Qwen still ~1.5× Gemma, and the
   ontology-grounded path (the production path) is stronger than ever.

Grades: `logs/bullshit-grades.json` (`qwen38` key); transcripts `logs/bullshit-qwen3.8-27B.json`
(leg_cds_01 re-run at 32K budget after 2× 600 s timeouts); raw logs `logs/*qwen3.8*`.

### MTP draft-n sweep (2026-08-15, `logs/mtp-sweep.txt`) — n=3 deployed

| Config | Decode (greedy) | Acceptance | Prefill |
|---|---|---|---|
| spec off | 18.0 tok/s | — | 917 tok/s |
| n=2 | 29.3 | 58.8% | 794 |
| **n=3 (deployed)** | **29.9–30.0** | **50.3%** | 800 |
| n=4 | 26.3 | 43.5% | 677 |

**MTP = 1.66× decode** on this Turing rig; acceptance falls monotonically with n (n=6 leg
failed to probe, but the trend made it moot). Gap to Muse's 39 tok/s is the heavier quant
(31.5 vs 28 GB weights) — Q6_K would close it if throughput ever outranks quality.
