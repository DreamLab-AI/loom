//! `POST /loom/attest` and `GET /loom/attest/verify` — the governance ledger
//! route (ADR-141, sovereign-corpus contract C5).
//!
//! # What this route is for
//!
//! The corpus is promoted by a human, not by the node: a forum `31403`
//! ActionResponse is applied by agentbox as a `vault edit`, and the decision
//! itself is recorded here. The ledger is the **audit trail of who admitted
//! what**, chain-hashed so a decision cannot be quietly removed or rewritten
//! after the fact. It is not an approval mechanism — by the time a decision
//! reaches this route it has already been made and applied.
//!
//! # Why it is behind a feature
//!
//! `AttestationLedger` was a build/CI-time port: nothing on the serving path
//! wrote to it. This route puts one writer on the serving path, so it compiles
//! only with the `attest` feature (on by default) and a deployment that wants a
//! strictly read-only node can build it out entirely. The chain persists to
//! `LOOM_LEDGER_PATH` (default `data/ledger.jsonl`) beside the corpus, so it
//! survives the restart that every reload is.
//!
//! # Honesty
//!
//! `passed` on a ledger entry means *this decision admitted content into the
//! served corpus* — true for `promote` and false for `demote`, `reject` and
//! `expired`. A demotion is a change, but it is not an admission, and
//! collapsing the two would make the ledger unable to answer the only question
//! it exists to answer: what is in the corpus because someone said so.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

use loom_domain::{AttestationLedger, GateVerdict, Iri, LoomError};

use crate::error::ApiError;
use crate::state::AppState;

/// The outcome a human signed on the forum (contract C5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DecisionOutcome {
    /// `Promote{iri}` — the page becomes `status: stable` and gains a human
    /// `verified` attestation. The only outcome that admits content.
    Promote,
    /// `Demote{iri}` — the page becomes `status: deprecated`.
    Demote,
    /// The proposal was rejected; nothing was written to the corpus.
    Reject,
    /// The proposal's `stale_after` passed unsigned and it was discarded.
    Expired,
}

impl DecisionOutcome {
    /// The wire spelling, reused as the ledger predicate's suffix.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Promote => "promote",
            Self::Demote => "demote",
            Self::Reject => "reject",
            Self::Expired => "expired",
        }
    }

    /// Whether this decision admitted content into the served corpus.
    #[must_use]
    pub fn admits(self) -> bool {
        matches!(self, Self::Promote)
    }
}

/// `POST /loom/attest` body.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AttestRequest {
    /// The governance case — the forum `31402` request this answers.
    pub case_id: String,
    /// The `PatchProposal` digest the decision was made against, so the ledger
    /// records WHAT was signed and not merely that something was.
    pub digest: String,
    /// What was decided.
    pub outcome: DecisionOutcome,
    /// The signer's actor URI (`human:<npub>`).
    pub signer: String,
    /// When they signed it, RFC 3339.
    pub at: String,
}

/// `POST /loom/attest` 201 body.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AttestResponse {
    /// The appended entry's hash — its identity in the chain.
    pub entry_id: String,
    /// The chain head after the append. Equal to `entry_id` for the entry just
    /// written; quoted separately because a caller reconciling several
    /// decisions compares heads, not entries.
    pub chain_head: String,
}

/// `GET /loom/attest/verify` body.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct VerifyResponse {
    /// Whether the chain re-hashes end-to-end AND its tail matches the
    /// checkpoint (so truncation is caught, not just tampering).
    pub ok: bool,
    /// How many entries the chain holds.
    pub length: usize,
}

/// Record one human-signed governance decision.
pub(super) async fn attest(State(st): State<AppState>, Json(req): Json<AttestRequest>) -> Response {
    if req.case_id.trim().is_empty() || req.digest.trim().is_empty() {
        return ApiError(LoomError::BadQuery(
            "case_id and digest are required".to_owned(),
        ))
        .into_response();
    }
    let verdict = GateVerdict {
        predicate: format!("governance-decision:{}", req.outcome.as_str()),
        passed: req.outcome.admits(),
        detail: Some(
            json!({ "digest": req.digest, "signer": req.signer, "at": req.at }).to_string(),
        ),
        // The case is the subject: the ledger answers "what happened to case X",
        // and the IRI the case is about is inside the proposal the digest pins.
        subject: Some(Iri::new(req.case_id)),
    };

    let entry_id = match st.ledger.attest(&verdict).await {
        Ok(id) => id.0,
        Err(e) => return ApiError::from(e).into_response(),
    };
    let chain_head = match st.ledger.head() {
        Ok(Some(head)) => head.entry_sha256,
        // An append that left no checkpoint is a broken ledger, not a partial
        // success: say so rather than answering 201 with a hole in it.
        Ok(None) => {
            return ApiError(LoomError::Attest(
                "ledger appended but wrote no chain head".to_owned(),
            ))
            .into_response()
        }
        Err(e) => return ApiError::from(e).into_response(),
    };

    (
        StatusCode::CREATED,
        Json(AttestResponse {
            entry_id,
            chain_head,
        }),
    )
        .into_response()
}

/// Re-hash the whole chain and report its length.
pub(super) async fn verify(State(st): State<AppState>) -> Response {
    let ok = match st.ledger.verify_chain().await {
        Ok(ok) => ok,
        Err(e) => return ApiError::from(e).into_response(),
    };
    let length = match st.ledger.entry_count() {
        Ok(n) => n,
        Err(e) => return ApiError::from(e).into_response(),
    };
    Json(VerifyResponse { ok, length }).into_response()
}
