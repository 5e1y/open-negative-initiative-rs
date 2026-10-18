//! The decode stays in the Swift app's neighbourhood: its balance and exposure, not its pixels.

mod common;

use std::path::{Path, PathBuf};

/// `CIRAWFilter`'s frame inside the decode, which the maker's crop already cuts to its size, by
/// size, measured by correlating the two decodes.
const CROPS: &[((usize, usize), [usize; 4])] = &[
    ((6048, 4032), [0, 0, 6048, 4030]),
    ((8368, 5584), [2, 0, 8364, 5584]),
];

/// Luminance within 0.1 stop; a channel within 0.4, the gap two camera profiles leave on a negative.
const LUMA_STOPS: f64 = 0.1;
const CHANNEL_STOPS: f64 = 0.4;

fn reference_dir() -> PathBuf {
    std::env::var_os("ONI_REFERENCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| "/mnt/user-data/uploads/Downloads/oni-reference".into())
}

/// Each RAW of the test folder with the Swift dump of the same file: RenderDump's Int32 size, then RGBA f32.
fn pairs() -> Vec<(PathBuf, PathBuf)> {
    let (raws, refs) = (common::raw_dir(), reference_dir());
    let mut out = Vec::new();
    for (list, folder) in [("list.txt", "decode"), ("negs-list.txt", "negs-decode")] {
        let Ok(text) = std::fs::read_to_string(refs.join(list)) else {
            continue;
        };
        for (i, line) in text.lines().enumerate() {
            let Some(name) = Path::new(line.trim()).file_name() else {
                continue;
            };
            let (raw, dump) = (
                raws.join(name),
                refs.join(folder).join(format!("{i:05}.f32")),
            );
            if raw.is_file() && dump.is_file() {
                out.push((raw, dump));
            }
        }
    }
    out
}

fn dump(path: &Path) -> (usize, usize, Vec<[f64; 3]>) {
    let b = std::fs::read(path).expect("dump");
    let int = |o: usize| i32::from_le_bytes(b[o..o + 4].try_into().unwrap()) as usize;
    let f = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap()) as f64;
    let (w, h) = (int(0), int(4));
    (
        w,
        h,
        (0..w * h)
            .map(|i| [0, 1, 2].map(|c| f(8 + 16 * i + 4 * c)))
            .collect(),
    )
}

/// The decode's crop box-reduced to `w` by `h`, as the Swift preview reduces its own.
fn reduced(
    d: &oni_decode::Linear,
    [x0, y0, cw, ch]: [usize; 4],
    w: usize,
    h: usize,
) -> Vec<[f64; 3]> {
    let edge = |i: usize, n: usize, of: usize| (i * of + n / 2) / n;
    let mut out = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let (ya, yb, xa, xb) = (
                edge(y, h, ch),
                edge(y + 1, h, ch),
                edge(x, w, cw),
                edge(x + 1, w, cw),
            );
            let mut s = [0f64; 3];
            for yy in y0 + ya..y0 + yb {
                for p in &d.pixels[yy * d.width + x0 + xa..yy * d.width + x0 + xb] {
                    for c in 0..3 {
                        s[c] += p[c] as f64;
                    }
                }
            }
            let n = ((yb - ya) * (xb - xa)) as f64;
            out.push(s.map(|v| v / n));
        }
    }
    out
}

/// Stops by which the Swift dump sits above the decode: Rec. 2020 luminance, then each channel.
fn gaps(swift: &[[f64; 3]], ours: &[[f64; 3]]) -> (f64, [f64; 3]) {
    let mean = |px: &[[f64; 3]], c: usize| px.iter().map(|p| p[c]).sum::<f64>() / px.len() as f64;
    let luma = |px: &[[f64; 3]]| 0.2627 * mean(px, 0) + 0.6780 * mean(px, 1) + 0.0593 * mean(px, 2);
    (
        (luma(swift) / luma(ours)).log2(),
        [0, 1, 2].map(|c| (mean(swift, c) / mean(ours, c)).log2()),
    )
}

fn near((luma, channels): (f64, [f64; 3])) -> bool {
    luma.abs() <= LUMA_STOPS && channels.iter().all(|c| c.abs() <= CHANNEL_STOPS)
}

#[test]
fn the_decode_lands_near_the_swift_one() {
    let found = pairs();
    if found.is_empty() {
        println!(
            "skipped reference: no RAW of {} has a dump in {}",
            common::raw_dir().display(),
            reference_dir().display()
        );
        return;
    }
    for (raw, swift) in found.iter().take(2) {
        let (w, h, s) = dump(swift);
        let d = oni_decode::decode(raw, 1.0).expect("decode");
        let Some(&(_, crop)) = CROPS.iter().find(|(size, _)| *size == (d.width, d.height)) else {
            println!(
                "skipped {}: no measured crop for {}x{}",
                raw.display(),
                d.width,
                d.height
            );
            continue;
        };
        let g = gaps(&s, &reduced(&d, crop, w, h));
        println!(
            "{}: swift above ours by {:.3} stop, channels {:.3?}",
            raw.display(),
            g.0,
            g.1
        );
        assert!(near(g), "{}: {g:?}", raw.display());
        // Twin: the same frame one stop over must leave the neighbourhood.
        let up = oni_decode::decode(raw, 2.0).expect("decode +1 EV");
        let g = gaps(&s, &reduced(&up, crop, w, h));
        assert!(!near(g), "the bound cannot see a stop: {g:?}");
    }
}
