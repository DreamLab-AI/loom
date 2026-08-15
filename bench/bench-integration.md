# Wiring `ontology_scaffold` into a bench harness

Target: any `bench-headhead.py`-style harness that builds an OpenAI chat
payload and POSTs it to `/v1/chat/completions`. The scaffold is a pure
message-list transform, so integration is three small edits: an import, a
flag, and one line at the point where `messages` is built.

Prerequisites on HP-Desktop:

- `ontology_scaffold.py` next to the harness (or on `PYTHONPATH`). Stdlib
  only — no pip needed.
- The index at `~/githubs/loom/app/data/scaffold-index.json`, or
  `export ONTOLOGY_INDEX=/path/to/scaffold-index.json`.
- Sanity check: `python3 ontology_scaffold.py --selftest` (no index needed)
  and `python3 ontology_scaffold.py --stats` (index needed).

## Minimal diff

```diff
--- a/bench-headhead.py
+++ b/bench-headhead.py
@@
 import argparse
 import json
 import urllib.request
+
+try:
+    from ontology_scaffold import scaffold_messages
+except ImportError:          # module absent -> scaffold silently unavailable
+    scaffold_messages = None
@@
 parser = argparse.ArgumentParser()
 parser.add_argument("--endpoint", default="http://192.168.2.132:8084/v1")
+parser.add_argument("--scaffold", action="store_true",
+                    help="inject ontology context before each request")
+parser.add_argument("--scaffold-budget", type=int, default=1500,
+                    help="token budget for the ontology block")
@@ def run_case(prompt, args):
     messages = [{"role": "user", "content": prompt}]
+    if args.scaffold and scaffold_messages is not None:
+        # No-op when the ontology has no match for the prompt, so this is
+        # always safe: the baseline path IS the fallback path.
+        messages = scaffold_messages(messages,
+                                     budget_tokens=args.scaffold_budget)
     payload = {"model": args.model, "messages": messages,
                "temperature": args.temperature}
```

That is the whole integration. `scaffold_messages` returns a **new** list:
it merges the ontology block into an existing `system` message if the harness
already sets one, otherwise it inserts a `system` message at position 0. When
no ontology class matches the prompt, the messages come back unchanged, so
scored comparisons stay honest — a non-match run is bit-identical to
baseline.

## A/B loop pattern

For a head-to-head inside one run (rather than two invocations), loop the
variant per case and tag the results:

```python
from ontology_scaffold import scaffold, scaffold_messages

VARIANTS = ("baseline", "scaffold")

for case in cases:
    for variant in VARIANTS:
        messages = [{"role": "user", "content": case.prompt}]
        if variant == "scaffold":
            messages = scaffold_messages(messages, budget_tokens=args.scaffold_budget)
        t0 = time.perf_counter()
        reply = post_chat(args.endpoint, args.model, messages)   # existing helper
        results.append({
            "case": case.id,
            "variant": variant,
            "latency_s": time.perf_counter() - t0,
            # record whether the scaffold actually engaged for this prompt:
            "scaffold_engaged": variant == "scaffold"
                                and bool(scaffold(case.prompt)),
            "reply": reply,
        })
```

Report `scaffold_engaged` alongside scores: prompts where the scaffold never
engaged should show identical A/B behaviour and belong in a separate bucket,
otherwise they dilute the measured effect.

## Two-invocation pattern (simplest)

```bash
python3 bench-headhead.py --out results-baseline.jsonl
python3 bench-headhead.py --scaffold --out results-scaffold.jsonl
```

Same seeds/temperature in both runs; diff the two JSONL files per case id.

## Knobs

| Knob | Where | Default | Notes |
|---|---|---|---|
| `--scaffold` | harness flag | off | baseline runs untouched |
| `--scaffold-budget` | harness flag | 1500 | approx tokens (chars/4); whole sections are trimmed to fit |
| `ONTOLOGY_INDEX` | env | `~/githubs/loom/app/data/scaffold-index.json` | index location |
| `max_seeds`, `hops` | `scaffold_messages(...)` kwargs | 4, 1 | tighten for small-context models |

## Cost notes

- Index load is lazy and cached in-process (~tens of ms for 8k classes,
  first call only); per-prompt scaffolding is ~5 ms — negligible against
  generation latency, so it does not distort latency measurements after the
  first case. To keep case 1 clean, warm it up before the loop:
  `scaffold("warmup")`.
- The injected block costs prompt tokens (up to the budget). When comparing
  tokens/s, compare on completion tokens, not total.
