//! A photo's grain seed as the app reads it: one at every size it decodes at, kept through a rename,
//! its own for each photo, and the same on the editor's preview and the export's full render.

use super::*;
use std::path::PathBuf;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oni-app-seed-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A 3000 × 10 PNG whose pixels `salt` sets apart.
fn png(path: &Path, salt: u8) {
    let (w, h) = (3000u32, 10u32);
    let mut e = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(path).unwrap()), w, h);
    e.set_color(png::ColorType::Rgb);
    let pixels: Vec<u8> = (0..w * h * 3).map(|i| ((i / 3) as u8).wrapping_mul(salt) ^ salt).collect();
    e.write_header().unwrap().write_image_data(&pixels).unwrap();
}

/// Every way the app decodes `path`: the editor's preview, a copy for a side, the whole frame and one alone.
fn seeds(path: &Path) -> [[u32; 2]; 4] {
    let reading = Reading::default();
    [decode(path, reading.clone()), decode_for(path, reading.clone(), 500), decode_full(path, reading), decode_alone(path, Some(400.0))].map(|d| d.unwrap().seed)
}

#[test]
fn every_decode_of_a_photo_reads_one_seed_its_rename_keeps_and_another_photo_does_not_share() {
    let dir = scratch("decodes");
    let (first, other, renamed) = (dir.join("scan.png"), dir.join("other.png"), dir.join("roll").join("renamed.png"));
    png(&first, 3);
    png(&other, 5);
    let [seed, ..] = seeds(&first);
    assert_ne!(seed, [0, 0], "a read file is seeded");
    assert_eq!(seeds(&first), [seed; 4], "a decode's size moves the seed");
    std::fs::create_dir_all(renamed.parent().unwrap()).unwrap();
    std::fs::rename(&first, &renamed).unwrap();
    assert_eq!(seeds(&renamed), [seed; 4], "a rename and a move keep the seed");
    // Twin: another photo, its own seed.
    assert_ne!(seeds(&other)[0], seed);
    // A RAW at LibRaw's half size, for the preview's side, and whole, on the reference roll when given.
    let nef = |p: &PathBuf| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("nef"));
    let Some(raw) = std::env::var_os("ONI_RAW_DIR").and_then(|d| std::fs::read_dir(d).ok()?.flatten().map(|e| e.path()).filter(nef).min()) else {
        return eprintln!("no ONI_RAW_DIR: the RAW's decodes are not compared");
    };
    let reading = Reading::default();
    let raws = [decode(&raw, reading.clone()), decode_for(&raw, reading.clone(), PREVIEW_SIDE), decode_full(&raw, reading)].map(|d| d.unwrap().seed);
    println!("a RAW's seed at half size, at the preview's side and whole: {raws:?}");
    assert_eq!(raws, [raws[0]; 3], "a RAW's decodes part");
    assert_ne!(raws[0], [0, 0]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn each_photo_lays_its_own_grain_and_the_export_lays_the_preview_s() {
    let Ok(device) = Device::open() else {
        return eprintln!("no adapter: the seeded renders are not exercised");
    };
    let mut engine = Engine::on(&device);
    let (fw, fh, side) = (2400usize, 1600usize, 4usize);
    let grey = |w: usize, h: usize, seed: [u32; 2]| Decoded { linear: Linear { width: w, height: h, pixels: vec![[0.18, 0.18, 0.18, 1.0]; w * h] }, full_width: fw as f64, undeclared: None, seed };
    let mut s = PipelineSettings::rest(oni_settings::ConversionMode::Digital, false);
    (s.grain.amount, s.grain.size) = (1.0, 1.0);
    let (a, b) = ([0x2545_f491, 7], [11, 0x9e37_79b9]);
    let stops = |l: &Linear| -> Vec<f64> { l.pixels.iter().map(|p| f64::from(p[1]).log2()).collect() };
    let mut preview = |seed| {
        let negative = engine.prepare(grey(fw / side, fh / side, seed)).unwrap();
        stops(&engine.render(&negative, &s, None).unwrap())
    };
    let (first, second) = (preview(a), preview(b));
    let correlation = |x: &[f64], y: &[f64]| {
        let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
        let (mx, my) = (mean(x), mean(y));
        let cov: f64 = x.iter().zip(y).map(|(p, q)| (p - mx) * (q - my)).sum();
        cov / (x.iter().map(|p| (p - mx).powi(2)).sum::<f64>() * y.iter().map(|q| (q - my).powi(2)).sum::<f64>()).sqrt()
    };
    let apart = correlation(&first, &second);
    assert!(apart.abs() < 0.05, "two photos' grains correlate at {apart}");
    // Twin: one seed for every photo.
    assert!(correlation(&first, &preview(a)) > 0.999);
    // The export's full render, its 4 × 4 means, against the preview the editor shows.
    let mut means = |seed| {
        let full = crate::export::render_full(&mut engine, grey(fw, fh, seed), &s).unwrap();
        let (w, h) = (fw / side, fh / side);
        let light = |i: usize| f64::from(full.pixels[i * 4 + 1]);
        (0..w * h).map(|i| (0..side * side).map(|j| light(((i / w) * side + j / side) * fw + (i % w) * side + j % side)).sum::<f64>() / (side * side) as f64).map(f64::log2).collect::<Vec<_>>()
    };
    let exported = correlation(&means(a), &first);
    println!("the export's means against the preview: correlation {exported:.4}, another photo's {apart:.4}");
    assert!(exported > 0.9, "the export lays another grain than the preview: {exported}");
    // Twin: the export of another photo's seed.
    assert!(correlation(&means(b), &first).abs() < 0.05);
}
