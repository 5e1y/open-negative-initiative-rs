//! A small copy read at half size: half the sides, the same light, and the whole frame when the
//! copy asked is longer than half the sensor.

use std::path::PathBuf;

fn raw() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("ONI_RAW_DIR")?);
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("nef")))
}

fn mean(p: &[[f32; 4]]) -> [f64; 3] {
    let mut s = [0f64; 3];
    for x in p {
        for c in 0..3 {
            s[c] += x[c] as f64;
        }
    }
    s.map(|v| v / p.len() as f64)
}

#[test]
fn a_small_copy_is_read_at_half_size_with_the_same_light() {
    let Some(raw) = raw() else { return eprintln!("ONI_RAW_DIR names no NEF: the half-size read is not exercised") };
    let full = oni_decode::decode(&raw, 1.0).expect("decode");
    let (half, factor) = oni_decode::decode_for(&raw, 1.0, 2000).expect("decode at half size");
    assert_eq!((factor, half.width, half.height), (2, full.width / 2, full.height / 2));
    let (a, b) = (mean(&full.pixels), mean(&half.pixels));
    assert!((0..3).all(|c| (a[c] - b[c]).abs() < 1e-3 * a[c].max(1e-6)), "{a:?} against {b:?}");
    // Twin: a copy longer than half the sensor is read whole.
    let (whole, one) = oni_decode::decode_for(&raw, 1.0, full.width).expect("decode whole");
    assert_eq!((one, whole.width), (1, full.width));
}
