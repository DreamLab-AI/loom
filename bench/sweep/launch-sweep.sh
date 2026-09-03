#!/usr/bin/env bash
# FROZEN (2026-09-03): the harness this drives, bench/bench_ontology_uplift.py, was
# DELETED (broken import of the retired app/ontology_scaffold.py). This script is kept
# as the paper's cited reproduction driver (docs/research/latex/report.tex) and as the
# record of the exact configuration behind uplift-results/. It CANNOT run as-is; see
# bench/LEGACY-PYTHON-NOTE.md for the checkout recipe that restores the harness.
# Launch the 10-model cross-provider uplift sweep. 4 provider groups run in
# PARALLEL (independent rate limits); models within a group run sequentially.
set -uo pipefail
cd /home/devuser/workspace/loom
D=bench/sweep/run-one-model.sh
GEM=https://generativelanguage.googleapis.com/v1beta/openai/
OR=https://openrouter.ai/api/v1
DS=https://api.deepseek.com/v1
L=uplift-results/sweep/logs; mkdir -p "$L"

# group: google (native GOOGLE_API_KEY)
( bash $D gemini-2.5-flash-lite "$GEM" gemini-2.5-flash-lite GOOGLE_API_KEY ""    ;
  bash $D gemini-3.5-flash-lite "$GEM" gemini-3.5-flash-lite GOOGLE_API_KEY low  ;
  bash $D gemini-3.7-flash-t0   "$GEM" gemini-3.7-flash      GOOGLE_API_KEY low  ) > "$L/google.log" 2>&1 &
PG=$!

# group: deepseek (native)
( bash $D deepseek-chat "$DS" deepseek-chat DEEPSEEK_API_KEY "" ) > "$L/deepseek.log" 2>&1 &
PD=$!

# group: openrouter A
( bash $D gpt-4.1-mini     "$OR" openai/gpt-4.1-mini                     OPENROUTER_API_KEY "" ;
  bash $D claude-haiku-4.5 "$OR" anthropic/claude-haiku-4.5              OPENROUTER_API_KEY "" ;
  bash $D llama-3.3-70b    "$OR" meta-llama/llama-3.3-70b-instruct       OPENROUTER_API_KEY "" ) > "$L/or_a.log" 2>&1 &
PA=$!

# group: openrouter B
( bash $D mistral-small-24b "$OR" mistralai/mistral-small-24b-instruct-2501 OPENROUTER_API_KEY "" ;
  bash $D qwen-2.5-72b      "$OR" qwen/qwen-2.5-72b-instruct               OPENROUTER_API_KEY "" ;
  bash $D glm-4.6           "$OR" z-ai/glm-4.6                             OPENROUTER_API_KEY "" ) > "$L/or_b.log" 2>&1 &
PB=$!

wait $PG $PD $PA $PB
echo "SWEEP_DONE $(date -u +%H:%M:%S)"
