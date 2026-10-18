//! The Swift app's self-checks on the linear scan's container, on the same adversarial files and
//! each with its twin, a variant built to fail when the property it guards is broken.

use oni_decode::container::{self, Kind};
use oni_decode::{Linear, is_low_precision};
use std::path::{Path, PathBuf};

mod common;

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("oni-decode-swift-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `ContainerRead.writeTestDNG`: a LinearRaw scan in one strip, its table and white level after the IFD.
fn test_dng(path: &Path, curve: &[u16], samples: &[u16], w: usize, h: usize, bits: usize) {
    assert_eq!(samples.len(), w * h * 3);
    let mut out: Vec<u8> = Vec::new();
    let u16 = |o: &mut Vec<u8>, v: usize| o.extend((v as u16).to_le_bytes());
    let u32 = |o: &mut Vec<u8>, v: usize| o.extend((v as u32).to_le_bytes());
    out.extend(b"II");
    u16(&mut out, 42);
    u32(&mut out, 8);
    let entries = if curve.is_empty() { 11 } else { 12 };
    let curve_at = 8 + 2 + entries * 12 + 4;
    let white_at = curve_at + curve.len() * 2;
    let bps_at = white_at + 6;
    let pixels_at = bps_at + 6;
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
    tag(&mut out, 254, 4, 1, 0);
    tag(&mut out, 256, 3, 1, w);
    tag(&mut out, 257, 3, 1, h);
    tag(&mut out, 258, 3, 3, bps_at);
    tag(&mut out, 259, 3, 1, 1);
    tag(&mut out, 262, 3, 1, 34892);
    tag(&mut out, 273, 4, 1, pixels_at);
    tag(&mut out, 277, 3, 1, 3);
    tag(&mut out, 278, 3, 1, h);
    tag(&mut out, 279, 4, 1, w * h * 3 * bits / 8);
    if !curve.is_empty() {
        tag(&mut out, 50712, 3, curve.len(), curve_at);
    }
    tag(&mut out, 50717, 3, 3, white_at);
    u32(&mut out, 0);
    for &v in curve {
        u16(&mut out, v as usize);
    }
    for _ in 0..3 {
        u16(&mut out, 65535);
    }
    for _ in 0..3 {
        u16(&mut out, bits);
    }
    for &v in samples {
        if bits == 16 {
            u16(&mut out, v as usize);
        } else {
            out.push(v as u8);
        }
    }
    std::fs::write(path, out).unwrap();
}

/// `ContainerRead.writeRawTIFF`: one directory of the given entries, then 512 bytes of 0x40.
fn raw_tiff(path: &Path, entries: &[(u16, u16, Vec<f64>)]) {
    let mut sorted = entries.to_vec();
    sorted.sort_by_key(|e| e.0);
    let mut data = vec![0x49, 0x49, 0x2A, 0, 8, 0, 0, 0];
    data.extend((sorted.len() as u16).to_le_bytes());
    let overflow_base = (8 + 2 + sorted.len() * 12 + 4) as u32;
    let mut overflow: Vec<u8> = Vec::new();
    for (tag, t, values) in &sorted {
        data.extend(tag.to_le_bytes());
        data.extend(t.to_le_bytes());
        data.extend((values.len() as u32).to_le_bytes());
        let inline = (*t == 3 && values.len() <= 2) || (*t == 4 && values.len() <= 1);
        if inline {
            if *t == 3 {
                for v in values {
                    data.extend((*v as i64 as u16).to_le_bytes());
                }
                for _ in values.len()..2 {
                    data.extend(0u16.to_le_bytes());
                }
            } else {
                data.extend((values[0] as i64 as u32).to_le_bytes());
            }
        } else {
            data.extend((overflow_base + overflow.len() as u32).to_le_bytes());
            for v in values {
                match t {
                    3 => overflow.extend((*v as i64 as u16).to_le_bytes()),
                    10 => {
                        overflow.extend((*v as i64).clamp(i32::MIN as i64, i32::MAX as i64).to_le_bytes()[..4].to_vec());
                        overflow.extend(1i32.to_le_bytes());
                    }
                    _ => overflow.extend((*v as i64 as u32).to_le_bytes()),
                }
            }
        }
    }
    data.extend(0u32.to_le_bytes());
    data.extend(overflow);
    data.extend([0x40u8; 512]);
    std::fs::write(path, data).unwrap();
}

fn rgb(image: &Linear) -> Vec<[f64; 3]> {
    image.pixels.iter().map(|p| [p[0] as f64, p[1] as f64, p[2] as f64]).collect()
}

/// `ContainerRead.chromaEnergy`: the RMS gradient of Rec. 2020's two colour differences, two pixels in.
fn chroma_energy(w: usize, h: usize, px: &[[f64; 3]]) -> f64 {
    let margin = 2;
    if w <= 2 * margin + 1 || h <= 2 * margin + 1 {
        return 0.0;
    }
    let chroma = |i: usize| {
        let [r, g, b] = px[i];
        let y = 0.2627 * r + 0.6780 * g + 0.0593 * b;
        (r - y, b - y)
    };
    let (mut sum, mut n) = (0.0, 0usize);
    for y in margin..h - margin - 1 {
        for x in margin..w - margin - 1 {
            let i = y * w + x;
            let (c, cx, cy) = (chroma(i), chroma(i + 1), chroma(i + w));
            let (a0, a1, b0, b1) = (cx.0 - c.0, cy.0 - c.0, cx.1 - c.1, cy.1 - c.1);
            sum += a0 * a0 + a1 * a1 + b0 * b0 + b1 * b1;
            n += 1;
        }
    }
    (sum / (4 * n) as f64).sqrt()
}

/// `ContainerRead.grainSamples`: a seeded field box-blurred twice to a 7 px grain, 0.10 spread around 0.5.
fn grain(side: usize) -> Vec<u16> {
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut field: Vec<f32> = (0..side * side * 3)
        .map(|_| {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            ((seed >> 33) & 0xFFFF) as f32 / 65535.0
        })
        .collect();
    let c = 7usize;
    let mut next = field.clone();
    for pass in 0..2 {
        for line in 0..side {
            for ch in 0..3 {
                let at = |k: usize| (if pass == 0 { line * side + k } else { k * side + line }) * 3 + ch;
                let mut run: f64 = (0..c).map(|k| field[at(k.min(side - 1))] as f64).sum();
                for k in 0..side {
                    next[at(k)] = (run / c as f64) as f32;
                    run -= field[at(k.saturating_sub(c / 2))] as f64;
                    run += field[at((k + c - c / 2).min(side - 1))] as f64;
                }
            }
        }
        field = next.clone();
    }
    let n = field.len() as f64;
    let mean = field.iter().map(|&v| v as f64).sum::<f64>() / n;
    let spread = (field.iter().map(|&v| (v as f64 - mean).powi(2)).sum::<f64>() / n).sqrt();
    field.iter().map(|&v| ((0.5 + 0.10 * (v as f64 - mean) / spread.max(1e-9)).clamp(0.05, 0.95) * 65535.0) as u16).collect()
}

/// A Lanczos-3 reduction by `1 / step`, separable, its kernel widened by the step as a scaler's is.
fn lanczos(w: usize, h: usize, px: &[[f64; 3]], step: usize) -> Vec<[f64; 3]> {
    let kernel = |x: f64| {
        if x.abs() < 1e-12 {
            1.0
        } else if x.abs() >= 3.0 {
            0.0
        } else {
            let p = std::f64::consts::PI * x;
            3.0 * p.sin() * (p / 3.0).sin() / (p * p)
        }
    };
    let s = step as f64;
    let taps = |o: usize, len: usize| -> Vec<(usize, f64)> {
        let centre = (o as f64 + 0.5) * s - 0.5;
        let lo = (centre - 3.0 * s).floor() as i64;
        let hi = (centre + 3.0 * s).ceil() as i64;
        let mut t: Vec<(usize, f64)> = (lo..=hi).map(|i| (i.clamp(0, len as i64 - 1) as usize, kernel((i as f64 - centre) / s))).collect();
        let total: f64 = t.iter().map(|x| x.1).sum();
        t.iter_mut().for_each(|x| x.1 /= total);
        t
    };
    let (ow, oh) = (w / step, h / step);
    let mut rows = vec![[0.0; 3]; ow * h];
    for ox in 0..ow {
        let t = taps(ox, w);
        for y in 0..h {
            let mut acc = [0.0; 3];
            for &(x, k) in &t {
                (0..3).for_each(|c| acc[c] += k * px[y * w + x][c]);
            }
            rows[y * ow + ox] = acc;
        }
    }
    let mut out = vec![[0.0; 3]; ow * oh];
    for oy in 0..oh {
        let t = taps(oy, h);
        for ox in 0..ow {
            let mut acc = [0.0; 3];
            for &(y, k) in &t {
                (0..3).for_each(|c| acc[c] += k * rows[y * ow + ox][c]);
            }
            out[oy * ow + ox] = acc;
        }
    }
    out
}

/// The mean of the given in-step offsets, square, which a reduced reading is compared with.
fn resampled(full: &Linear, step: usize, out: (usize, usize), offsets: &[usize]) -> Vec<[f64; 3]> {
    let src = rgb(full);
    let mut px = vec![[0.0; 3]; out.0 * out.1];
    for oy in 0..out.1 {
        for ox in 0..out.0 {
            let mut s = [0.0; 3];
            for &a in offsets {
                for &b in offsets {
                    let (sy, sx) = ((oy * step + a).min(full.height - 1), (ox * step + b).min(full.width - 1));
                    (0..3).for_each(|c| s[c] += src[sy * full.width + sx][c]);
                }
            }
            let n = (offsets.len() * offsets.len()) as f64;
            px[oy * out.0 + ox] = s.map(|v| v / n);
        }
    }
    px
}

// MARK: - ContainerRead.selfCheck

#[test]
fn a_linear_scan_is_read_through_its_table_and_its_reduction_does_not_alias() {
    let dir = scratch("container");
    let curve: Vec<u16> = (0..256).map(|i| ((i as f64 / 255.0).powf(2.2) * 65535.0).round().min(65535.0) as u16).collect();
    let indices: [u16; 5] = [0, 64, 128, 192, 255];
    let flat: Vec<u16> = indices.iter().chain(indices.iter()).flat_map(|&i| [i; 3]).collect();
    let dng = dir.join("scan.dng");
    test_dng(&dng, &curve, &flat, 5, 2, 8);
    assert_eq!(container::probe(&dng), Some(Kind::LinearRgb), "an uncompressed LinearRaw DNG is recognised by its bytes");

    let read = container::linear(&dng, None).unwrap();
    let green: Vec<f64> = read.pixels.iter().map(|p| p[1] as f64).collect();
    assert_eq!(green.len(), 10);
    let off = |wanted: &dyn Fn(f64) -> f64| {
        indices.iter().chain(indices.iter()).zip(&green).map(|(&i, &g)| (g - wanted(i as f64 / 255.0)).abs()).fold(0.0, f64::max)
    };
    assert!(off(&|x| x.powf(2.2)) < 0.01, "the table is applied, {:.4} off", off(&|x| x.powf(2.2)));
    // Twin: the same reading against the bare indices deviates, so a reader ignoring the table fails.
    assert!(off(&|x| x) > 0.1);

    assert_eq!(container::bits_per_sample(&dng), Some(8), "the real depth is read from the container");
    assert_eq!(container::pixel_size(&dng), Some((read.width, read.height)), "the directory's size is the full decode's");
    assert!(is_low_precision(&dng), "an 8-bit source is flagged even under a RAW extension");
    let deep = dir.join("deep.dng");
    test_dng(&deep, &[], &flat.iter().map(|v| v * 257).collect::<Vec<_>>(), 5, 2, 16);
    // Twin: the same file at 16 bits is not.
    assert!(!is_low_precision(&deep));

    let renamed = dir.join("actually-a-scan.dng");
    std::fs::copy(&dng, &renamed).unwrap();
    assert_eq!(container::probe(&renamed), Some(Kind::LinearRgb), "the extension plays no part");
    let not_tiff = dir.join("x.bin");
    std::fs::write(&not_tiff, [0xFF, 0xD8, 0xFF, 0xE0, 0, 0, 0, 0, 0, 0]).unwrap();
    assert_eq!(container::probe(&not_tiff), None, "a file that is not a TIFF is not claimed");
    assert!(is_low_precision(&not_tiff), "a JPEG's bytes read 8 bits whatever its name");
    let png = |depth: u8| [&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13][..], b"IHDR", &[0, 0, 0, 1, 0, 0, 0, 1, depth, 2]].concat();
    std::fs::write(dir.join("deep.png"), png(16)).unwrap();
    std::fs::write(dir.join("shallow.png"), png(8)).unwrap();
    // Twin: the same header at 8 bits is flagged, so the 16 above was read.
    assert!(!is_low_precision(&dir.join("deep.png")) && is_low_precision(&dir.join("shallow.png")));

    let base: Vec<(u16, u16, Vec<f64>)> = vec![
        (256, 3, vec![4.0]),
        (257, 3, vec![2.0]),
        (258, 3, vec![8.0]),
        (259, 3, vec![1.0]),
        (262, 3, vec![34892.0]),
        (277, 3, vec![3.0]),
        (278, 3, vec![2.0]),
        (279, 4, vec![24.0]),
    ];
    let with = |extra: (u16, u16, Vec<f64>)| base.iter().cloned().chain([extra]).collect::<Vec<_>>();
    let fixed = |w: f64, h: f64| -> Vec<(u16, u16, Vec<f64>)> {
        vec![
            (256, 4, vec![w]),
            (257, 4, vec![h]),
            (258, 3, vec![8.0]),
            (259, 3, vec![1.0]),
            (262, 3, vec![34892.0]),
            (277, 3, vec![3.0]),
            (273, 4, vec![24.0]),
            (279, 4, vec![24.0]),
        ]
    };
    let hostile: Vec<(&str, Vec<(u16, u16, Vec<f64>)>)> = vec![
        ("negative-offset", with((273, 10, vec![-1.0]))),
        ("negative-tile", with((324, 10, vec![-1.0]))),
        ("negative-subifd", with((330, 10, vec![-1.0]))),
        ("overflowing-sides", fixed(3e9, 3e9)),
        ("huge-product", fixed(100_000.0, 100_000.0)),
        (
            "float-with-table",
            vec![
                (256, 4, vec![4.0]),
                (257, 4, vec![2.0]),
                (258, 3, vec![32.0]),
                (259, 3, vec![1.0]),
                (262, 3, vec![34892.0]),
                (277, 3, vec![3.0]),
                (339, 3, vec![3.0]),
                (273, 4, vec![200.0]),
                (279, 4, vec![96.0]),
                (50712, 3, (0..8).map(f64::from).collect()),
            ],
        ),
    ];
    for (name, entries) in hostile {
        let file = dir.join(format!("hostile-{name}.dng"));
        raw_tiff(&file, &entries);
        let _ = container::probe(&file);
        let _ = container::linear(&file, None);
        let _ = is_low_precision(&file);
    }

    // Rows resolving onto bytes the file does not hold: the header is claimed, so the refusal is the pixels'.
    let truncated = dir.join("truncated.dng");
    raw_tiff(
        &truncated,
        &[
            (256, 3, vec![64.0]),
            (257, 3, vec![64.0]),
            (258, 3, vec![8.0]),
            (259, 3, vec![1.0]),
            (262, 3, vec![34892.0]),
            (277, 3, vec![3.0]),
            (278, 3, vec![64.0]),
            (273, 4, vec![1_048_576.0]),
            (279, 4, vec![12288.0]),
        ],
    );
    assert_eq!(container::probe(&truncated), Some(Kind::LinearRgb), "a truncated scan is still claimed by its header");
    assert!(container::linear(&truncated, None).is_none(), "and decodes to nothing rather than to a black frame");
    // Twin: the legitimate DNG still passes after the hostile ones.
    assert!(container::linear(&dng, None).is_some());

    // The comb stays centred on its step; the step's first pixels drift by (step - taps) / 2.
    let drift = |offsets: &dyn Fn(usize) -> Vec<usize>| {
        (1..=64)
            .map(|step| {
                let o = offsets(step);
                (o.iter().sum::<usize>() as f64 / o.len() as f64 - (step as f64 - 1.0) / 2.0).abs()
            })
            .fold(0.0, f64::max)
    };
    assert!(drift(&container::comb_offsets) <= 0.5, "the comb drifts {:.1}", drift(&container::comb_offsets));
    assert!(drift(&|step| (0..step.clamp(1, 4)).collect()) > 0.5);

    // Reducing must not invent colour out of grain, at every step the shipped sides take off a scanner's width.
    const ALIAS_LIMIT: f64 = 1.7;
    let shown = 32usize;
    let mut confined_worst = (0.0f64, 0usize);
    for (label, side) in [("preview", 2000.0), ("measure", 1400.0), ("thumbnail", 320.0)] {
        let step = ((10416.0f64 / side).ceil() as usize).max(1);
        let mire = step * shown;
        let file = dir.join(format!("grain-{step}.dng"));
        test_dng(&file, &[], &grain(mire), mire, mire, 16);
        let preview = container::linear(&file, Some(shown as f64)).unwrap();
        let full = container::linear(&file, None).unwrap();
        let out = (preview.width, preview.height);
        let reference = chroma_energy(out.0, out.1, &lanczos(full.width, full.height, &rgb(&full), step));
        assert!(reference > 0.0, "the Lanczos reference carries no chroma");
        let offsets = container::comb_offsets(step);
        let got = chroma_energy(out.0, out.1, &rgb(&preview)) / reference;
        let alone = chroma_energy(out.0, out.1, &resampled(&full, step, out, &offsets[..1])) / reference;
        let confined =
            chroma_energy(out.0, out.1, &resampled(&full, step, out, &(0..offsets.len()).collect::<Vec<_>>())) / reference;
        println!("{label} at step {step}: {got:.2}x, one tap {alone:.2}x, confined {confined:.2}x");
        if confined > confined_worst.0 {
            confined_worst = (confined, step);
        }
        assert!(got < ALIAS_LIMIT, "{label} at step {step} aliases: {got:.2}x a Lanczos reduction");
        // Twin: one of the very taps the mean averages carries strictly more fine energy alone.
        assert!(alone > got, "{label} at step {step}: one tap alone reaches {alone:.2}x against {got:.2}x");
    }
    // Twin: the same taps confined to the step's first pixels pass the limit, so it bites.
    assert!(confined_worst.0 > ALIAS_LIMIT, "confined taps reach only {:.2}x at step {}", confined_worst.0, confined_worst.1);
    let _ = std::fs::remove_dir_all(&dir);
}

// MARK: - RawDecode.pinningCheck

/// Under this a sample lands on the guard at the mosaic's resting ceiling, one value no window separates.
fn piling() -> f32 {
    use oni_settings::axis::density_axis;
    density_axis::log_guard(density_axis::MOSAIC_REST) - density_axis::pedestal(density_axis::MOSAIC_REST)
}

fn floor(pixels: &[[f32; 4]], offset: f32) -> (f32, f64) {
    let (mut low, mut under) = (f32::INFINITY, 0usize);
    for p in pixels {
        for &v in &p[..3] {
            let v = v - offset;
            low = low.min(v);
            under += usize::from(v < piling());
        }
    }
    (low, under as f64 / (pixels.len() * 3) as f64 * 100.0)
}

/// The decode at most `side` long, each output pixel the mean of its whole step.
fn reduced(image: &Linear, side: usize) -> Vec<[f32; 4]> {
    let step = image.width.max(image.height).div_ceil(side).max(1);
    let (w, h) = (image.width / step, image.height / step);
    let mut out = vec![[0.0f32; 4]; w * h];
    for oy in 0..h {
        for ox in 0..w {
            let mut s = [0.0f64; 3];
            for y in oy * step..(oy + 1) * step {
                for p in &image.pixels[y * image.width + ox * step..y * image.width + (ox + 1) * step] {
                    (0..3).for_each(|c| s[c] += p[c] as f64);
                }
            }
            let n = (step * step) as f64;
            out[oy * w + ox] = [(s[0] / n) as f32, (s[1] / n) as f32, (s[2] / n) as f32, 1.0];
        }
    }
    out
}

#[test]
fn nothing_the_pinned_decode_hands_out_piles_at_the_resting_ceiling() {
    let roll = common::frames("pinning floor", 1).map(|f| f[0].clone());
    let lumix = Path::new("/mnt/user-data/uploads/Negs/P1072392.RW2");
    let files: Vec<PathBuf> = roll.into_iter().chain(lumix.exists().then(|| lumix.to_path_buf())).collect();
    for file in &files {
        let decoded = oni_decode::decode(file, 1.0).expect("decode");
        let small = reduced(&decoded, 1000);
        let (low, under) = floor(&small, 0.0);
        let (bias_low, bias_under) = floor(&small, 0.0038);
        println!(
            "{}: floor {low:.5}, {under:.3} % under {:.2e}; biased floor {bias_low:.5}, {bias_under:.3} %",
            file.file_name().unwrap().to_string_lossy(), piling());
        assert_eq!(under, 0.0, "the pinned decode piles at the resting ceiling, floor {low:.5}");
        // Twin: the camera's own black offset, 0.0038 of white on the S1R, piles samples there.
        assert!(bias_under > 0.0, "a decode biased by 0.0038 piles nothing, floor {bias_low:.5}");
    }
}
