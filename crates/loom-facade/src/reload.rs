//! The published-generation reload transition.
//!
//! One tick takes a **complete published bundle** on disk to a **verified served
//! bundle**: read the `.generation.json` commit marker, hash every artefact it
//! lists, write a `reload-pending` receipt, restart the service if it is not
//! already serving that bundle, then prove the served identity and only then
//! write a `served` receipt. This is the Rust port of
//! `scripts/reload-published-generation.py` (ontomark migration LM-06).
//!
//! The marker is read by [`crate::mirror::parse_marker_strict`], the strict side
//! of the one reader in `mirror.rs`, and the bundle digest is
//! [`ServingIdentity::digest_input`] hashed with [`hex_sha256`], so the digest
//! proved here is the one `LoadedBundle` serves. There is no second marker check.
//!
//! Every external effect sits behind a trait ([`Reloader`], [`IdentityProbe`],
//! [`Clock`]) so [`reconcile`] is driven by fakes in tests; the production
//! implementations ([`CommandReloader`], [`HttpProbe`], [`SystemClock`]) back the
//! `reload_published_generation` binary.

use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use loom_domain::{ArtifactSha, ServingIdentity};
use serde_json::Value;

use crate::bundle::IN_FLIGHT_MARKER;
use crate::mirror::{hex_sha256, parse_marker_strict};

/// The six artefacts a published generation must list: both indexes, asserted
/// and inferred Turtle, the semantic RVDB and its generation sidecar. A legacy
/// four-file mirror is refused.
pub const REQUIRED: &[&str] = &[
    "scaffold-index.json",
    "prose-index.json",
    "ontology.ttl",
    "ontology-inferred.ttl",
    "ontology-corpus.rvdb",
    "ontology-corpus.rvdb.generation.json",
];

const SIDECAR: &str = "ontology-corpus.rvdb.generation.json";
const MODEL: &str = "bge-small-en-v1.5";
const DIMENSIONS: f64 = 384.0;
/// Interval between served-identity polls after a restart.
const POLL_INTERVAL: Duration = Duration::from_secs(1);
/// Cap on the identity response body (`/loom/generation`).
const MAX_IDENTITY_BYTES: usize = 1_048_576;
/// Per-request timeout of the identity probe.
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// A verified bundle: the generation and the digest of its artefacts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bundle {
    /// The generation id the marker declares.
    pub generation: String,
    /// sha256 over the sorted `name:sha256` artefact pairs, as served.
    pub content_digest: String,
}

/// Why a tick failed. Every variant maps to process exit code 1.
#[derive(Debug)]
pub enum ReloadError {
    /// The published bundle is incomplete, mismatched, indirect or in flight.
    Invalid(String),
    /// The restart command failed or timed out.
    Reload(String),
    /// The service did not prove the expected bundle before the deadline.
    Timeout,
    /// The receipt could not be written.
    Io(io::Error),
}

impl fmt::Display for ReloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(why) => write!(f, "{why}"),
            Self::Reload(why) => write!(f, "reload failed: {why}"),
            Self::Timeout => write!(f, "reloaded service did not prove the expected bundle"),
            Self::Io(e) => write!(f, "receipt: {e}"),
        }
    }
}

impl std::error::Error for ReloadError {}

fn invalid(why: impl Into<String>) -> ReloadError {
    ReloadError::Invalid(why.into())
}

/// What a successful tick did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The service already served the bundle; nothing was restarted.
    Current,
    /// The service was restarted and then proved the bundle.
    Reloaded,
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Current => "current",
            Self::Reloaded => "reloaded",
        })
    }
}

/// Restarts the service. The production implementation is [`CommandReloader`].
pub trait Reloader {
    /// Perform the restart; `Err` carries the reason.
    fn reload(&mut self) -> Result<(), String>;
}

impl<F: FnMut() -> Result<(), String>> Reloader for F {
    fn reload(&mut self) -> Result<(), String> {
        self()
    }
}

/// Fetches the served identity report. The production implementation is
/// [`HttpProbe`]. Any `Err` means "not proved", never "fatal".
pub trait IdentityProbe {
    /// Fetch the `/loom/generation` report.
    fn fetch(&mut self) -> Result<Value, String>;
}

impl<F: FnMut() -> Result<Value, String>> IdentityProbe for F {
    fn fetch(&mut self) -> Result<Value, String> {
        self()
    }
}

/// A monotonic clock and a sleeper.
pub trait Clock {
    /// Time elapsed since an arbitrary fixed origin; never decreases.
    fn now(&self) -> Duration;
    /// Block for `d`.
    fn sleep(&mut self, d: Duration);
}

/// The real monotonic clock.
#[derive(Debug)]
pub struct SystemClock(Instant);

impl SystemClock {
    /// Start a clock at the current instant.
    #[must_use]
    pub fn new() -> Self {
        Self(Instant::now())
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Duration {
        self.0.elapsed()
    }
    fn sleep(&mut self, d: Duration) {
        std::thread::sleep(d);
    }
}

/// Resolve a marker artefact name to a regular file inside `dir`.
///
/// A subdirectory is allowed (`graph/full.bin`). What is refused is any way out
/// of the bundle or around its hashes: an absolute path, an empty or `.`/`..`
/// component, a backslash, or a symlink at any level of the path.
fn contained_artifact(dir: &Path, name: &str) -> Result<PathBuf, ReloadError> {
    if name.starts_with('/')
        || name.contains('\\')
        || name.split('/').any(|part| matches!(part, "" | "." | ".."))
    {
        return Err(invalid(format!(
            "artifact must be a relative path inside the bundle: {name}"
        )));
    }
    let mut current = dir.to_path_buf();
    for part in name.split('/') {
        current.push(part);
        if fs::symlink_metadata(&current).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(invalid(format!("artifact missing or indirect: {name}")));
        }
    }
    if !current.is_file() {
        return Err(invalid(format!("artifact missing or indirect: {name}")));
    }
    Ok(current)
}

/// Verify the published bundle in `dir` and return its identity.
///
/// Refuses an in-flight promotion or indirect marker, an unparsable or
/// incomplete marker, any missing, indirect, escaping, mis-hashed or mis-sized
/// artefact, a semantic sidecar that disagrees with the generation, the
/// embedding model or the dimensions, and a marker that changed (or a promotion
/// that started) while verification ran.
///
/// The marker's declared `content_digest` identifies the source corpus, so it is
/// carried by the marker and discarded here, not compared: the digest returned
/// is the one recomputed from the individually verified artefacts, and a
/// tampered artefact list is caught by those per-file hashes.
pub fn verified_bundle(dir: &Path) -> Result<Bundle, ReloadError> {
    let marker_path = dir.join(".generation.json");
    let in_flight = dir.join(IN_FLIGHT_MARKER);
    if in_flight.exists()
        || fs::symlink_metadata(&marker_path).is_ok_and(|m| m.file_type().is_symlink())
    {
        return Err(invalid("publication in flight or indirect marker"));
    }
    let raw =
        fs::read(&marker_path).map_err(|e| invalid(format!("{}: {e}", marker_path.display())))?;
    let manifest: Value = serde_json::from_slice(&raw)
        .map_err(|e| invalid(format!("{}: {e}", marker_path.display())))?;
    let marker = parse_marker_strict(&manifest).map_err(ReloadError::Invalid)?;
    if !REQUIRED
        .iter()
        .all(|name| marker.artifacts.iter().any(|a| a.name == *name))
    {
        return Err(invalid("complete graph and semantic generation required"));
    }
    let mut observed = Vec::with_capacity(marker.artifacts.len());
    for spec in &marker.artifacts {
        let path = contained_artifact(dir, &spec.name)?;
        let bytes = fs::read(&path).map_err(|e| invalid(format!("{}: {e}", spec.name)))?;
        let sha256 = hex_sha256(&bytes);
        let size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if spec.sha256.as_deref() != Some(sha256.as_str()) || spec.bytes != Some(size) {
            return Err(invalid(format!(
                "artifact hash or size mismatch: {}",
                spec.name
            )));
        }
        observed.push(ArtifactSha {
            name: spec.name.clone(),
            sha256,
            bytes: size,
        });
    }
    let content_digest = hex_sha256(ServingIdentity::digest_input(&observed).as_bytes());
    check_sidecar(dir, &marker.generation)?;
    if fs::read(&marker_path).ok().as_deref() != Some(raw.as_slice()) || in_flight.exists() {
        return Err(invalid("publication changed during verification"));
    }
    Ok(Bundle {
        generation: marker.generation,
        content_digest,
    })
}

fn check_sidecar(dir: &Path, generation: &str) -> Result<(), ReloadError> {
    let raw = fs::read(dir.join(SIDECAR)).map_err(|e| invalid(format!("{SIDECAR}: {e}")))?;
    let sidecar: Value =
        serde_json::from_slice(&raw).map_err(|e| invalid(format!("{SIDECAR}: {e}")))?;
    // Python `dict.get(key, fallback)`: the fallback key counts only when the
    // first key is absent, not when it is present and null.
    let pick = |first: &str, second: &str| sidecar.get(first).or_else(|| sidecar.get(second));
    let ok = pick("generatedAt", "generation").and_then(Value::as_str) == Some(generation)
        && pick("embeddingModel", "embedding_model").and_then(Value::as_str) == Some(MODEL)
        && sidecar.get("dimensions").and_then(Value::as_f64) == Some(DIMENSIONS);
    if ok {
        Ok(())
    } else {
        Err(invalid(
            "semantic sidecar must match generation/model/dimensions",
        ))
    }
}

/// Whether a `/loom/generation` report proves `bundle` is the one being served:
/// the generation at every level, the artefact digest, verified atomicity, disk
/// matching what is loaded, no drift, and a valid bge-small-en-v1.5 / 384 /
/// cosine embedding qualification with no rejections. A report that is not a
/// JSON object proves nothing.
#[must_use]
pub fn served_matches(report: &Value, bundle: &Bundle) -> bool {
    let at = |path: &[&str]| path.iter().try_fold(report, |v, key| v.get(*key));
    let is = |path: &[&str], want: &Value| at(path) == Some(want);
    let id = Value::String(bundle.generation.clone());
    report.is_object()
        && is(&["id"], &id)
        && is(&["identity", "generation", "id"], &id)
        && is(
            &["identity", "content_digest"],
            &Value::String(bundle.content_digest.clone()),
        )
        && is(&["identity", "atomicity_verified"], &Value::Bool(true))
        && is(&["disk", "matches_loaded"], &Value::Bool(true))
        && is(&["drift", "ok"], &Value::Bool(true))
        && is(&["semantic_generation", "id"], &id)
        && is(&["embedding", "model_id"], &Value::String(MODEL.to_owned()))
        && at(&["embedding", "dimensions"]).and_then(Value::as_f64) == Some(DIMENSIONS)
        && is(
            &["embedding", "metric"],
            &Value::String("cosine".to_owned()),
        )
        && at(&["embedding", "rejections"])
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
}

/// `serde_json` formatter that reproduces Python's `json.dump` defaults
/// (`", "` and `": "` separators, non-ASCII as `\uXXXX`), so the receipt is
/// byte-compatible with the one the Python reloader wrote.
struct PythonJson;

impl serde_json::ser::Formatter for PythonJson {
    fn begin_object_key<W: ?Sized + Write>(&mut self, w: &mut W, first: bool) -> io::Result<()> {
        if first {
            Ok(())
        } else {
            w.write_all(b", ")
        }
    }
    fn begin_object_value<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        w.write_all(b": ")
    }
    fn write_string_fragment<W: ?Sized + Write>(
        &mut self,
        w: &mut W,
        fragment: &str,
    ) -> io::Result<()> {
        for c in fragment.chars() {
            if c.is_ascii() {
                w.write_all(&[c as u8])?;
            } else {
                let mut units = [0_u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    write!(w, "\\u{unit:04x}")?;
                }
            }
        }
        Ok(())
    }
}

/// Atomically write the receipt `{content_digest, generation, state}` (sorted
/// keys, trailing newline) to `path`: a private `<path>.tmp` is written,
/// fsynced and renamed over `path`.
pub fn write_receipt(path: &Path, state: &str, bundle: &Bundle) -> io::Result<()> {
    use serde::Serialize as _;
    let receipt = std::collections::BTreeMap::from([
        ("content_digest", bundle.content_digest.as_str()),
        ("generation", bundle.generation.as_str()),
        ("state", state),
    ]);
    let mut body = Vec::new();
    receipt
        .serialize(&mut serde_json::Serializer::with_formatter(
            &mut body, PythonJson,
        ))
        .map_err(io::Error::from)?;
    body.push(b'\n');
    let mut temp = path.as_os_str().to_owned();
    temp.push(".tmp");
    let temp = PathBuf::from(temp);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&temp)?;
    file.write_all(&body)?;
    file.flush()?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temp, path)
}

/// One tick. Verifies the bundle (no receipt if it is refused); returns
/// [`Outcome::Current`] and records `served` if the service already proves it;
/// otherwise records `reload-pending`, restarts, and polls until the service
/// proves the same bundle or `timeout` passes, re-verifying the disk on every
/// poll so a concurrent publication is refused rather than acknowledged.
/// Failed attempts are never recorded as served, so the next tick retries.
///
/// `timeout` is not validated here (zero means one poll after the restart).
pub fn reconcile<P: IdentityProbe, R: Reloader, C: Clock>(
    dir: &Path,
    receipt: &Path,
    mut probe: P,
    mut reloader: R,
    clock: &mut C,
    timeout: Duration,
) -> Result<Outcome, ReloadError> {
    let bundle = verified_bundle(dir)?;
    if probe
        .fetch()
        .is_ok_and(|report| served_matches(&report, &bundle))
    {
        write_receipt(receipt, "served", &bundle).map_err(ReloadError::Io)?;
        return Ok(Outcome::Current);
    }
    write_receipt(receipt, "reload-pending", &bundle).map_err(ReloadError::Io)?;
    reloader.reload().map_err(ReloadError::Reload)?;
    let deadline = clock.now() + timeout;
    loop {
        if verified_bundle(dir)? != bundle {
            return Err(invalid("publication changed during reload"));
        }
        if probe
            .fetch()
            .is_ok_and(|report| served_matches(&report, &bundle))
        {
            write_receipt(receipt, "served", &bundle).map_err(ReloadError::Io)?;
            return Ok(Outcome::Reloaded);
        }
        if clock.now() >= deadline {
            return Err(ReloadError::Timeout);
        }
        clock.sleep(POLL_INTERVAL);
    }
}

/// Production [`Reloader`]: runs an argument vector directly (no shell, stdio
/// inherited) and fails on a non-zero exit or when it outlives `timeout`, in
/// which case the child is killed.
#[derive(Debug, Clone)]
pub struct CommandReloader {
    argv: Vec<String>,
    timeout: Duration,
}

impl CommandReloader {
    /// `argv[0]` is the program. Callers must pass a non-empty vector.
    #[must_use]
    pub fn new(argv: Vec<String>, timeout: Duration) -> Self {
        Self { argv, timeout }
    }
}

impl Reloader for CommandReloader {
    fn reload(&mut self) -> Result<(), String> {
        let (program, args) = self.argv.split_first().ok_or("empty reload command")?;
        let mut child = Command::new(program)
            .args(args)
            .spawn()
            .map_err(|e| format!("{program}: {e}"))?;
        let deadline = Instant::now() + self.timeout;
        loop {
            match child.try_wait().map_err(|e| e.to_string())? {
                Some(status) if status.success() => return Ok(()),
                Some(status) => return Err(format!("{program} exited with {status}")),
                None if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("{program} timed out after {:?}", self.timeout));
                }
                None => std::thread::sleep(Duration::from_millis(50)),
            }
        }
    }
}

/// Production [`IdentityProbe`]: `GET <url>/loom/generation` with a five second
/// timeout and a 1 MiB body cap.
#[derive(Debug)]
pub struct HttpProbe {
    endpoint: String,
    client: reqwest::Client,
    runtime: tokio::runtime::Runtime,
}

impl HttpProbe {
    /// Probe the node at `base_url` (a trailing `/` is ignored).
    pub fn new(base_url: &str) -> io::Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(PROBE_TIMEOUT)
            .build()
            .map_err(io::Error::other)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        Ok(Self {
            endpoint: format!("{}/loom/generation", base_url.trim_end_matches('/')),
            client,
            runtime,
        })
    }
}

impl IdentityProbe for HttpProbe {
    fn fetch(&mut self) -> Result<Value, String> {
        self.runtime.block_on(async {
            let mut response = self
                .client
                .get(&self.endpoint)
                .send()
                .await
                .and_then(reqwest::Response::error_for_status)
                .map_err(|e| e.to_string())?;
            let mut body = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
                body.extend_from_slice(&chunk);
                if body.len() > MAX_IDENTITY_BYTES {
                    return Err("oversized identity response".to_owned());
                }
            }
            serde_json::from_slice(&body).map_err(|e| e.to_string())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The expected bytes were produced by the retired Python
    /// `write_receipt` (`json.dump(..., sort_keys=True)` plus a newline) for the
    /// same value, including non-ASCII, a surrogate pair, a quote and a backslash.
    #[test]
    fn receipt_bytes_match_what_the_python_reloader_wrote() {
        let dir = std::env::temp_dir().join(format!("loom-receipt-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("receipt.json");
        let bundle = Bundle {
            generation: "visionGraph@é😀\"\\".to_owned(),
            content_digest: "ab".to_owned(),
        };
        write_receipt(&path, "served", &bundle).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "{\"content_digest\": \"ab\", \"generation\": \"visionGraph@\\u00e9\\ud83d\\ude00\\\"\\\\\", \"state\": \"served\"}\n"
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
