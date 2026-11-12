//! Prints a sidecar's reading with the facts of its original: `reading <name.json> [<name.facts.json>]`,
//! the facts beside it by default. The corpus' goldens are its output.

use oni_settings::migrate::{self, Facts, M3};
use oni_settings::sidecar::Loaded;
use serde::Deserialize;
use std::path::PathBuf;

/// What the original beside a sidecar says, as `<name>.facts.json` keeps it.
#[derive(Deserialize)]
struct Kept {
    mosaic: bool,
    matrix: Option<M3>,
    scan: Option<(f64, f64)>,
}

fn main() {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let Some(path) = args.first() else {
        eprintln!("usage: reading <name.json> [<name.facts.json>]");
        std::process::exit(2);
    };
    let name = path.to_string_lossy();
    let stem = name.strip_suffix(".json").unwrap_or(&name).to_owned();
    let facts_path = args.get(1).cloned().unwrap_or_else(|| PathBuf::from(format!("{stem}.facts.json")));
    let kept: Kept = serde_json::from_slice(&std::fs::read(&facts_path).unwrap_or_else(|e| panic!("{}: {e}", facts_path.display())))
        .unwrap_or_else(|e| panic!("{}: {e}", facts_path.display()));
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{name}: {e}"));
    let facts = Facts { mosaic: kept.mosaic, matrix: &|| kept.matrix, scan: &|_| kept.scan };
    match migrate::converted(&bytes, &facts) {
        Ok(converted) => print!("{}", Loaded::of(converted).said()),
        Err(e) => {
            println!("refused: {e}");
            std::process::exit(1);
        }
    }
}
