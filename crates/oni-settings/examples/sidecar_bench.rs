//! Times a sidecar's read and write in this format on the typical frame of the corpus, medians of
//! 2 000 runs: `sidecar_bench [<sidecar.json>]`. Targets: read at most 35 µs, encode at most 12 µs.

use oni_settings::{document, migrate};
use std::time::Instant;

fn median(mut runs: Vec<f64>) -> f64 {
    runs.sort_by(f64::total_cmp);
    runs[runs.len() / 2]
}

fn timed(runs: usize, mut f: impl FnMut()) -> f64 {
    median((0..runs).map(|_| {
        let start = Instant::now();
        f();
        start.elapsed().as_secs_f64() * 1e6
    }).collect())
}

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| concat!(env!("CARGO_MANIFEST_DIR"), "/tests/corpus/19-plain/047-DSC_6438.NEF.json").into());
    let stored = std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let converted = migrate::converted(&stored, &migrate::Facts::bare(true)).unwrap_or_else(|e| panic!("{path}: {e}"));
    let text = document::encode(&converted.document).unwrap();
    let encode = timed(2000, || {
        std::hint::black_box(document::encode(std::hint::black_box(&converted.document)).unwrap());
    });
    let decode = timed(2000, || {
        std::hint::black_box(document::decode(std::hint::black_box(&text)).unwrap());
    });
    let sniff = timed(2000, || {
        std::hint::black_box(migrate::standing(std::hint::black_box(&text)));
    });
    let migration = timed(300, || {
        std::hint::black_box(migrate::converted(std::hint::black_box(&stored), &migrate::Facts::bare(true)).unwrap());
    });
    println!("{} B in this format, {} B stored", text.len(), stored.len());
    println!("encode {encode:.1} µs, decode {decode:.1} µs, sniff {sniff:.1} µs, conversion of the stored file {migration:.1} µs");
}
