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
            br#"{"version":1,"classes":{}}"#.to_vec(),
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
