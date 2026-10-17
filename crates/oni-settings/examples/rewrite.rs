//! Prints a sidecar as this crate rewrites it: `rewrite <sidecar.json> [--plain]`.

use oni_settings::codec::{self, Source};

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: rewrite <sidecar.json> [--plain]");
        std::process::exit(2);
    };
    let mosaic = args.next().as_deref() != Some("--plain");
    let data = std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let source = Source { mosaic: &|| mosaic, plain: &|| !mosaic };
    match codec::decode_with(&data, &source, true) {
        Ok(d) => match codec::encode(&d.settings, d.imported_at, Some(d.label), d.exif, d.roll_draft, d.trashed_at) {
            Ok(text) => print!("{text}"),
            Err(e) => panic!("{e}"),
        },
        Err(e) => {
            eprintln!("{path}: {e}");
            std::process::exit(1);
        }
    }
}
