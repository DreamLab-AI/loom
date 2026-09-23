//! `promote_vault_build` — turn a `vault build --with-rvdb` bundle's portable
//! records into Loom's serving HNSW artefact, in place.
//!
//! Usage: `promote_vault_build --data <bundle data dir>`
//!
//! See [`loom_vector_ruvector::artifact::promote_vault_build`] for what is
//! verified and written. Exit 0 on success, 2 on refusal.

use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some("--data"), Some(dir)) = (args.next().as_deref(), args.next()) else {
        eprintln!("usage: promote_vault_build --data <bundle data dir>");
        std::process::exit(2);
    };
    let data = PathBuf::from(dir);
    match loom_vector_ruvector::artifact::promote_vault_build(&data) {
        Ok(report) => println!(
            "promoted {}: {} records -> ontology-corpus.rvdb (sha256 {})",
            report.generation, report.records, report.artifact_sha256
        ),
        Err(e) => {
            eprintln!("promotion refused: {e}");
            std::process::exit(2);
        }
    }
}
