//! `measure <raw>...`: each frame's shift, Apple's `CIRAWFilter` over LibRaw per channel in stops, as
//! the transition measures it, then the camera's median of the first three that measure.

#[cfg(target_os = "macos")]
fn main() {
    use oni_transition::apple::{AppleDecode, CoreImage};
    use oni_transition::measure::{self, Plane};
    let raws: Vec<std::path::PathBuf> = std::env::args().skip(1).map(Into::into).collect();
    let mut shifts = Vec::new();
    for raw in &raws {
        let started = std::time::Instant::now();
        let grid = CoreImage.blocks(raw).expect("Apple's decode");
        let apple_ms = started.elapsed().as_millis();
        let d = oni_decode::decode(raw, 1.0).expect("LibRaw's decode");
        match measure::fit(&grid, Plane { width: d.width, height: d.height, pixels: &d.pixels }) {
            Ok(s) => {
                println!(
                    "{}: {:+.3} {:+.3} {:+.3} at {:?}, r {:.4}, {:.0} % usable, Apple {apple_ms} ms, all {} ms",
                    raw.display(), s.stops[0], s.stops[1], s.stops[2], s.offset, s.correlation, s.share * 100.0, started.elapsed().as_millis()
                );
                shifts.push(s.stops);
            }
            Err(e) => println!("{}: {e}", raw.display()),
        }
    }
    let m = measure::combined(&shifts);
    println!("median of {}: {:+.3} {:+.3} {:+.3}", shifts.len(), m[0], m[1], m[2]);
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Apple's decode exists on macOS alone");
}
