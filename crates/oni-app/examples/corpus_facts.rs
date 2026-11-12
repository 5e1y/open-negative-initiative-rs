//! A sidecar's original as a corpus `.facts.json` says it, from its metadata, never its pixels:
//! `cargo run -p oni-app --example corpus_facts -- <support> <name.json>`.

use oni_app::migration::facts;
use oni_settings::PipelineSettings;
use std::path::PathBuf;

fn main() {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let (Some(support), Some(sidecar)) = (args.first(), args.get(1)) else {
        eprintln!("usage: corpus_facts <support> <name.json>");
        std::process::exit(2);
    };
    let original = sidecar.with_extension("");
    // A file kept in the cloud is never downloaded for its facts: it reads as absent.
    let away = std::fs::metadata(&original).is_ok_and(|m| oni_settings::sidecar::identity::in_cloud(&m));
    let present = original.is_file() && !away;
    let mosaic = if present { oni_decode::decodes_via_mosaic(&original) } else { oni_decode::is_raw(&original) };
    let table = facts::matrices(support);
    let matrix = present.then(|| facts::matrix_of(&table, &original).ok().flatten()).flatten();
    let scan = present.then(|| facts::scan_of(&original, &PipelineSettings::default())).flatten();
    let facts = serde_json::json!({ "mosaic": mosaic, "matrix": matrix, "scan": scan });
    println!("{}", serde_json::to_string(&facts).unwrap_or_default());
}
