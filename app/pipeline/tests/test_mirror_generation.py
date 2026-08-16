#!/usr/bin/env python3
"""Behavioural tests for the mirror.sh generation verifier — the SSOT-boundary
guarantee that the Loom never serves a mixed-build set (ADR-136 D4 / ADR-135 D2.1).

Run (from the app/ dir, `pipeline` importable as a package):
    <venv python> -m pytest pipeline/tests/test_mirror_generation.py -q

The verifier is the inline python block inside ../../mirror.sh. These tests extract
that exact block (no copy that can drift from the shipped code) and drive it as a
subprocess against synthetic staging/live sets, asserting:
  * a mixed-build candidate (stamps spanning > GEN_TOL) is REJECTED (exit 2) and the
    live set is left untouched — never a partial promotion;
  * a consistent fresh build is PROMOTED atomically (exit 0) with a .generation.json
    manifest written;
  * an all-current run (nothing downloaded) is a clean no-op (exit 0);
  * a failed fetch with no prior copy is REJECTED (exit 2).
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

import pytest

MIRROR = Path(__file__).resolve().parents[2] / "mirror.sh"
ARTIFACTS = ["scaffold-index.json", "prose-index.json", "ontology.ttl", "ontology-inferred.ttl"]


def _verifier_src() -> str:
    """Extract the exact inline python block from mirror.sh (between <<'PY' and PY)."""
    text = MIRROR.read_text()
    m = re.search(r"<<'PY'\n(.*?)\nPY\n", text, re.DOTALL)
    assert m, "could not locate the PY verifier block in mirror.sh"
    return m.group(1)


@pytest.fixture(scope="module")
def verifier(tmp_path_factory) -> Path:
    p = tmp_path_factory.mktemp("verifier") / "verify.py"
    p.write_text(_verifier_src())
    return p


def _write_set(dir_: Path, base_iso: str, offsets=(0.3, 0.9, 4.9)) -> None:
    """Write a consistent artifact set whose stamps cluster around base seconds."""
    base = _parse(base_iso)
    dir_.mkdir(parents=True, exist_ok=True)
    (dir_ / "scaffold-index.json").write_text(json.dumps(
        {"version": 1, "generated": _iso(base + offsets[0]), "counts": {"classes": 8146}}))
    (dir_ / "prose-index.json").write_text(json.dumps(
        {"version": 1, "generated": _iso(base + offsets[1]), "counts": {"pages": 5854}}))
    (dir_ / "ontology-inferred.ttl").write_text(
        f'@prefix vc: <x:> .\nvc:o vc:generatedAt "{_iso(base + offsets[2])}" .\n')
    (dir_ / "ontology.ttl").write_text("@prefix vc: <x:> .\nvc:o a vc:Ontology .\n")


def _parse(iso: str) -> float:
    from datetime import datetime
    return datetime.fromisoformat(iso.replace("Z", "+00:00")).timestamp()


def _iso(epoch: float) -> str:
    from datetime import datetime, timezone
    return datetime.fromtimestamp(epoch, timezone.utc).isoformat()


def _run(verifier: Path, data: Path, stage: Path, downloaded, failed=""):
    return subprocess.run(
        [sys.executable, str(verifier), str(data), str(stage), "300", "https://test",
         " ".join(downloaded), failed],
        capture_output=True, text=True)


def test_rejects_mixed_build(verifier, tmp_path):
    data, stage = tmp_path / "data", tmp_path / "data" / ".stage"
    _write_set(data, "2026-08-15T13:22:45+00:00")          # consistent live set
    stage.mkdir(parents=True, exist_ok=True)
    # a freshly-"downloaded" scaffold from a DIFFERENT build (5 days off)
    (stage / "scaffold-index.json").write_text(json.dumps(
        {"version": 1, "generated": "2026-08-10T09:00:00+00:00", "counts": {"classes": 8146}}))

    r = _run(verifier, data, stage, ["scaffold-index.json"])
    assert r.returncode == 2, r.stderr
    assert "mixed build" in r.stderr.lower()
    # live set untouched; no manifest written
    live = json.loads((data / "scaffold-index.json").read_text())["generated"]
    assert live.startswith("2026-08-15"), "live scaffold must be kept, not the mixed one"
    assert not (data / ".generation.json").exists()


def test_promotes_consistent_new_build(verifier, tmp_path):
    data, stage = tmp_path / "data", tmp_path / "data" / ".stage"
    _write_set(data, "2026-08-15T13:22:45+00:00")          # old live set
    _write_set(stage, "2026-08-16T20:00:00+00:00")         # consistent new build, all fresh

    r = _run(verifier, data, stage, ARTIFACTS)
    assert r.returncode == 0, r.stderr
    assert "PROMOTED" in r.stdout
    man = json.loads((data / ".generation.json").read_text())
    assert man["generation"].startswith("2026-08-16")
    assert set(man["artifacts"]) == set(ARTIFACTS)
    assert all("sha256" in a for a in man["artifacts"].values())
    # live scaffold now the new generation
    assert json.loads((data / "scaffold-index.json").read_text())["generated"].startswith("2026-08-16")


def test_current_when_nothing_downloaded(verifier, tmp_path):
    data, stage = tmp_path / "data", tmp_path / "data" / ".stage"
    _write_set(data, "2026-08-15T13:22:45+00:00")
    stage.mkdir(parents=True, exist_ok=True)

    r = _run(verifier, data, stage, [])
    assert r.returncode == 0, r.stderr
    assert "current" in r.stdout.lower()
    assert not (data / ".generation.json").exists()         # no promotion => no manifest


def test_rejects_failed_fetch_with_no_prior(verifier, tmp_path):
    data, stage = tmp_path / "data", tmp_path / "data" / ".stage"
    _write_set(data, "2026-08-15T13:22:45+00:00")
    stage.mkdir(parents=True, exist_ok=True)

    r = _run(verifier, data, stage, [], failed="ontology.ttl")
    assert r.returncode == 2, r.stderr
    assert "unreachable" in r.stderr.lower()
