//! The Swift app's self-checks on the linear scan's container, on the same adversarial files and
//! each with its twin, a variant built to fail when the property it guards is broken.

use oni_decode::container::{self, Kind};
use oni_decode::{Linear, is_low_precision};
use oni_fixtures::scratch;
use std::path::{Path, PathBuf};

mod common;
#[path = "swift_checks/files.rs"]
mod files;

use files::*;

// MARK: - ContainerRead.selfCheck

#[test]
fn a_linear_scan_is_read_through_its_table_and_its_reduction_does_not_alias() {
    let dir = scratch("decode-swift-container");
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
