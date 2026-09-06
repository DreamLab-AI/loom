//! The immutable loaded-bundle vocabulary (ADR-135 closeout).
//!
//! The estate review's finding: `MirrorStore::current()` reads disk metadata on
//! every call, while the retriever and the graph load their CONTENT once at
//! startup. After a promotion the reported generation can therefore advance
//! while the served content has not — availability outliving grounding, in the
//! most literal way. The mirror script's file-at-a-time `os.replace` makes the
//! same window visible from the other side: a reader between two replaces sees
//! a mixed set.
//!
//! The fix has one shape: **the generation is a property of the loaded content,
//! not of the directory it came from.** A process stages, verifies and then
//! ACTIVATES exactly one bundle; that bundle's identity — its generation plus
//! the digest of every artefact actually read — is captured at load and is
//! immutable for the life of the process. Every serving surface reports that
//! captured identity. A later promotion on disk changes the disk, and nothing
//! else, until the process reloads.
//!
//! This module holds the pure vocabulary: the four [`BundlePhase`] states the
//! review asked to be distinguishable, the [`ServingIdentity`] that names one
//! activated bundle, and the [`BundleError`] cases activation must reject. The
//! filesystem work lives in `loom-facade::bundle`.

use serde::{Deserialize, Serialize};

use crate::model::{ArtifactSha, Generation, GenerationId};

/// The lifecycle stage a generation has reached, as the review required:
/// "generation reporting should distinguish downloaded, validated, activated and
/// served state".
///
/// The distinction is not decorative. A successful download says nothing about
/// hash agreement; a successful validation says nothing about whether a process
/// loaded it; and an activated bundle is only *served* once a request has been
/// answered from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BundlePhase {
    /// Bytes are on disk in a staging area. Nothing is verified.
    Downloaded,
    /// Every artefact's digest matched the commit marker. Not yet loaded.
    Validated,
    /// One process has loaded this bundle's content and captured its identity.
    Activated,
    /// The activated bundle has answered at least one request.
    Served,
}

/// The immutable identity of ONE activated bundle.
///
/// `content_digest` is the discriminator the review asked for: a digest over the
/// (name, sha256) pairs of the artefacts this process actually loaded. Two
/// processes reporting the same `generation` but different `content_digest` are
/// serving different bytes, and that is now visible rather than inferred.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServingIdentity {
    /// The generation descriptor resolved AT LOAD, frozen.
    pub generation: Generation,
    /// Digest over the loaded artefact set — see [`Self::digest_of`].
    pub content_digest: String,
    /// The artefacts whose bytes this identity covers, with the digests observed
    /// at load (not the ones the marker claimed — those were checked against
    /// these before activation).
    pub artefacts: Vec<ArtifactSha>,
    /// When this process activated the bundle (RFC 3339, UTC).
    pub activated_at: String,
    /// The phase this identity has reached in THIS process. Never regresses.
    pub phase: BundlePhase,
    /// Whether activation re-hashed every artefact and found agreement. False
    /// only for a bundle activated in explicitly-degraded mode.
    pub atomicity_verified: bool,
}

impl ServingIdentity {
    /// The content digest over an artefact set: `sha256` of the newline-joined
    /// `name:sha256` pairs, sorted by name so the digest is order-independent.
    ///
    /// Takes the pairs rather than doing the hashing itself — the domain crate
    /// holds no dependencies, so the caller passes a hasher's output through
    /// [`Self::digest_input`]. See `loom-facade::bundle` for the one call site.
    #[must_use]
    pub fn digest_input(artefacts: &[ArtifactSha]) -> String {
        let mut pairs: Vec<String> = artefacts
            .iter()
            .map(|a| format!("{}:{}", a.name, a.sha256))
            .collect();
        pairs.sort();
        pairs.join("\n")
    }

    /// The generation id this identity serves — the ONE answer every surface
    /// must give.
    #[must_use]
    pub fn generation_id(&self) -> &GenerationId {
        &self.generation.id
    }

    /// Whether another identity is the same loaded bundle: same generation AND
    /// same content. Generation equality alone is what the review found
    /// insufficient.
    #[must_use]
    pub fn is_same_bundle(&self, other: &Self) -> bool {
        self.generation.id == other.generation.id && self.content_digest == other.content_digest
    }

    /// Advance the phase, never regressing (a `Served` bundle stays served).
    pub fn advance_to(&mut self, phase: BundlePhase) {
        let rank = |p: BundlePhase| match p {
            BundlePhase::Downloaded => 0_u8,
            BundlePhase::Validated => 1,
            BundlePhase::Activated => 2,
            BundlePhase::Served => 3,
        };
        if rank(phase) > rank(self.phase) {
            self.phase = phase;
        }
    }
}

/// The typed ways a bundle fails to become servable.
///
/// Every variant is a REJECTION, not a degrade: unlike an absent accelerator, a
/// bundle that does not verify must never be activated, because the whole point
/// of the identity is that it can be trusted once reported.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum BundleError {
    /// The commit marker lists an artefact that is not on disk — the signature
    /// of an incomplete download or a promotion that stopped part-way.
    #[error("bundle incomplete: {name:?} is listed in the commit marker but absent from {dir:?}")]
    MissingArtefact { name: String, dir: String },

    /// An artefact's bytes do not hash to what the marker recorded — a mixed
    /// set, the file-at-a-time promotion window made visible.
    #[error("bundle mixed: {name:?} sha256 {got:?} != recorded {want:?}")]
    HashMismatch {
        name: String,
        got: String,
        want: String,
    },

    /// There is no commit marker at all, but artefacts are present — a
    /// promotion interrupted before it wrote the marker it commits with.
    #[error("bundle uncommitted: artefacts present in {dir:?} but no commit marker")]
    NoCommitMarker { dir: String },

    /// The marker exists but lists nothing to verify. Accepting it would let
    /// "verified" mean "there was a file", the exact weakness the review named
    /// in `verify_atomicity`'s no-artefacts success case.
    #[error("bundle unverifiable: commit marker in {dir:?} records no artefact digests")]
    EmptyManifest { dir: String },

    /// The marker itself is unreadable or malformed.
    #[error("bundle marker unreadable in {dir:?}: {detail}")]
    MarkerUnreadable { dir: String, detail: String },

    /// An artefact could not be read during verification.
    #[error("bundle artefact {name:?} unreadable: {detail}")]
    ArtefactUnreadable { name: String, detail: String },

    /// Post-activation drift: an artefact's bytes changed underneath a running
    /// process. The loaded bundle is still consistent (it is in memory); the
    /// DISK no longer matches it, and that must be reported, not hidden.
    #[error("serving drift: {name:?} on disk no longer matches the activated bundle")]
    ActivatedDrift { name: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::GenerationSource;

    fn art(name: &str, sha: &str) -> ArtifactSha {
        ArtifactSha {
            name: name.to_owned(),
            sha256: sha.to_owned(),
            bytes: 1,
        }
    }

    fn identity(gen: &str, digest: &str) -> ServingIdentity {
        ServingIdentity {
            generation: Generation {
                id: GenerationId(gen.to_owned()),
                source: GenerationSource::MirrorManifest,
                generated_at: Some(gen.to_owned()),
                commit_sha: None,
                promoted_at: None,
                cluster_span_seconds: None,
                artifacts: Vec::new(),
                verified_single_generation: true,
                class_count: None,
            },
            content_digest: digest.to_owned(),
            artefacts: Vec::new(),
            activated_at: "2026-09-05T00:00:00Z".to_owned(),
            phase: BundlePhase::Activated,
            atomicity_verified: true,
        }
    }

    #[test]
    fn digest_input_is_order_independent() {
        let a = ServingIdentity::digest_input(&[art("b.json", "22"), art("a.json", "11")]);
        let b = ServingIdentity::digest_input(&[art("a.json", "11"), art("b.json", "22")]);
        assert_eq!(a, b);
    }

    #[test]
    fn digest_input_changes_when_content_changes() {
        let a = ServingIdentity::digest_input(&[art("a.json", "11")]);
        let b = ServingIdentity::digest_input(&[art("a.json", "12")]);
        assert_ne!(a, b);
    }

    /// The review's core case: equal generation is NOT equal bundle.
    #[test]
    fn same_generation_different_content_is_a_different_bundle() {
        let a = identity("2026-09-05T00:00:00Z", "digest-a");
        let b = identity("2026-09-05T00:00:00Z", "digest-b");
        assert!(!a.is_same_bundle(&b));
        assert_eq!(a.generation_id(), b.generation_id());
    }

    #[test]
    fn phase_advances_but_never_regresses() {
        let mut id = identity("g", "d");
        id.advance_to(BundlePhase::Served);
        assert_eq!(id.phase, BundlePhase::Served);
        id.advance_to(BundlePhase::Downloaded);
        assert_eq!(id.phase, BundlePhase::Served, "phase must not regress");
    }

    #[test]
    fn bundle_errors_name_both_sides() {
        let e = BundleError::HashMismatch {
            name: "scaffold-index.json".to_owned(),
            got: "aa".to_owned(),
            want: "bb".to_owned(),
        };
        let msg = e.to_string();
        assert!(msg.contains("aa") && msg.contains("bb"), "{msg}");
    }
}
