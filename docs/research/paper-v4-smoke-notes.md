# v4 Rewrite Harness — Smoke Notes & Mode Recommendation

Driver: `tools/paper/rewrite_v4.py`. Endpoint `http://127.0.0.1:18085/v1/chat/completions`,
model `qwen3.8-27b-heretic-q8_0` (~19.5 tok/s). The full run is **deferred to overnight**
(cancelled for now); these notes drive that run's mode choice.

## Two-mode smoke (first 3 rewriteable paragraphs, fresh caches, `--limit 3`)

| mode          | acceptance | retries | latency each (s)     | latency mean | diversity each        | diversity mean | div warnings |
|---------------|-----------:|--------:|----------------------|-------------:|-----------------------|---------------:|-------------:|
| thinking      | 3/3        | 0       | 108.0 / 69.0 / 127.5 | 101.5        | 0.593 / 0.779 / 0.730 | 0.701          | 0            |
| **--no-think**| **3/3**    | 0       | 23.1 / 13.3 / 18.8   | **18.4**     | 0.626 / 0.468 / 0.923 | **0.672**      | 1 (idx2)     |

difflib ratio: lower = more re-voiced. Both modes returned clean content (reasoning, when
present, arrives in `reasoning_content` and is ignored). Validation identical: all invariants
held, zero hard-validation retries in either mode.

## Decision rule (from orchestrator)

> If `--no-think` achieves 3/3 acceptance AND mean diversity ratio ≤ 0.75 → full run `--no-think`;
> otherwise thinking enabled.

`--no-think`: acceptance 3/3 ✓, mean diversity 0.672 ≤ 0.75 ✓ → **RULE SELECTS `--no-think`.**

## Recommendation

**Run the overnight full pass with `--no-think`.** Same acceptance and (slightly better) mean
diversity as thinking, at ~5.5× less wall-clock. Compliance is mechanically enforced regardless
of mode.

- Estimated full run (48 rewriteable paragraphs): **~15 min** happy-path; **~18–20 min** with a
  handful of diversity retries. (Thinking mode would be ~81 min.)
- Sequential only — the backend is single-slot (HTTP resets observed under concurrent calls) and
  each prompt feeds the prior rewrite as flow-context. Run backgrounded; resumable via the cache.

### The one tail case

idx2 (the dense `\textbf{We introduce the \emph{copy ceiling}...}` definition carrying the long
footnote) is the only paragraph where the modes differ:
- thinking: div 0.730 (genuinely restructured).
- no-think: div 0.923 (near-verbatim, single-word swap "introduce"→"propose").
Without the reasoning pass the model plays safest on the hardest paragraphs. This is now handled
by the diversity-retry (below), which re-attempts such paragraphs and, if they stay rigid,
accepts the least-similar attempt rather than forcing a fallback.

## Driver changes applied since the smoke (per orchestrator decisions 1–3)

1. **Diversity soft-fail.** A hard-valid rewrite with difflib ratio > 0.9 is retried with
   FEEDBACK "too close to source, restructure sentence order and connectives", capped at 2
   diversity retries; then the best (lowest-ratio) hard-valid attempt is ACCEPTED and flagged
   `rigid` in the report (dense definitional paragraphs are legitimately rigid, not forced to
   fallback). Prompt stays frozen verbatim.
2. **Abstract label.** Abstract-inner prose blocks are tagged `section="Abstract"` (was
   "(untitled)" in the flow prompt).
3. **`\texttt{}` arg-set check.** Source and output must carry the same multiset of `\texttt`
   arguments. Footnote *prose* remains unchecked by design; footnote-bearing paragraphs are now
   listed in `REWRITE-REPORT.md` for a human diff.

All three verified without model calls: segment→reassemble round-trip byte-identical on the
frozen `main.tex` (commit `c2a9922`); `\texttt` and diversity-retry logic unit-tested; abstract
label confirmed on idx0.

## Overnight run command

```
python3 tools/paper/rewrite_v4.py --no-think \
    --input docs/research/paper-v2/main.tex \
    --outdir docs/research/paper-v4
```

Outputs `docs/research/paper-v4/{main.tex, refs.bib, figures/, rewrite-cache.jsonl,
REWRITE-REPORT.md}`. The `(block_index, sha256(source))` cache absorbs the pending
adversarial-pass-2 edits incrementally: only paragraphs whose text changed are re-rewritten on
rerun. Post-run: `latexmk` the v4 (expect 0 errors) and Layer-A scan
(`prose-sanitiser/inspect_text.py`, expect Suspicious: 0). Do not commit; orchestrator stages.

## Smoke segmentation (frozen `main.tex`, commit `c2a9922`)

48 rewriteable paragraphs, 52 protected blocks, 1 protected-with-note (the §5 definitional
paragraph whose prose wraps a display-math `\[...\]` block with no blank line, protected whole).
