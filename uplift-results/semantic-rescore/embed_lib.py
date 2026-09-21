#!/usr/bin/env python3
"""Embedding support library for ``rescore.py`` (semantic re-score of Study 1).

Reconstructed 2026-09-21. The original ``embed_lib.py`` was imported by the
released ``rescore.py`` but never committed; this module reimplements it from
the calling conventions in ``rescore.py`` and the method description in
``RESCORE.md``. It provides exactly the four names ``rescore.py`` imports:

    EmbedCache        persistent text -> unit vector store
    cosine            cosine similarity of two unit vectors
    extract_candidates  text -> set of candidate spans
    normalise_title   title -> matcher-normalised form

Backend
-------
bge-small-en-v1.5 (384-d) served by Xinference at ``192.168.2.132:9997`` over
the OpenAI-compatible ``/v1/embeddings`` route. Override with ``EMBED_BASE`` /
``EMBED_MODEL``.

Batching. The task brief warned that batched requests may return global rather
than per-request offsets. That was re-verified against this endpoint on
2026-09-21 before this module was written: for batches of 3 and 256, the
returned ``index`` fields were exactly ``0..n-1`` and every spot-checked vector
was identical (cosine 1.0 to 1e-6) to the vector returned by a single-text call
for the same string. Batching is therefore used, with the response re-ordered by
``index`` rather than trusted positionally, and a hard assertion that the
returned index set equals ``range(len(batch))``. Set ``EMBED_BATCH=1`` to force
one text per call.

Vectors are L2-normalised on ingest so ``cosine`` is a plain dot product.

Cache format
------------
``embed_cache.json`` is append-only JSONL (one JSON object per line), as
``RESCORE.md`` describes ("274,616 vectors, JSONL"). Each line is
``{"t": <text>, "v": <base64 of 384 little-endian float32>}``. Append-only
persistence is deliberate: ``RESCORE.md`` records that the original run had an
O(n^2) cache-serialisation bug from rewriting the whole cache each flush.
Base64 float32 is used rather than a JSON float array because the latter costs
~4x the bytes for the same values; float32 (not float16) keeps similarities
exact to ~1e-7 so threshold decisions at 0.80/0.85/0.90 are not perturbed by
the storage format.

STDLIB ONLY, matching the rest of the harness.
"""
from __future__ import annotations

import base64
import json
import os
import re
import struct
import sys
import threading
import urllib.error
import urllib.request
from array import array
from concurrent.futures import ThreadPoolExecutor

EMBED_BASE = os.environ.get("EMBED_BASE", "http://192.168.2.132:9997/v1")
EMBED_MODEL = os.environ.get("EMBED_MODEL", "bge-small-en-v1.5")
EMBED_DIM = 384
BATCH = int(os.environ.get("EMBED_BATCH", "128"))
WORKERS = int(os.environ.get("EMBED_WORKERS", "6"))
RETRIES = 5
TIMEOUT = 300

# --- normalisation ---------------------------------------------------------
# Identical to bench_ontology_uplift.py / decompose_exposure.py's `normalise`,
# so gold titles and candidate spans share one surface vocabulary and the
# semantic matcher differs from the lexical one only in the comparison step.
_PUNCT_RE = re.compile(r"[^a-z0-9\s]+")
_WS_RE = re.compile(r"\s+")


def normalise_title(s: str) -> str:
    """Lowercase, strip punctuation, collapse whitespace."""
    return _WS_RE.sub(" ", _PUNCT_RE.sub(" ", (s or "").lower())).strip()


# --- candidate span extraction ---------------------------------------------
# RESCORE.md: "chunking text on structural delimiters (newline/;/,/:/parens ...)
# then taking every word n-gram (length 1-6) within each chunk plus the whole
# chunk and the whole text."
_CHUNK_RE = re.compile(r"[\n;,:()\[\]{}]+")
MAX_NGRAM = 6


def extract_candidates(text: str) -> set:
    """Return the set of normalised candidate spans for ``text``."""
    if not text:
        return set()
    out = set()
    whole = normalise_title(text)
    if whole:
        out.add(whole)
    for raw_chunk in _CHUNK_RE.split(text):
        chunk = normalise_title(raw_chunk)
        if not chunk:
            continue
        out.add(chunk)
        words = chunk.split()
        n = len(words)
        for i in range(n):
            # n-grams of length 1..MAX_NGRAM starting at i
            for k in range(1, min(MAX_NGRAM, n - i) + 1):
                out.add(" ".join(words[i:i + k]))
    return out


# --- similarity ------------------------------------------------------------

def cosine(u, v) -> float:
    """Cosine similarity. Vectors from EmbedCache are already L2-normalised,
    so this is a dot product; it stays correct for unnormalised input only if
    both are unit length, which is the invariant this module maintains."""
    return sum(a * b for a, b in zip(u, v))


# --- embedding backend -----------------------------------------------------

def _post(payload: dict) -> dict:
    req = urllib.request.Request(
        EMBED_BASE.rstrip("/") + "/embeddings",
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=TIMEOUT) as fh:
        return json.loads(fh.read().decode("utf-8"))


def _embed_batch(texts: list) -> list:
    """Embed ``texts``, returning vectors in the same order. Raises on failure."""
    last = None
    for attempt in range(RETRIES):
        try:
            d = _post({"model": EMBED_MODEL, "input": texts})
            rows = d["data"]
            idx = sorted(r["index"] for r in rows)
            if idx != list(range(len(texts))):
                raise RuntimeError(
                    f"endpoint returned non-local offsets {idx[:4]}..{idx[-1]} "
                    f"for a batch of {len(texts)}; re-run with EMBED_BATCH=1")
            ordered = [None] * len(texts)
            for r in rows:
                ordered[r["index"]] = r["embedding"]
            if any(v is None or len(v) != EMBED_DIM for v in ordered):
                raise RuntimeError("short or missing vector in batch response")
            return ordered
        except Exception as exc:  # noqa: BLE001 - retry any transport/server error
            last = exc
            if attempt == RETRIES - 1:
                break
    raise RuntimeError(f"embedding failed after {RETRIES} attempts: {last}")


def _unit(vec) -> array:
    a = array("f", vec)
    norm = sum(x * x for x in a) ** 0.5
    if norm > 0:
        inv = 1.0 / norm
        for i in range(len(a)):
            a[i] = a[i] * inv
    return a


def _encode(a: array) -> str:
    return base64.b64encode(struct.pack("<%df" % len(a), *a)).decode("ascii")


def _decode(b64: str) -> array:
    raw = base64.b64decode(b64)
    return array("f", struct.unpack("<%df" % (len(raw) // 4), raw))


class EmbedCache:
    """Persistent text -> unit-vector cache backed by append-only JSONL."""

    def __init__(self, path: str):
        self.path = path
        self.map = {}
        self._lock = threading.Lock()
        self._fh = None
        self._load()

    # -- persistence --------------------------------------------------------
    def _load(self) -> None:
        if not os.path.exists(self.path):
            return
        bad = 0
        with open(self.path, "r", encoding="utf-8") as fh:
            for line in fh:
                line = line.strip()
                if not line:
                    continue
                try:
                    rec = json.loads(line)
                    self.map[rec["t"]] = _decode(rec["v"])
                except Exception:  # noqa: BLE001 - tolerate a truncated tail
                    bad += 1
        if bad:
            sys.stderr.write(f"[embed_lib] skipped {bad} unreadable cache line(s)\n")
        sys.stderr.write(f"[embed_lib] loaded {len(self.map)} cached vectors\n")

    def _append(self, records: list) -> None:
        if self._fh is None:
            self._fh = open(self.path, "a", encoding="utf-8")
        for text, vec in records:
            self._fh.write(json.dumps({"t": text, "v": _encode(vec)},
                                      ensure_ascii=False) + "\n")
        self._fh.flush()

    def close(self) -> None:
        if self._fh is not None:
            self._fh.close()
            self._fh = None

    # -- API used by rescore.py --------------------------------------------
    def get(self, text: str):
        return self.map[text]

    def ensure(self, texts) -> None:
        """Embed and cache every string in ``texts`` not already present."""
        todo = []
        seen = set()
        for t in texts:
            if t and t not in self.map and t not in seen:
                seen.add(t)
                todo.append(t)
        if not todo:
            return
        total = len(todo)
        batches = [todo[i:i + BATCH] for i in range(0, total, BATCH)]
        done = [0]

        def work(batch):
            vecs = _embed_batch(batch)
            recs = [(t, _unit(v)) for t, v in zip(batch, vecs)]
            with self._lock:
                for t, a in recs:
                    self.map[t] = a
                self._append(recs)
                done[0] += len(batch)
                if done[0] % (BATCH * 40) < BATCH or done[0] == total:
                    sys.stderr.write(f"\r[embed_lib] embedded {done[0]}/{total}")
                    sys.stderr.flush()

        if WORKERS <= 1:
            for b in batches:
                work(b)
        else:
            with ThreadPoolExecutor(max_workers=WORKERS) as pool:
                list(pool.map(work, batches))
        sys.stderr.write("\n")
