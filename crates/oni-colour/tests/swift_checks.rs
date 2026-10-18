//! The Swift app's self-check on reading a file decoded outside RAW, on the same adversarial codes and
//! with its twins, variants built to fail when the property they guard is broken.

use oni_colour::source::Source;
use oni_settings::SourceEncoding;
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("oni-colour-swift-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `SourceEncoding.writeUntaggedTIFF`, with the ICC profile tag added when one is given.
fn tiff(path: &Path, samples: &[u8], width: usize, channels: usize, icc: Option<&[u8]>) {
    let mut out: Vec<u8> = Vec::new();
    let u16 = |o: &mut Vec<u8>, v: usize| o.extend((v as u16).to_le_bytes());
    let u32 = |o: &mut Vec<u8>, v: usize| o.extend((v as u32).to_le_bytes());
    out.extend(b"II");
    u16(&mut out, 42);
    u32(&mut out, 8);
    let entries = if icc.is_some() { 11 } else { 10 };
    let bps_at = 8 + 2 + entries * 12 + 4;
    let pixels_at = bps_at + 2 * channels;
    let icc_at = pixels_at + samples.len();
    u16(&mut out, entries);
    let tag = |o: &mut Vec<u8>, id: usize, t: usize, n: usize, v: usize| {
        u16(o, id);
        u16(o, t);
        u32(o, n);
        if t == 3 && n == 1 {
            u16(o, v);
            u16(o, 0);
        } else {
            u32(o, v);
        }
    };
    tag(&mut out, 256, 3, 1, width);
    tag(&mut out, 257, 3, 1, 1);
    if channels == 1 {
        tag(&mut out, 258, 3, 1, 8);
    } else {
        tag(&mut out, 258, 3, channels, bps_at);
    }
    tag(&mut out, 259, 3, 1, 1);
    tag(&mut out, 262, 3, 1, if channels == 1 { 1 } else { 2 });
    tag(&mut out, 273, 4, 1, pixels_at);
    tag(&mut out, 277, 3, 1, channels);
    tag(&mut out, 278, 3, 1, 1);
    tag(&mut out, 279, 4, 1, samples.len());
    tag(&mut out, 284, 3, 1, 1);
    if let Some(icc) = icc {
        tag(&mut out, 34675, 7, icc.len(), icc_at);
    }
    u32(&mut out, 0);
    for _ in 0..channels {
        u16(&mut out, 8);
    }
    out.extend(samples);
    out.extend(icc.unwrap_or(&[]));
    std::fs::write(path, out).unwrap();
}

/// The file read as the app reads it: its own codes, then its curve undone in the space it names or is given.
fn read(path: &Path, side: Option<f64>, chosen: Option<SourceEncoding>, converted: bool) -> Vec<f32> {
    let mut e = oni_decode::plain::read(path, side).expect("the TIFF reads");
    Source::of(e.icc.as_deref(), chosen, e.grey).linearise(&mut e.pixels, !converted);
    e.pixels.iter().flat_map(|p| [p[0], p[1], p[2]]).collect()
}

fn worst(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f32::max)
}

fn srgb_decoded(code: f32) -> f32 {
    if code <= 0.04045 { code / 12.92 } else { ((code + 0.055) / 1.055).powf(2.4) }
}

/// sRGB's primaries in Rec. 2020, both on D65.
const SRGB_TO_2020: [[f32; 3]; 3] = [[0.6274, 0.3293, 0.0433], [0.0691, 0.9195, 0.0114], [0.0164, 0.0880, 0.8956]];

// MARK: - SourceEncoding.selfCheck

#[test]
fn an_undeclared_file_is_read_as_the_hand_says_and_a_profile_as_it_says() {
    let dir = scratch("source-encoding");
    // Off every round value, dark enough that sRGB's linear toe and its power part both show.
    let codes: [u8; 12] = [23, 23, 23, 131, 131, 131, 201, 201, 201, 9, 77, 190];
    let file = dir.join("scan.tif");
    tiff(&file, &codes, 4, 3, None);
    let e = oni_decode::plain::read(&file, None).unwrap();
    assert!(e.icc.is_none() && !e.grey, "a TIFF with no profile is undeclared");

    let as_codes: Vec<f32> = codes.iter().map(|&c| f32::from(c) / 255.0).collect();
    let srgb: Vec<f32> = as_codes.iter().map(|&c| srgb_decoded(c)).collect();
    let none = worst(&read(&file, None, None, false), &srgb);
    assert!(none < 2e-3, "with no choice it reads through sRGB's curve, channel for channel, {none:.1e} off");
    // Display P3 shares sRGB's curve, so with the channels held it reads as sRGB does.
    for (choice, expected) in [(SourceEncoding::Linear, &as_codes), (SourceEncoding::DisplayP3, &srgb)] {
        for side in [None, Some(4.0)] {
            let off = worst(&read(&file, side, Some(choice), false), expected);
            assert!(off < 2e-3, "chosen {choice:?} at {side:?}, {off:.1e} off");
        }
    }
    // Twin: under the linear choice the same pixels no longer follow sRGB's curve.
    assert!(worst(&read(&file, None, Some(SourceEncoding::Linear), false), &srgb) > 0.1);

    // A frame keeping the converted decode reads sRGB's primaries carried into the working space.
    let mixed: Vec<f32> = srgb
        .chunks(3)
        .flat_map(|v| SRGB_TO_2020.map(|row| row[0] * v[0] + row[1] * v[1] + row[2] * v[2]))
        .collect();
    for side in [None, Some(4.0)] {
        let off = worst(&read(&file, side, None, true), &mixed);
        assert!(off < 2e-3, "converted at {side:?}, {off:.1e} off");
    }
    // Twin: the conversion moves the coloured pixel off the channels held.
    assert!(worst(&mixed, &srgb) > 0.03, "{:.3}", worst(&mixed, &srgb));

    // A file carrying a profile holds its channels too: Display P3 tagged reads through its curve alone.
    let p3 = moxcms::ColorProfile::new_display_p3().encode().expect("the P3 profile encodes");
    let tagged = dir.join("tagged.tif");
    tiff(&tagged, &codes, 4, 3, Some(&p3));
    assert!(oni_decode::plain::read(&tagged, None).unwrap().icc.is_some(), "a Display P3 TIFF is not undeclared");
    let off = worst(&read(&tagged, None, Some(SourceEncoding::Linear), false), &srgb);
    assert!(off < 2e-3, "a Display P3 file comes back through its own curve whatever the choice, {off:.1e} off");
    // Twin: kept converted, P3's primaries move them.
    assert!(worst(&read(&tagged, None, None, true), &srgb) > 0.03);

    // One grey channel: linear reaches it, which an RGB choice cannot replace.
    let grey = dir.join("grey.tif");
    let grey_codes: [u8; 4] = [23, 131, 201, 77];
    tiff(&grey, &grey_codes, 4, 1, None);
    let g = oni_decode::plain::read(&grey, None).unwrap();
    assert!(g.icc.is_none() && g.grey, "a grey TIFF with no profile is grey and undeclared");
    let as_grey: Vec<f32> = grey_codes.iter().flat_map(|&c| [f32::from(c) / 255.0; 3]).collect();
    for side in [None, Some(4.0)] {
        let off = worst(&read(&grey, side, Some(SourceEncoding::Linear), false), &as_grey);
        assert!(off < 2e-3, "chosen Linear on grey at {side:?}, {off:.1e} off");
    }
    // Twin: a grey chosen Display P3 keeps sRGB's curve.
    assert!(worst(&read(&grey, None, Some(SourceEncoding::DisplayP3), false), &as_grey) > 0.1);
    let _ = std::fs::remove_dir_all(&dir);
}
