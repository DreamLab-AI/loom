//! loom-attest-proofgate — the `AttestationLedger` adapter (build/CI-time only,
//! never on the serving hot path).
//!
//! # What ships
//!
//! [`ChainedLedger`] — an append-only JSONL ledger whose entries are linked by
//! a SHA-256 chain hash. Each entry is
//! `{ seq, ts, predicate, passed, detail, subject, prev_sha256, entry_sha256 }`
//! where `entry_sha256 = sha256(prev_sha256 || canonical-json(entry-sans-hashes))`.
//! [`ChainedLedger::attest`] appends and returns the entry's
//! [`LedgerEntryId`](loom_domain::LedgerEntryId); [`ChainedLedger::verify_chain`]
//! re-hashes the file end-to-end and reports whether the chain is intact — a
//! single flipped byte anywhere breaks a hash and fails the check. Each
//! [`attest`](ChainedLedger::attest) also advances an atomically-written HEAD
//! checkpoint sidecar (`<ledger>.head` = `{seq, entry_sha256}` of the last
//! entry); `verify_chain` requires the on-disk tail to match it, so truncating
//! trailing entries — a valid-prefix attack the re-hash alone accepts — is
//! detected too (audit finding 4). This is the DEFAULT path, always compiled,
//! and it makes the `AttestationLedger` contract complete regardless of feature
//! selection.
//!
//! # The `attest` feature and the ProofGate reality
//!
//! The design (RUST-ARCHITECTURE §11.5, ADR-047/ADR-136) names RuVector's
//! `ProofGate<T>` / `MutationLedger` as the attestation substrate. The frozen
//! feature wiring is `attest = ["dep:ruvector-core"]`. **Those types are not in
//! `ruvector-core`.** Evidence, run against the sibling workspace:
//!
//! ```text
//! $ rg -n "struct ProofGate|struct MutationLedger" ruvector/crates/ruvector-core/src
//! (no matches)
//! $ rg -l "ProofGate|MutationLedger" ruvector/crates --files-with-matches | head -1
//! ruvector/crates/ruvector-graph-transformer/src/proof_gated.rs
//! ```
//!
//! `ProofGate<T>`/`MutationLedger` live in `ruvector-graph-transformer`
//! (`proof_gated.rs`), built atop `ruvector-verified` — a different crate,
//! outside the frozen `dep:ruvector-core` wiring. The only chain-hash primitive
//! reachable from `ruvector-core` is `agenticdb::WitnessLog`, which is unsuited
//! as an attestation anchor: it hashes with the non-cryptographic
//! `DefaultHasher` (despite a "SHA256" doc-comment), and its `verify_chain`
//! reconstructs the chain via placeholder-embedding vector search — recall-
//! dependent, not a byte-exact re-hash.
//!
//! So, per the mission's fallback: the `attest` feature **re-exports the sha2
//! ledger** ([`ProofGateLedger`] is a re-export of [`ChainedLedger`]) and links
//! `ruvector-core` to prove the frozen wiring compiles ([`ruvector_core`] is
//! re-exported under the feature). Binding to the real `ProofGate` is a
//! one-line wiring change (`ruvector-graph-transformer` + `ruvector-verified`)
//! deferred out of this crate's frozen scope.

// The module docs name product terms (JSONL, SHA-256, ProofGate, RuVector) and
// on-disk field identifiers (`prev_sha256`, …) that read as prose, not code —
// same rationale as loom-domain's blanket allow.
#![allow(clippy::doc_markdown)]

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use loom_domain::{AttestationLedger, GateVerdict, LedgerEntryId, LoomError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// `LOOM_ATTEST_LEDGER` default path (relative to the CI working directory).
const DEFAULT_LEDGER_PATH: &str = ".attest/ledger.jsonl";

/// The `prev_sha256` of the genesis (first) entry — an empty chain root.
const GENESIS_PREV: &str = "";

// --- clock injection --------------------------------------------------------

/// Time source for entry timestamps. Injected so tests can pin `ts` and make
/// [`ChainedLedger::attest`] deterministic given fixed inputs.
pub trait Clock: Send + Sync {
    /// Unix seconds.
    fn now_unix(&self) -> i64;
}

/// Wall-clock `Clock` (production default).
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_unix(&self) -> i64 {
        // Pre-1970 clocks are not a real deployment concern; clamp to 0.
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
    }
}

/// A fixed `Clock` — every `now_unix` returns the same second. Handy for
/// reproducible ledgers in tests and golden fixtures.
#[derive(Clone, Copy, Debug)]
pub struct FixedClock(pub i64);

impl Clock for FixedClock {
    fn now_unix(&self) -> i64 {
        self.0
    }
}

// --- the on-disk entry ------------------------------------------------------

/// One persisted ledger line. Field order is the canonical order; `serde`
/// serialises structs in declaration order, so the JSONL is stable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub seq: u64,
    pub ts: i64,
    pub predicate: String,
    pub passed: bool,
    pub detail: Option<String>,
    pub subject: Option<String>,
    pub prev_sha256: String,
    pub entry_sha256: String,
}

/// The atomically-written HEAD checkpoint (`<ledger>.head`): the seq and hash of
/// the last appended entry. `verify_chain` requires the on-disk tail to match it,
/// so truncating trailing entries (a valid-prefix attack) is detected even though
/// the surviving prefix re-hashes cleanly (audit finding 4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct HeadCheckpoint {
    seq: u64,
    entry_sha256: String,
}

/// The hashed projection of an entry — everything except the two hash fields,
/// in a fixed field order so `canonical-json` is deterministic.
#[derive(Serialize)]
struct CanonicalEntry<'a> {
    seq: u64,
    ts: i64,
    predicate: &'a str,
    passed: bool,
    detail: Option<&'a str>,
    subject: Option<&'a str>,
}

/// `entry_sha256 = sha256(prev_sha256 || canonical-json(entry-sans-hashes))`,
/// lower-hex. The canonical JSON is compact (no whitespace), so the hash is a
/// pure function of the semantic fields.
fn chain_hash(prev_sha256: &str, canonical: &CanonicalEntry<'_>) -> Result<String, LoomError> {
    let canon = serde_json::to_vec(canonical)
        .map_err(|e| LoomError::Attest(format!("canonicalise entry: {e}")))?;
    let mut hasher = Sha256::new();
    hasher.update(prev_sha256.as_bytes());
    hasher.update(&canon);
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        // Infallible into a String.
        let _ = write!(hex, "{byte:02x}");
    }
    Ok(hex)
}

// --- the ledger -------------------------------------------------------------

/// Append-only, SHA-256-chained JSONL attestation ledger.
#[derive(Clone)]
pub struct ChainedLedger {
    path: PathBuf,
    clock: Arc<dyn Clock>,
    /// Serialises appends within a process (the ledger is single-writer at
    /// CI-time, but a shared handle must not interleave reads-then-appends).
    write_lock: Arc<Mutex<()>>,
}

impl std::fmt::Debug for ChainedLedger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChainedLedger")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl ChainedLedger {
    /// Build from `LOOM_ATTEST_LEDGER` (default `.attest/ledger.jsonl`) on the
    /// wall clock.
    #[must_use]
    pub fn from_env() -> Self {
        let path =
            std::env::var("LOOM_ATTEST_LEDGER").unwrap_or_else(|_| DEFAULT_LEDGER_PATH.to_owned());
        Self::with_path(path)
    }

    /// Build at an explicit path on the wall clock.
    #[must_use]
    pub fn with_path(path: impl Into<PathBuf>) -> Self {
        Self::with_path_and_clock(path, Arc::new(SystemClock))
    }

    /// Build at an explicit path with an injected clock (deterministic tests).
    #[must_use]
    pub fn with_path_and_clock(path: impl Into<PathBuf>, clock: Arc<dyn Clock>) -> Self {
        Self {
            path: path.into(),
            clock,
            write_lock: Arc::new(Mutex::new(())),
        }
    }

    /// The ledger file path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The HEAD checkpoint sidecar path — the ledger path with `.head` appended
    /// (audit finding 4).
    #[must_use]
    pub fn head_path(&self) -> PathBuf {
        let mut os = self.path.clone().into_os_string();
        os.push(".head");
        PathBuf::from(os)
    }

    /// Write the HEAD checkpoint atomically (tmp file + rename), so a reader never
    /// observes a half-written head.
    fn write_head(&self, seq: u64, entry_sha256: &str) -> Result<(), LoomError> {
        let head = self.head_path();
        let mut tmp_os = head.clone().into_os_string();
        tmp_os.push(".tmp");
        let tmp = PathBuf::from(tmp_os);
        let checkpoint = HeadCheckpoint {
            seq,
            entry_sha256: entry_sha256.to_owned(),
        };
        let json = serde_json::to_string(&checkpoint)
            .map_err(|e| LoomError::Attest(format!("serialise head: {e}")))?;
        fs::write(&tmp, json.as_bytes())
            .map_err(|e| LoomError::Attest(format!("write head tmp: {e}")))?;
        fs::rename(&tmp, &head).map_err(|e| LoomError::Attest(format!("rename head: {e}")))
    }

    /// Read the HEAD checkpoint. A missing head is `None` (tamper vs fresh is
    /// decided by `verify_chain` against the ledger's own emptiness).
    fn read_head(&self) -> Result<Option<HeadCheckpoint>, LoomError> {
        match fs::read_to_string(self.head_path()) {
            Ok(s) => serde_json::from_str(&s)
                .map(Some)
                .map_err(|e| LoomError::Attest(format!("parse head: {e}"))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(LoomError::Attest(format!("open head: {e}"))),
        }
    }

    /// Read and parse every entry, in file order. A missing file is an empty
    /// (intact) chain. Blank lines are skipped.
    fn read_entries(&self) -> Result<Vec<LedgerEntry>, LoomError> {
        let file = match File::open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(LoomError::Attest(format!("open ledger: {e}"))),
        };
        let mut entries = Vec::new();
        for (i, line) in BufReader::new(file).lines().enumerate() {
            let line = line.map_err(|e| LoomError::Attest(format!("read ledger line {i}: {e}")))?;
            if line.trim().is_empty() {
                continue;
            }
            let entry: LedgerEntry = serde_json::from_str(&line)
                .map_err(|e| LoomError::Attest(format!("parse ledger line {i}: {e}")))?;
            entries.push(entry);
        }
        Ok(entries)
    }
}

#[async_trait::async_trait]
impl AttestationLedger for ChainedLedger {
    async fn attest(&self, verdict: &GateVerdict) -> Result<LedgerEntryId, LoomError> {
        // Hold the append lock across read-tail → hash → write so concurrent
        // handles cannot both compute against the same prev hash.
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| LoomError::Attest("ledger write lock poisoned".to_owned()))?;

        let existing = self.read_entries()?;
        let (seq, prev_sha256) = existing.last().map_or_else(
            || (0_u64, GENESIS_PREV.to_owned()),
            |last| (last.seq + 1, last.entry_sha256.clone()),
        );

        let ts = self.clock.now_unix();
        let subject = verdict.subject.as_ref().map(|iri| iri.as_str().to_owned());
        let canonical = CanonicalEntry {
            seq,
            ts,
            predicate: &verdict.predicate,
            passed: verdict.passed,
            detail: verdict.detail.as_deref(),
            subject: subject.as_deref(),
        };
        let entry_sha256 = chain_hash(&prev_sha256, &canonical)?;

        let entry = LedgerEntry {
            seq,
            ts,
            predicate: verdict.predicate.clone(),
            passed: verdict.passed,
            detail: verdict.detail.clone(),
            subject,
            prev_sha256,
            entry_sha256: entry_sha256.clone(),
        };

        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .map_err(|e| LoomError::Attest(format!("create ledger dir: {e}")))?;
            }
        }
        let mut line = serde_json::to_string(&entry)
            .map_err(|e| LoomError::Attest(format!("serialise entry: {e}")))?;
        line.push('\n');
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| LoomError::Attest(format!("open ledger for append: {e}")))?;
        file.write_all(line.as_bytes())
            .map_err(|e| LoomError::Attest(format!("append entry: {e}")))?;

        // Advance the HEAD checkpoint to this entry (audit finding 4). Written
        // under the same append lock, so head and tail never diverge.
        self.write_head(seq, &entry_sha256)?;

        Ok(LedgerEntryId(entry_sha256))
    }

    async fn verify_chain(&self) -> Result<bool, LoomError> {
        let entries = self.read_entries()?;
        let mut prev = GENESIS_PREV.to_owned();
        for (i, entry) in entries.iter().enumerate() {
            // seq is dense and monotonic from 0.
            if entry.seq != i as u64 {
                return Ok(false);
            }
            // The stored back-link must match the running chain head.
            if entry.prev_sha256 != prev {
                return Ok(false);
            }
            // Re-hash the semantic fields; any tampered byte diverges here.
            let canonical = CanonicalEntry {
                seq: entry.seq,
                ts: entry.ts,
                predicate: &entry.predicate,
                passed: entry.passed,
                detail: entry.detail.as_deref(),
                subject: entry.subject.as_deref(),
            };
            let recomputed = chain_hash(&entry.prev_sha256, &canonical)?;
            if recomputed != entry.entry_sha256 {
                return Ok(false);
            }
            prev.clone_from(&entry.entry_sha256);
        }

        // Terminal checkpoint (audit finding 4): a valid PREFIX is not enough —
        // the on-disk tail must equal the HEAD checkpoint, else trailing entries
        // were truncated. Empty/missing ledger with a missing head is fresh
        // (intact); a non-empty ledger with a missing head is tamper.
        match (entries.last(), self.read_head()?) {
            (None, _) => Ok(true),
            (Some(_), None) => Ok(false),
            (Some(last), Some(head)) => {
                Ok(last.seq == head.seq && last.entry_sha256 == head.entry_sha256)
            }
        }
    }
}

// --- the `attest` feature surface -------------------------------------------

/// Under `--features attest`, the ProofGate-backed ledger is a re-export of the
/// sha2 [`ChainedLedger`] — see the module docs for why `ruvector-core` cannot
/// supply `ProofGate<T>`/`MutationLedger`. Same trait, same behaviour.
#[cfg(feature = "attest")]
pub type ProofGateLedger = ChainedLedger;

/// Re-exported under `attest` to prove the frozen `dep:ruvector-core` wiring
/// compiles and links. The reachable attestation primitive it exposes is
/// `ruvector_core::agenticdb::WitnessLog` (see module docs).
#[cfg(feature = "attest")]
pub use ruvector_core;
