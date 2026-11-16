//! `promote_generation` — validate a built bundle, install it where the service
//! reads it, stamp `promoted_at` and append the promotion log (ADR-141 D6).
//!
//! ```text
//! promote_generation --bundle DIR --data-dir DIR --source-repo DIR
//!     [--log FILE] [--promoted-by NAME] [--allow-rollback]
//! ```
//!
//! `--bundle` is the `data/` directory `ontomark build` wrote. `--log` defaults
//! to `<data-dir>/promotions.jsonl`; `--promoted-by` defaults to `$USER`. It
//! does not restart the service: the reload tick
//! (`reload_published_generation`) does that.
//!
//! Exit codes: 0 after printing `promoted <generation>` or `current
//! <generation>` (same generation already installed; nothing written), 1 for a
//! refused or failed promotion (a refusal leaves the installed generation
//! untouched), 2 for a usage error.

use std::path::PathBuf;
use std::process::ExitCode;

use loom_facade::promote::{promote, PromoteOptions, PromoteOutcome};

const USAGE: &str = "usage: promote_generation --bundle DIR --data-dir DIR --source-repo DIR \
[--log FILE] [--promoted-by NAME] [--allow-rollback]";

fn usage(message: &str) -> ExitCode {
    eprintln!("{USAGE}\npromote_generation: error: {message}");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mut bundle, mut data_dir, mut source_repo, mut log, mut by) =
        (None, None, None, None, None);
    let mut allow_rollback = false;
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if matches!(arg, "-h" | "--help") {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        if arg == "--allow-rollback" {
            allow_rollback = true;
            i += 1;
            continue;
        }
        let slot = match arg {
            "--bundle" => &mut bundle,
            "--data-dir" => &mut data_dir,
            "--source-repo" => &mut source_repo,
            "--log" => &mut log,
            "--promoted-by" => &mut by,
            _ => return usage(&format!("unrecognised argument: {arg}")),
        };
        i += 1;
        let Some(value) = args.get(i) else {
            return usage(&format!("{arg} needs a value"));
        };
        *slot = Some(value.clone());
        i += 1;
    }
    let (Some(bundle), Some(data_dir), Some(source_repo)) = (bundle, data_dir, source_repo) else {
        return usage("--bundle, --data-dir and --source-repo are required");
    };
    let data_dir = PathBuf::from(data_dir);
    let promoted_by = by
        .or_else(|| std::env::var("USER").ok())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_owned());
    let opts = PromoteOptions {
        bundle: PathBuf::from(bundle),
        log: log.map_or_else(|| data_dir.join("promotions.jsonl"), PathBuf::from),
        data_dir,
        source_repo: PathBuf::from(source_repo),
        promoted_by,
        allow_rollback,
        now: None,
    };
    match promote(&opts) {
        Ok(PromoteOutcome::Promoted(record)) => {
            println!("promoted {}", record.generation);
            ExitCode::SUCCESS
        }
        Ok(PromoteOutcome::Current(generation)) => {
            println!("current {generation}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("promote_generation: error: {e}");
            ExitCode::FAILURE
        }
    }
}
