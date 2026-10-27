//! A copy LibRaw reads at half size is the editor's half decode pixel for pixel: one kept on disk
//! serves it, a copy longer than the kept half is never served from it, and such a copy keeps nothing.

use oni_decode::HALF;
use std::path::PathBuf;

fn raw() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("ONI_RAW_DIR")?);
    let mut nefs: Vec<PathBuf> = std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("nef"))).collect();
    nefs.sort();
    nefs.into_iter().next()
}

fn kept(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir).map_or(0, |d| d.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "decode")).count())
}

#[test]
fn the_half_decode_kept_serves_a_half_size_copy_as_libraw_reads_it() {
    let Some(raw) = raw() else { return eprintln!("ONI_RAW_DIR names no NEF: the shared half decode is not exercised") };
    let dir = std::env::temp_dir().join(format!("oni-decode-half-kept-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    // LibRaw's own copy, nothing kept: a copy at a preview's side writes no decode to disk.
    oni_decode::cache::keep_in(dir.clone(), 1 << 30);
    let (copy, factor) = oni_decode::decode_for(&raw, 1.0, 2000).expect("LibRaw");
    assert_eq!((factor, kept(&dir)), (2, 0), "a preview-side copy kept a decode");

    // The editor's half decode kept, the copy is read from it, the same pixels as LibRaw's.
    oni_decode::decode_for(&raw, 1.0, HALF).expect("LibRaw");
    assert_eq!(kept(&dir), 1);
    let started = std::time::Instant::now();
    let (served, two) = oni_decode::decode_for(&raw, 1.0, 2000).expect("kept");
    println!("served from the kept half decode in {:?}", started.elapsed());
    assert!(two == 2 && (served.width, served.height) == (copy.width, copy.height) && served.pixels == copy.pixels, "the copy served from the half decode departs from LibRaw's");

    // Twin: a copy longer than half the sensor is LibRaw's whole decode, never the half one kept.
    let (whole, one) = oni_decode::decode_for(&raw, 1.0, copy.width.max(copy.height) + 100).expect("LibRaw");
    assert_eq!(one, 1, "a copy longer than the half was served from it");
    assert!(whole.width > copy.width);
    let _ = std::fs::remove_dir_all(&dir);
}
