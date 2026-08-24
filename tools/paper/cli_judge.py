#!/usr/bin/env python3
"""CLI-based judges — Claude Code and Codex — replacing the OpenRouter judge path.

Motivation (reviewer W1): Claude has never judged anything in this paper (Study 2
used gpt-4.1, controls used gemini-3.1-pro, page-quality used gemini+gpt-5.6), so it
is the clean third judge family. Codex (gpt-5.6-sol) is a first-party GPT judge. Both
run locally with no OpenRouter dependency.

Each judge takes a rendered prompt (rubric + item) and returns parsed JSON. Blind
protocol, seeded order, and rubric text are the caller's responsibility — this module
only wraps the two CLIs behind one interface.

Usage:
    from cli_judge import judge_claude, judge_codex
    verdict = judge_claude(prompt_text)   # -> dict from the model's JSON reply
"""
import json, re, subprocess, tempfile
from pathlib import Path

_JSON_RE = re.compile(r"\{.*\}", re.DOTALL)


def _extract_json(text: str) -> dict | None:
    text = re.sub(r"<think>.*?</think>", "", text, flags=re.DOTALL)
    text = re.sub(r"```(?:json)?|```", "", text)
    m = _JSON_RE.search(text)
    if not m:
        return None
    try:
        return json.loads(m.group(0))
    except json.JSONDecodeError:
        # salvage the first complete object
        depth, start = 0, None
        for i, c in enumerate(m.group(0)):
            if c == "{":
                if depth == 0:
                    start = i
                depth += 1
            elif c == "}":
                depth -= 1
                if depth == 0 and start is not None:
                    try:
                        return json.loads(m.group(0)[start:i + 1])
                    except json.JSONDecodeError:
                        pass
    return None


def judge_claude(prompt: str, model: str = "claude-haiku-4-5-20251001", timeout: int = 180) -> dict | None:
    """Judge via the Claude Code CLI in headless print mode. Default model is a
    small fast Claude; pass a larger id for the primary judge. Deterministic-ish
    (no temperature knob exposed by the CLI; runs are cached off by design here)."""
    r = subprocess.run(
        ["claude", "-p", prompt, "--model", model, "--output-format", "json"],
        capture_output=True, text=True, timeout=timeout,
    )
    if r.returncode != 0:
        raise RuntimeError(f"claude cli rc={r.returncode}: {r.stderr[-200:]}")
    # --output-format json may return an object with .result, or a list of
    # stream events whose final 'result'/'assistant' entry holds the text.
    inner = r.stdout
    try:
        outer = json.loads(r.stdout)
        if isinstance(outer, dict):
            inner = outer.get("result", r.stdout)
        elif isinstance(outer, list):
            for ev in reversed(outer):
                if isinstance(ev, dict) and ev.get("type") in ("result", "assistant") and ev.get("result"):
                    inner = ev["result"]; break
                if isinstance(ev, dict) and ev.get("result"):
                    inner = ev["result"]; break
    except json.JSONDecodeError:
        pass
    return _extract_json(inner if isinstance(inner, str) else json.dumps(inner))


def judge_codex(prompt: str, timeout: int = 300) -> dict | None:
    """Judge via the Codex CLI (gpt-5.6-sol) in non-interactive exec mode."""
    with tempfile.NamedTemporaryFile(mode="w", suffix=".txt", delete=False) as tf:
        tf.write(prompt)
        last = tf.name + ".last"
    r = subprocess.run(
        ["codex", "exec", "--sandbox", "read-only", "--skip-git-repo-check",
         "--output-last-message", last, "-"],
        stdin=open(tf.name), capture_output=True, text=True, timeout=timeout,
    )
    if r.returncode != 0:
        raise RuntimeError(f"codex cli rc={r.returncode}: {r.stderr[-200:]}")
    return _extract_json(Path(last).read_text())


if __name__ == "__main__":
    # smoke test: a trivial 0-5 grading item
    demo = ('You are a strict evaluation judge. Grade the CANDIDATE against the '
            'REFERENCE on 0-5. QUESTION: capital of France? REFERENCE: Paris. '
            'CANDIDATE: The capital is Paris. Respond ONLY with JSON '
            '{"score": <0-5>, "why": "<one sentence>"}.')
    import sys
    which = sys.argv[1] if len(sys.argv) > 1 else "claude"
    fn = {"claude": judge_claude, "codex": judge_codex}[which]
    print(which, "->", fn(demo))
