#!/usr/bin/env python3
"""Embed the concept records via Xinference (bge-small-en-v1.5, 384-dim) and stage
idempotent upserts into ruvector-postgres namespace `ontology-corpus` (task #17).

Matches the live memory_entries schema + our conventions exactly:
  - key       = urn:ngm:class:<slug>          (IRI, matches legacy ontology-classes)
  - id        = loom:ontology-corpus:<key>    (PK; ON CONFLICT (id) => idempotent)
  - value     = to_jsonb(record text)         (jsonb string, as memory_store stores it)
  - embedding = ruvector(384), unit-norm      (cosine HNSW: idx_memory_embedding_hnsw)
  - metadata  = slug/title/domain/maturity/quality/has_prose/generation (close-the-loop)
  - source_type = loom ; project_id = NULL     (legacy ontology-classes convention)

Embedder is LOCKED to bge-small-en-v1.5/384 per project-state ops law (Qwen3 + bge-m3
rejected). No pg driver needed: emits a .sql file streamed into the container via
`docker exec -i psql`. HNSW rebuild is a SEPARATE step (never CONCURRENTLY).

Usage:
  python3 tools/ingest/embed_and_stage.py \
    --records uplift-results/ingest/concept-records.jsonl \
    --out uplift-results/ingest/ontology-corpus.sql \
    --namespace ontology-corpus --batch 96
"""
from __future__ import annotations
import argparse, json, sys, urllib.request
from pathlib import Path

XINFER = "http://xinference:9997/v1/embeddings"
MODEL = "bge-small-en-v1.5"
DIM = 384


def embed_batch(texts, timeout=60):
    body = json.dumps({"model": MODEL, "input": texts}).encode()
    req = urllib.request.Request(XINFER, data=body,
                                 headers={"Content-Type": "application/json"}, method="POST")
    r = json.load(urllib.request.urlopen(req, timeout=timeout))
    # Xinference returns data ordered by input index
    return [d["embedding"] for d in sorted(r["data"], key=lambda d: d["index"])]


def sql_str(s: str) -> str:
    """A SQL single-quoted literal from an arbitrary python string."""
    return "'" + s.replace("'", "''") + "'"


def main(argv=None):
    ap = argparse.ArgumentParser()
    ap.add_argument("--records", default="uplift-results/ingest/concept-records.jsonl", type=Path)
    ap.add_argument("--out", default="uplift-results/ingest/ontology-corpus.sql", type=Path)
    ap.add_argument("--namespace", default="ontology-corpus")
    ap.add_argument("--source-type", default="loom")
    ap.add_argument("--batch", type=int, default=96)
    ap.add_argument("--rows-per-insert", type=int, default=250)
    args = ap.parse_args(argv)

    records = [json.loads(l) for l in open(args.records) if l.strip()]
    print(f"embedding {len(records)} records via {MODEL} (batch {args.batch})...", file=sys.stderr)

    # 1. embed in batches
    vecs = []
    for i in range(0, len(records), args.batch):
        chunk = records[i:i + args.batch]
        vs = embed_batch([r["text"] for r in chunk])
        assert all(len(v) == DIM for v in vs), f"bad dim in batch {i}"
        vecs.extend(vs)
        if (i // args.batch) % 10 == 0:
            print(f"  embedded {min(i + args.batch, len(records))}/{len(records)}", file=sys.stderr)
    assert len(vecs) == len(records)

    # 2. emit staged upsert SQL
    ns = args.namespace
    args.out.parent.mkdir(parents=True, exist_ok=True)
    cols = "(id, namespace, key, value, embedding, metadata, source_type, project_id)"
    n = 0
    with open(args.out, "w") as f:
        f.write("BEGIN;\n")
        f.write(f"-- ontology-corpus concept index: {len(records)} classes, bge-small-en-v1.5/384\n")
        for i in range(0, len(records), args.rows_per_insert):
            batch = list(enumerate(records[i:i + args.rows_per_insert], start=i))
            f.write(f"INSERT INTO memory_entries {cols} VALUES\n")
            rows = []
            for idx, r in batch:
                slug = r["id"]
                key = f"urn:ngm:class:{slug}"
                rid = f"{args.source_type}:{ns}:{key}"
                emb = "[" + ",".join(f"{x:.6f}" for x in vecs[idx]) + "]"
                meta = json.dumps(r["metadata"], ensure_ascii=False)
                rows.append(
                    f"({sql_str(rid)}, {sql_str(ns)}, {sql_str(key)}, "
                    f"to_jsonb({sql_str(r['text'])}::text), {sql_str(emb)}::ruvector(384), "
                    f"{sql_str(meta)}::jsonb, {sql_str(args.source_type)}, NULL)"
                )
                n += 1
            f.write(",\n".join(rows))
            f.write("\nON CONFLICT (id) DO UPDATE SET value=EXCLUDED.value, "
                    "embedding=EXCLUDED.embedding, metadata=EXCLUDED.metadata, "
                    "source_type=EXCLUDED.source_type, updated_at=CURRENT_TIMESTAMP;\n")
        f.write("COMMIT;\n")
    print(f"staged {n} upserts -> {args.out} ({args.out.stat().st_size//1024} KB)", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
