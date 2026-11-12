//! Prints a sidecar's conversion to this format, writing nothing: `migrate <sidecar.json> [--plain]`,
//! `--plain` for an original outside the RAW engine. No fact of the original is asked.

use oni_settings::{document, migrate};

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: migrate <sidecar.json> [--plain]");
        std::process::exit(2);
    };
    let mosaic = args.next().as_deref() != Some("--plain");
    let data = std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    match migrate::converted(&data, &migrate::Facts::bare(mosaic)).map_err(|e| e.to_string()).and_then(|c| document::encode(&c.document).map_err(|e| e.to_string())) {
        Ok(text) => print!("{}", String::from_utf8_lossy(&text)),
        Err(e) => {
            eprintln!("{path}: {e}");
            std::process::exit(1);
        }
    }
}
