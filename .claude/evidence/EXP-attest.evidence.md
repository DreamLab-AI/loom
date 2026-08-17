# EXP-attest — chained attestation ledger is tamper-evident — EVIDENCE

**Verdict: PASS**

- Date (UTC): 2026-08-17T14:48:52Z
- Git SHA (evidence produced against): abb544b6ae76ebb1349696667d4c54fe90723387
- Toolchain: `cargo 1.97.0 (c980f4866 2026-06-30)` (container plain cargo, no rustup)
- Crate: `crates/loom-attest-proofgate` (`ChainedLedger` impl
  `loom_domain::AttestationLedger`); build/CI-time only, not on the serving path.
- Reference: RUST-ARCHITECTURE §11.5 (ADR-047 / ADR-136 D5).

## Assertions ↔ evidence

| # | Assertion | Test | Result |
|---|---|---|---|
| 1 | chain grows; seqs dense 0/1/2; `prev_sha256` links to prior `entry_sha256`; genesis prev = ""; returned `LedgerEntryId` == `entry_sha256`; ids distinct | `chain_grows_and_links` | ok |
| 2 | `verify_chain()` true on intact file | `chain_grows_and_links` (final assert) | ok |
| 3 | `verify_chain()` true on missing/empty file (empty chain is intact) | `verify_chain_true_on_missing_file` | ok |
| 4 | `verify_chain()` false after a byte-flip in a **mid-file** entry's hashed field | `verify_chain_false_after_byte_flip_mid_file` | ok |
| 5 | `attest()` deterministic given fixed inputs (injected `FixedClock`); differing `ts` ⇒ differing hash | `attest_is_deterministic_given_fixed_inputs` | ok |
| 6 | `GateVerdict` serialises with expected shape (`predicate/passed/detail/subject`, `Iri` → `urn:ngm:class:<slug>`) | `gate_verdict_serialises_with_expected_shape` | ok |

- **Entry shape:** `{seq, ts, predicate, passed, detail, subject, prev_sha256,
  entry_sha256}`, one JSON object per line (JSONL).
- **Chain hash:** `entry_sha256 = sha256(prev_sha256 || canonical-json(entry-sans-hashes))`,
  lower-hex, where the canonical projection is a fixed-field-order struct
  (`seq, ts, predicate, passed, detail, subject`) serialised compact — a pure
  function of the semantic fields, so it is reproducible.
- **Clock injection:** `Clock` trait; `SystemClock` (wall clock, prod default),
  `FixedClock(i64)` (tests). Path from `LOOM_ATTEST_LEDGER`
  (default `.attest/ledger.jsonl`).
- **verify_chain** re-derives the hash of every line and checks (a) dense
  monotonic `seq`, (b) the stored back-link equals the running chain head, and
  (c) the recomputed hash equals the stored one. Any tampered byte diverges (c).

## The ProofGate reality (the `attest` feature decision)

RUST-ARCHITECTURE §11.5 names RuVector `ProofGate<T>` / `MutationLedger` (ADR-047)
as the attestation substrate; the FROZEN feature wiring is
`attest = ["dep:ruvector-core"]`. **Those types are not in `ruvector-core`.**

rg evidence (run 2026-08-17 against the sibling workspace):

```
$ rg -c "ProofGate|MutationLedger" /home/devuser/workspace/ruvector/crates/ruvector-core/src
(no output; match count = 0)

$ rg -l "struct ProofGate|struct MutationLedger" /home/devuser/workspace/ruvector/crates | grep -v wasm
/home/devuser/workspace/ruvector/crates/ruvector-graph-transformer/src/proof_gated.rs
/home/devuser/workspace/ruvector/crates/ruvector-graph-transformer-node/src/transformer.rs

$ rg -n "pub struct WitnessLog|pub fn append\(|pub fn verify_chain" \
     /home/devuser/workspace/ruvector/crates/ruvector-core/src/agenticdb.rs
1148:pub struct WitnessLog<'a> {
1208:    pub fn append(&self, agent_id: &str, action_type: &str, details: &str) -> Result<String> {
1317:    pub fn verify_chain(&self) -> Result<bool> {
```

- `ProofGate<T>`/`MutationLedger` live in **`ruvector-graph-transformer`**
  (`proof_gated.rs`), built atop `ruvector-verified` — a different crate, outside
  the frozen `dep:ruvector-core` wiring.
- The only chain-hash primitive reachable from `ruvector-core` is
  `agenticdb::WitnessLog`. It is **unsuitable as an attestation anchor**: it
  hashes with the non-cryptographic `std::hash::DefaultHasher` (despite a
  "SHA256" doc-comment — see `agenticdb.rs:1186-1205`), and its `verify_chain`
  reconstructs the chain via placeholder-embedding vector search
  (`agenticdb.rs:1318`, `search("", 10000)`) — recall-dependent, not a byte-exact
  re-hash. Not a sound tamper anchor for a CI gate.

**Decision (per the mission's fallback path):** the DEFAULT sha2 `ChainedLedger`
is the complete `AttestationLedger` contract, always compiled. Under
`--features attest`, `ProofGateLedger` is a re-export of `ChainedLedger` (same
trait, same behaviour), and `ruvector-core` is re-exported to prove the frozen
`dep:ruvector-core` wiring compiles and links (verified below). Binding to the
real `ProofGate` is a one-line wiring change to `ruvector-graph-transformer` +
`ruvector-verified`, deferred out of this crate's frozen scope.

## Raw command tails

```
### $ cargo build -p loom-attest-proofgate
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.57s
[exit=0]

### $ cargo build -p loom-attest-proofgate --features attest
   Compiling ruvector-core v2.0.5 (/home/devuser/workspace/ruvector/crates/ruvector-core)
   Compiling loom-attest-proofgate v0.1.0 (/home/devuser/workspace/loom/crates/loom-attest-proofgate)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 19.92s
[exit=0]

### $ cargo test -p loom-attest-proofgate
running 5 tests
test gate_verdict_serialises_with_expected_shape ... ok
test verify_chain_true_on_missing_file ... ok
test attest_is_deterministic_given_fixed_inputs ... ok
test chain_grows_and_links ... ok
test verify_chain_false_after_byte_flip_mid_file ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
[exit=0]

### $ cargo test -p loom-attest-proofgate --all-features
running 5 tests
test gate_verdict_serialises_with_expected_shape ... ok
test verify_chain_true_on_missing_file ... ok
test attest_is_deterministic_given_fixed_inputs ... ok
test chain_grows_and_links ... ok
test verify_chain_false_after_byte_flip_mid_file ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
[exit=0]

### $ cargo clippy -p loom-attest-proofgate --all-targets --all-features -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.46s
[exit=0]
```

**Test count: 5 passed / 0 failed** (identical under default and `--all-features`).
Build (default + `--features attest`), test (default + `--all-features`), and
clippy (`-D warnings`, `--all-targets --all-features`) all exit 0.
