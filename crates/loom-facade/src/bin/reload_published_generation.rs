//! `reload_published_generation` — reload only a complete, hash-verified
//! published bundle, and write the receipt after the served proof.
//!
//! ```text
//! reload_published_generation --data-dir DIR --receipt FILE --url URL
//!     [--timeout 1..300] -- RELOAD_COMMAND [ARGS...]
//! reload_published_generation verify --data-dir DIR
//! ```
//!
//! Run once per tick from the operator-installed timer. It does not download or
//! promote data, change deployment configuration, or construct a semantic index.
//! The reload command is run directly, never through a shell.
//!
//! `verify` checks the bundle only (no lock, receipt, probe or restart) and
//! prints `{generation, content_digest}`.
//!
//! Exit codes: 0 success (prints `current` or `reloaded`), 1 any failure
//! (refused bundle, failed or timed-out restart, unproved service, lock held),
//! 2 usage error.

use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use loom_facade::reload::{reconcile, verified_bundle, CommandReloader, HttpProbe, SystemClock};

const USAGE: &str = "usage: reload_published_generation --data-dir DIR --receipt FILE --url URL \
[--timeout 1..300] -- RELOAD_COMMAND [ARGS...]\n       reload_published_generation verify --data-dir DIR";

struct Args {
    data_dir: Option<PathBuf>,
    receipt: Option<PathBuf>,
    url: Option<String>,
    timeout: u64,
    command: Vec<String>,
}

enum Parsed {
    Help,
    Usage(String),
    Run(Args),
}

fn parse(args: &[String]) -> Parsed {
    let mut out = Args {
        data_dir: None,
        receipt: None,
        url: None,
        timeout: 60,
        command: Vec::new(),
    };
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--" {
            out.command = args[i + 1..].to_vec();
            break;
        }
        if arg == "-h" || arg == "--help" {
            return Parsed::Help;
        }
        if !arg.starts_with('-') {
            // argparse REMAINDER: the first positional starts the command.
            out.command = args[i..].to_vec();
            break;
        }
        let (flag, inline) = match arg.split_once('=') {
            Some((flag, value)) => (flag, Some(value.to_owned())),
            None => (arg.as_str(), None),
        };
        if !matches!(flag, "--data-dir" | "--receipt" | "--url" | "--timeout") {
            return Parsed::Usage(format!("unrecognised argument: {arg}"));
        }
        let value = if let Some(value) = inline {
            value
        } else {
            i += 1;
            match args.get(i) {
                Some(value) => value.clone(),
                None => return Parsed::Usage(format!("{flag} needs a value")),
            }
        };
        match flag {
            "--data-dir" => out.data_dir = Some(PathBuf::from(value)),
            "--receipt" => out.receipt = Some(PathBuf::from(value)),
            "--url" => out.url = Some(value),
            _ => match value.parse() {
                Ok(timeout) => out.timeout = timeout,
                Err(_) => return Parsed::Usage(format!("--timeout: invalid int value: {value}")),
            },
        }
        i += 1;
    }
    Parsed::Run(out)
}

fn usage(message: &str) -> ExitCode {
    eprintln!("{USAGE}\nreload_published_generation: error: {message}");
    ExitCode::from(2)
}

fn fail(message: &dyn std::fmt::Display) -> ExitCode {
    eprintln!("reload_published_generation: error: {message}");
    ExitCode::FAILURE
}

fn verify(args: &[String]) -> ExitCode {
    let Parsed::Run(args) = parse(args) else {
        return usage("verify takes --data-dir DIR only");
    };
    let Some(data_dir) = args
        .data_dir
        .filter(|_| args.receipt.is_none() && args.url.is_none() && args.command.is_empty())
    else {
        return usage("verify takes --data-dir DIR only");
    };
    match verified_bundle(&data_dir) {
        Ok(bundle) => {
            println!(
                "{}",
                serde_json::json!({
                    "generation": bundle.generation,
                    "content_digest": bundle.content_digest,
                })
            );
            ExitCode::SUCCESS
        }
        Err(e) => fail(&e),
    }
}

fn tick(
    data_dir: &Path,
    receipt: &Path,
    url: &str,
    timeout: u64,
    command: Vec<String>,
) -> ExitCode {
    if let Some(parent) = receipt.parent().filter(|p| !p.as_os_str().is_empty()) {
        if let Err(e) = fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)
        {
            return fail(&format!("{}: {e}", parent.display()));
        }
    }
    let mut lock_path = receipt.as_os_str().to_owned();
    lock_path.push(".lock");
    let lock = match fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&lock_path)
    {
        Ok(lock) => lock,
        Err(e) => return fail(&format!("{}: {e}", Path::new(&lock_path).display())),
    };
    // Held until the process exits: concurrent ticks are refused, not queued.
    if let Err(e) = lock.try_lock() {
        return fail(&format!("another reload holds the lock: {e}"));
    }
    let probe = match HttpProbe::new(url) {
        Ok(probe) => probe,
        Err(e) => return fail(&e),
    };
    let timeout = Duration::from_secs(timeout);
    match reconcile(
        data_dir,
        receipt,
        probe,
        CommandReloader::new(command, timeout),
        &mut SystemClock::new(),
        timeout,
    ) {
        Ok(outcome) => {
            println!("{outcome}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(&e),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "verify") {
        return verify(&args[1..]);
    }
    let args = match parse(&args) {
        Parsed::Help => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Parsed::Usage(message) => return usage(&message),
        Parsed::Run(args) => args,
    };
    let (Some(data_dir), Some(receipt), Some(url)) = (args.data_dir, args.receipt, args.url) else {
        return usage("--data-dir, --receipt and --url are required");
    };
    if args.command.is_empty() || !(1..=300).contains(&args.timeout) {
        return usage("a reload command and timeout 1..300 are required");
    }
    tick(&data_dir, &receipt, &url, args.timeout, args.command)
}
