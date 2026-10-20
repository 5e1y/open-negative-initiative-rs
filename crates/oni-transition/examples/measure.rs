//! `measure <raw>...`: each frame's gap, Apple's `CIRAWFilter` over LibRaw per channel in stops, then
//! the camera's 3x3 fitted on all of them, its residual, and the time each step took.

#[cfg(target_os = "macos")]
fn main() {
    use oni_transition::apple::{AppleDecode, CoreImage};
    use oni_transition::measure::{self, Plane};
    let raws: Vec<std::path::PathBuf> = std::env::args().skip(1).map(Into::into).collect();
    let mut pairs = Vec::new();
    for raw in &raws {
        let started = std::time::Instant::now();
        let grid = CoreImage.blocks(raw).expect("Apple's decode");
        let d = oni_decode::decode_native(raw, 1.0).expect("LibRaw's decode");
        match measure::fit(&grid, Plane { width: d.width, height: d.height, pixels: &d.pixels }) {
            Ok(s) => {
                println!("{}: {:+.3} {:+.3} {:+.3} at {:?}, r {:.4}, {:.0} % usable, {} ms", raw.display(), s.stops[0], s.stops[1], s.stops[2], s.offset, s.correlation, s.share * 100.0, started.elapsed().as_millis());
                pairs.extend(s.pairs);
            }
            Err(e) => println!("{}: {e}", raw.display()),
        }
    }
    let started = std::time::Instant::now();
    let m = measure::matrix(&pairs).expect("a matrix");
    let r = measure::residual(&pairs, &m);
    let rows: Vec<String> = m.iter().flatten().map(|v| format!("{v:.5}")).collect();
    println!("matrix, rows: {} (fitted in {} ms), residual {:.4} {:.4} {:.4}", rows.join(","), started.elapsed().as_millis(), r[0], r[1], r[2]);
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Apple's decode exists on macOS alone");
}
