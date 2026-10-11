//! Ports `tests/test_reload_published_generation.py` (16 tests) to the Rust
//! `reload_published_generation` transition (ontomark migration LM-06).
//!
//! The library half (`loom_facade::reload`) is driven with a fake restart, a
//! fake identity probe and a fake clock. The binary half is driven as a real
//! process against a wiremock identity endpoint, because only that proves exit
//! codes, argv passthrough, the lock and the receipt location.
//!
//! Python -> Rust mapping (all 16 apply; none is dropped):
//!
//! | # | Python test | Rust test |
//! |---|---|---|
//! | 1 | `test_matching_server_does_not_restart` | `matching_server_does_not_restart` |
//! | 2 | `test_new_publication_reloads_then_records_verified_receipt` | `new_publication_reloads_then_records_verified_receipt` |
//! | 3 | `test_failed_reload_never_records_success` | `failed_reload_never_records_success` |
//! | 4 | `test_wrong_generation_after_restart_is_not_acknowledged` | `wrong_generation_after_restart_is_not_acknowledged` |
//! | 5 | `test_hash_tamper_prevents_restart` | `hash_tamper_prevents_restart` |
//! | 6 | `test_missing_semantic_and_inflight_publication_refused` | `missing_semantic_and_inflight_publication_refused` |
//! | 7 | `test_indirect_and_traversal_artifacts_refused` | `indirect_and_traversal_artifacts_refused` |
//! | 8 | `test_concurrent_publication_after_restart_not_acknowledged` | `concurrent_publication_after_restart_not_acknowledged` |
//! | 9 | `test_graph_tier_subdirectory_artifact_is_accepted` | `graph_tier_subdirectory_artifact_is_accepted` |
//! | 10 | `test_escaping_or_indirect_subpaths_are_refused` (6 subtests) | `escaping_or_indirect_subpaths_are_refused` (loop) |
//! | 11 | `test_vault_build_bundle_is_accepted` | `vault_build_bundle_is_accepted` |
//! | 12 | `test_declared_corpus_digest_is_carried_not_compared` | `declared_corpus_digest_is_carried_not_compared` |
//! | 13 | `test_a_tampered_artifact_still_refuses_whatever_the_declared_digest` | `a_tampered_artifact_still_refuses_whatever_the_declared_digest` |
//! | 14 | `test_identity_must_name_its_own_commit` | `identity_must_name_its_own_commit` |
//! | 15 | `test_missing_c3_field_is_refused` | `missing_c3_field_is_refused` |
//! | 16 | `test_semantic_sidecar_must_declare_the_same_generation` | `semantic_sidecar_must_declare_the_same_generation` |

use std::cell::{Cell, RefCell};
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::rc::Rc;
use std::time::Duration;

use loom_facade::reload::{
    reconcile, served_matches, verified_bundle, Clock, Outcome, ReloadError, REQUIRED,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const SIDECAR: &str = "ontology-corpus.rvdb.generation.json";

fn sha_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The bundle hash, computed independently of the library: sha256 over the
/// sorted, newline-joined `name:sha256` pairs.
fn expected_digest(pairs: &[(String, String)]) -> String {
    let mut lines: Vec<String> = pairs.iter().map(|(n, d)| format!("{n}:{d}")).collect();
    lines.sort();
    sha_hex(lines.join("\n").as_bytes())
}

fn sidecar_bytes(generation: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "generatedAt": generation,
        "embeddingModel": "bge-small-en-v1.5",
        "dimensions": 384
    }))
    .unwrap()
}

/// A fake clock: `now` moves only when something sleeps.
#[derive(Default)]
struct FakeClock {
    now: Duration,
    sleeps: Vec<Duration>,
}

impl Clock for FakeClock {
    fn now(&self) -> Duration {
        self.now
    }
    fn sleep(&mut self, d: Duration) {
        self.sleeps.push(d);
        self.now += d;
    }
}

/// A legacy (mirror-shape) fixture bundle, `generation: "g1"`.
struct Legacy {
    _temp: TempDir,
    root: PathBuf,
    receipt: PathBuf,
    manifest: Value,
}

impl Legacy {
    fn new() -> Self {
        let temp = TempDir::new().unwrap();
        let root = temp.path().to_path_buf();
        let mut artifacts = serde_json::Map::new();
        for name in REQUIRED {
            let content = if name.ends_with(".generation.json") {
                sidecar_bytes("g1")
            } else {
                b"fixture".to_vec()
            };
            fs::write(root.join(name), &content).unwrap();
            artifacts.insert(
                (*name).to_owned(),
                json!({"sha256": sha_hex(&content), "bytes": content.len()}),
            );
        }
        let fixture = Self {
            receipt: root.join("receipt.json"),
            root,
            manifest: json!({"generation": "g1", "artifacts": artifacts}),
            _temp: temp,
        };
        fixture.save();
        fixture
    }
    fn save(&self) {
        fs::write(
            self.root.join(".generation.json"),
            serde_json::to_vec(&self.manifest).unwrap(),
        )
        .unwrap();
    }
    fn receipt(&self) -> Value {
        serde_json::from_slice(&fs::read(&self.receipt).unwrap()).unwrap()
    }
}

/// The serving report a correct node returns for `dir`'s verified bundle.
fn serving(dir: &Path, generation: &str) -> Value {
    let bundle = verified_bundle(dir).unwrap();
    json!({
        "id": generation,
        "identity": {
            "generation": {"id": generation},
            "content_digest": bundle.content_digest,
            "atomicity_verified": true
        },
        "disk": {"matches_loaded": true},
        "drift": {"ok": true},
        "semantic_generation": {"id": generation},
        "embedding": {
            "model_id": "bge-small-en-v1.5", "dimensions": 384,
            "metric": "cosine", "rejections": []
        }
    })
}

fn run_reconcile(
    f: &Legacy,
    probe: impl FnMut() -> Result<Value, String>,
    restart: impl FnMut() -> Result<(), String>,
    timeout: Duration,
) -> Result<Outcome, ReloadError> {
    let mut clock = FakeClock::default();
    reconcile(&f.root, &f.receipt, probe, restart, &mut clock, timeout)
}

const SIXTY: Duration = Duration::from_secs(60);

// --- 1-8: the legacy-marker `ReloadTests` ------------------------------------

#[test]
fn matching_server_does_not_restart() {
    let f = Legacy::new();
    let restarts = Cell::new(0);
    let out = run_reconcile(
        &f,
        || Ok(serving(&f.root, "g1")),
        || {
            restarts.set(restarts.get() + 1);
            Ok(())
        },
        SIXTY,
    )
    .unwrap();
    assert_eq!(out, Outcome::Current);
    assert_eq!(out.to_string(), "current");
    assert_eq!(
        restarts.get(),
        0,
        "an already-matching service is not restarted"
    );
    // The receipt carries the independently computed bundle hash.
    let pairs: Vec<(String, String)> = REQUIRED
        .iter()
        .map(|n| ((*n).to_owned(), sha_hex(&fs::read(f.root.join(n)).unwrap())))
        .collect();
    assert_eq!(
        f.receipt(),
        json!({"state": "served", "generation": "g1", "content_digest": expected_digest(&pairs)})
    );
}

#[test]
fn new_publication_reloads_then_records_verified_receipt() {
    let f = Legacy::new();
    let restarted = Rc::new(Cell::new(0));
    let (r1, r2) = (restarted.clone(), restarted.clone());
    let out = run_reconcile(
        &f,
        || {
            if r1.get() > 0 {
                Ok(serving(&f.root, "g1"))
            } else {
                Ok(json!({}))
            }
        },
        move || {
            // The pending receipt must already be on disk when the restart runs.
            r2.set(r2.get() + 1);
            Ok(())
        },
        SIXTY,
    )
    .unwrap();
    assert_eq!(out, Outcome::Reloaded);
    assert_eq!(out.to_string(), "reloaded");
    assert_eq!(restarted.get(), 1, "exactly one restart call");
    assert_eq!(f.receipt()["state"], "served");
}

#[test]
fn pending_receipt_is_written_before_the_restart_runs() {
    let f = Legacy::new();
    let seen = RefCell::new(None);
    let _ = run_reconcile(
        &f,
        || Ok(json!({})),
        || {
            *seen.borrow_mut() = Some(f.receipt());
            Ok(())
        },
        Duration::ZERO,
    );
    let seen = seen.into_inner().expect("restart ran");
    assert_eq!(seen["state"], "reload-pending");
    assert_eq!(seen["generation"], "g1");
}

#[test]
fn failed_reload_never_records_success() {
    let f = Legacy::new();
    let err = run_reconcile(
        &f,
        || Ok(json!({})),
        || Err("restart failed".to_owned()),
        SIXTY,
    )
    .unwrap_err();
    assert!(matches!(err, ReloadError::Reload(_)), "{err:?}");
    assert_eq!(f.receipt()["state"], "reload-pending");
}

#[test]
fn wrong_generation_after_restart_is_not_acknowledged() {
    let f = Legacy::new();
    let err = run_reconcile(&f, || Ok(json!({})), || Ok(()), Duration::ZERO).unwrap_err();
    assert!(matches!(err, ReloadError::Timeout), "{err:?}");
    assert_eq!(f.receipt()["state"], "reload-pending");
}

#[test]
fn timeout_polls_every_second_until_the_deadline() {
    let f = Legacy::new();
    let mut clock = FakeClock::default();
    let polls = Cell::new(0);
    let err = reconcile(
        &f.root,
        &f.receipt,
        || {
            polls.set(polls.get() + 1);
            Err("connection refused".to_owned())
        },
        || Ok(()),
        &mut clock,
        Duration::from_secs(3),
    )
    .unwrap_err();
    assert!(matches!(err, ReloadError::Timeout));
    // One probe before the restart, then polls at t=0,1,2,3; sleeps between.
    assert_eq!(polls.get(), 5);
    assert_eq!(clock.sleeps, vec![Duration::from_secs(1); 3]);
    assert_eq!(f.receipt()["state"], "reload-pending");
}

#[test]
fn hash_tamper_prevents_restart() {
    let f = Legacy::new();
    fs::write(f.root.join("ontology.ttl"), "tampered").unwrap();
    let restarts = Cell::new(0);
    let err = run_reconcile(
        &f,
        || Ok(json!({})),
        || {
            restarts.set(1);
            Ok(())
        },
        SIXTY,
    )
    .unwrap_err();
    assert!(matches!(err, ReloadError::Invalid(_)), "{err:?}");
    assert_eq!(restarts.get(), 0);
    assert!(!f.receipt.exists(), "a refused bundle writes no receipt");
}

#[test]
fn missing_semantic_and_inflight_publication_refused() {
    let mut f = Legacy::new();
    f.manifest["artifacts"]
        .as_object_mut()
        .unwrap()
        .remove("ontology-corpus.rvdb");
    f.save();
    assert!(matches!(
        verified_bundle(&f.root),
        Err(ReloadError::Invalid(_))
    ));
    let g = Legacy::new();
    fs::write(g.root.join(".promotion-in-flight"), "").unwrap();
    assert!(matches!(
        verified_bundle(&g.root),
        Err(ReloadError::Invalid(_))
    ));
}

#[test]
fn indirect_and_traversal_artifacts_refused() {
    let mut f = Legacy::new();
    f.manifest["artifacts"]["../escape"] = json!({"sha256": "x", "bytes": 0});
    f.save();
    assert!(matches!(
        verified_bundle(&f.root),
        Err(ReloadError::Invalid(_))
    ));
}

#[test]
fn concurrent_publication_after_restart_not_acknowledged() {
    let f = Legacy::new();
    let err = run_reconcile(
        &f,
        || Ok(json!({})),
        || {
            fs::write(f.root.join(".promotion-in-flight"), "").unwrap();
            Ok(())
        },
        SIXTY,
    )
    .unwrap_err();
    assert!(matches!(err, ReloadError::Invalid(_)), "{err:?}");
    assert_eq!(f.receipt()["state"], "reload-pending");
}

// --- 9-16: the C3 (`vault build` / `ontomark build`) marker ------------------

struct C3 {
    _temp: TempDir,
    root: PathBuf,
    manifest: Value,
}

impl C3 {
    const IDENTITY: &'static str = "visionGraph@abc1234";

    fn new() -> Self {
        let temp = TempDir::new().unwrap();
        let root = temp.path().to_path_buf();
        let mut names: Vec<&str> = REQUIRED.to_vec();
        names.sort_unstable();
        let mut artifacts = Vec::new();
        let mut pairs = Vec::new();
        for name in names {
            let content = if name.ends_with(".generation.json") {
                sidecar_bytes(Self::IDENTITY)
            } else {
                b"fixture".to_vec()
            };
            fs::write(root.join(name), &content).unwrap();
            let digest = sha_hex(&content);
            artifacts.push(json!({"name": name, "sha256": digest, "bytes": content.len()}));
            pairs.push((name.to_owned(), digest));
        }
        let fixture = Self {
            manifest: json!({
                "id": Self::IDENTITY, "commit": "abc1234",
                "content_digest": expected_digest(&pairs),
                "generated_at": "2026-09-22T00:00:00Z", "class_count": 8146,
                "page_count": 8454, "vocabulary_version": 1,
                "stale_after": "2026-10-06", "artifacts": artifacts
            }),
            root,
            _temp: temp,
        };
        fixture.save();
        fixture
    }
    fn save(&self) {
        fs::write(
            self.root.join(".generation.json"),
            serde_json::to_vec(&self.manifest).unwrap(),
        )
        .unwrap();
    }
    fn artifacts(&mut self) -> &mut Vec<Value> {
        self.manifest["artifacts"].as_array_mut().unwrap()
    }
    /// Recompute the declared digest from the listed artefacts, as Python did.
    fn restate_digest(&mut self) {
        let pairs: Vec<(String, String)> = self
            .artifacts()
            .iter()
            .map(|a| {
                (
                    a["name"].as_str().unwrap().to_owned(),
                    a["sha256"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        self.manifest["content_digest"] = json!(expected_digest(&pairs));
    }
}

#[test]
fn graph_tier_subdirectory_artifact_is_accepted() {
    let mut f = C3::new();
    fs::create_dir(f.root.join("graph")).unwrap();
    fs::write(f.root.join("graph/full.bin"), b"tier").unwrap();
    f.artifacts()
        .push(json!({"name": "graph/full.bin", "sha256": sha_hex(b"tier"), "bytes": 4}));
    f.restate_digest();
    f.save();
    let bundle = verified_bundle(&f.root).unwrap();
    // The served digest covers the subdirectory artefact too, and (here, where
    // the declared digest was restated from the same list) equals it.
    assert_eq!(
        bundle.content_digest,
        f.manifest["content_digest"].as_str().unwrap()
    );
}

#[test]
fn escaping_or_indirect_subpaths_are_refused() {
    let mut f = C3::new();
    fs::create_dir(f.root.join("graph")).unwrap();
    symlink(f.root.join("graph"), f.root.join("linked")).unwrap();
    let required: Vec<Value> = f.artifacts().clone();
    for name in [
        "graph/../../outside.bin",
        "/etc/passwd",
        "graph//full.bin",
        "./graph/full.bin",
        "graph\\full.bin",
        "linked/full.bin",
    ] {
        let mut artifacts = required.clone();
        artifacts.push(json!({"name": name, "sha256": "0".repeat(64), "bytes": 0}));
        f.manifest["artifacts"] = Value::Array(artifacts);
        f.save();
        assert!(
            matches!(verified_bundle(&f.root), Err(ReloadError::Invalid(_))),
            "{name} must be refused"
        );
    }
}

#[test]
fn symlinked_marker_and_symlinked_artifact_are_refused() {
    let f = C3::new();
    let real = f.root.join("real-marker.json");
    fs::rename(f.root.join(".generation.json"), &real).unwrap();
    symlink(&real, f.root.join(".generation.json")).unwrap();
    assert!(matches!(
        verified_bundle(&f.root),
        Err(ReloadError::Invalid(_))
    ));
    let g = C3::new();
    fs::rename(g.root.join("ontology.ttl"), g.root.join("elsewhere.ttl")).unwrap();
    symlink(g.root.join("elsewhere.ttl"), g.root.join("ontology.ttl")).unwrap();
    assert!(matches!(
        verified_bundle(&g.root),
        Err(ReloadError::Invalid(_))
    ));
}

#[test]
fn vault_build_bundle_is_accepted() {
    let f = C3::new();
    let bundle = verified_bundle(&f.root).unwrap();
    assert_eq!(bundle.generation, C3::IDENTITY);
    assert_eq!(
        bundle.content_digest,
        f.manifest["content_digest"].as_str().unwrap()
    );
}

#[test]
fn declared_corpus_digest_is_carried_not_compared() {
    // `content_digest` in a C3 marker is the source-corpus digest. A mismatch is
    // DISCARDED, not refused; what is served and proved is the artefact digest.
    let mut f = C3::new();
    let artefact_digest = verified_bundle(&f.root).unwrap().content_digest;
    f.manifest["content_digest"] = json!("0".repeat(64));
    f.save();
    let bundle = verified_bundle(&f.root).unwrap();
    assert_eq!(bundle.content_digest, artefact_digest);
    assert_ne!(bundle.content_digest, "0".repeat(64));
}

#[test]
fn a_tampered_artifact_still_refuses_whatever_the_declared_digest() {
    let f = C3::new();
    fs::write(f.root.join("ontology.ttl"), b"tampered").unwrap();
    f.save();
    assert!(matches!(
        verified_bundle(&f.root),
        Err(ReloadError::Invalid(_))
    ));
}

#[test]
fn identity_must_name_its_own_commit() {
    let mut f = C3::new();
    f.manifest["commit"] = json!("deadbee");
    f.save();
    assert!(matches!(
        verified_bundle(&f.root),
        Err(ReloadError::Invalid(_))
    ));
}

#[test]
fn missing_c3_field_is_refused() {
    let mut f = C3::new();
    f.manifest
        .as_object_mut()
        .unwrap()
        .remove("vocabulary_version");
    f.save();
    assert!(matches!(
        verified_bundle(&f.root),
        Err(ReloadError::Invalid(_))
    ));
}

#[test]
fn semantic_sidecar_must_declare_the_same_generation() {
    // Re-stamp the marker too, so the ONLY disagreement is the sidecar's
    // generation rather than its hash.
    let mut f = C3::new();
    let content = sidecar_bytes("visionGraph@other");
    fs::write(f.root.join(SIDECAR), &content).unwrap();
    for spec in f.artifacts().iter_mut() {
        if spec["name"] == SIDECAR {
            spec["sha256"] = json!(sha_hex(&content));
            spec["bytes"] = json!(content.len());
        }
    }
    f.restate_digest();
    f.save();
    assert!(matches!(
        verified_bundle(&f.root),
        Err(ReloadError::Invalid(_))
    ));
}

// --- additions: Python behaviour with no test of its own ---------------------

#[test]
fn sidecar_model_and_dimensions_are_enforced() {
    for (model, dims) in [("other-model", 384), ("bge-small-en-v1.5", 768)] {
        let mut f = C3::new();
        let content = serde_json::to_vec(
            &json!({"generatedAt": C3::IDENTITY, "embeddingModel": model, "dimensions": dims}),
        )
        .unwrap();
        fs::write(f.root.join(SIDECAR), &content).unwrap();
        for spec in f.artifacts().iter_mut() {
            if spec["name"] == SIDECAR {
                spec["sha256"] = json!(sha_hex(&content));
                spec["bytes"] = json!(content.len());
            }
        }
        f.save();
        assert!(
            matches!(verified_bundle(&f.root), Err(ReloadError::Invalid(_))),
            "{model}/{dims}"
        );
    }
}

#[test]
fn c3_marker_with_a_duplicate_artifact_name_or_non_list_artifacts_is_refused() {
    let mut f = C3::new();
    let first = f.artifacts()[0].clone();
    f.artifacts().push(first);
    f.save();
    assert!(matches!(
        verified_bundle(&f.root),
        Err(ReloadError::Invalid(_))
    ));
    let mut g = C3::new();
    g.manifest["artifacts"] = json!({});
    g.save();
    assert!(matches!(
        verified_bundle(&g.root),
        Err(ReloadError::Invalid(_))
    ));
}

#[test]
fn an_empty_id_falls_back_to_the_legacy_shape() {
    let mut f = Legacy::new();
    f.manifest["id"] = json!("");
    f.save();
    assert_eq!(verified_bundle(&f.root).unwrap().generation, "g1");
}

#[test]
fn size_mismatch_is_refused() {
    let mut f = Legacy::new();
    f.manifest["artifacts"]["ontology.ttl"]["bytes"] = json!(999);
    f.save();
    assert!(matches!(
        verified_bundle(&f.root),
        Err(ReloadError::Invalid(_))
    ));
}

#[test]
fn served_matches_requires_every_proof() {
    let f = Legacy::new();
    let bundle = verified_bundle(&f.root).unwrap();
    let good = serving(&f.root, "g1");
    assert!(served_matches(&good, &bundle));
    let breakers: [(&str, Value); 11] = [
        ("/id", json!("g0")),
        ("/identity/generation/id", json!("g0")),
        ("/identity/content_digest", json!("00")),
        ("/identity/atomicity_verified", json!(false)),
        ("/disk/matches_loaded", json!(false)),
        ("/drift/ok", json!(false)),
        ("/semantic_generation/id", json!("g0")),
        ("/embedding/model_id", json!("other")),
        ("/embedding/dimensions", json!(768)),
        ("/embedding/metric", json!("dot")),
        ("/embedding/rejections", json!(["bad"])),
    ];
    for (pointer, value) in breakers {
        let mut bad = good.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(!served_matches(&bad, &bundle), "{pointer}");
    }
    // A non-object report is "not matching" (Python raised AttributeError here).
    assert!(!served_matches(&json!([1, 2]), &bundle));
    assert!(!served_matches(&json!({}), &bundle));
}

#[test]
fn receipt_is_private_atomic_and_leaves_no_temp_file() {
    let f = Legacy::new();
    run_reconcile(&f, || Ok(serving(&f.root, "g1")), || Ok(()), SIXTY).unwrap();
    let mode = fs::metadata(&f.receipt).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    assert!(!f.root.join("receipt.json.tmp").exists());
    assert!(fs::read_to_string(&f.receipt).unwrap().ends_with('\n'));
}

// --- the binary, as a real process -------------------------------------------

mod cli {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const BIN: &str = env!("CARGO_BIN_EXE_reload_published_generation");

    fn run(args: &[&str]) -> Output {
        Command::new(BIN).args(args).output().unwrap()
    }

    fn code(o: &Output) -> i32 {
        o.status.code().unwrap()
    }

    fn stdout(o: &Output) -> String {
        String::from_utf8_lossy(&o.stdout).into_owned()
    }

    fn s(p: &Path) -> &str {
        p.to_str().unwrap()
    }

    async fn serve(report: Value) -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/loom/generation"))
            .respond_with(ResponseTemplate::new(200).set_body_json(report))
            .mount(&server)
            .await;
        server
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn matching_node_exits_zero_prints_current_and_runs_nothing() {
        let f = Legacy::new();
        let server = serve(serving(&f.root, "g1")).await;
        let ran = f.root.join("ran");
        let o = run(&[
            "--data-dir",
            s(&f.root),
            "--receipt",
            s(&f.receipt),
            "--url",
            &format!("{}/", server.uri()),
            "--",
            "/usr/bin/env",
            "touch",
            s(&ran),
        ]);
        assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stderr));
        assert_eq!(stdout(&o), "current\n");
        assert!(!ran.exists());
        assert_eq!(f.receipt()["state"], "served");
        assert!(f.root.join("receipt.json.lock").exists());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn stale_node_is_restarted_then_proved() {
        let f = Legacy::new();
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/loom/generation"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .up_to_n_times(1)
            .with_priority(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/loom/generation"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serving(&f.root, "g1")))
            .with_priority(2)
            .mount(&server)
            .await;
        let argv_out = f.root.join("argv");
        let o = run(&[
            "--data-dir",
            s(&f.root),
            "--receipt",
            s(&f.receipt),
            "--url",
            &server.uri(),
            "--timeout",
            "30",
            "--",
            "/bin/sh",
            "-c",
            "printf '%s|' \"$@\" > \"$0\"",
            s(&argv_out),
            "--foo",
            "--",
            "bar",
        ]);
        assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stderr));
        assert_eq!(stdout(&o), "reloaded\n");
        // argv reaches the command untouched, with no shell in between.
        assert_eq!(fs::read_to_string(&argv_out).unwrap(), "--foo|--|bar|");
        assert_eq!(f.receipt()["state"], "served");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn failing_restart_exits_one_and_leaves_pending() {
        let f = Legacy::new();
        let server = serve(json!({})).await;
        let o = run(&[
            "--data-dir",
            s(&f.root),
            "--receipt",
            s(&f.receipt),
            "--url",
            &server.uri(),
            "--",
            "/bin/false",
        ]);
        assert_eq!(code(&o), 1);
        assert_eq!(stdout(&o), "");
        assert_eq!(f.receipt()["state"], "reload-pending");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn restart_that_outlives_the_timeout_is_killed() {
        let f = Legacy::new();
        let server = serve(json!({})).await;
        let started = std::time::Instant::now();
        let o = run(&[
            "--data-dir",
            s(&f.root),
            "--receipt",
            s(&f.receipt),
            "--url",
            &server.uri(),
            "--timeout",
            "1",
            "--",
            "/bin/sleep",
            "30",
        ]);
        assert_eq!(code(&o), 1);
        assert!(started.elapsed() < Duration::from_secs(20));
        assert_eq!(f.receipt()["state"], "reload-pending");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn node_that_never_proves_the_bundle_times_out_with_exit_one() {
        let f = Legacy::new();
        let server = serve(json!({})).await;
        let o = run(&[
            "--data-dir",
            s(&f.root),
            "--receipt",
            s(&f.receipt),
            "--url",
            &server.uri(),
            "--timeout",
            "1",
            "--",
            "/bin/true",
        ]);
        assert_eq!(code(&o), 1);
        assert_eq!(f.receipt()["state"], "reload-pending");
    }

    #[test]
    fn tampered_bundle_exits_one_before_any_restart_or_receipt() {
        let f = Legacy::new();
        fs::write(f.root.join("ontology.ttl"), "tampered").unwrap();
        let ran = f.root.join("ran");
        let o = run(&[
            "--data-dir",
            s(&f.root),
            "--receipt",
            s(&f.receipt),
            "--url",
            "http://127.0.0.1:1",
            "--",
            "/usr/bin/env",
            "touch",
            s(&ran),
        ]);
        assert_eq!(code(&o), 1);
        assert!(!ran.exists());
        assert!(!f.receipt.exists());
    }

    #[test]
    fn usage_errors_exit_two() {
        let f = Legacy::new();
        let base = [
            "--data-dir",
            s(&f.root),
            "--receipt",
            s(&f.receipt),
            "--url",
            "http://127.0.0.1:1",
        ];
        let with = |extra: &[&str]| {
            let mut args = base.to_vec();
            args.extend_from_slice(extra);
            run(&args)
        };
        assert_eq!(code(&with(&[])), 2, "no reload command");
        assert_eq!(code(&with(&["--"])), 2, "empty reload command after --");
        assert_eq!(code(&with(&["--timeout", "0", "--", "/bin/true"])), 2);
        assert_eq!(code(&with(&["--timeout", "301", "--", "/bin/true"])), 2);
        assert_eq!(code(&with(&["--timeout", "abc", "--", "/bin/true"])), 2);
        assert_eq!(code(&run(&["--receipt", "r", "--url", "u", "--", "x"])), 2);
        assert_eq!(code(&run(&["--bogus", "1"])), 2);
        assert!(!f.receipt.exists());
    }

    #[test]
    fn a_second_invocation_cannot_take_the_lock() {
        let f = Legacy::new();
        let lock = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(f.root.join("receipt.json.lock"))
            .unwrap();
        lock.try_lock().unwrap();
        let o = run(&[
            "--data-dir",
            s(&f.root),
            "--receipt",
            s(&f.receipt),
            "--url",
            "http://127.0.0.1:1",
            "--",
            "/bin/true",
        ]);
        assert_eq!(code(&o), 1);
        assert!(!f.receipt.exists());
    }

    #[test]
    fn receipt_directory_is_created_private() {
        let f = Legacy::new();
        let receipt = f.root.join("state/loom/reload.json");
        let _ = run(&[
            "--data-dir",
            s(&f.root),
            "--receipt",
            s(&receipt),
            "--url",
            "http://127.0.0.1:1",
            "--timeout",
            "1",
            "--",
            "/bin/true",
        ]);
        let mode = fs::metadata(receipt.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o700);
    }

    #[test]
    fn verify_subcommand_checks_the_bundle_only() {
        let f = C3::new();
        let o = run(&["verify", "--data-dir", s(&f.root)]);
        assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stderr));
        let out: Value = serde_json::from_str(&stdout(&o)).unwrap();
        assert_eq!(out["generation"], C3::IDENTITY);
        assert_eq!(
            out["content_digest"],
            verified_bundle(&f.root).unwrap().content_digest
        );
        fs::write(f.root.join("ontology.ttl"), "tampered").unwrap();
        assert_eq!(code(&run(&["verify", "--data-dir", s(&f.root)])), 1);
        assert_eq!(code(&run(&["verify"])), 2);
    }
}
