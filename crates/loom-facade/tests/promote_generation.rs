//! ADR-141 D6 — explicit, recorded promotion.
//!
//! Every case runs against a real filesystem and a real git repository: the
//! candidate bundles are built on disk with true hashes, the source repository
//! has real commits, and the installed generation is read back through the same
//! `LoadedBundle` activation and `/health` route the service uses.

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::{call, sha256_hex, TestEnvBuilder, FIXTURE};
use loom_domain::GenerationStore;
use loom_facade::bundle::{LoadedBundle, IN_FLIGHT_MARKER};
use loom_facade::promote::{promote, PromoteError, PromoteOptions, PromoteOutcome};
use loom_facade::reload::verified_bundle;
use serde_json::{json, Value};
use tempfile::TempDir;

const ASSERTED_TTL: &str = "@prefix ex: <http://example.org/> .\nex:a ex:p ex:b .\n";
const INFERRED_TTL: &str = "@prefix ex: <http://example.org/> .\nex:b ex:q ex:c .\n";

/// A source repository with a linear history: `commits[0]` is the oldest.
struct Source {
    _dir: TempDir,
    path: PathBuf,
    commits: Vec<String>,
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.org")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.org")
        .output()
        .expect("git runs");
    assert!(out.status.success(), "git {args:?}: {out:?}");
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

impl Source {
    fn new(n: usize) -> Self {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();
        git(&path, &["init", "-q", "-b", "main"]);
        let commits = (0..n)
            .map(|i| {
                fs::write(path.join("page.md"), format!("revision {i}")).unwrap();
                git(&path, &["add", "."]);
                git(&path, &["commit", "-q", "-m", &format!("rev {i}")]);
                git(&path, &["rev-parse", "HEAD"])
            })
            .collect();
        Self {
            _dir: dir,
            path,
            commits,
        }
    }
}

/// Build a complete, correctly hashed C3 bundle for `commit` under `root`.
fn make_bundle(root: &Path, commit: &str) -> PathBuf {
    let dir = root.join(format!("bundle-{}", &commit[..9]));
    fs::create_dir_all(&dir).unwrap();
    let id = format!("visionGraph@{commit}");
    let files: Vec<(&str, Vec<u8>)> = vec![
        ("scaffold-index.json", FIXTURE.as_bytes().to_vec()),
        (
            "prose-index.json",
            serde_json::to_vec(&json!({
                "pages": {"knowledge-graph": {"dfull": format!("prose of {commit}")}}
            }))
            .unwrap(),
        ),
        (
            "ontology.ttl",
            format!("{ASSERTED_TTL}# {commit}\n").into_bytes(),
        ),
        ("ontology-inferred.ttl", INFERRED_TTL.as_bytes().to_vec()),
        ("graph/full.bin", commit.as_bytes().to_vec()),
        ("ontology-corpus.rvdb", commit.as_bytes().to_vec()),
        (
            "ontology-corpus.rvdb.generation.json",
            serde_json::to_vec(&json!({
                "generatedAt": id,
                "embeddingModel": "bge-small-en-v1.5",
                "dimensions": 384
            }))
            .unwrap(),
        ),
    ];
    let mut artifacts = Vec::new();
    for (name, bytes) in &files {
        let path = dir.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        artifacts.push(json!({"name": name, "sha256": sha256_hex(bytes), "bytes": bytes.len()}));
    }
    let marker = json!({
        "id": id,
        "commit": commit,
        "content_digest": "0".repeat(64),
        "generated_at": "2026-11-16T02:00:00Z",
        "class_count": 7,
        "page_count": 7,
        "vocabulary_version": 1,
        "built_by": {"name": "ontomark-compile", "version": "0.9.0"},
        "artifacts": artifacts,
    });
    fs::write(
        dir.join(".generation.json"),
        serde_json::to_vec_pretty(&marker).unwrap(),
    )
    .unwrap();
    dir
}

struct Rig {
    _temp: TempDir,
    root: PathBuf,
    data: PathBuf,
    source: Source,
}

impl Rig {
    fn new(commits: usize) -> Self {
        let temp = TempDir::new().unwrap();
        let root = temp.path().to_path_buf();
        let data = root.join("data");
        fs::create_dir_all(&data).unwrap();
        Self {
            _temp: temp,
            root,
            data,
            source: Source::new(commits),
        }
    }

    fn bundle(&self, n: usize) -> PathBuf {
        make_bundle(&self.root, &self.source.commits[n])
    }

    fn opts(&self, bundle: PathBuf) -> PromoteOptions {
        PromoteOptions {
            bundle,
            data_dir: self.data.clone(),
            source_repo: self.source.path.clone(),
            log: self.data.join("promotions.jsonl"),
            promoted_by: "loom-nightly".to_owned(),
            allow_rollback: false,
            now: Some("2026-11-16T03:00:00Z".to_owned()),
        }
    }

    fn log_lines(&self) -> Vec<Value> {
        fs::read_to_string(self.data.join("promotions.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).expect("each log line is JSON"))
            .collect()
    }

    fn marker_bytes(&self) -> Vec<u8> {
        fs::read(self.data.join(".generation.json")).unwrap_or_default()
    }

    fn serving_id(&self) -> String {
        let index = self.data.join("scaffold-index.json");
        LoadedBundle::activate(&index.to_string_lossy())
            .expect("the installed bundle activates")
            .current()
            .id
            .0
    }

    fn id(&self, n: usize) -> String {
        format!("visionGraph@{}", self.source.commits[n])
    }
}

fn refused(result: Result<PromoteOutcome, PromoteError>) -> String {
    match result {
        Err(PromoteError::Refused(why)) => why,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_valid_bundle_is_stamped_installed_and_logged() {
    let rig = Rig::new(2);
    let bundle = rig.bundle(0);
    let outcome = promote(&rig.opts(bundle.clone())).expect("promotes");
    let PromoteOutcome::Promoted(record) = outcome else {
        panic!("expected a promotion");
    };
    assert_eq!(record.generation, rig.id(0));
    assert_eq!(record.promoted_at, "2026-11-16T03:00:00Z");
    assert_eq!(record.previous_generation, None);

    // The marker the service reads carries the stamp; the staging marker is untouched.
    let marker: Value = serde_json::from_slice(&rig.marker_bytes()).unwrap();
    assert_eq!(marker["promoted_at"], "2026-11-16T03:00:00Z");
    assert_eq!(marker["promoted_by"], "loom-nightly");
    assert!(marker.get("previous_generation").is_none());
    let staged: Value =
        serde_json::from_slice(&fs::read(bundle.join(".generation.json")).unwrap()).unwrap();
    assert!(staged.get("promoted_at").is_none());

    // It activates, verifies for the reload tick, and reports the stamp.
    verified_bundle(&rig.data).expect("installed bundle verifies");
    let index = rig.data.join("scaffold-index.json");
    let loaded = LoadedBundle::activate(&index.to_string_lossy()).unwrap();
    let generation = loaded.current();
    assert_eq!(
        generation.promoted_at.as_deref(),
        Some("2026-11-16T03:00:00Z")
    );
    assert_eq!(generation.promoted_by.as_deref(), Some("loom-nightly"));

    // Exactly one log line, with every field the brief asks for.
    let lines = rig.log_lines();
    assert_eq!(lines.len(), 1);
    let line = &lines[0];
    assert_eq!(line["generation"], rig.id(0));
    assert_eq!(line["source_repository"], "visionGraph");
    assert_eq!(line["commit"], rig.source.commits[0]);
    assert_eq!(line["built_by"]["name"], "ontomark-compile");
    assert_eq!(line["promoted_at"], "2026-11-16T03:00:00Z");
    assert_eq!(line["previous_generation"], Value::Null);
    assert_eq!(line["bundle"], bundle.display().to_string());
    assert!(!rig.data.join(IN_FLIGHT_MARKER).exists());
}

#[test]
fn a_second_promotion_records_the_generation_it_replaced() {
    let rig = Rig::new(2);
    promote(&rig.opts(rig.bundle(0))).unwrap();
    promote(&rig.opts(rig.bundle(1))).unwrap();
    assert_eq!(rig.serving_id(), rig.id(1));
    let lines = rig.log_lines();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[1]["previous_generation"], rig.id(0));
    let marker: Value = serde_json::from_slice(&rig.marker_bytes()).unwrap();
    assert_eq!(marker["previous_generation"], rig.id(0));
}

#[test]
fn an_older_bundle_is_refused_without_the_flag_and_allowed_with_it() {
    let rig = Rig::new(3);
    promote(&rig.opts(rig.bundle(2))).unwrap();
    let before = rig.marker_bytes();

    let why = refused(promote(&rig.opts(rig.bundle(0))));
    assert!(why.contains("older"), "{why}");
    assert_eq!(rig.marker_bytes(), before, "installed marker untouched");
    assert_eq!(rig.serving_id(), rig.id(2));
    assert_eq!(rig.log_lines().len(), 1, "no log line for a refusal");

    let mut opts = rig.opts(rig.bundle(0));
    opts.allow_rollback = true;
    promote(&opts).expect("rollback with the flag");
    assert_eq!(rig.serving_id(), rig.id(0));
    let lines = rig.log_lines();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[1]["previous_generation"], rig.id(2));
}

#[test]
fn a_corrupt_bundle_is_refused_and_the_previous_generation_still_serves() {
    let rig = Rig::new(3);
    promote(&rig.opts(rig.bundle(0))).unwrap();
    let before = rig.marker_bytes();
    let ontology_before = fs::read(rig.data.join("ontology.ttl")).unwrap();

    // A byte changed after the marker was written: hash mismatch.
    let tampered = rig.bundle(1);
    fs::write(tampered.join("ontology-inferred.ttl"), b"tampered").unwrap();
    refused(promote(&rig.opts(tampered)));

    // A truncated marker.
    let truncated = rig.bundle(1);
    let marker = fs::read(truncated.join(".generation.json")).unwrap();
    fs::write(
        truncated.join(".generation.json"),
        &marker[..marker.len() / 2],
    )
    .unwrap();
    refused(promote(&rig.opts(truncated)));

    // Hashes are right but the Turtle is not Turtle: the service would fail to
    // load it, so promotion must catch it first.
    let unparsable = rig.bundle(2);
    let ttl = b"this is { not turtle".to_vec();
    fs::write(unparsable.join("ontology.ttl"), &ttl).unwrap();
    let mut m: Value =
        serde_json::from_slice(&fs::read(unparsable.join(".generation.json")).unwrap()).unwrap();
    for a in m["artifacts"].as_array_mut().unwrap() {
        if a["name"] == "ontology.ttl" {
            a["sha256"] = json!(sha256_hex(&ttl));
            a["bytes"] = json!(ttl.len());
        }
    }
    fs::write(
        unparsable.join(".generation.json"),
        serde_json::to_vec(&m).unwrap(),
    )
    .unwrap();
    let why = refused(promote(&rig.opts(unparsable)));
    assert!(why.contains("Turtle"), "{why}");

    // The previous generation is byte-for-byte what it was, and still the one
    // an activation would serve.
    assert_eq!(rig.marker_bytes(), before);
    assert_eq!(
        fs::read(rig.data.join("ontology.ttl")).unwrap(),
        ontology_before
    );
    assert_eq!(rig.serving_id(), rig.id(0));
    assert_eq!(rig.log_lines().len(), 1);
    assert!(!rig.data.join(IN_FLIGHT_MARKER).exists());
}

#[test]
fn a_commit_absent_from_the_source_repository_is_refused() {
    let rig = Rig::new(1);
    let stranger = make_bundle(&rig.root, &"ab".repeat(20));
    let why = refused(promote(&rig.opts(stranger)));
    assert!(why.contains("not present"), "{why}");
    assert!(!rig.data.join(".generation.json").exists());
}

#[test]
fn a_marker_without_a_commit_is_refused() {
    let rig = Rig::new(1);
    let bundle = rig.bundle(0);
    // The legacy mirror shape: valid to read, but not a promotable build.
    let mut artifacts = serde_json::Map::new();
    for name in loom_facade::reload::REQUIRED {
        let bytes = fs::read(bundle.join(name)).unwrap();
        artifacts.insert(
            (*name).to_owned(),
            json!({"sha256": sha256_hex(&bytes), "bytes": bytes.len()}),
        );
    }
    let sidecar = br#"{"generatedAt":"g1","embeddingModel":"bge-small-en-v1.5","dimensions":384}"#;
    fs::write(bundle.join("ontology-corpus.rvdb.generation.json"), sidecar).unwrap();
    artifacts.insert(
        "ontology-corpus.rvdb.generation.json".to_owned(),
        json!({"sha256": sha256_hex(sidecar), "bytes": sidecar.len()}),
    );
    fs::write(
        bundle.join(".generation.json"),
        serde_json::to_vec(&json!({"generation": "g1", "artifacts": artifacts})).unwrap(),
    )
    .unwrap();
    let why = refused(promote(&rig.opts(bundle)));
    assert!(why.contains("source commit"), "{why}");
}

#[test]
fn promoting_what_is_already_installed_is_a_no_op() {
    let rig = Rig::new(1);
    promote(&rig.opts(rig.bundle(0))).unwrap();
    let before = rig.marker_bytes();
    let outcome = promote(&rig.opts(rig.bundle(0))).unwrap();
    assert_eq!(outcome, PromoteOutcome::Current(rig.id(0)));
    assert_eq!(rig.marker_bytes(), before, "the stamp is not rewritten");
    assert_eq!(rig.log_lines().len(), 1);
}

#[test]
fn a_directory_left_mid_promotion_is_repaired_by_promoting() {
    let rig = Rig::new(1);
    promote(&rig.opts(rig.bundle(0))).unwrap();
    fs::write(rig.data.join(IN_FLIGHT_MARKER), "crashed").unwrap();
    fs::remove_file(rig.data.join(".generation.json")).unwrap();
    promote(&rig.opts(rig.bundle(0))).expect("repairs");
    assert_eq!(rig.serving_id(), rig.id(0));
    assert!(!rig.data.join(IN_FLIGHT_MARKER).exists());
    assert_eq!(rig.log_lines()[1]["previous_generation"], Value::Null);
}

#[tokio::test]
async fn health_shows_the_stamp_and_the_previous_generation() {
    let rig = Rig::new(2);
    promote(&rig.opts(rig.bundle(0))).unwrap();
    promote(&rig.opts(rig.bundle(1))).unwrap();

    // The harness activates the data directory exactly as the composition root
    // does (its fixture index is the bundle's scaffold index), then serves it.
    let env = TestEnvBuilder::new().with_data_dir(&rig.data).build();
    let (_, health) = call(env.router(), "GET", "/health", None).await;
    assert_eq!(health["generation"]["id"], rig.id(1));
    assert_eq!(health["generation"]["promoted_at"], "2026-11-16T03:00:00Z");
    assert_eq!(health["generation"]["promoted_by"], "loom-nightly");
    assert_eq!(health["generation"]["previous_generation"], rig.id(0));
    assert_eq!(health["generation"]["built_by"]["name"], "ontomark-compile");
}

#[test]
fn the_command_exits_zero_on_success_and_non_zero_on_refusal() {
    let rig = Rig::new(2);
    let run = |bundle: &Path| {
        Command::new(env!("CARGO_BIN_EXE_promote_generation"))
            .args(["--bundle"])
            .arg(bundle)
            .arg("--data-dir")
            .arg(&rig.data)
            .arg("--source-repo")
            .arg(&rig.source.path)
            .args(["--promoted-by", "tester"])
            .output()
            .unwrap()
    };
    let ok = run(&rig.bundle(1));
    assert!(ok.status.success(), "{ok:?}");
    assert!(String::from_utf8_lossy(&ok.stdout).starts_with("promoted visionGraph@"));
    let again = run(&rig.bundle(1));
    assert!(again.status.success());
    assert!(String::from_utf8_lossy(&again.stdout).starts_with("current "));

    let older = run(&rig.bundle(0));
    assert_eq!(older.status.code(), Some(1), "{older:?}");
    assert!(String::from_utf8_lossy(&older.stderr).contains("older"));
    assert_eq!(rig.serving_id(), rig.id(1));

    let usage = Command::new(env!("CARGO_BIN_EXE_promote_generation"))
        .output()
        .unwrap();
    assert_eq!(usage.status.code(), Some(2));
    assert_eq!(rig.log_lines().len(), 1);
}

#[test]
fn installed_files_are_readable_by_the_containers_user_whatever_the_umask() {
    let rig = Rig::new(1);
    let bundle = rig.bundle(0);
    // A private build (umask 077) produces 0600 files in 0700 directories.
    for entry in [
        "graph",
        "graph/full.bin",
        "ontology.ttl",
        ".generation.json",
    ] {
        let mode = if entry == "graph" { 0o700 } else { 0o600 };
        fs::set_permissions(bundle.join(entry), fs::Permissions::from_mode(mode)).unwrap();
    }
    promote(&rig.opts(bundle)).unwrap();
    let mode = |p: &str| fs::metadata(rig.data.join(p)).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode("ontology.ttl"), 0o644);
    assert_eq!(mode(".generation.json"), 0o644);
    assert_eq!(mode("graph/full.bin"), 0o644);
    assert_eq!(mode("graph"), 0o755);
}

#[test]
fn a_damaged_installed_bundle_still_counts_for_ordering_and_the_log() {
    let rig = Rig::new(3);
    promote(&rig.opts(rig.bundle(2))).unwrap();
    // Post-install damage: the installed generation no longer verifies, but the
    // running process still serves it, so an older bundle must still be refused.
    fs::write(rig.data.join("ontology.ttl"), b"damaged").unwrap();
    assert!(verified_bundle(&rig.data).is_err());
    let why = refused(promote(&rig.opts(rig.bundle(0))));
    assert!(why.contains("older"), "{why}");

    // A newer bundle replaces it and the log names what it replaced.
    let newer = rig.bundle(2);
    promote(&rig.opts(newer)).expect("the same generation repairs the damage");
    verified_bundle(&rig.data).expect("repaired");
    let lines = rig.log_lines();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[1]["previous_generation"], rig.id(2));
}

// --- start-up fallback (ADR-141 amendment) ------------------------------------

use loom_domain::BundleError;
use loom_facade::{build_router, try_app_state, Config};

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

impl Rig {
    /// Keep generation `n`'s built bundle where the nightly keeps it:
    /// `<retained>/<commit>/data`.
    fn retain(&self, n: usize) {
        let commit = &self.source.commits[n];
        copy_tree(
            &self.bundle(n),
            &self.root.join("retained").join(commit).join("data"),
        );
    }

    fn config(&self) -> Config {
        Config {
            index_path: self
                .data
                .join("scaffold-index.json")
                .to_string_lossy()
                .into_owned(),
            prose_path: self
                .data
                .join("prose-index.json")
                .to_string_lossy()
                .into_owned(),
            hnsw_artifact: self
                .data
                .join("ontology-corpus.rvdb")
                .to_string_lossy()
                .into_owned(),
            ledger_path: self
                .root
                .join("ledger.jsonl")
                .to_string_lossy()
                .into_owned(),
            retained_bundles_dir: Some(self.root.join("retained").to_string_lossy().into_owned()),
            ..Config::default()
        }
    }

    /// Both generations promoted and retained, then the current one corrupted.
    fn corrupt_current(&self) {
        promote(&self.opts(self.bundle(0))).unwrap();
        promote(&self.opts(self.bundle(1))).unwrap();
        self.retain(0);
        self.retain(1);
        let ttl = self.data.join("ontology.ttl");
        let mut bytes = fs::read(&ttl).unwrap();
        bytes.extend_from_slice(b"# bit rot\n");
        fs::write(ttl, bytes).unwrap();
    }

    fn data_snapshot(&self) -> Vec<(String, Vec<u8>)> {
        fn walk(dir: &Path, base: &Path, out: &mut Vec<(String, Vec<u8>)>) {
            for entry in fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(&path, base, out);
                } else {
                    let name = path.strip_prefix(base).unwrap().display().to_string();
                    out.push((name, fs::read(&path).unwrap()));
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.data, &self.data, &mut out);
        out.sort();
        out
    }
}

fn start(config: Config) -> Result<axum::Router, BundleError> {
    try_app_state(config).map(build_router)
}

#[tokio::test]
async fn corrupt_current_and_valid_previous_serves_the_previous_and_says_so() {
    let rig = Rig::new(2);
    rig.corrupt_current();
    let before = rig.data_snapshot();
    let retained_before = rig.root.join("retained");
    let retained_digest =
        verified_bundle(&retained_before.join(&rig.source.commits[0]).join("data"))
            .expect("the retained bundle verifies")
            .content_digest;

    let router = start(rig.config()).unwrap_or_else(|e| panic!("{e}"));
    let (_, health) = call(router.clone(), "GET", "/health", None).await;
    assert_eq!(health["status"], "fallback", "{health}");
    assert_eq!(
        health["generation"]["id"],
        rig.id(0),
        "the previous is served"
    );
    assert_eq!(health["fallback"]["failed_generation"], rig.id(1));
    assert_eq!(health["fallback"]["serving_generation"], rig.id(0));
    assert!(
        health["fallback"]["reason"]
            .as_str()
            .is_some_and(|r| r.contains("ontology.ttl")),
        "the reason names the failing artefact: {health}"
    );
    // Previous, not current: the digest is the retained bundle's own.
    assert_eq!(
        health["serving_bundle"]["identity"]["content_digest"],
        retained_digest
    );
    assert_ne!(
        health["serving_bundle"]["disk_matches_loaded"],
        Value::Bool(true),
        "the data directory still holds the failed generation"
    );
    // Never the normal `served`, even after answering requests.
    let (_, generation) = call(router, "GET", "/loom/generation", None).await;
    assert_eq!(generation["identity"]["phase"], "activated", "{generation}");
    assert_eq!(generation["fallback"]["failed_generation"], rig.id(1));

    // Read-only: the failed bundle is left for inspection, the retained one intact.
    assert_eq!(
        rig.data_snapshot(),
        before,
        "the data directory is untouched"
    );
    verified_bundle(&retained_before.join(&rig.source.commits[0]).join("data"))
        .expect("the retained bundle still verifies after start");
}

#[tokio::test]
async fn corrupt_current_and_no_valid_previous_refuses() {
    let rig = Rig::new(2);
    rig.corrupt_current();
    // The only previous generation is itself damaged.
    let victim = rig
        .root
        .join("retained")
        .join(&rig.source.commits[0])
        .join("data")
        .join("ontology-inferred.ttl");
    fs::write(victim, b"not what the marker hashed").unwrap();
    let result = start(rig.config());
    assert!(
        matches!(result, Err(BundleError::HashMismatch { .. })),
        "refuses with the original failure: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn corrupt_current_with_no_retained_directory_refuses_as_before() {
    let rig = Rig::new(2);
    rig.corrupt_current();
    let mut config = rig.config();
    config.retained_bundles_dir = None;
    assert!(start(config).is_err());
    let mut config = rig.config();
    config.retained_bundles_dir = Some(rig.root.join("missing").display().to_string());
    assert!(start(config).is_err());
}

#[tokio::test]
async fn a_valid_bundle_the_log_does_not_name_is_never_chosen() {
    let rig = Rig::new(2);
    rig.corrupt_current();
    fs::remove_file(rig.data.join("promotions.jsonl")).unwrap();
    assert!(
        start(rig.config()).is_err(),
        "a retained directory alone is not enough"
    );
}

#[tokio::test]
async fn a_valid_current_generation_is_unaffected() {
    let rig = Rig::new(2);
    promote(&rig.opts(rig.bundle(0))).unwrap();
    promote(&rig.opts(rig.bundle(1))).unwrap();
    rig.retain(0);
    let router = start(rig.config()).unwrap_or_else(|e| panic!("{e}"));
    let (_, health) = call(router.clone(), "GET", "/health", None).await;
    assert_eq!(health["status"], "serving", "{health}");
    assert_eq!(health["fallback"], Value::Null);
    assert_eq!(health["generation"]["id"], rig.id(1));
    assert_eq!(health["serving_bundle"]["disk_matches_loaded"], true);
    let (_, generation) = call(router, "GET", "/loom/generation", None).await;
    assert_eq!(generation["fallback"], Value::Null);
}

#[tokio::test]
async fn a_later_good_promotion_returns_to_normal() {
    let rig = Rig::new(3);
    rig.corrupt_current();
    let _ = start(rig.config()).expect("falls back first");

    // A rebuild of the same generation repairs the damaged install ...
    promote(&rig.opts(rig.bundle(1))).expect("repairs the corrupt copy");
    verified_bundle(&rig.data).expect("the repaired install verifies");
    let router = start(rig.config()).unwrap_or_else(|e| panic!("{e}"));
    let (_, health) = call(router, "GET", "/health", None).await;
    assert_eq!(health["status"], "serving", "{health}");
    assert_eq!(health["generation"]["id"], rig.id(1));

    // ... and so does a newer one.
    let ttl = rig.data.join("ontology.ttl");
    fs::write(&ttl, b"damaged again").unwrap();
    assert!(start(rig.config()).is_ok(), "falls back again");
    promote(&rig.opts(rig.bundle(2))).expect("promotes over the damaged install");
    let router = start(rig.config()).unwrap_or_else(|e| panic!("{e}"));
    let (_, health) = call(router, "GET", "/health", None).await;
    assert_eq!(health["status"], "serving", "{health}");
    assert_eq!(health["generation"]["id"], rig.id(2));
    assert_eq!(health["fallback"], Value::Null);
}

#[tokio::test]
async fn the_retained_semantic_artefact_is_copied_not_opened_in_place() {
    let rig = Rig::new(2);
    rig.corrupt_current();
    let retained = rig
        .root
        .join("retained")
        .join(&rig.source.commits[0])
        .join("data");
    // As the container's entrypoint leaves it: a writable runtime directory.
    let mut config = rig.config();
    let runtime = rig.root.join("run");
    fs::create_dir_all(&runtime).unwrap();
    config.hnsw_artifact = runtime.join("ontology-corpus.rvdb").display().to_string();
    let _router = start(config).unwrap_or_else(|e| panic!("{e}"));
    for name in [
        "ontology-corpus.rvdb",
        "ontology-corpus.rvdb.generation.json",
    ] {
        assert_eq!(
            fs::read(runtime.join("fallback").join(name)).unwrap(),
            fs::read(retained.join(name)).unwrap(),
            "{name} is copied for opening"
        );
    }
    verified_bundle(&retained).expect("the retained bundle is untouched");
}

/// The container sets `ONTOLOGY_PROSE_INDEX=/app/data/prose-index.json`, and the
/// lexical adapter reads that variable rather than the configured path. A
/// fallback must still serve the prose of the bundle it serves.
#[tokio::test]
async fn a_fallback_serves_the_prose_of_the_retained_bundle_not_the_failed_one() {
    let rig = Rig::new(2);
    rig.corrupt_current();
    let prose = |commit: &str| format!("prose of {commit}");
    std::env::set_var("ONTOLOGY_PROSE_INDEX", rig.data.join("prose-index.json"));
    let state = try_app_state(rig.config()).unwrap_or_else(|e| panic!("{e}"));
    std::env::remove_var("ONTOLOGY_PROSE_INDEX");
    let unit = state
        .retriever
        .resolve(&loom_domain::Iri::from_slug("knowledge-graph"))
        .expect("the fixture class resolves");
    assert_eq!(
        unit.dfull.as_deref(),
        Some(prose(&rig.source.commits[0]).as_str()),
        "prose comes from the generation being served"
    );
}

#[tokio::test]
async fn a_garbled_marker_falls_back_and_otherwise_keeps_the_old_degraded_start() {
    let rig = Rig::new(2);
    rig.corrupt_current();
    fs::write(rig.data.join(".generation.json"), b"{").unwrap();
    let router = start(rig.config()).unwrap_or_else(|e| panic!("{e}"));
    let (_, health) = call(router, "GET", "/health", None).await;
    assert_eq!(health["status"], "fallback", "{health}");
    assert_eq!(health["fallback"]["failed_generation"], Value::Null);
    // The failed id is unknown, so the newest logged generation is eligible: here
    // the same generation, from its intact retained copy.
    assert_eq!(health["generation"]["id"], rig.id(1));

    // Nothing retained: today's behaviour, an unattested start on the data directory.
    let mut config = rig.config();
    config.retained_bundles_dir = None;
    let (_, health) = call(start(config).unwrap(), "GET", "/health", None).await;
    assert_eq!(health["status"], "serving");
    assert_eq!(
        health["serving_bundle"]["identity"]["atomicity_verified"],
        false
    );
}
