//! The render module's checks: decodes, the engine against the CPU reference, the slot.
use super::*;
use std::sync::mpsc;
use std::time::Duration;

#[test]
fn a_pass_shrunk_to_a_side_keeps_its_mean_and_a_smaller_one_is_kept() {
    let (w, h) = (9usize, 6usize);
    let l = Linear { width: w, height: h, pixels: (0..w * h).map(|i| [i as f32, 1.0, (i % w) as f32, 1.0]).collect() };
    let s = shrunk(&l, 3.0);
    assert_eq!((s.width, s.height), (3, 2));
    let mean = |l: &Linear, c: usize| l.pixels.iter().map(|p| f64::from(p[c])).sum::<f64>() / l.pixels.len() as f64;
    assert!((mean(&s, 0) - mean(&l, 0)).abs() < 1e-4 && (mean(&s, 2) - mean(&l, 2)).abs() < 1e-4);
    assert_eq!(s.pixels[0][2], 1.0, "the first column of three averages columns 0 to 2");
    // Twin: asked a side past its own, the image is its own.
    assert_eq!(shrunk(&l, 20.0).pixels, l.pixels);
}

#[test]
fn three_passes_give_each_its_own_channel_and_a_registration_moves_its_pass() {
    let (w, h) = (8usize, 4usize);
    // Each pass lit only at one column, every channel alike, as a narrowband pass is not.
    let pass = |col: usize, v: f32| Linear { width: w, height: h, pixels: (0..w * h).map(|i| if i % w == col { [v, v, v, 1.0] } else { [0.0, 0.0, 0.0, 1.0] }).collect() };
    let fit = oni_settings::frame::Fit::default();
    let red = laid(&pass(3, 0.5), &fit, w, h, 0);
    assert_eq!(red[3], 0.5, "identity: the red pass's own column");
    // Two pixels right, as a fraction of the frame's width.
    let shifted = oni_settings::frame::Fit { dx: 2.0 / w as f64, ..fit };
    let moved = laid(&pass(3, 0.5), &shifted, w, h, 0);
    assert!((moved[5] - 0.5).abs() < 1e-6 && moved[3].abs() < 1e-6, "{moved:?}");
    // Twin: dy moves along the rows, y up: one pixel up is the row above.
    let up = oni_settings::frame::Fit { dy: 1.0 / w as f64, ..fit };
    let rows = laid(&Linear { width: w, height: h, pixels: (0..w * h).map(|i| if i / w == 2 { [1.0; 4] } else { [0.0, 0.0, 0.0, 1.0] }).collect() }, &up, w, h, 0);
    assert!((rows[w] - 1.0).abs() < 1e-6 && rows[2 * w].abs() < 1e-6, "{rows:?}");
}

#[test]
fn a_plain_image_opens_linearised_on_its_own_primaries_and_reduced_to_the_preview() {
    let dir = std::env::temp_dir().join(format!("oni-app-plain-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("scan.png");
    let (w, h) = (3000u32, 10u32);
    let mut e = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(&path).unwrap()), w, h);
    e.set_color(png::ColorType::Rgb);
    let row: Vec<u8> = (0..w).flat_map(|_| [128u8, 128, 255]).collect();
    e.write_header().unwrap().write_image_data(&row.repeat(h as usize)).unwrap();
    let held = decode(&path, Reading::default()).unwrap();
    assert_eq!((held.linear.width, held.full_width), (PREVIEW_SIDE, 3000.0));
    let p = held.linear.pixels[held.linear.pixels.len() / 2];
    // Untagged, the file is sRGB: 128 is 0.216 in light, blue's 255 stays one on its own primaries.
    assert!((p[0] - 0.2158).abs() < 2e-3 && (p[2] - 1.0).abs() < 1e-4, "{p:?}");
    // Twin: measured on converted primaries, sRGB's blue mixes into the working red and green.
    let converted = decode(&path, Reading { primaries_converted: true, ..Reading::default() }).unwrap();
    let q = converted.linear.pixels[converted.linear.pixels.len() / 2];
    assert!((q[0] - p[0]).abs() > 1e-2, "{q:?} against {p:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Counts its renders and holds each one long enough for jobs to pile up behind it.
struct Slow(Arc<Mutex<Vec<u64>>>);

impl Renderer<Linear> for Slow {
    fn render(&mut self, negative: &Linear, settings: &PipelineSettings) -> Result<Linear, String> {
        std::thread::sleep(Duration::from_millis(60));
        self.0.lock().unwrap().push(settings.print.exposure.to_bits() as u64);
        Ok(negative.clone())
    }
}

#[test]
fn a_burst_of_notches_renders_the_first_and_the_last_only() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let (tx, rx) = mpsc::channel();
    let worker = Worker::new(Box::new(Slow(seen.clone())), Screen::Srgb, move |d| {
        if let Delivery::Render(d) = d {
            tx.send(d.map(|d| d.revision)).unwrap();
        }
    });
    let negative = Arc::new(Linear { width: 2, height: 1, pixels: vec![[0.5; 4]; 2] });
    for revision in 1..=20u64 {
        let mut settings = PipelineSettings::default();
        settings.print.exposure = revision as f32;
        let (kind, asked) = (crate::timing::Kind::Notch, Instant::now());
        // At once: a timer's sleep lasts 15 ms on Windows, which would spread the burst over four renders.
        worker.submit(Job { revision, path: PathBuf::new(), negative: negative.clone(), settings, kind, asked, view: None, reference: None, held: false });
    }
    let mut revisions = Vec::new();
    while let Ok(r) = rx.recv_timeout(Duration::from_millis(500)) {
        revisions.push(r.unwrap());
    }
    assert_eq!(revisions.last(), Some(&20), "the latest settings are always rendered");
    assert!(revisions.len() <= 3, "{revisions:?}: the slot replaced the waiting jobs");
    // Twin: twenty submissions, so a queue would have rendered twenty.
    assert!(seen.lock().unwrap().len() < 20);
}

#[test]
fn the_parallel_encode_writes_what_the_reference_writes() {
    let (w, h) = (37usize, 23usize);
    let pixels: Vec<[f32; 4]> = (0..w * h).map(|i| [(i % w) as f32 / w as f32, (i / w) as f32 / h as f32, 0.3, 1.0]).collect();
    let linear = Linear { width: w, height: h, pixels };
    let k = Params::new(Screen::Srgb, w as u32, h as u32, 5);
    let reference = display::encode(&linear.pixels, &k);
    assert_eq!(encoded(&linear, &k, 1), reference);
    let sample = encoded(&linear, &k, 2);
    assert_eq!(sample.len(), 19 * 12 * 4);
    assert_eq!(sample[4..8], reference[8..12], "the second sample is the third pixel");
    // Twin: another seed moves the dither, so a band encoded with the wrong one is seen.
    assert_ne!(encoded(&linear, &Params::new(Screen::Srgb, w as u32, h as u32, 6), 1), reference);
}

#[test]
fn previews_keep_the_last_four_used() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the previews are not exercised");
        return;
    };
    let mut engine = Engine::on(&device);
    let one = |e: &mut Engine| {
        let linear = Linear { width: 8, height: 8, pixels: vec![[0.5; 4]; 64] };
        Arc::new(e.prepare(Decoded { linear, full_width: 8.0, undeclared: None }).unwrap())
    };
    let mut kept = Previews::default();
    for k in 0..4 {
        kept.insert(k.to_string(), one(&mut engine));
    }
    assert!(kept.get("0").is_some(), "a use moves the frame to the back");
    kept.insert("4".into(), one(&mut engine));
    assert!(kept.get("1").is_none(), "the least recently used frame left");
    // Twin: the frame used just before the insertion stayed.
    assert!(kept.get("0").is_some());
}

/// A zoomed view renders only what the canvas shows: at 100 % from the full frame, the canvas's
/// own size, laid edge to edge; fitted, the whole image from the preview.
#[test]
fn a_view_renders_its_visible_region_from_the_source_its_scale_needs() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the views are not exercised");
        return;
    };
    let mut engine = Engine::on(&device);
    let (fw, fh) = (1600usize, 1000usize);
    let gradient = |w: usize, h: usize| (0..w * h).map(|i| [0.2 + 0.5 * (i % w) as f32 / w as f32, 0.3, 0.4, 1.0]).collect::<Vec<_>>();
    // A 400 px preview of a 1600 px frame: the preview's ratio is a quarter.
    let negative = engine.prepare(Decoded { linear: Linear { width: 400, height: 250, pixels: gradient(400, 250) }, full_width: fw as f64, undeclared: None }).unwrap();
    let full = engine.full(Decoded { linear: Linear { width: fw, height: fh, pixels: gradient(fw, fh) }, full_width: fw as f64, undeclared: None }).unwrap();
    let settings = PipelineSettings::rest(oni_settings::ConversionMode::Positive, true);
    let size = (600.0, 400.0);
    let hundred = crate::viewport::Viewport { scale: Some(1.0), offset: (0.0, 0.0) };
    let view = View { viewport: hundred, size, full: Some(full.clone()), settled: true, preview: true };
    let (source, placed) = engine.resident(&negative, &settings, Some(&view)).unwrap();
    let placed = placed.unwrap();
    assert!(placed.at_full_resolution && !placed.whole);
    let e = source.extent();
    // The canvas's pixels and an eighth of them past each side, which a pan shows before its render lands.
    assert!((e.w as f64 - size.0 * 1.25).abs() <= 2.0 && (e.h as f64 - size.1 * 1.25).abs() <= 2.0, "the canvas and its margin only: {e:?}");
    assert!(placed.x <= 0.0 && placed.y <= 0.0 && placed.x + placed.width >= size.0 && placed.y + placed.height >= size.1);
    // Twin: unsettled, the same view stays on the preview, a quarter of the pixels stretched four times.
    let moving = View { settled: false, ..view.clone() };
    let (stretched, placed) = engine.resident(&negative, &settings, Some(&moving)).unwrap();
    assert!(!placed.unwrap().at_full_resolution);
    assert!(stretched.extent().w * 3 < e.w, "{:?}", stretched.extent());
    // Off a preview copy, the hand's view reads the full resolution as well.
    let (_, placed) = engine.resident(&negative, &settings, Some(&View { preview: false, ..moving })).unwrap();
    assert!(placed.unwrap().at_full_resolution);
    // Fitted on a canvas smaller than the preview: the whole image, from the preview on a preview copy.
    let fitted = View { viewport: crate::viewport::Viewport::default(), size: (300.0, 200.0), full: Some(full), settled: true, preview: true };
    let (_, placed) = engine.resident(&negative, &settings, Some(&fitted)).unwrap();
    let placed = placed.unwrap();
    assert!(placed.whole && !placed.at_full_resolution, "{placed:?}");
    assert!((placed.width - 300.0).abs() < 1.0 && (placed.height - 187.5).abs() < 1.0, "{placed:?}");
    let (_, placed) = engine.resident(&negative, &settings, Some(&View { preview: false, ..fitted })).unwrap();
    let placed = placed.unwrap();
    assert!(placed.whole && placed.at_full_resolution && (placed.width - 300.0).abs() < 1.0, "fitted down from the full resolution: {placed:?}");
}

/// `Loupe.selfCheck`: the loupe shows the exported file's pixels over its region; cropped before
/// the pipeline, the blurs read invented rows along the border and the same region diverges.
#[test]
fn the_loupe_shows_the_exported_pixels_and_a_crop_first_does_not() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the loupe is not exercised");
        return;
    };
    let mut engine = Engine::on(&device);
    let (fw, fh) = (1200usize, 800usize);
    // Grain to bite on and both neighbourhood stages well above rest, so no arm agrees by luck.
    let grain = |i: usize| ((i as u32).wrapping_mul(2_654_435_761) >> 8) as f32 / 16_777_216.0;
    let pixels: Vec<[f32; 4]> = (0..fw * fh).map(|i| [0.1 + 0.6 * grain(i), 0.1 + 0.6 * grain(i + 7), 0.1 + 0.6 * grain(i + 13), 1.0]).collect();
    let full = engine.full(Decoded { linear: Linear { width: fw, height: fh, pixels: pixels.clone() }, full_width: fw as f64, undeclared: None }).unwrap();
    let mut settings = PipelineSettings::rest(oni_settings::ConversionMode::Positive, true);
    settings.chroma = oni_settings::blocks::ChromaDenoise { radius: 5.0, force: 0.8 };
    settings.sharpen.radius = 5.0;
    settings.sharpen.amount = 2.0;
    let port = crate::viewport::Viewport { scale: Some(1.0), offset: (-137.0, 91.0) };
    let (seen, zone) = engine.loupe(&full, &settings, &port, (640.0, 360.0), 1.0).unwrap().unwrap();
    assert_eq!((zone.x, zone.y, zone.w, zone.h), (417, 129, 640, 360));
    let gap = |a: &[[f32; 4]], b: &[[f32; 4]]| a.iter().zip(b).flat_map(|(p, q)| (0..3).map(move |c| (p[c] - q[c]).abs())).fold(0.0f32, f32::max);
    let negative = engine.prepare(Decoded { linear: Linear { width: fw, height: fh, pixels: pixels.clone() }, full_width: fw as f64, undeclared: None }).unwrap();
    let whole = engine.render(&negative, &settings, None).unwrap();
    let sliced: Vec<[f32; 4]> = (0..zone.h as usize).flat_map(|r| whole.pixels[(zone.y as usize + r) * fw + zone.x as usize..][..zone.w as usize].to_vec()).collect();
    let exported = gap(&seen.pixels, &sliced);
    assert!(exported < 1e-4, "the loupe is the exported file: {exported}");
    // Twin: the frame cropped to the region before the pipeline diverges along its border.
    let cut: Vec<[f32; 4]> = (0..zone.h as usize).flat_map(|r| pixels[(zone.y as usize + r) * fw + zone.x as usize..][..zone.w as usize].to_vec()).collect();
    let cropped = engine.full(Decoded { linear: Linear { width: zone.w as usize, height: zone.h as usize, pixels: cut }, full_width: f64::from(zone.w), undeclared: None }).unwrap();
    let (first, _) = engine.loupe(&cropped, &settings, &crate::viewport::Viewport::default(), (640.0, 360.0), 1.0).unwrap().unwrap();
    assert!(gap(&first.pixels, &seen.pixels) > 1e-3, "twin: a crop first reads invented rows");
}

/// A frame naming a custom flat-field is divided by that mask, read from the support folder;
/// one naming a mask that is gone renders as with none, as the Swift app leaves the stage out.
#[test]
fn a_custom_flat_field_reaches_the_render_by_its_id() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the custom flat-field is not exercised");
        return;
    };
    let support = std::env::temp_dir().join(format!("oni-render-custom-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&support);
    std::fs::create_dir_all(crate::flat_fields::folder(&support)).unwrap();
    // A panel falling off to the right: the division lifts the right of the frame.
    let (mw, mh) = (8u32, 4u32);
    let mask: Vec<f32> = (0..mw * mh).flat_map(|i| { let v = 1.0 - 0.6 * (i % mw) as f32 / mw as f32; [v, v, v, 1.0] }).collect();
    let out = std::fs::File::create(crate::flat_fields::folder(&support).join("panel.tiff")).unwrap();
    tiff::encoder::TiffEncoder::new(out).unwrap().write_image::<tiff::encoder::colortype::RGBA32Float>(mw, mh, &mask).unwrap();
    let mut engine = Engine::on(&device);
    engine.support = Some(support.clone());
    let (w, h) = (16usize, 8usize);
    let negative = engine.prepare(Decoded { linear: Linear { width: w, height: h, pixels: vec![[0.3, 0.3, 0.3, 1.0]; w * h] }, full_width: w as f64, undeclared: None }).unwrap();
    let mut settings = PipelineSettings::rest(oni_settings::ConversionMode::Positive, true);
    let plain = engine.render(&negative, &settings, None).unwrap();
    settings.flat_field = oni_settings::frame::FlatField { mask: FlatFieldMask::Custom, custom: Some("panel".into()), ..Default::default() };
    let divided = engine.render(&negative, &settings, None).unwrap();
    let at = |l: &Linear, x: usize| l.pixels[(h / 2) * w + x][1];
    assert!((at(&divided, w - 3) - at(&divided, 2)).abs() > 1e-3, "the mask's falloff shows: {} {}", at(&divided, 2), at(&divided, w - 3));
    assert!((at(&plain, w - 3) - at(&plain, 2)).abs() < 1e-5, "the flat frame without it stays flat");
    // Twin: a mask that is gone leaves the stage out, so the frame renders as with none.
    settings.flat_field.custom = Some("gone".into());
    let gone = engine.render(&negative, &settings, None).unwrap();
    assert!(gone.pixels.iter().zip(&plain.pixels).all(|(a, b)| (a[1] - b[1]).abs() < 1e-6));
    // The same id loaded afterwards is read on the next render, the miss not kept.
    std::fs::copy(crate::flat_fields::folder(&support).join("panel.tiff"), crate::flat_fields::folder(&support).join("gone.tiff")).unwrap();
    let found = engine.render(&negative, &settings, None).unwrap();
    assert!((at(&found, w - 3) - at(&divided, w - 3)).abs() < 1e-6, "{} {}", at(&found, w - 3), at(&divided, w - 3));
    let _ = std::fs::remove_dir_all(&support);
}

/// A correction's patch from the cache is laid through its stroke; one not generated yet is
/// left out, so the frame renders as with no correction.
#[test]
fn a_dust_patch_from_the_cache_heals_its_stroke() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the dust patches are not exercised");
        return;
    };
    let caches = std::env::temp_dir().join(format!("oni-render-patches-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&caches);
    let mut engine = Engine::on(&device);
    engine.caches = Some(caches.clone());
    let (w, h) = (64usize, 48usize);
    let negative = engine.prepare(Decoded { linear: Linear { width: w, height: h, pixels: vec![[0.3, 0.3, 0.3, 1.0]; w * h] }, full_width: w as f64, undeclared: None }).unwrap();
    let mut settings = PipelineSettings::rest(oni_settings::ConversionMode::Positive, true);
    let plain = engine.render(&negative, &settings, None).unwrap();
    let id = Id::parse("0F2B6E4C-9A1D-4E7B-8C3A-5D6E7F809A1B").unwrap();
    let point = |x: f64, y: f64| oni_settings::cg::Point { x, y };
    settings.corrections = vec![oni_settings::frame::Correction { id, stroke: vec![point(0.5, 0.5)], radius: 6.0 }];
    let missing = engine.render(&negative, &settings, None).unwrap();
    assert!(missing.pixels.iter().zip(&plain.pixels).all(|(a, b)| (a[1] - b[1]).abs() < 1e-6), "no patch, no change");
    // A bright patch the size of the tile: the frame is 64 × 48, so the tile is the whole frame.
    std::fs::create_dir_all(caches.join("Corrections")).unwrap();
    let out = std::fs::File::create(crate::patches::file(&caches, id)).unwrap();
    let bright: Vec<f32> = (0..w * h).flat_map(|_| [0.9f32, 0.9, 0.9, 1.0]).collect();
    tiff::encoder::TiffEncoder::new(out).unwrap().write_image::<tiff::encoder::colortype::RGBA32Float>(w as u32, h as u32, &bright).unwrap();
    let healed = engine.render(&negative, &settings, None).unwrap();
    let at = |l: &Linear, x: usize, y: usize| l.pixels[y * w + x][1];
    assert!((at(&healed, w / 2, h / 2) - at(&plain, w / 2, h / 2)).abs() > 1e-2, "the stroke's centre takes the patch");
    // Twin: far from the stroke the frame is its own, so a patch laid everywhere would be seen.
    assert!((at(&healed, 2, 2) - at(&plain, 2, 2)).abs() < 1e-5);
    let _ = std::fs::remove_dir_all(&caches);
}

/// The real pipeline on a two-pixel frame, on the first adapter wgpu finds.
#[test]
fn the_input_plots_read_in_front_of_their_stage_and_a_curve_moves_only_what_follows() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the engine is not exercised");
        return;
    };
    let mut engine = Engine::on(&device);
    let (w, h) = (300usize, 200usize);
    let pixels = (0..w * h).map(|i| { let v = 0.02 + 0.6 * (i % w) as f32 / w as f32; [v, v * 0.7, v * 0.5, 1.0] }).collect();
    let negative = engine.prepare(Decoded { linear: Linear { width: w, height: h, pixels }, full_width: w as f64, undeclared: None }).unwrap();
    let rest = PipelineSettings::rest(oni_settings::ConversionMode::Negative, true);
    let before = engine.inputs(&negative, &rest, false).expect("readings");
    assert!(before.levels.rgb.len() > 2 && before.curves.rgb.len() > 2);
    // An adversarial manual curve: the output moves, the curves' own input does not.
    let mut curved = rest.clone();
    curved.manual_curves.linked.points = vec![oni_settings::cg::Point::new(0.0, 0.0), oni_settings::cg::Point::new(0.3, 0.8), oni_settings::cg::Point::new(1.0, 1.0)];
    let after = engine.inputs(&negative, &curved, false).expect("readings");
    assert_eq!(after.curves, before.curves, "the curves' input is read in front of them");
    let shown = |s: &PipelineSettings, e: &mut Engine| e.render(&negative, s, None).unwrap().pixels;
    let moved = shown(&rest, &mut engine).iter().zip(shown(&curved, &mut engine)).map(|(a, b)| (a[1] - b[1]).abs()).fold(0.0, f32::max);
    assert!(moved > 0.05, "the curve reaches the output: {moved}");
    // Twin: gains, in front of the levels, move the levels' own input.
    let mut gained = rest.clone();
    gained.gains.stops = [0.8, 0.0, -0.8];
    let gained = engine.inputs(&negative, &gained, false).expect("readings");
    assert_ne!(gained.levels, before.levels, "the levels' input reads the gains");
    // The small copy under the hand reads a third of the pixels.
    let small = engine.inputs(&negative, &rest, true).expect("readings");
    let count = |h: &oni_measure::histogram::Histogram| h.rgb.iter().map(|b| b[0]).sum::<f32>();
    assert!(count(&small.levels) < 0.2 * count(&before.levels), "{} of {}", count(&small.levels), count(&before.levels));
}

#[test]
fn the_engine_inverts_a_negative_and_keeps_a_positive_in_order() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the engine is not exercised");
        return;
    };
    let mut engine = Engine::on(&device);
    // Six columns, so the rebuilt edge leaves the two middle ones as they are.
    let (w, h) = (6usize, 6usize);
    let pixels = (0..w * h).map(|i| if (i % w) < 3 { [0.6, 0.4, 0.3, 1.0] } else { [0.05, 0.03, 0.02, 1.0] }).collect();
    let negative = engine.prepare(Decoded { linear: Linear { width: w, height: h, pixels }, full_width: w as f64, undeclared: None }).unwrap();
    let luma = |p: [f32; 4]| p[0] + p[1] + p[2];
    let settings = PipelineSettings::rest(oni_settings::ConversionMode::Negative, true);
    let out = engine.render(&negative, &settings, None).unwrap();
    assert_eq!((out.width, out.height), (w, h));
    let (thin, dense) = (out.pixels[2 * w + 2], out.pixels[2 * w + 3]);
    assert!(luma(thin) < luma(dense), "light through a thin negative prints dark: {thin:?} {dense:?}");
    // Twin: the positive mode renders the same frame without inverting it.
    let mut positive = PipelineSettings::rest(oni_settings::ConversionMode::Positive, true);
    positive.face = settings.face;
    let out = engine.render(&negative, &positive, None).unwrap();
    assert!(luma(out.pixels[2 * w + 2]) > luma(out.pixels[2 * w + 3]));
}

#[test]
fn a_reduced_preview_rebuilds_its_two_outer_rows_from_the_third() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the edge is not exercised");
        return;
    };
    let mut engine = Engine::on(&device);
    let (w, h) = (8usize, 7usize);
    let pixels: Vec<[f32; 4]> = (0..w * h).map(|i| [(i % w) as f32, (i / w) as f32, 0.0, 1.0]).collect();
    let negative = engine.prepare(Decoded { linear: Linear { width: w, height: h, pixels: pixels.clone() }, full_width: w as f64, undeclared: None }).unwrap();
    let mut graph_out = engine.graph.render(&Image::new(&negative.preview), negative.preview.extent(), None).unwrap();
    assert_eq!(graph_out.len(), w * h);
    let at = |x: usize, y: usize| graph_out[y * w + x];
    assert_eq!(at(0, 0)[..2], [2.0, 2.0], "the corner takes the third row and column");
    assert_eq!(at(7, 3)[..2], [5.0, 3.0]);
    assert_eq!(at(4, 4)[..2], [4.0, 4.0], "the inside is left as it is");
    // Twin: the decode itself had other values on its border.
    graph_out = pixels;
    assert_ne!(graph_out[0][..2], [2.0, 2.0]);
}

struct Failing;

impl Renderer<Linear> for Failing {
    fn render(&mut self, _: &Linear, _: &PipelineSettings) -> Result<Linear, String> {
        Err("unreadable".into())
    }
}

#[test]
fn a_failed_render_names_its_frame() {
    let (tx, rx) = mpsc::channel();
    let worker = Worker::new(Box::new(Failing), Screen::Srgb, move |d| {
        if let Delivery::Render(Err((path, _))) = d {
            tx.send(path).unwrap();
        }
    });
    let negative = Arc::new(Linear { width: 2, height: 1, pixels: vec![[0.5; 4]; 2] });
    let (kind, asked) = (crate::timing::Kind::Notch, Instant::now());
    worker.submit(Job { revision: 1, path: PathBuf::from("A.tif"), negative, settings: PipelineSettings::default(), kind, asked, view: None, reference: None, held: false });
    let failed = rx.recv_timeout(Duration::from_secs(10)).expect("a failure delivered");
    assert_eq!(failed, PathBuf::from("A.tif"));
    // Twin: the frame opened since is not the one that failed, so the landing drops it.
    assert_ne!(failed, PathBuf::from("B.tif"));
}
