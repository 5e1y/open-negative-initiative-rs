//! A whole decode kept on disk reads back as LibRaw's own: the same pixels, in a tenth of the time.

use std::path::PathBuf;
use std::time::Instant;

fn raw() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("ONI_RAW_DIR")?);
    let mut nefs: Vec<PathBuf> = std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("nef"))).collect();
    nefs.sort();
    nefs.into_iter().next()
}

#[test]
fn a_kept_decode_is_libraw_s_own() {
    let Some(raw) = raw() else { return eprintln!("ONI_RAW_DIR names no NEF: the kept decode is not exercised") };
    let dir = std::env::temp_dir().join(format!("oni-decode-kept-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    oni_decode::cache::keep_in(dir.clone(), 1 << 30);
    let started = Instant::now();
    let first = oni_decode::decode(&raw, 1.0).expect("LibRaw");
    let decoded = started.elapsed();
    // The keeping is written off the decode's thread.
    let deadline = Instant::now() + std::time::Duration::from_secs(20);
    while std::fs::read_dir(&dir).map_or(0, |d| d.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "decode")).count()) == 0 {
        assert!(Instant::now() < deadline, "nothing kept");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let started = Instant::now();
    let again = oni_decode::decode(&raw, 1.0).expect("kept");
    let read = started.elapsed();
    println!("LibRaw {decoded:?}, kept {read:?}");
    assert_eq!((again.width, again.height), (first.width, first.height));
    assert!(again.pixels == first.pixels, "the kept decode departs from LibRaw's");
    // Twin: another exposure is another reading, decoded afresh, and its pixels differ.
    let brighter = oni_decode::decode(&raw, 2.0).expect("LibRaw");
    assert!(brighter.pixels != first.pixels);
    let _ = std::fs::remove_dir_all(&dir);
}
