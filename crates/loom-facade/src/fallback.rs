//! Start-up fallback to a retained previous generation (ADR-141 amendment).
//!
//! If the bundle in the data directory fails to activate, the service does not
//! have to stay down: promotion keeps every bundle it installed
//! (`loom-nightly-promotion.sh` keeps the newest `LOOM_KEEP_BUNDLES` under
//! `<state>/bundles/<commit>/data`) and logs each install in
//! `<data>/promotions.jsonl`. [`select`] walks that log newest first and takes
//! the first generation whose retained bundle passes the same checks promotion
//! applies ([`crate::promote::validate_bundle`]).
//!
//! # What this never does
//!
//! * It never writes: not into the data directory, not into the retained
//!   bundle. The failed bundle stays exactly as it was for inspection.
//! * It never serves a bundle that fails validation, and never one the log does
//!   not name. A directory that merely sits under the retained root is ignored.
//! * It never reports itself as normal: see [`FallbackInfo`] and the `/health`
//!   `status` field.
//!
//! The source-repository check promotion makes cannot run here, because the
//! service has no checkout. The promotion log line is its stand-in: it was
//! written only after that check passed at promotion time.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::promote::{validate_bundle, PromotionRecord};

/// File name of the promotion log inside the data directory.
pub const PROMOTION_LOG: &str = "promotions.jsonl";

/// What the service reports when it started on a retained generation instead of
/// the one in the data directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FallbackInfo {
    /// The generation in the data directory that failed to activate, or `None`
    /// when its marker could not be read at all.
    pub failed_generation: Option<String>,
    /// Why it failed to activate (the activation error, verbatim).
    pub reason: String,
    /// The generation being served instead.
    pub serving_generation: String,
    /// The retained directory it is served from.
    pub serving_dir: String,
}

/// A retained generation that passed validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selected {
    /// The generation id the retained bundle declares.
    pub generation: String,
    /// The retained `data/` directory.
    pub dir: PathBuf,
}

/// Choose the retained generation to fall back to, or `None` when there is
/// none: no retained root configured, no readable log, or nothing that
/// validates. `failed` (the generation that failed to activate) is skipped.
#[must_use]
pub fn select(config: &Config, failed: Option<&str>) -> Option<Selected> {
    let root = Path::new(config.retained_bundles_dir.as_deref()?);
    let log = fs::read_to_string(config.data_dir().join(PROMOTION_LOG)).ok()?;
    let mut tried: Vec<String> = Vec::new();
    for record in log
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<PromotionRecord>(line).ok())
    {
        if Some(record.generation.as_str()) == failed || tried.contains(&record.generation) {
            continue;
        }
        tried.push(record.generation.clone());
        // The commit names a directory: only a plain hex sha may.
        if record.commit.len() < 7 || !record.commit.bytes().all(|b| b.is_ascii_hexdigit()) {
            tracing::warn!(generation = %record.generation, "fallback candidate skipped: bad commit in the promotion log");
            continue;
        }
        let dir = root.join(&record.commit).join("data");
        match validate_bundle(&dir, None) {
            Ok(valid) if valid.generation == record.generation => {
                return Some(Selected {
                    generation: valid.generation,
                    dir,
                });
            }
            Ok(valid) => tracing::warn!(
                logged = %record.generation,
                found = %valid.generation,
                "fallback candidate skipped: the retained bundle is not the logged generation"
            ),
            Err(e) => tracing::warn!(
                generation = %record.generation,
                error = %e,
                "fallback candidate skipped: it does not validate"
            ),
        }
    }
    None
}

/// `config` with the paths of the artefacts the service loads moved into `dir`,
/// keeping their file names. The ledger path is left alone: it is the
/// governance record, not a bundle artefact.
///
/// The semantic artefact is the exception. Opening it mutates it (redb repacks
/// on open), and the retained bundle must stay byte-for-byte as promoted, so it
/// is never opened in place. When the entrypoint has already pointed
/// `hnsw_artifact` at a writable runtime directory (anywhere other than the
/// data directory), the retained artefact and its sidecar are copied into a
/// `fallback/` subdirectory of that and opened there. Otherwise, or if the copy
/// fails, `hnsw_artifact` names a file that does not exist, so the semantic
/// index reports itself unavailable and the node serves the lexical floor.
#[must_use]
pub fn rebased(config: &Config, dir: &Path) -> Config {
    let name_of = |path: &str| -> String {
        Path::new(path)
            .file_name()
            .map_or_else(|| path.to_owned(), |n| n.to_string_lossy().into_owned())
    };
    let move_into = |path: &str| dir.join(name_of(path)).to_string_lossy().into_owned();
    Config {
        index_path: move_into(&config.index_path),
        prose_path: move_into(&config.prose_path),
        hnsw_artifact: writable_semantic_copy(config, dir, &name_of(&config.hnsw_artifact)),
        ..config.clone()
    }
}

fn writable_semantic_copy(config: &Config, dir: &Path, name: &str) -> String {
    let unavailable = || dir.join(format!("{name}.unavailable-in-fallback"));
    let runtime = Path::new(&config.hnsw_artifact)
        .parent()
        .filter(|runtime| *runtime != config.data_dir());
    let target = runtime.map(|runtime| runtime.join("fallback"));
    let copied = target.as_deref().and_then(|target| {
        fs::create_dir_all(target).ok()?;
        for file in [name.to_owned(), format!("{name}.generation.json")] {
            let from = dir.join(&file);
            if from.is_file() {
                fs::copy(&from, target.join(&file)).ok()?;
            }
        }
        Some(target.join(name))
    });
    if copied.is_none() {
        tracing::warn!(
            "the retained semantic artefact is not opened in place; semantic fallback is unavailable while serving the fallback generation"
        );
    }
    copied
        .unwrap_or_else(unavailable)
        .to_string_lossy()
        .into_owned()
}
