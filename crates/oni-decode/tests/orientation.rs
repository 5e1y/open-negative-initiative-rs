//! A RAW held in portrait opens upright, as `CIRAWFilter` hands it out: the file's orientation tag
//! turns the frame, its default crop with it.

use std::path::{Path, PathBuf};

fn raw() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("ONI_RAW_DIR")?);
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("nef")))
}

/// A copy of `from` whose first directory's Orientation tag reads `value`.
fn tagged(from: &Path, to: &Path, value: u16) {
    let mut b = std::fs::read(from).unwrap();
    let le = &b[..2] == b"II";
    let u16_at = |b: &[u8], i: usize| if le { u16::from_le_bytes([b[i], b[i + 1]]) } else { u16::from_be_bytes([b[i], b[i + 1]]) };
    let u32_at = |b: &[u8], i: usize| {
        let w = [b[i], b[i + 1], b[i + 2], b[i + 3]];
        if le { u32::from_le_bytes(w) } else { u32::from_be_bytes(w) }
    };
    let ifd = u32_at(&b, 4) as usize;
    let entries = u16_at(&b, ifd) as usize;
    let entry = (0..entries).map(|i| ifd + 2 + 12 * i).find(|&e| u16_at(&b, e) == 0x112).expect("an Orientation tag");
    let v = if le { value.to_le_bytes() } else { value.to_be_bytes() };
    b[entry + 8..entry + 10].copy_from_slice(&v);
    std::fs::write(to, b).unwrap();
}

#[test]
fn a_portrait_raw_opens_upright() {
    let Some(raw) = raw() else { return eprintln!("ONI_RAW_DIR names no NEF: the orientation is not exercised") };
    let scratch = std::env::temp_dir().join(format!("oni-orientation-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let (level, right, left) = (scratch.join("level.nef"), scratch.join("right.nef"), scratch.join("left.nef"));
    tagged(&raw, &level, 1);
    tagged(&raw, &right, 6);
    tagged(&raw, &left, 8);
    let read = |p: &Path| oni_decode::decode_for(p, 1.0, 1000).expect("decode").0;
    let (a, r, l) = (read(&level), read(&right), read(&left));
    let _ = std::fs::remove_dir_all(&scratch);
    assert_eq!((r.width, r.height), (a.height, a.width), "a quarter turn swaps the sides");
    // Turned right, the level frame's bottom-left pixel lands top-left; turned left, its top-right.
    assert_eq!(r.pixels[0], a.pixels[(a.height - 1) * a.width]);
    assert_eq!(l.pixels[0], a.pixels[a.width - 1]);
    // Twin: the two quarter turns are not the same picture.
    assert_ne!(r.pixels[0], l.pixels[0]);
}
