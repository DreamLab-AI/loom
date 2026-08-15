"""
pipeline.gate — domain-true autonomous continuation gate.

Adapted from prime-agent's ``--autonomous-gate`` (PrimeIntellect-ai/prime-agent),
but bound to *our* substrate instead of its generic "did the command exit 0" check.

Prime's autonomous mode runs work inside turn/token/time budgets with a continuation
predicate, under two contracts we keep verbatim:

  * a PASSED gate verifies ONLY what that gate checks — here, that the knowledge graph
    stays logically well-formed. It does NOT prove the enrichment is *correct*.
  * hitting a budget limit is NOT success — the loop stops "out of budget", not "done".

Where prime uses a generic shell exit, we compose the deterministic domain-true
predicates the corpus already owns, so a swarm refining thousands of classes proceeds
only while the graph is consistent:

  quick  →  pipeline.validate  (errors == 0)                       — fast, in-process
  full   →  quick + OWL/turtle build (no build errors) + RuVector recall band

Usage as a gate command (exit 0 = keep going, non-zero = stop):

    python -m pipeline.gate mainKnowledgeGraph/pages
    python -m pipeline.gate mainKnowledgeGraph/pages --tier full
    python -m pipeline.gate mainKnowledgeGraph/pages --json

Usage as an autonomous loop driver (enforces the budget contract above):

    python -m pipeline.gate mainKnowledgeGraph/pages --loop \
        --max-iterations 20 --max-seconds 1800 -- <refine-command ...>

The loop runs <refine-command> repeatedly, re-checking the gate between iterations,
and stops with an explicit, distinguishable outcome:
    gate_failed        — the graph broke; STOP (this is the safety win)
    budget_exhausted   — ran out of iterations/time; STOP, NOT success
    command_failed     — the refine command itself errored; STOP
    converged          — the refine command reported nothing left to do; DONE

Token budgets are intentionally *not* enforced here — a shell gate cannot observe model
tokens; the orchestrator (Workflow ``budget`` / swarm) owns that axis. This module owns
the graph-consistency predicate and the iteration/wall-clock budget.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path

from .jsonld_parser import parse_corpus
from .validate import validate_corpus

# A refine command may signal "nothing left to change" with this exit code so the loop
# can distinguish honest convergence from an error. Chosen to avoid clashing with the
# common 0/1/2 exits and 124 (timeout) / 130 (SIGINT).
CONVERGED_EXIT_CODE = 3

# Recall-gate bands (mirrors ~/workspace/CLAUDE.md recall-gate: self >=175/200, true >=107/120).
RECALL_SELF_MIN = 175
RECALL_TRUE_MIN = 107


@dataclass
class CheckResult:
    """One predicate's outcome. ``passed`` is the only thing the gate acts on;
    ``detail`` is for the human reading the verdict."""

    name: str
    passed: bool
    detail: str
    skipped: bool = False


@dataclass
class GateVerdict:
    """The composite gate result. ``passed`` is true only if every non-skipped check
    passed. A skipped check (e.g. recall unavailable in this shell) never fails the gate
    — it is reported honestly rather than silently dropped."""

    tier: str
    checks: list[CheckResult] = field(default_factory=list)

    @property
    def passed(self) -> bool:
        return all(c.passed for c in self.checks if not c.skipped)

    def summary(self) -> dict:
        return {
            "gate": "pass" if self.passed else "fail",
            "tier": self.tier,
            # The prime contract, made explicit in every machine-readable verdict:
            "caveat": "A passed gate proves graph consistency only, NOT enrichment correctness.",
            "checks": [
                {
                    "name": c.name,
                    "status": "skip" if c.skipped else ("pass" if c.passed else "fail"),
                    "detail": c.detail,
                }
                for c in self.checks
            ],
        }


# ── predicates ────────────────────────────────────────────────────────────────────────

def check_validate(pages_dir: Path) -> CheckResult:
    """The primary domain-true predicate: JSON-LD structural validity with zero errors.

    Runs in-process (no subprocess) so the loop can re-check cheaply between refines.
    Warnings do not fail the gate — only hard errors (DUPLICATE_IRI, SLUG_MISMATCH,
    missing-IRI, self-reference, …) do, matching ``python -m pipeline.validate``'s
    own ``exit(1 if report.errors else 0)``.
    """
    pages = parse_corpus(pages_dir)
    report = validate_corpus(pages)
    s = report.summary()
    n_err = s["errors"]
    passed = n_err == 0
    if passed:
        detail = f"{s['total_pages']} pages, {s['public_pages']} public, 0 errors, {s['warnings']} warnings"
    else:
        codes = ", ".join(sorted({i.code for i in report.errors}))
        detail = f"{n_err} errors across [{codes}]; first: {report.errors[0].path}: {report.errors[0].message}"
    return CheckResult("validate", passed, detail)


def check_build(pages_dir: Path, out_dir: Path | None = None) -> CheckResult:
    """Deeper predicate: the full OWL/turtle/site build completes without error.

    Heavy (minutes) — only run in the ``full`` tier. Builds into a throwaway directory
    so it never touches the tracked ``www/`` (which CI regenerates via ``rm -rf www``).
    """
    target = out_dir or (pages_dir.parent / ".gate-build")
    proc = subprocess.run(
        [sys.executable, "-m", "pipeline.build", str(pages_dir), str(target)],
        capture_output=True,
        text=True,
    )
    if proc.returncode == 0:
        return CheckResult("build", True, f"OWL/site build clean → {target}")
    tail = (proc.stderr or proc.stdout).strip().splitlines()[-1:] or ["(no output)"]
    return CheckResult("build", False, f"build exit {proc.returncode}: {tail[0]}")


def check_recall(repo_root: Path) -> CheckResult:
    """Optional predicate: RuVector recall stays in band (self >=175/200, true >=107/120).

    Skipped (not failed) when ``agentbox.sh`` is not runnable from this shell — recall is
    an environment capability, not a corpus property, so its absence must not block work.
    """
    script = repo_root / "agentbox.sh"
    if not script.exists():
        # try the workspace root one level up (repo layout: ~/workspace/agentbox.sh)
        script = repo_root.parent / "agentbox.sh"
    if not script.exists():
        return CheckResult("recall", True, "agentbox.sh not found — skipped", skipped=True)
    proc = subprocess.run(
        [str(script), "ruvector", "recall"], capture_output=True, text=True
    )
    if proc.returncode != 0:
        return CheckResult(
            "recall", True, f"recall not runnable here (exit {proc.returncode}) — skipped", skipped=True
        )
    # Parse "self N/200" and "true N/120" out of the tool output, tolerant of formatting.
    import re

    out = proc.stdout + proc.stderr
    self_m = re.search(r"self[^0-9]*(\d+)\s*/\s*200", out, re.I)
    true_m = re.search(r"true[^0-9]*(\d+)\s*/\s*120", out, re.I)
    if not (self_m and true_m):
        return CheckResult("recall", True, "recall output unparseable — skipped", skipped=True)
    self_n, true_n = int(self_m.group(1)), int(true_m.group(1))
    passed = self_n >= RECALL_SELF_MIN and true_n >= RECALL_TRUE_MIN
    detail = f"self {self_n}/200 (>= {RECALL_SELF_MIN}), true {true_n}/120 (>= {RECALL_TRUE_MIN})"
    return CheckResult("recall", passed, detail)


def run_gate(pages_dir: Path, tier: str, repo_root: Path) -> GateVerdict:
    verdict = GateVerdict(tier=tier)
    verdict.checks.append(check_validate(pages_dir))
    # Short-circuit: if the cheap structural check already failed, don't spend minutes
    # on a build — the graph is known-broken.
    if tier == "full" and verdict.checks[-1].passed:
        verdict.checks.append(check_build(pages_dir))
        verdict.checks.append(check_recall(repo_root))
    return verdict


# ── autonomous loop ─────────────────────────────────────────────────────────────────────

@dataclass
class LoopOutcome:
    status: str  # gate_failed | budget_exhausted | command_failed | converged
    iterations: int
    elapsed_s: float
    note: str

    def summary(self) -> dict:
        return {
            "outcome": self.status,
            # budget_exhausted is explicitly NOT success — surfaced here so no caller can
            # mistake "ran out of budget" for "task complete".
            "success": self.status == "converged",
            "iterations": self.iterations,
            "elapsed_s": round(self.elapsed_s, 1),
            "note": self.note,
        }


def autonomous_loop(
    pages_dir: Path,
    command: list[str],
    tier: str,
    repo_root: Path,
    max_iterations: int,
    max_seconds: float,
) -> LoopOutcome:
    """Run ``command`` repeatedly while the gate passes and budget remains.

    Contract (from prime-agent, kept verbatim):
      * gate re-checked BEFORE the first iteration and AFTER every iteration — a refine
        that breaks the graph stops the loop immediately;
      * budget exhaustion stops the loop but is reported as ``budget_exhausted``, never
        as success.
    """
    start = time.monotonic()
    iterations = 0

    # Pre-flight: refuse to start on an already-broken graph.
    pre = run_gate(pages_dir, "quick", repo_root)
    if not pre.passed:
        return LoopOutcome(
            "gate_failed", 0, time.monotonic() - start,
            f"pre-flight gate failed: {[c.detail for c in pre.checks if not c.passed]}",
        )

    while True:
        if iterations >= max_iterations:
            return LoopOutcome(
                "budget_exhausted", iterations, time.monotonic() - start,
                f"reached max-iterations={max_iterations} (NOT success)",
            )
        if time.monotonic() - start >= max_seconds:
            return LoopOutcome(
                "budget_exhausted", iterations, time.monotonic() - start,
                f"reached max-seconds={max_seconds} (NOT success)",
            )

        proc = subprocess.run(command)
        iterations += 1

        if proc.returncode == CONVERGED_EXIT_CODE:
            # Command says nothing left to do — verify the graph is still clean, then done.
            post = run_gate(pages_dir, tier, repo_root)
            if not post.passed:
                return LoopOutcome(
                    "gate_failed", iterations, time.monotonic() - start,
                    "command converged but final gate failed — graph left inconsistent",
                )
            return LoopOutcome(
                "converged", iterations, time.monotonic() - start,
                "refine command reported convergence and the gate holds",
            )
        if proc.returncode != 0:
            return LoopOutcome(
                "command_failed", iterations, time.monotonic() - start,
                f"refine command exited {proc.returncode}",
            )

        post = run_gate(pages_dir, tier, repo_root)
        if not post.passed:
            return LoopOutcome(
                "gate_failed", iterations, time.monotonic() - start,
                f"gate failed after iteration {iterations}: "
                f"{[c.detail for c in post.checks if not c.passed and not c.skipped]}",
            )


# ── cli ────────────────────────────────────────────────────────────────────────────────

def main(argv: list[str] | None = None) -> int:
    # Split the loop command off on the literal ``--`` BEFORE argparse sees it. Using
    # argparse.REMAINDER for this would greedily swallow --json/--loop/--tier (a known
    # argparse footgun), so we partition argv by hand and hand argparse only its flags.
    raw = list(sys.argv[1:] if argv is None else argv)
    command: list[str] = []
    if "--" in raw:
        idx = raw.index("--")
        command = raw[idx + 1:]
        raw = raw[:idx]

    parser = argparse.ArgumentParser(
        prog="pipeline.gate",
        description="Domain-true autonomous continuation gate for the ontology corpus.",
    )
    parser.add_argument("pages_dir", nargs="?", default="mainKnowledgeGraph/pages", type=Path)
    parser.add_argument("--tier", choices=["quick", "full"], default="quick",
                        help="quick=validate only (default); full=+build+recall")
    parser.add_argument("--json", action="store_true", help="emit the verdict as JSON")
    parser.add_argument("--loop", action="store_true",
                        help="run as an autonomous loop driver over a refine command after --")
    parser.add_argument("--max-iterations", type=int, default=20)
    parser.add_argument("--max-seconds", type=float, default=1800.0)
    args = parser.parse_args(raw)

    repo_root = Path.cwd()

    if args.loop:
        if not command:
            parser.error("--loop requires a command after --, e.g. --loop -- python refine.py")
        outcome = autonomous_loop(
            args.pages_dir, command, args.tier, repo_root,
            args.max_iterations, args.max_seconds,
        )
        out = outcome.summary()
        print(json.dumps(out, indent=2) if args.json else
              f"[gate:loop] {out['outcome']} (success={out['success']}) "
              f"after {out['iterations']} iterations / {out['elapsed_s']}s — {out['note']}")
        # exit 0 only on genuine convergence
        return 0 if outcome.status == "converged" else 1

    verdict = run_gate(args.pages_dir, args.tier, repo_root)
    if args.json:
        json.dump(verdict.summary(), sys.stdout, indent=2)
        sys.stdout.write("\n")
    else:
        print(f"[gate:{verdict.tier}] {'PASS' if verdict.passed else 'FAIL'} "
              "(proves graph consistency only, NOT enrichment correctness)")
        for c in verdict.checks:
            tag = "skip" if c.skipped else ("pass" if c.passed else "FAIL")
            print(f"  [{tag}] {c.name}: {c.detail}")
    return 0 if verdict.passed else 1


if __name__ == "__main__":
    sys.exit(main())
