//! Both routes into LibRaw on a mosaic DNG built here: the same pixels to the bit, any folder name,
//! any path length, and on Windows a file left free to rename while it is read.

use super::*;
use crate::{Colours, HALF, Linear, decoded_by};
use std::path::PathBuf;

const W: usize = 96;
const H: usize = 64;

/// A scratch folder of this run's own, under `name`.
pub(crate) fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oni-source-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub(crate) type Tag = (u16, u16, u32, Vec<u8>);

/// Appends one directory, its long values and its strip, `273` pointing at the strip; the next
/// directory follows when `more`.
pub(crate) fn directory(out: &mut Vec<u8>, tags: &[Tag], strip: &[u8], more: bool) {
    let long = |v: usize| (v as u32).to_le_bytes();
    let start = out.len();
    let overflow_at = start + 2 + tags.len() * 12 + 4;
    let strip_at = overflow_at + tags.iter().filter(|t| t.3.len() > 4).map(|t| t.3.len()).sum::<usize>();
    let next = (strip_at + strip.len() + 1) & !1;
    out.extend((tags.len() as u16).to_le_bytes());
    let mut extra = Vec::new();
    for (id, kind, count, value) in tags {
        let value = if *id == 273 { long(strip_at).to_vec() } else { value.clone() };
        out.extend(id.to_le_bytes());
        out.extend(kind.to_le_bytes());
        out.extend(count.to_le_bytes());
        if value.len() <= 4 {
            out.extend(&value);
            out.extend(vec![0; 4 - value.len()]);
        } else {
            out.extend(long(overflow_at + extra.len()));
            extra.extend(value);
        }
    }
    out.extend(long(if more { next } else { 0 }));
    out.extend(extra);
    out.extend(strip);
    out.resize(next, 0);
}

/// A 96 x 64 RGGB mosaic DNG in one uncompressed strip, named for the Z f, whose matrix LibRaw's table
/// holds, so it demosaics as a camera's file does; a 16 x 8 RGB preview follows it.
fn mosaic_dng(path: &Path) {
    let pixels: Vec<u8> = (0..W * H).flat_map(|i| ((2000 + (i % W) * 300 + (i / W) * 150 + (i * 7919) % 900) as u16).to_le_bytes()).collect();
    let preview: Vec<u8> = (0..16 * 8 * 3).map(|i| (i * 37 % 251) as u8).collect();
    let rational = |v: f64| [((v * 10000.0).round() as i32).to_le_bytes(), 10000i32.to_le_bytes()].concat();
    let matrix: Vec<u8> = [1.1607, -0.4491, -0.0977, -0.4522, 1.246, 0.2304, -0.0458, 0.1519, 0.7616].iter().flat_map(|&v| rational(v)).collect();
    let short = |v: u16| v.to_le_bytes().to_vec();
    let long = |v: usize| (v as u32).to_le_bytes().to_vec();
    let raw: Vec<Tag> = vec![
        (254, 4, 1, long(0)),
        (256, 4, 1, long(W)),
        (257, 4, 1, long(H)),
        (258, 3, 1, short(16)),
        (259, 3, 1, short(1)),
        (262, 3, 1, short(32803)),
        (271, 2, 18, b"NIKON CORPORATION\0".to_vec()),
        (272, 2, 10, b"NIKON Z f\0".to_vec()),
        (273, 4, 1, Vec::new()),
        (274, 3, 1, short(1)),
        (277, 3, 1, short(1)),
        (278, 4, 1, long(H)),
        (279, 4, 1, long(pixels.len())),
        (284, 3, 1, short(1)),
        (33421, 3, 2, [short(2), short(2)].concat()),
        (33422, 1, 4, vec![0, 1, 1, 2]),
        (50706, 1, 4, vec![1, 4, 0, 0]),
        (50708, 2, 10, b"Nikon Z f\0".to_vec()),
        (50717, 4, 1, long(65535)),
        (50721, 10, 9, matrix),
        (50778, 3, 1, short(21)),
    ];
    let thumb: Vec<Tag> = vec![
        (254, 4, 1, long(1)),
        (256, 4, 1, long(16)),
        (257, 4, 1, long(8)),
        (258, 3, 3, [short(8), short(8), short(8)].concat()),
        (259, 3, 1, short(1)),
        (262, 3, 1, short(2)),
        (273, 4, 1, Vec::new()),
        (277, 3, 1, short(3)),
        (278, 4, 1, long(8)),
        (279, 4, 1, long(preview.len())),
        (284, 3, 1, short(1)),
    ];
    let mut out = b"II*\0".to_vec();
    out.extend(long(8));
    directory(&mut out, &raw, &pixels, true);
    directory(&mut out, &thumb, &preview, false);
    std::fs::write(path, out).unwrap();
}

fn decode_by(path: &Path, route: Route, side: usize, gain: f32) -> Result<Linear, DecodeError> {
    decoded_by(path, gain, side, Colours::Matrix, true, route).map(|(linear, _)| linear)
}

/// Every channel of every pixel, as its bits, so two decodes compare exactly.
fn bits(l: &Linear) -> (usize, usize, Vec<u32>) {
    (l.width, l.height, l.pixels.iter().flatten().map(|v| v.to_bits()).collect())
}

/// A path past Windows' 260 characters, each folder well inside the 255 every system allows.
fn long_folder(root: &Path) -> PathBuf {
    let deep = (0..5).fold(root.to_path_buf(), |p, i| p.join(format!("{i} Été long folder name to pass the old limit {i}")));
    std::fs::create_dir_all(&deep).unwrap();
    deep
}

#[test]
fn a_dng_in_an_ete_folder_decodes() {
    let root = scratch("ete");
    let ete = root.join("Été 🎞");
    std::fs::create_dir_all(&ete).unwrap();
    let accented = ete.join("Décembre.DNG");
    mosaic_dng(&accented);
    let long = long_folder(&root).join("Frame.DNG");
    mosaic_dng(&long);
    assert!(long.to_string_lossy().chars().count() > 260, "{}", long.display());
    let here = crate::decode(&accented, 1.0).expect("the accented folder decodes");
    assert_eq!((here.width, here.height), (W, H));
    let greens: Vec<u32> = here.pixels.iter().map(|p| p[1].to_bits()).collect();
    assert!(greens.iter().any(|&g| g != greens[0]), "a decode, not a flat field");
    assert!(crate::embedded::embedded(&accented).is_some(), "the preview reads too");
    let far = crate::decode(&long, 1.0).expect("the long path decodes");
    assert_eq!(bits(&here), bits(&far), "one file, one decode, wherever it lies");
    assert_eq!(crate::camera::identify(&accented).map(|c| c.dng), Ok(true));
    // Twin: the name route on Windows is the one that failed there; elsewhere a name one letter off fails.
    if cfg!(windows) {
        assert!(decode_by(&accented, Route::Name, 0, 1.0).is_err(), "a char* name loses its accents");
        assert!(decode_by(&long, Route::Name, 0, 1.0).is_err(), "a char* name stops at 260");
        assert!(crate::camera::identify_by(&accented, Route::Name).is_err());
    } else {
        assert_eq!(crate::decode(&ete.join("Decembre.DNG"), 1.0).err(), Some(DecodeError(IO_ERROR)));
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_handle_route_decodes_bit_for_bit_as_the_name_route() {
    let root = scratch("routes");
    let file = root.join("Frame.DNG");
    mosaic_dng(&file);
    let mut frames = vec![file.clone()];
    // Real frames as well when `ONI_RAW_DIR` names a folder: the two first RAWs, read and never written.
    if let Some(dir) = std::env::var_os("ONI_RAW_DIR").map(PathBuf::from) {
        let mut raws: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| crate::is_raw(p)).collect();
        raws.sort();
        frames.extend(raws.into_iter().take(2));
    }
    for frame in &frames {
        for side in [0, HALF] {
            let by_name = decode_by(frame, Route::Name, side, 1.0).unwrap();
            let by_handle = decode_by(frame, Route::Handle, side, 1.0).unwrap();
            assert!(bits(&by_name) == bits(&by_handle), "{} at side {side}", frame.display());
        }
        assert_eq!(crate::camera::identify_by(frame, Route::Name), crate::camera::identify_by(frame, Route::Handle));
        let previews = [Route::Name, Route::Handle].map(|r| crate::embedded::embedded_by(frame, r).map(|e| (e.flip, preview_bytes(e.preview))));
        assert!(previews[0].is_some() && previews[0] == previews[1], "{}", frame.display());
    }
    // Twin: the comparison sees a decode a hundredth of a stop apart.
    let shifted = decode_by(&file, Route::Handle, 0, 1.007).unwrap();
    assert!(bits(&shifted) != bits(&decode_by(&file, Route::Name, 0, 1.0).unwrap()));
    let _ = std::fs::remove_dir_all(&root);
}

fn preview_bytes(p: crate::embedded::Preview) -> Vec<u8> {
    match p {
        crate::embedded::Preview::Jpeg(bytes) => bytes,
        crate::embedded::Preview::Rgb { pixels, .. } => pixels,
    }
}

/// While LibRaw holds the handle, the file renames: a roll made during the thumbnails pass keeps it.
#[cfg(windows)]
#[test]
fn a_file_being_read_renames() {
    use std::os::windows::fs::OpenOptionsExt;
    let root = scratch("held");
    let file = root.join("Frame.DNG");
    mosaic_dng(&file);
    let held = Source::open(&file, Route::Handle).unwrap();
    let moved = root.join("Moved.DNG");
    std::fs::rename(&file, &moved).expect("a file read by handle renames");
    drop(held);
    // Twin: shared for reading and writing alone, as LibRaw's own open shares it, the rename is refused.
    let denying = std::fs::OpenOptions::new().read(true).share_mode(1 | 2).open(&moved).unwrap();
    assert!(std::fs::rename(&moved, &file).is_err());
    drop(denying);
    let _ = std::fs::remove_dir_all(&root);
}

/// A copy stopped half way through its pixels says it is cut short, by either route, never a
/// picture filled with black.
#[test]
fn a_truncated_raw_says_so() {
    let root = scratch("cut");
    let whole = root.join("Whole.DNG");
    mosaic_dng(&whole);
    let bytes = std::fs::read(&whole).unwrap();
    let cut = root.join("Cut.DNG");
    // The header and its directory, then half of the 12 288 bytes of the strip.
    std::fs::write(&cut, &bytes[..bytes.len() - W * H]).unwrap();
    for route in [Route::Name, Route::Handle] {
        let refused = decode_by(&cut, route, 0, 1.0).err();
        assert!(refused.as_ref().is_some_and(DecodeError::is_cut_short), "{route:?}: {refused:?}");
        // Twin: the whole file decodes, so the refusal is the cut's.
        assert!(decode_by(&whole, route, 0, 1.0).is_ok());
    }
    // Twin: a file whose head is not a RAW's is refused otherwise, never as cut short.
    let garbled = root.join("Garbled.DNG");
    std::fs::write(&garbled, vec![7u8; bytes.len()]).unwrap();
    assert!(decode_by(&garbled, Route::Name, 0, 1.0).is_err_and(|e| !e.is_cut_short()));
    let _ = std::fs::remove_dir_all(&root);
}

/// A name that is not UTF-8 reaches LibRaw byte for byte, as a volume from another system may hold one.
#[cfg(unix)]
#[test]
fn a_name_that_is_not_utf8_keeps_its_bytes() {
    use std::os::unix::ffi::OsStrExt;
    let raw = b"/Volumes/Share/Roll \xe9t\xe9/Frame.NEF";
    let path = Path::new(std::ffi::OsStr::from_bytes(raw));
    let source = Source::open(path, Route::Name).unwrap();
    assert_eq!(source.name.as_bytes(), raw);
    // Twin: the lossy text of the same name is another file's.
    assert_ne!(path.to_string_lossy().as_bytes(), raw);
}
