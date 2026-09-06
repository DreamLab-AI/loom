//! EXP-013 — immutable loaded-bundle activation (ADR-135 closeout).
//!
//! The estate review asked for exactly this experiment: *"Stage and validate an
//! immutable bundle, then activate it as one serving identity. Inject incomplete
//! downloads, mixed hashes, interrupted promotion and process reload; compare
//! scaffold, graph and chat generation fields."*
//!
//! Each failure below is INJECTED against a real filesystem — a marker listing a
//! file that is not there, a file whose bytes were changed after the marker was
//! written, a directory caught mid-swap — rather than mocked, because the whole
//! finding was that the previous checks passed on artefacts nobody had hashed.
//!
//! The reload group is the one that closes the review's central mismatch: after
//! a promotion lands, a RUNNING process must keep reporting the generation it
//! loaded, on every surface, and only a restart may change that.

mod common;

use common::{call, write_commit_marker, write_commit_marker_at, TestEnvBuilder, FIXTURE};

use axum::http::StatusCode;
use loom_domain::{BundleError, BundlePhase, GenerationStore};
use loom_facade::bundle::{BundlePromoter, LoadedBundle, IN_FLIGHT_MARKER};
use serde_json::{json, Value};
use tempfile::TempDir;

/// A data directory holding the golden fixture as `scaffold-index.json`.
fn fixture_dir() -> TempDir {
    let dir = TempDir::new().expect("tempdir");
    std::fs::write(dir.path().join("scaffold-index.json"), FIXTURE).expect("write fixture");
    dir
}

fn index_path(dir: &TempDir) -> String {
    dir.path()
        .join("scaffold-index.json")
        .to_string_lossy()
        .into_owned()
}

// --- activation: the happy path ---------------------------------------------

#[test]
fn activation_captures_one_identity_over_verified_content() {
    let dir = fixture_dir();
    write_commit_marker(dir.path(), &["scaffold-index.json"]);

    let bundle = LoadedBundle::activate(&index_path(&dir)).expect("verified bundle activates");
    let id = bundle.identity();

    assert!(id.atomicity_verified, "a marker-verified bundle is attested");
    assert_eq!(id.phase, BundlePhase::Activated);
    assert_eq!(id.artefacts.len(), 1, "the marker's one artefact is covered");
    assert_eq!(
        id.content_digest.len(),
        64,
        "content digest is a sha256 hex: {:?}",
        id.content_digest
    );
    assert_eq!(
        id.generation.id.0, "2026-09-05T00:00:00Z",
        "the mirror marker is the generation source"
    );
}

/// A directory with no marker at all is a development checkout, not a fault —
/// but the identity it produces must not CLAIM verification it did not do.
#[test]
fn marker_less_directory_activates_degraded_but_still_content_bound() {
    let dir = fixture_dir();
    let bundle = LoadedBundle::activate_or_degraded(&index_path(&dir)).expect("degrades");
    let id = bundle.identity();

    assert!(
        !id.atomicity_verified,
        "an unattested bundle must not report as verified"
    );
    assert!(
        !id.artefacts.is_empty(),
        "the degraded identity still hashes the bytes it loaded"
    );
    assert_eq!(id.artefacts[0].name, "scaffold-index.json");
    assert_eq!(
        id.artefacts[0].sha256,
        common::sha256_hex(FIXTURE.as_bytes()),
        "the captured digest is of the real file"
    );
}

/// Two directories holding DIFFERENT content under the SAME generation string
/// must not be mistaken for one another. This is the discriminator the review
/// found missing: generation equality was the only test available.
#[test]
fn same_generation_over_different_content_yields_different_identities() {
    let a = fixture_dir();
    write_commit_marker(a.path(), &["scaffold-index.json"]);
    let bundle_a = LoadedBundle::activate(&index_path(&a)).unwrap();

    let b = TempDir::new().unwrap();
    let mutated = FIXTURE.replace("Knowledge Graph", "Knowledge Graphs");
    assert_ne!(mutated, FIXTURE, "the fixture must actually change");
    std::fs::write(b.path().join("scaffold-index.json"), &mutated).unwrap();
    write_commit_marker(b.path(), &["scaffold-index.json"]);
    let bundle_b = LoadedBundle::activate(&index_path(&b)).unwrap();

    assert_eq!(
        bundle_a.identity().generation.id,
        bundle_b.identity().generation.id,
        "fixture: both stamp the same generation"
    );
    assert!(
        !bundle_a.identity().is_same_bundle(bundle_b.identity()),
        "different content under one generation must not compare equal"
    );
}

// --- injected failure 1: incomplete download --------------------------------

#[test]
fn incomplete_download_is_rejected_by_name() {
    let dir = fixture_dir();
    // The marker promises a prose index the download never finished fetching.
    std::fs::write(dir.path().join("prose-index.json"), "{}").unwrap();
    write_commit_marker(dir.path(), &["scaffold-index.json", "prose-index.json"]);
    std::fs::remove_file(dir.path().join("prose-index.json")).unwrap();

    let err = LoadedBundle::activate(&index_path(&dir)).unwrap_err();
    match err {
        BundleError::MissingArtefact { name, .. } => assert_eq!(name, "prose-index.json"),
        other => panic!("expected MissingArtefact, got {other:?}"),
    }
}

// --- injected failure 2: mixed hashes ---------------------------------------

#[test]
fn mixed_generation_is_rejected_with_both_digests_named() {
    let dir = fixture_dir();
    write_commit_marker(dir.path(), &["scaffold-index.json"]);
    // A later generation's file lands under the earlier generation's marker —
    // the window the file-at-a-time promotion leaves open.
    std::fs::write(
        dir.path().join("scaffold-index.json"),
        FIXTURE.replace("2026-08-09", "2026-09-01"),
    )
    .unwrap();

    let err = LoadedBundle::activate(&index_path(&dir)).unwrap_err();
    match err {
        BundleError::HashMismatch { name, got, want } => {
            assert_eq!(name, "scaffold-index.json");
            assert_ne!(got, want, "the two digests must genuinely differ");
        }
        other => panic!("expected HashMismatch, got {other:?}"),
    }
}

/// The old `verify_atomicity` returned success when the marker recorded no
/// artefacts. A "verification" that passes because there was nothing to check is
/// worse than none, because it is reported as a pass.
#[test]
fn marker_with_no_artefact_digests_is_unverifiable_not_verified() {
    let dir = fixture_dir();
    std::fs::write(
        dir.path().join(".generation.json"),
        json!({ "generation": "2026-09-05T00:00:00Z", "artifacts": {} }).to_string(),
    )
    .unwrap();

    let err = LoadedBundle::activate(&index_path(&dir)).unwrap_err();
    assert!(
        matches!(err, BundleError::EmptyManifest { .. }),
        "expected EmptyManifest, got {err:?}"
    );
}

// --- injected failure 3: interrupted promotion ------------------------------

#[test]
fn a_directory_mid_promotion_is_refused() {
    let dir = fixture_dir();
    write_commit_marker(dir.path(), &["scaffold-index.json"]);
    // A promoter crashed between declaring the swap and committing it.
    std::fs::write(dir.path().join(IN_FLIGHT_MARKER), "/staging").unwrap();

    let err = LoadedBundle::activate_or_degraded(&index_path(&dir)).unwrap_err();
    assert!(
        matches!(err, BundleError::NoCommitMarker { .. }),
        "a mid-promotion directory must never activate, got {err:?}"
    );

    // Clearing the sentinel (the promoter's final step) makes it servable again.
    std::fs::remove_file(dir.path().join(IN_FLIGHT_MARKER)).unwrap();
    assert!(LoadedBundle::activate(&index_path(&dir)).is_ok());
}

// --- atomic promotion --------------------------------------------------------

#[test]
fn promotion_stages_verifies_and_swaps_as_one_commit() {
    let target = fixture_dir();
    write_commit_marker(target.path(), &["scaffold-index.json"]);
    let before = LoadedBundle::activate(&index_path(&target)).unwrap();

    // A new generation, staged and self-consistent.
    let staging = TempDir::new().unwrap();
    let next = FIXTURE.replace("2026-08-09T00:00:00Z", "2026-09-05T12:00:00Z");
    std::fs::write(staging.path().join("scaffold-index.json"), &next).unwrap();
    write_commit_marker_at(
        staging.path(),
        staging.path(),
        &["scaffold-index.json"],
        "2026-09-05T12:00:00Z",
    );

    BundlePromoter::new(staging.path(), target.path())
        .promote()
        .expect("a verified staging bundle promotes");

    // The sentinel is cleared and the target activates as the NEW generation.
    assert!(!target.path().join(IN_FLIGHT_MARKER).exists());
    let after = LoadedBundle::activate(&index_path(&target)).expect("promoted bundle activates");
    assert_eq!(after.identity().generation.id.0, "2026-09-05T12:00:00Z");
    assert!(!before.identity().is_same_bundle(after.identity()));
}

#[test]
fn promotion_refuses_an_incomplete_staging_set_without_touching_the_target() {
    let target = fixture_dir();
    write_commit_marker(target.path(), &["scaffold-index.json"]);
    let original = std::fs::read(target.path().join("scaffold-index.json")).unwrap();

    // Staging promises two files and ships one.
    let staging = TempDir::new().unwrap();
    std::fs::write(staging.path().join("scaffold-index.json"), FIXTURE).unwrap();
    std::fs::write(staging.path().join("prose-index.json"), "{}").unwrap();
    write_commit_marker_at(
        staging.path(),
        staging.path(),
        &["scaffold-index.json", "prose-index.json"],
        "2026-09-05T12:00:00Z",
    );
    std::fs::remove_file(staging.path().join("prose-index.json")).unwrap();

    let err = BundlePromoter::new(staging.path(), target.path())
        .promote()
        .unwrap_err();
    assert!(matches!(err, BundleError::MissingArtefact { .. }), "{err:?}");

    // The target is untouched and still activates as its original generation.
    assert_eq!(
        std::fs::read(target.path().join("scaffold-index.json")).unwrap(),
        original,
        "a refused promotion must not have written to the target"
    );
    assert!(LoadedBundle::activate(&index_path(&target)).is_ok());
}

// --- verify_atomicity on the serving path ------------------------------------

#[tokio::test]
async fn verify_atomicity_runs_per_request_and_reports_post_activation_drift() {
    let dir = fixture_dir();
    write_commit_marker(dir.path(), &["scaffold-index.json"]);
    let bundle = LoadedBundle::activate(&index_path(&dir)).unwrap();

    bundle
        .verify_atomicity()
        .await
        .expect("a freshly activated bundle verifies");

    // Someone rewrites an artefact underneath the running process.
    std::fs::write(
        dir.path().join("scaffold-index.json"),
        FIXTURE.replace("mature", "emerging"),
    )
    .unwrap();

    let err = bundle.verify_loaded().unwrap_err();
    assert!(
        matches!(err, BundleError::ActivatedDrift { .. }),
        "expected ActivatedDrift, got {err:?}"
    );
}

#[tokio::test]
async fn generation_route_invokes_the_verifier_and_publishes_the_result() {
    let env = TestEnvBuilder::new().with_commit_marker(true).build();
    let (status, body) = call(env.router(), "GET", "/loom/generation", None).await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(
        body["drift"]["checked"], json!(true),
        "the serving path must actually run verify_atomicity"
    );
    assert_eq!(body["drift"]["ok"], json!(true));
    assert_eq!(body["identity"]["phase"], json!("activated"));
    assert_eq!(body["disk"]["matches_loaded"], json!(true));
    // Python-parity: the descriptor fields are still at the top level.
    assert!(body["id"].is_string() && body["source"].is_string());
}

// --- process reload semantics ------------------------------------------------

/// THE central case. A promotion lands while a process is running: every serving
/// surface must keep reporting the LOADED generation, and the pending change
/// must be visible as pending rather than silently adopted.
#[tokio::test]
async fn a_promotion_after_activation_does_not_change_what_a_running_process_serves() {
    let env = TestEnvBuilder::new().with_commit_marker(true).build();
    let loaded = env.state.generation.identity().generation.id.0.clone();
    let loaded_digest = env.state.generation.identity().content_digest.clone();

    // Collect the generation every surface reports BEFORE the promotion.
    let before = generation_fields(&env).await;
    for (surface, seen) in &before {
        assert_eq!(seen, &loaded, "{surface} must report the loaded generation");
    }

    // A new generation is promoted into the data directory the process loaded
    // from. The process is NOT restarted.
    let data_dir = std::path::Path::new(&env.state.config.index_path)
        .parent()
        .unwrap()
        .to_path_buf();
    let staging = TempDir::new().unwrap();
    let next = FIXTURE.replace("2026-08-09T00:00:00Z", "2026-09-06T00:00:00Z");
    std::fs::write(staging.path().join("scaffold-index.json"), &next).unwrap();
    write_commit_marker_at(
        staging.path(),
        staging.path(),
        &["scaffold-index.json"],
        "2026-09-06T00:00:00Z",
    );
    BundlePromoter::new(staging.path(), &data_dir)
        .promote()
        .expect("promotion succeeds");

    // Every surface still reports the LOADED generation — unchanged.
    let after = generation_fields(&env).await;
    assert_eq!(
        before, after,
        "a disk promotion must not move any served generation field"
    );

    // …and the pending difference is REPORTED rather than hidden.
    let (_, health) = call(env.router(), "GET", "/health", None).await;
    assert_eq!(health["serving_bundle"]["disk_matches_loaded"], json!(false));
    assert_eq!(
        health["serving_bundle"]["disk_generation"]["id"],
        json!("2026-09-06T00:00:00Z"),
        "the disk view shows the promoted generation"
    );
    assert_eq!(
        health["generation"]["id"],
        json!(loaded),
        "the SERVED generation is still the loaded one"
    );
    assert_eq!(
        health["serving_bundle"]["identity"]["content_digest"],
        json!(loaded_digest)
    );

    // A RELOAD — a new process over the same directory — adopts it.
    let reloaded =
        LoadedBundle::activate(&env.state.config.index_path).expect("reload activates the new set");
    assert_eq!(reloaded.identity().generation.id.0, "2026-09-06T00:00:00Z");
    assert_ne!(reloaded.identity().content_digest, loaded_digest);
}

/// Every generation-bearing surface, so a single promotion cannot move one of
/// them while leaving the others behind.
async fn generation_fields(env: &common::TestEnv) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();

    let (_, health) = call(env.router(), "GET", "/health", None).await;
    out.push(("health.generation", str_at(&health, &["generation", "id"])));
    out.push((
        "health.serving_bundle",
        str_at(&health, &["serving_bundle", "identity", "generation", "id"]),
    ));

    let (_, gen) = call(env.router(), "GET", "/loom/generation", None).await;
    out.push(("generation.id", str_at(&gen, &["id"])));

    let (_, scaffold) = call(
        env.router(),
        "POST",
        "/loom/scaffold",
        Some(json!({ "prompt": "knowledge graph" })),
    )
    .await;
    out.push((
        "scaffold.generation",
        str_at(&scaffold, &["generation", "id"]),
    ));
    out.push((
        "scaffold.grounding",
        str_at(&scaffold, &["grounding", "generation"]),
    ));

    let (_, chat) = call(
        env.router(),
        "POST",
        "/v1/chat/completions",
        Some(json!({ "messages": [{ "role": "user", "content": "knowledge graph" }] })),
    )
    .await;
    // No backend is configured in this env, so chat fails — and the failure body
    // must ALSO carry the loaded generation (ADR-138 closeout).
    out.push((
        "chat.loom.grounding",
        str_at(&chat, &["loom", "grounding", "generation"]),
    ));

    out
}

fn str_at(v: &Value, path: &[&str]) -> String {
    let mut cur = v;
    for k in path {
        cur = &cur[*k];
    }
    cur.as_str()
        .unwrap_or_else(|| panic!("{path:?} is not a string in {v}"))
        .to_owned()
}

/// The bundle's lifecycle phase advances once it has answered something —
/// `activated` and `served` are different facts, as the review asked.
#[tokio::test]
async fn phase_advances_from_activated_to_served_on_first_request() {
    let env = TestEnvBuilder::new().with_commit_marker(true).build();
    let (_, before) = call(env.router(), "GET", "/loom/generation", None).await;
    assert_eq!(before["identity"]["phase"], json!("activated"));

    let (_, _) = call(
        env.router(),
        "POST",
        "/loom/scaffold",
        Some(json!({ "prompt": "knowledge graph" })),
    )
    .await;

    let (_, after) = call(env.router(), "GET", "/health", None).await;
    assert_eq!(
        after["serving_bundle"]["identity"]["phase"],
        json!("served"),
        "a bundle that has answered is served, not merely activated"
    );
}
