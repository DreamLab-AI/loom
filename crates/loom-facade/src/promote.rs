//! Explicit, recorded promotion of a built bundle into the directory the
//! service reads (ADR-141 D6).
//!
//! Until this module existed nothing put a new closure in service: the
//! published `.generation.json` carried no `promoted_at` and nobody recorded who
//! installed it. [`promote`] is the one step that does, and it does it in this
//! order:
//!
//! 1. take a lock beside the promotion log (concurrent promotions are refused);
//! 2. **validate the candidate** without touching the data directory: the
//!    strict reload check ([`crate::reload::verified_bundle`]: six artefacts,
//!    hashes, sizes, semantic sidecar), an `ontomark build` marker that names
//!    its source commit, the scaffold index, prose index and both Turtle files
//!    actually parse, and the declared commit exists in the source repository;
//! 3. compare it with what is installed: the same generation is a no-op, an
//!    older one is refused unless `allow_rollback` is set;
//! 4. install it through [`BundlePromoter`] (sentinel, write beside, rename,
//!    marker last) with a marker that adds `promoted_at`, `promoted_by` and
//!    `previous_generation`;
//! 5. append one JSON line to the promotion log, then fsync it.
//!
//! Any failure in steps 1 to 3 leaves the installed generation untouched. The
//! service reads the stamped marker at start-up and reports it on `/health`
//! (`generation.promoted_at`, `promoted_by`, `previous_generation`).
//!
//! # What "older" means
//!
//! A generation id (`visionGraph@<sha>`) does not order. The candidate is older
//! when its commit is a strict ancestor of the installed one in `source_repo`.
//! When that cannot be established (the installed marker is legacy and names no
//! commit, or its commit is not in the repository), the candidate is refused
//! without `allow_rollback` rather than guessed at.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use loom_domain::{GenerationStore, GraphStore, LexicalIndex};
use loom_graph_oxigraph::OxigraphStore;
use loom_scaffold::LexicalRetriever;
use serde_json::{json, Value};

use crate::bundle::{rfc3339_utc_now, BundlePromoter, IN_FLIGHT_MARKER};
use crate::mirror::MirrorStore;
use crate::reload::verified_bundle;

/// Everything [`promote`] needs. All paths are used as given.
#[derive(Debug, Clone)]
pub struct PromoteOptions {
    /// The built bundle: the `data/` directory `ontomark build` wrote, holding
    /// `.generation.json`. It is only read.
    pub bundle: PathBuf,
    /// The directory the service reads (`/app/data` in the container).
    pub data_dir: PathBuf,
    /// A checkout of the source repository (visionGraph) the bundle was built
    /// from. The bundle's commit must exist in it.
    pub source_repo: PathBuf,
    /// The promotion log (JSON Lines, append only).
    pub log: PathBuf,
    /// Who or what is promoting; recorded in the marker and the log.
    pub promoted_by: String,
    /// Allow installing a bundle that is older than, or cannot be ordered
    /// against, the installed one.
    pub allow_rollback: bool,
    /// The promotion time as RFC 3339 UTC; the system clock when `None`.
    pub now: Option<String>,
}

/// One line of the promotion log.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PromotionRecord {
    /// The generation id now in service (`visionGraph@<sha>`).
    pub generation: String,
    /// The source repository name: the part of the id before `@`.
    pub source_repository: String,
    /// The `origin` URL of the source checkout, when it has one.
    pub source_remote: Option<String>,
    /// The source commit the bundle was built from.
    pub commit: String,
    /// The compiler identity the bundle's marker recorded, if any.
    pub built_by: Option<Value>,
    /// When it was promoted (RFC 3339 UTC).
    pub promoted_at: String,
    /// Who or what promoted it.
    pub promoted_by: String,
    /// The generation it replaced, if one was installed and verified.
    pub previous_generation: Option<String>,
    /// The artefact digest of the installed bundle.
    pub content_digest: String,
    /// The bundle directory it was installed from; a rollback re-promotes the
    /// previous line's `bundle` with `allow_rollback`.
    pub bundle: String,
}

/// What a successful [`promote`] did.
#[derive(Debug, Clone, PartialEq)]
pub enum PromoteOutcome {
    /// The bundle was installed and logged.
    Promoted(Box<PromotionRecord>),
    /// The same generation is already installed and verified: nothing changed
    /// and no log line was written.
    Current(String),
}

/// Why a promotion did not happen.
#[derive(Debug)]
pub enum PromoteError {
    /// The candidate or the request was refused before anything was installed.
    /// The installed generation is untouched.
    Refused(String),
    /// Installation started and failed. The data directory is left explicitly
    /// uncommitted (the service refuses to start on it); re-run with a good
    /// bundle to repair it.
    Install(String),
    /// The bundle was installed but the log line could not be written.
    Log(String),
}

impl std::fmt::Display for PromoteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused(why) => write!(f, "refused: {why}"),
            Self::Install(why) => write!(
                f,
                "install failed, the data directory is left uncommitted: {why}"
            ),
            Self::Log(why) => write!(f, "installed, but the promotion log was not written: {why}"),
        }
    }
}

impl std::error::Error for PromoteError {}

fn refuse(why: impl Into<String>) -> PromoteError {
    PromoteError::Refused(why.into())
}

/// Promote `opts.bundle` into `opts.data_dir`. See the module documentation.
///
/// # Errors
/// [`PromoteError::Refused`] for every validation failure (nothing installed),
/// [`PromoteError::Install`] if the swap itself fails, [`PromoteError::Log`] if
/// the bundle went in but the log append failed.
pub fn promote(opts: &PromoteOptions) -> Result<PromoteOutcome, PromoteError> {
    if let Some(parent) = opts.log.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|e| refuse(format!("{}: {e}", parent.display())))?;
    }
    let mut lock_path = opts.log.as_os_str().to_owned();
    lock_path.push(".lock");
    let lock = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&lock_path)
        .map_err(|e| refuse(format!("{}: {e}", Path::new(&lock_path).display())))?;
    lock.try_lock()
        .map_err(|e| refuse(format!("another promotion holds the lock: {e}")))?;

    let candidate = Candidate::read(&opts.bundle, &opts.source_repo)?;
    let installed = Installed::read(&opts.data_dir);

    if let Some(installed) = &installed {
        if installed.id == candidate.id {
            if installed.verified {
                return Ok(PromoteOutcome::Current(candidate.id));
            }
            // The same generation, but the installed copy no longer verifies:
            // fall through and reinstall it. That repairs, it does not regress.
        } else if !opts.allow_rollback {
            match is_older(
                &opts.source_repo,
                &candidate.commit,
                installed.commit.as_deref(),
            ) {
                Some(false) => {}
                Some(true) => {
                    return Err(refuse(format!(
                        "{} is older than the installed {}; pass --allow-rollback to install it",
                        candidate.id, installed.id
                    )))
                }
                None => {
                    return Err(refuse(format!(
                        "cannot establish that {} is newer than the installed {} \
                         (the installed commit is unknown or absent from the source repository); \
                         pass --allow-rollback to install it",
                        candidate.id, installed.id
                    )))
                }
            }
        }
    }

    let promoted_at = opts.now.clone().unwrap_or_else(rfc3339_utc_now);
    let previous_generation = installed.map(|i| i.id);
    let stamped = candidate.stamped_marker(
        &promoted_at,
        &opts.promoted_by,
        previous_generation.as_deref(),
    );

    fs::create_dir_all(&opts.data_dir)
        .map_err(|e| PromoteError::Install(format!("{}: {e}", opts.data_dir.display())))?;
    BundlePromoter::new(&opts.bundle, &opts.data_dir)
        .promote_with_marker(&stamped)
        .map_err(|e| PromoteError::Install(e.to_string()))?;

    let record = PromotionRecord {
        source_repository: candidate
            .id
            .split_once('@')
            .map_or_else(|| candidate.id.clone(), |(repo, _)| repo.to_owned()),
        source_remote: git_output(&opts.source_repo, &["remote", "get-url", "origin"]),
        generation: candidate.id,
        commit: candidate.commit,
        built_by: candidate.built_by,
        promoted_at,
        promoted_by: opts.promoted_by.clone(),
        previous_generation,
        content_digest: candidate.content_digest,
        bundle: opts.bundle.display().to_string(),
    };
    append_log(&opts.log, &record).map_err(|e| PromoteError::Log(e.to_string()))?;
    Ok(PromoteOutcome::Promoted(Box::new(record)))
}

/// The candidate bundle, validated.
struct Candidate {
    id: String,
    commit: String,
    content_digest: String,
    built_by: Option<Value>,
    marker: Value,
}

impl Candidate {
    fn read(bundle: &Path, source_repo: &Path) -> Result<Self, PromoteError> {
        let verified = verified_bundle(bundle).map_err(|e| refuse(e.to_string()))?;
        let raw = fs::read(bundle.join(".generation.json"))
            .map_err(|e| refuse(format!(".generation.json: {e}")))?;
        let marker: Value =
            serde_json::from_slice(&raw).map_err(|e| refuse(format!(".generation.json: {e}")))?;
        if !marker.is_object() {
            return Err(refuse(".generation.json is not a JSON object"));
        }
        let commit = marker
            .get("commit")
            .and_then(Value::as_str)
            .filter(|c| !c.is_empty())
            .ok_or_else(|| {
                refuse("the marker is not an ontomark build marker: it declares no source commit")
            })?
            .to_owned();
        if commit.len() < 7 || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(refuse(format!(
                "the declared commit is not a hex sha: {commit}"
            )));
        }
        if !commit_exists(source_repo, &commit) {
            return Err(refuse(format!(
                "the declared source commit {commit} is not present in {}",
                source_repo.display()
            )));
        }
        parse_check(bundle)?;
        Ok(Self {
            id: verified.generation,
            commit,
            content_digest: verified.content_digest,
            built_by: marker.get("built_by").cloned(),
            marker,
        })
    }

    /// The staging marker with the promotion stamp, as the bytes to commit.
    fn stamped_marker(&self, at: &str, by: &str, previous: Option<&str>) -> Vec<u8> {
        let mut marker = self.marker.clone();
        if let Some(obj) = marker.as_object_mut() {
            obj.insert("promoted_at".to_owned(), json!(at));
            obj.insert("promoted_by".to_owned(), json!(by));
            match previous {
                Some(prev) => obj.insert("previous_generation".to_owned(), json!(prev)),
                None => obj.remove("previous_generation"),
            };
        }
        let mut bytes = serde_json::to_vec_pretty(&marker).unwrap_or_default();
        bytes.push(b'\n');
        bytes
    }
}

/// The generation named by the installed commit marker.
struct Installed {
    id: String,
    commit: Option<String>,
    /// Whether the installed bundle still passes the strict reload check.
    verified: bool,
}

impl Installed {
    /// `None` only when there is nothing to regress from: no readable marker,
    /// or an interrupted promotion (the sentinel is present). A marker whose
    /// artefacts no longer verify still names the generation the running
    /// process serves, so it still counts for ordering and for the log; it is
    /// merely not `verified`, which lets a same-generation promotion repair it.
    fn read(data_dir: &Path) -> Option<Self> {
        let store = MirrorStore::new(&data_dir.join("scaffold-index.json").to_string_lossy());
        if data_dir.join(IN_FLIGHT_MARKER).exists() || !store.has_commit_marker() {
            return None;
        }
        let current = store.current();
        Some(Self {
            id: current.id.0,
            commit: current.commit_sha,
            verified: verified_bundle(data_dir).is_ok(),
        })
    }
}

/// The checks that the files are the formats the service loads them as. The
/// semantic artefact is not opened: redb mutates a database on open, and its
/// sidecar was already checked against the generation, model and dimensions.
fn parse_check(bundle: &Path) -> Result<(), PromoteError> {
    let scaffold = fs::read_to_string(bundle.join("scaffold-index.json"))
        .map_err(|e| refuse(format!("scaffold-index.json: {e}")))?;
    let retriever = LexicalRetriever::from_json_str(&scaffold)
        .map_err(|e| refuse(format!("scaffold-index.json does not parse: {e}")))?;
    if retriever.class_count() == 0 {
        return Err(refuse("scaffold-index.json holds no classes"));
    }
    let prose = fs::read(bundle.join("prose-index.json"))
        .map_err(|e| refuse(format!("prose-index.json: {e}")))?;
    serde_json::from_slice::<Value>(&prose)
        .map_err(|e| refuse(format!("prose-index.json does not parse: {e}")))?;
    let status = OxigraphStore::load(bundle).status();
    if !status.available || status.loaded_files.len() < 2 {
        return Err(refuse(format!(
            "the Turtle files do not load: {}",
            status.error.unwrap_or_else(|| {
                "both ontology.ttl and ontology-inferred.ttl are required".to_owned()
            })
        )));
    }
    Ok(())
}

fn git_output(repo: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn git_succeeds(repo: &Path, args: &[&str]) -> Option<bool> {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()
        .and_then(|s| match s.code() {
            Some(0) => Some(true),
            Some(1) => Some(false),
            _ => None,
        })
}

/// `commit` must be hex (checked by the caller), so it cannot be read as an option.
fn commit_exists(repo: &Path, commit: &str) -> bool {
    git_succeeds(repo, &["cat-file", "-e", &format!("{commit}^{{commit}}")]) == Some(true)
}

/// Whether `candidate` is a strict ancestor of `installed`. `None` when that
/// cannot be established.
fn is_older(repo: &Path, candidate: &str, installed: Option<&str>) -> Option<bool> {
    let installed = installed.filter(|c| c.bytes().all(|b| b.is_ascii_hexdigit()))?;
    if !commit_exists(repo, installed) {
        return None;
    }
    git_succeeds(repo, &["merge-base", "--is-ancestor", candidate, installed])
}

fn append_log(path: &Path, record: &PromotionRecord) -> std::io::Result<()> {
    let mut line = serde_json::to_string(record).map_err(std::io::Error::other)?;
    line.push('\n');
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(line.as_bytes())?;
    file.sync_all()
}
