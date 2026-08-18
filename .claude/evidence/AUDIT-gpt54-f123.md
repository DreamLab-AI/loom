# Adversarial audit — F1/F2/F3 serving features (GPT-5.4, anti-fox)

Model family different from the implementer (Claude). Read-only invocation over
the full diff + the three new files.

```
codex exec -m gpt-5.4 --sandbox danger-full-access \
  "<adversarial-review prompt, explicit NO-WRITE>"   # over the full feature diff (git range below)
```

- date (UTC): 2026-08-18
- tokens used: 82,419
- scope: F1 verbatim serving, F2 exposure telemetry, F3 thinking/budget control
- input: the working-tree diff of the F1/F2/F3 commits (crates/ + deploy/ +
  the three new files exposure.rs / serving.rs / exp012_serving.rs).
- verbatim findings below.

## Findings

### Finding 1 — High — REMEDIATED
> `LOOM_THINK_TOKEN_FLOOR` is not actually default-off. Its default is `1536`, and
> `apply_thinking_controls()` runs on every engaged delegation, even when the user
> sets none of the new F3 env vars. Concrete failure: an existing deployment that
> intentionally had `LOOM_MIN_MAX_TOKENS=0` to allow small `max_tokens` asks will
> now still have engaged requests silently raised to `1536` … That changes current
> behaviour with the new feature unset.

**Real.** Violates the "defaults preserve current behaviour exactly" mandate (and
the F3 section header "defaults off"). **Fix:** the code default of
`LOOM_THINK_TOKEN_FLOOR` is now **0** (F3 fully off ⇒ the backend's
`LOOM_MIN_MAX_TOKENS` remains the sole token floor; a deployment that disabled it
is never re-floored). Profile A sets `1536` explicitly (matches the
`LOOM_MIN_MAX_TOKENS` it also runs). Added regression test
`f3_default_off_leaves_engaged_max_tokens_untouched`. Files:
`crates/loom-facade/src/config.rs` (default + doc), `tests/common/mod.rs` (builder
default), `tests/exp012_serving.rs` (regression test), EXPECTATIONS EXP-014.

### Finding 2 — Medium — ACCEPTED BY DESIGN (no change)
> F2 is not config-gated … On every engaged 200 response, the code now always
> injects `loom.served_mode` and `loom.exposure`; `LOOM_EXPOSURE_APPEND` only
> controls the extra answer line. Clients/tests written against the prior default
> response shape now see extra fields.

**By design, per the mission:** F2 is specified as "EXPOSURE TELEMETRY (always on
when a scaffold was injected; zero behaviour change to **content** by default)".
The mission explicitly separates always-on TELEMETRY from content mutation (gated
by `LOOM_EXPOSURE_APPEND`). The added keys live inside Loom's own additive `loom`
telemetry namespace — the OpenAI-standard response fields (`choices`, `usage`,
`model`, …) are untouched, and JSON consumers ignore unknown keys. This is the
intended, mission-sanctioned surface, so no code change. Documented in EXP-013.

### Explicitly cleared by the auditor
> I did not find another real bug in the verbatim gating, matcher port, panic
> surface, or OpenAI envelope beyond those regressions.

## Post-remediation gate
- `cargo test --workspace` green (incl. the new regression test).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean.
- `cargo deny check` green; changed files `rustfmt`-clean.
- `docker build -f deploy/Dockerfile -t loom:rust-f123` completes (258 MB) after
  the remediation.
