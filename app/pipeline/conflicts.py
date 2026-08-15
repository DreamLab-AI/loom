"""
pipeline.conflicts — semantica-style pre-merge conflict detection.

Adopts the ConflictDetector pattern from semantica (github.com/semantica-agi/semantica)
NATIVELY over our corpus — no VisionClaw or semantica dependency. Where
pipeline.validate enforces structural well-formedness (unique IRIs, slug/label
agreement, no dangling refs), this detects the SEMANTIC conflicts a multi-agent
swarm creates and that should be resolved BEFORE a merge:

  DUPLICATE_CONCEPT      distinct IRIs sharing a normalised label (the "duplicate
                         merges" failure mode); flagged louder when their
                         definitions also differ (a contradiction, not just a dupe)
  SUBCLASS_CYCLE         a cycle in subClassOf — a logical impossibility
  RELATION_CONTRADICTION a class both subClassOf and contrasts_with the same target
  TYPE_CONFLICT          a class whose subClassOf parent is declared an Individual

This is the highest-value semantica pattern for us and the one that needs no live
reasoner. Pair with pipeline.gate as a pre-merge guard: run it before a swarm
writes, resolve highs, then let the write proceed.

    python -m pipeline.conflicts mainKnowledgeGraph/pages
    python -m pipeline.conflicts mainKnowledgeGraph/pages --json
    python -m pipeline.conflicts mainKnowledgeGraph/pages --severity high   # gate: exit 1 on any high

Exit code is 1 when any conflict at or above --severity (default high) is found,
so it composes as an autonomous-gate predicate.
"""

from __future__ import annotations

import json
import re
import sys
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path

from .jsonld_parser import parse_corpus

SEVERITY_ORDER = {"high": 0, "medium": 1, "low": 2}


@dataclass
class Conflict:
    kind: str
    severity: str
    subjects: list  # list[str] of IRIs
    detail: str

    def as_dict(self) -> dict:
        return {"kind": self.kind, "severity": self.severity, "subjects": self.subjects, "detail": self.detail}


@dataclass
class ConflictReport:
    """Typed, JSON-serialisable result of a conflict scan (PRD-022 W-A).

    Importers call ``analyse(pages)`` to get one of these, inspect ``blocking()``
    / ``ok()`` / ``exit_code``, and decide their own control flow. The ``sys.exit``
    stays a CLI-boundary concern in ``main()`` — nothing here touches sys/argv.

        report = analyse(pages, severity_gate="high")
        if not report.ok():
            print(report.to_json())
        raise SystemExit(report.exit_code)   # caller's choice, never ours
    """

    conflicts: list  # list[Conflict]
    summary: dict
    severity_gate: str = "high"

    def blocking(self) -> list:
        """Conflicts at or above ``severity_gate`` (lower rank = more severe)."""
        rank = SEVERITY_ORDER.get(self.severity_gate, 0)
        return [c for c in self.conflicts if SEVERITY_ORDER.get(c.severity, 9) <= rank]

    def ok(self) -> bool:
        return not self.blocking()

    @property
    def exit_code(self) -> int:
        return 1 if self.blocking() else 0

    def to_dict(self) -> dict:
        return {
            "summary": self.summary,
            "severity_gate": self.severity_gate,
            "conflicts": [c.as_dict() for c in self.conflicts],
        }

    def to_json(self, indent: int = 2) -> str:
        return json.dumps(self.to_dict(), indent=indent)


def _norm_label(s: str) -> str:
    return re.sub(r"[^a-z0-9]+", " ", (s or "").lower()).strip()


def _entities(pages):
    return [p.ontology_class for p in pages if p.ontology_class and p.ontology_class.iri]


def detect_duplicate_concepts(ents) -> list:
    """Distinct IRIs that share a normalised label — likely un-merged duplicates."""
    by_label = defaultdict(list)
    for e in ents:
        by_label[_norm_label(e.label)].append(e)
    out = []
    for label, group in by_label.items():
        if not label:
            continue
        iris = sorted({e.iri for e in group})
        if len(iris) > 1:
            distinct_defs = len({(e.definition or "").strip() for e in group if (e.definition or "").strip()})
            detail = f'{len(iris)} classes share label "{group[0].label}"'
            if distinct_defs > 1:
                detail += " with DIFFERING definitions (contradiction, not just a duplicate)"
            out.append(Conflict("DUPLICATE_CONCEPT", "high", iris, detail))
    return out


def detect_subclass_cycles(ents) -> list:
    """Cycles in the subClassOf graph — a logical impossibility."""
    parents = {e.iri: [w.iri for w in e.sub_class_of if w.iri] for e in ents}
    out = []
    seen = set()
    WHITE, GREY, BLACK = 0, 1, 2
    color = defaultdict(int)

    # Iterative DFS (explicit stack) so a deep hierarchy can't blow the recursion limit.
    for root in list(parents.keys()):
        if color[root] != WHITE:
            continue
        stack = [(root, 0)]
        path = []
        while stack:
            node, idx = stack[-1]
            if idx == 0:
                color[node] = GREY
                path.append(node)
            kids = parents.get(node, [])
            if idx < len(kids):
                stack[-1] = (node, idx + 1)
                child = kids[idx]
                if color[child] == GREY:  # back-edge → cycle
                    i = path.index(child)
                    cyc = tuple(path[i:])
                    key = tuple(sorted(cyc))
                    if key not in seen:
                        seen.add(key)
                        out.append(Conflict("SUBCLASS_CYCLE", "high", list(cyc),
                                            f'subClassOf cycle: {" -> ".join(cyc)} -> {child}'))
                elif color[child] == WHITE and child in parents:
                    stack.append((child, 0))
            else:
                color[node] = BLACK
                path.pop()
                stack.pop()
    return out


def detect_relation_contradictions(ents) -> list:
    """A class that is both subClassOf and contrasts_with the same target."""
    out = []
    for e in ents:
        sc = {w.iri for w in e.sub_class_of if w.iri}
        cw = {w.iri for w in e.relations.contrasts_with if w.iri}
        for t in sorted(sc & cw):
            out.append(Conflict("RELATION_CONTRADICTION", "medium", [e.iri, t],
                                f"{e.iri} is both subClassOf and contrasts_with {t}"))
    return out


def detect_type_conflicts(ents) -> list:
    """A class whose subClassOf parent is declared an Individual (type mismatch)."""
    types = {e.iri: e.entity_type for e in ents}
    out = []
    for e in ents:
        for w in e.sub_class_of:
            t = types.get(w.iri)
            if t and t not in ("Class", "OntologyClass"):
                out.append(Conflict("TYPE_CONFLICT", "medium", [e.iri, w.iri],
                                    f"{e.iri} subClassOf {w.iri}, but {w.iri} is a {t}, not a Class"))
    return out


def detect_conflicts(pages) -> list:
    ents = _entities(pages)
    conflicts = (
        detect_duplicate_concepts(ents)
        + detect_subclass_cycles(ents)
        + detect_relation_contradictions(ents)
        + detect_type_conflicts(ents)
    )
    conflicts.sort(key=lambda c: (SEVERITY_ORDER.get(c.severity, 9), c.kind))
    return conflicts


def summarise(conflicts) -> dict:
    by_kind = defaultdict(int)
    by_sev = defaultdict(int)
    for c in conflicts:
        by_kind[c.kind] += 1
        by_sev[c.severity] += 1
    return {"total": len(conflicts), "by_kind": dict(by_kind), "by_severity": dict(by_sev)}


def analyse(pages, severity_gate: str = "high") -> ConflictReport:
    """Pure, importable entry point — runs the detectors on in-memory pages and
    returns a typed report. Touches no sys/argv/exit, so it is safe for gate.py
    (or any importer) to fold ``report.ok()`` into its own result."""
    conflicts = detect_conflicts(pages)
    return ConflictReport(conflicts=conflicts, summary=summarise(conflicts), severity_gate=severity_gate)


def analyse_dir(pages_dir, severity_gate: str = "high") -> ConflictReport:
    """``analyse`` wrapping ``parse_corpus`` so callers get parse + detect without
    re-implementing corpus loading or spawning a subprocess."""
    return analyse(parse_corpus(Path(pages_dir)), severity_gate=severity_gate)


def main(argv=None):
    argv = list(sys.argv[1:] if argv is None else argv)
    pages_dir = Path(argv[0]) if argv and not argv[0].startswith("--") else Path("mainKnowledgeGraph/pages")
    as_json = "--json" in argv
    gate = "high"
    if "--severity" in argv:
        i = argv.index("--severity")
        if i + 1 < len(argv):
            gate = argv[i + 1]

    pages = parse_corpus(pages_dir)
    report = analyse(pages, gate)
    conflicts = report.conflicts
    summary = report.summary

    if as_json:
        # Keep the historical CLI json shape ({"summary":…, "conflicts":[…]}, no
        # "severity_gate" key) byte-for-byte — importers use report.to_dict()
        # (which DOES carry severity_gate); the CLI contract stays frozen.
        json.dump({"summary": summary, "conflicts": [c.as_dict() for c in conflicts]}, sys.stdout, indent=2)
        sys.stdout.write("\n")
    else:
        print(f"Conflicts: {summary['total']} total — {summary['by_severity']}")
        for c in conflicts[:40]:
            print(f"  [{c.severity}] {c.kind}: {c.detail}")
            print(f"        subjects: {', '.join(c.subjects)}")
        if len(conflicts) > 40:
            print(f"  … {len(conflicts) - 40} more")

    sys.exit(report.exit_code)


if __name__ == "__main__":
    main()
