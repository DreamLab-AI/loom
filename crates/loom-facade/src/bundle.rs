//! `LoadedBundle` — the immutable serving identity (ADR-135 closeout).
//!
//! # The finding this module answers
//!
//! `MirrorStore::current()` reads the data directory on every call, while the
//! lexical retriever and the graph store read their CONTENT once, at startup.
//! The two can therefore disagree: after `app/mirror.sh` promotes, the reported
//! generation advances while the served content does not. And because promotion
//! replaces artefacts one `os.replace` at a time, a reader arriving mid-promote
//! sees a set that is neither the old generation nor the new one.
//!
//! # The shape of the fix
//!
//! A generation is a property of **loaded content**, not of a directory. So:
//!
//! 1. **Stage → verify → activate.** Activation reads the commit marker,
//!    re-hashes every artefact it lists, and only then captures an identity.
//!    An incomplete download (a listed artefact missing), a mixed set (a digest
//!    that disagrees) and an interrupted promotion (artefacts present, no
//!    marker) are three distinct typed rejections — see [`BundleError`].
//! 2. **The identity is immutable for the life of the process.** Every serving
//!    surface — `/health`, `/loom/generation`, and the `generation` field on
//!    every grounding object — reports the LOADED identity. A promotion that
//!    lands after activation changes the disk and nothing else.
//! 3. **Reload is a process boundary, explicitly.** There is no reload endpoint
//!    and this module does not add one: the way to serve a new bundle is to
//!    restart onto it. [`LoadedBundle::disk_matches_loaded`] makes the pending
//!    difference visible so an operator knows a restart is owed.
//! 4. **`verify_atomicity` runs on the serving path.** Activation calls it
//!    (that is what "verify" above means), and `GET /loom/generation` re-runs it
//!    against the captured digests, so post-activation tampering surfaces as
//!    [`BundleError::ActivatedDrift`] instead of being invisible. The old
//!    "success when there is nothing to check" hole is closed: an activation
//!    with a marker that records no digests is [`BundleError::EmptyManifest`].
//!
//! # Degraded activation
//!
//! A data directory with no commit marker at all — a development checkout, the
//! test fixtures, a pre-mirror deployment — is not an error; there is simply
//! nothing promoted to verify. [`LoadedBundle::activate_or_degraded`] then
//! captures an identity over the artefacts it can actually see, hashing their
//! real bytes, and records `atomicity_verified: false`. The identity is still
//! content-bound (the digest covers bytes that were read), it is just not
//! publisher-attested. What it must never do is claim verification it did not
//! perform.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use loom_domain::{
    ArtifactSha, BundleError, BundlePhase, Generation, GenerationStore, LoomError, ServingIdentity,
};

use crate::mirror::{hex_sha256, MirrorStore};

/// The in-flight sentinel this module's promoter writes for the whole duration
/// of a swap. Its presence means "the target directory is mid-promotion"; an
/// activation that sees it refuses, whatever else the directory contains. It is
/// the explicit form of the guarantee the per-file `os.replace` sequence could
/// only imply.
pub const IN_FLIGHT_MARKER: &str = ".promotion-in-flight";

/// Artefacts a degraded (marker-less) activation hashes when it finds them, so
/// the captured identity is still bound to real bytes. The scaffold index is
/// added separately (its filename is configurable); these are the fixed names
/// the mirror and the exporter write.
const DEGRADED_CANDIDATES: &[&str] = &[
    "prose-index.json",
    "ontology.ttl",
    "ontology-inferred.ttl",
    "ontology-corpus.rvdb",
    "ontology-corpus.rvdb.generation.json",
];

/// One activated bundle: the identity captured at load, plus the disk view it
/// was captured from (kept so the two can be compared without a second
/// resolution path).
///
/// Implements [`GenerationStore`], and its `current()` returns the LOADED
/// generation — the single behavioural change the closeout turns on.
#[derive(Debug)]
pub struct LoadedBundle {
    identity: ServingIdentity,
    /// The disk view, for `disk_generation()` / `disk_matches_loaded()`.
    disk: MirrorStore,
    /// Set the first time this bundle answers a request, so the reported phase
    /// can distinguish `activated` from `served`.
    served: AtomicBool,
}

impl LoadedBundle {
    /// Strict activation: verify the promoted bundle, or refuse to serve it.
    ///
    /// # Errors
    /// Every [`BundleError`] case — a missing artefact, a digest disagreement,
    /// an uncommitted directory, an empty or unreadable marker.
    pub fn activate(index_path: &str) -> Result<Self, BundleError> {
        let disk = MirrorStore::new(index_path);
        reject_if_in_flight(&disk)?;
        let observed = verify_marker(&disk)?;
        Ok(Self::capture(disk, observed, true))
    }

    /// Activate strictly when there is a commit marker, and fall back to a
    /// content-bound but unattested identity when there is not.
    ///
    /// This is the composition root's entry point. It fails ONLY where strict
    /// activation found a real inconsistency — a marker that lists artefacts
    /// which are missing or which disagree. An absent marker is a degrade, not
    /// a rejection, because there is nothing promoted to be inconsistent with.
    ///
    /// # Errors
    /// The strict cases above, when a marker is present.
    pub fn activate_or_degraded(index_path: &str) -> Result<Self, BundleError> {
        let disk = MirrorStore::new(index_path);
        // An interrupted promotion is a rejection whatever else is present: the
        // sentinel says the directory is being rewritten right now, so no set
        // read from it is a bundle.
        reject_if_in_flight(&disk)?;
        if disk.has_commit_marker() {
            let observed = verify_marker(&disk)?;
            return Ok(Self::capture(disk, observed, true));
        }
        let observed = hash_present_artefacts(&disk);
        Ok(Self::capture(disk, observed, false))
    }

    /// A bundle over an in-memory identity — the seam tests build a façade
    /// through without touching a filesystem promotion.
    #[must_use]
    pub fn from_identity(index_path: &str, identity: ServingIdentity) -> Self {
        Self {
            identity,
            disk: MirrorStore::new(index_path),
            served: AtomicBool::new(false),
        }
    }

    fn capture(disk: MirrorStore, observed: Vec<ArtifactSha>, verified: bool) -> Self {
        // Resolve the generation ONCE, here, from the same directory whose bytes
        // we just hashed — after this point nothing re-reads it.
        let generation = <MirrorStore as GenerationStore>::current(&disk);
        let content_digest = hex_sha256(ServingIdentity::digest_input(&observed).as_bytes());
        Self {
            identity: ServingIdentity {
                generation,
                content_digest,
                artefacts: observed,
                activated_at: rfc3339_utc_now(),
                phase: BundlePhase::Activated,
                atomicity_verified: verified,
            },
            disk,
            served: AtomicBool::new(false),
        }
    }

    /// The frozen serving identity. Cheap; no I/O.
    #[must_use]
    pub fn identity(&self) -> &ServingIdentity {
        &self.identity
    }

    /// The identity with its phase advanced to `served` once this bundle has
    /// answered something — the reporting distinction the review asked for.
    #[must_use]
    pub fn reported_identity(&self) -> ServingIdentity {
        let mut id = self.identity.clone();
        if self.served.load(Ordering::Relaxed) {
            id.advance_to(BundlePhase::Served);
        }
        id
    }

    /// Mark this bundle as having answered a request. Idempotent, lock-free.
    pub fn mark_served(&self) {
        self.served.store(true, Ordering::Relaxed);
    }

    /// What the data directory says NOW — deliberately separate from
    /// [`Self::identity`]. A difference means a promotion has landed that this
    /// process has not activated.
    #[must_use]
    pub fn disk_generation(&self) -> Generation {
        <MirrorStore as GenerationStore>::current(&self.disk)
    }

    /// Whether the disk still shows the generation this process loaded. `false`
    /// ⇒ a restart is owed before the promoted bundle is served.
    #[must_use]
    pub fn disk_matches_loaded(&self) -> bool {
        self.disk_generation().id == self.identity.generation.id
    }

    /// Re-hash every artefact the identity covers and compare with the digests
    /// captured at activation.
    ///
    /// This is `verify_atomicity` with teeth: it verifies against what THIS
    /// PROCESS loaded, so it detects post-activation tampering as well as a
    /// mixed promotion. An identity that covers no artefacts (nothing was on
    /// disk to hash) reports [`BundleError::EmptyManifest`] rather than success,
    /// closing the "verified because there was nothing to verify" hole.
    ///
    /// # Errors
    /// [`BundleError::ActivatedDrift`] on a changed artefact,
    /// [`BundleError::MissingArtefact`] on one that has been removed.
    pub fn verify_loaded(&self) -> Result<(), BundleError> {
        let dir = self.disk.data_dir();
        if self.identity.artefacts.is_empty() {
            return Err(BundleError::EmptyManifest {
                dir: dir.display().to_string(),
            });
        }
        for art in &self.identity.artefacts {
            let path = dir.join(&art.name);
            let Ok(bytes) = std::fs::read(&path) else {
                return Err(BundleError::MissingArtefact {
                    name: art.name.clone(),
                    dir: dir.display().to_string(),
                });
            };
            if hex_sha256(&bytes) != art.sha256 {
                return Err(BundleError::ActivatedDrift {
                    name: art.name.clone(),
                });
            }
        }
        Ok(())
    }
}

#[async_trait]
impl GenerationStore for LoadedBundle {
    /// The LOADED generation — never a fresh disk read. This is the whole
    /// point: `/health`, `/loom/generation`, the `loom` telemetry block and
    /// every grounding object now agree, and agree with the content in memory.
    fn current(&self) -> Generation {
        self.identity.generation.clone()
    }

    /// Verify on the serving path, against the captured digests.
    async fn verify_atomicity(&self) -> Result<(), LoomError> {
        self.verify_loaded().map_err(LoomError::from)
    }
}

// --- promotion --------------------------------------------------------------

/// Stage → hash-verify → swap, as one operation with a commit marker.
///
/// The mirror script's weakness was that its per-file `os.replace` sequence has
/// no point before which a reader sees only the old set and after which it sees
/// only the new one. This promoter restores that point by making the commit
/// marker the LAST write and the FIRST removal:
///
/// 1. verify the STAGING set completely (nothing is touched in the target if
///    the staging bundle is incomplete or mixed);
/// 2. remove the target's commit marker — from here the target is explicitly
///    uncommitted, and an activation against it fails
///    [`BundleError::NoCommitMarker`] rather than loading a half-swapped set;
/// 3. rename each artefact into place through a `.incoming` temporary (each
///    rename atomic, and on the same filesystem);
/// 4. rename the marker in last. That rename IS the commit.
///
/// A crash at any point leaves the target either committed-old (before 2) or
/// uncommitted (2–4). Uncommitted is refused by activation, so no process can
/// serve a partially promoted set.
#[derive(Debug, Clone)]
pub struct BundlePromoter {
    staging: PathBuf,
    target: PathBuf,
}

impl BundlePromoter {
    #[must_use]
    pub fn new(staging: impl Into<PathBuf>, target: impl Into<PathBuf>) -> Self {
        Self {
            staging: staging.into(),
            target: target.into(),
        }
    }

    /// Run the promotion. Returns the artefact digests committed.
    ///
    /// # Errors
    /// The staging bundle's own [`BundleError`] cases, plus
    /// [`BundleError::ArtefactUnreadable`] if the swap itself fails.
    pub fn promote(&self) -> Result<Vec<ArtifactSha>, BundleError> {
        let staged = MirrorStore::new(&self.staging.join("scaffold-index.json").to_string_lossy());
        // (1) Verify the staging set completely BEFORE touching the target.
        let verified = verify_marker(&staged)?;

        let marker = self.target.join(".generation.json");
        let in_flight = self.target.join(IN_FLIGHT_MARKER);
        // (2) Declare the swap, then uncommit the target. The sentinel goes down
        //     FIRST so there is no instant in which the directory is being
        //     rewritten without saying so.
        std::fs::write(&in_flight, self.staging.display().to_string().as_bytes()).map_err(|e| {
            BundleError::ArtefactUnreadable {
                name: IN_FLIGHT_MARKER.to_owned(),
                detail: format!("could not mark promotion in flight: {e}"),
            }
        })?;
        match std::fs::remove_file(&marker) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(BundleError::ArtefactUnreadable {
                    name: ".generation.json".to_owned(),
                    detail: format!("could not uncommit target: {e}"),
                })
            }
        }

        // (3) Swap each artefact through a same-directory temporary.
        for art in &verified {
            let from = self.staging.join(&art.name);
            let incoming = self.target.join(format!("{}.incoming", art.name));
            let to = self.target.join(&art.name);
            std::fs::copy(&from, &incoming).map_err(|e| BundleError::ArtefactUnreadable {
                name: art.name.clone(),
                detail: format!("stage copy failed: {e}"),
            })?;
            std::fs::rename(&incoming, &to).map_err(|e| BundleError::ArtefactUnreadable {
                name: art.name.clone(),
                detail: format!("swap failed: {e}"),
            })?;
        }

        // (4) The marker last — this rename is the commit.
        let marker_incoming = self.target.join(".generation.json.incoming");
        std::fs::copy(self.staging.join(".generation.json"), &marker_incoming).map_err(|e| {
            BundleError::ArtefactUnreadable {
                name: ".generation.json".to_owned(),
                detail: format!("marker stage failed: {e}"),
            }
        })?;
        std::fs::rename(&marker_incoming, &marker).map_err(|e| BundleError::ArtefactUnreadable {
            name: ".generation.json".to_owned(),
            detail: format!("commit failed: {e}"),
        })?;
        // (5) Clear the sentinel — the directory is a committed bundle again.
        std::fs::remove_file(&in_flight).map_err(|e| BundleError::ArtefactUnreadable {
            name: IN_FLIGHT_MARKER.to_owned(),
            detail: format!("could not clear in-flight marker: {e}"),
        })?;
        Ok(verified)
    }
}

// --- verification helpers ---------------------------------------------------

/// Verify every artefact the commit marker records, returning the OBSERVED
/// digests (which, having just been compared, equal the recorded ones).
fn verify_marker(store: &MirrorStore) -> Result<Vec<ArtifactSha>, BundleError> {
    let dir = store.data_dir();
    let dir_s = dir.display().to_string();

    if !store.has_commit_marker() {
        return Err(BundleError::MarkerUnreadable {
            dir: dir_s,
            detail: format!("{} absent or malformed", store.commit_marker_path().display()),
        });
    }
    let recorded = store
        .marker_artifacts()
        .ok_or_else(|| BundleError::MarkerUnreadable {
            dir: dir_s.clone(),
            detail: "commit marker did not parse".to_owned(),
        })?;
    if recorded.is_empty() {
        return Err(BundleError::EmptyManifest { dir: dir_s });
    }

    let mut observed = Vec::with_capacity(recorded.len());
    for art in recorded {
        let path = dir.join(&art.name);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(BundleError::MissingArtefact {
                    name: art.name,
                    dir: dir_s,
                })
            }
            Err(e) => {
                return Err(BundleError::ArtefactUnreadable {
                    name: art.name,
                    detail: e.to_string(),
                })
            }
        };
        let got = hex_sha256(&bytes);
        if got != art.sha256 {
            return Err(BundleError::HashMismatch {
                name: art.name,
                got,
                want: art.sha256,
            });
        }
        observed.push(ArtifactSha {
            name: art.name,
            sha256: got,
            bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        });
    }
    Ok(observed)
}

/// Refuse a directory that is mid-promotion. Checked before anything is read,
/// because a set sampled during a swap is not a bundle whatever it hashes to.
fn reject_if_in_flight(store: &MirrorStore) -> Result<(), BundleError> {
    if store.data_dir().join(IN_FLIGHT_MARKER).exists() {
        return Err(BundleError::NoCommitMarker {
            dir: store.data_dir().display().to_string(),
        });
    }
    Ok(())
}

/// Hash whatever mirror artefacts are actually present — the degraded path's
/// content binding. Order is deterministic (`BTreeMap`), so the digest is too.
fn hash_present_artefacts(store: &MirrorStore) -> Vec<ArtifactSha> {
    let dir = store.data_dir();
    let mut names: BTreeSet<String> = BTreeSet::new();
    names.insert(store.index_file().to_owned());
    for c in DEGRADED_CANDIDATES {
        names.insert((*c).to_owned());
    }
    names
        .into_iter()
        .filter_map(|name| {
            let bytes = std::fs::read(dir.join(&name)).ok()?;
            Some(ArtifactSha {
                name,
                sha256: hex_sha256(&bytes),
                bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            })
        })
        .collect()
}

/// RFC 3339 UTC stamp from the system clock, without pulling a date crate into
/// a workspace that has none. Civil-from-days is Howard Hinnant's algorithm.
#[must_use]
pub fn rfc3339_utc_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let tod = secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        tod / 3600,
        (tod % 3600) / 60,
        tod % 60
    )
}

/// Days-since-epoch → (year, month, day), UTC proleptic Gregorian.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (
        y,
        u32::try_from(m).unwrap_or(1),
        u32::try_from(d).unwrap_or(1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_from_days_matches_known_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1)); // leap-year boundary
    }

    #[test]
    fn rfc3339_stamp_is_well_formed() {
        let s = rfc3339_utc_now();
        assert_eq!(s.len(), 20, "{s}");
        assert!(s.ends_with('Z') && s.contains('T'), "{s}");
    }
}
