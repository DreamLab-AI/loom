# Bench harness — legacy-Python note (2026-08-17)

`bench_ontology_uplift.py`, `bench-uplift.py` and `bench-agentic-uplift.py`
import `app/ontology_scaffold.py`, which was retired with the Python serving
code (RUST-ARCHITECTURE §12 deprecation map). Their published results (the
preprint's data) are frozen in `uplift-results/`; to re-run them, check out the
pre-retirement tree (`git checkout eb678a0 -- app/ontology_scaffold.py`).

The behaviour they exercised is carried by the Rust workspace under test
coverage: `loom-scaffold` (byte-parity goldens, EXP-002; gate math, EXP-003)
and the criterion perf gate (EXP-010). `tests/test_confidence_injection.py`
was deleted for the same reason — the Rust policy table tests supersede it.
