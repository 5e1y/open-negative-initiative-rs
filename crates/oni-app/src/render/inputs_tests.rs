//! The input histograms read the frame the canvas shows: its custom mask and dust patches laid.
use super::*;

fn tiff(path: &std::path::Path, w: u32, h: u32, pixels: &[f32]) {
    let out = std::fs::File::create(path).unwrap();
    tiff::encoder::TiffEncoder::new(out).unwrap().write_image::<tiff::encoder::colortype::RGBA32Float>(w, h, pixels).unwrap();
}

/// A mask or a patch the frame names moves both readings; one whose file is missing reads the bare
/// frame, and the same file arriving later moves the reading kept for the same settings.
#[test]
fn the_input_histograms_lay_the_frames_mask_and_patches() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the input histograms' mask and patches are not exercised");
        return;
    };
    let root = std::env::temp_dir().join(format!("oni-inputs-laid-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (support, caches) = (root.join("support"), root.join("caches"));
    let masks = crate::flat_fields::folder(&support);
    std::fs::create_dir_all(&masks).unwrap();
    std::fs::create_dir_all(caches.join("Corrections")).unwrap();
    // A panel falling off to the right, so a flat frame divided by it spreads over many bins.
    let (mw, mh) = (8u32, 4u32);
    let mask: Vec<f32> = (0..mw * mh).flat_map(|i| { let v = 1.0 - 0.6 * (i % mw) as f32 / mw as f32; [v, v, v, 1.0] }).collect();
    tiff(&masks.join("panel.tiff"), mw, mh, &mask);
    let mut engine = Engine::on(&device);
    engine.support = Some(support);
    engine.caches = Some(caches.clone());
    // 64 × 48: a patch the size of the frame is cut to its stroke's tile, laid where it was made.
    let (w, h) = (64usize, 48usize);
    let negative = engine.prepare(Decoded { linear: Linear { width: w, height: h, pixels: vec![[0.3, 0.3, 0.3, 1.0]; w * h] }, full_width: w as f64, undeclared: None }).unwrap();
    let rest = PipelineSettings::rest(oni_settings::ConversionMode::Positive, true);
    let bare = engine.inputs(&negative, &rest, false).expect("readings");

    let mut masked = rest.clone();
    masked.flat_field = oni_settings::frame::FlatField { mask: FlatFieldMask::Custom, custom: Some("panel".into()), ..Default::default() };
    let divided = engine.inputs(&negative, &masked, false).expect("readings");
    assert_ne!(divided.levels, bare.levels, "the mask reaches the levels' input");
    assert_ne!(divided.curves, bare.curves, "the mask reaches the curves' input");
    // Twin: a mask whose file is missing lays nothing, so both read the bare frame.
    masked.flat_field.custom = Some("gone".into());
    let gone = engine.inputs(&negative, &masked, false).expect("readings");
    assert_eq!((&gone.levels, &gone.curves), (&bare.levels, &bare.curves));
    // The same id loaded afterwards moves the reading the same settings kept.
    std::fs::copy(masks.join("panel.tiff"), masks.join("gone.tiff")).unwrap();
    let found = engine.inputs(&negative, &masked, false).expect("readings");
    assert_eq!(found.levels, divided.levels, "the mask read later is laid");

    let id = Id::parse("0F2B6E4C-9A1D-4E7B-8C3A-5D6E7F809A1B").unwrap();
    let mut dusted = rest.clone();
    dusted.corrections = vec![oni_settings::frame::Correction { id, stroke: vec![oni_settings::cg::Point::new(0.5, 0.5)], radius: 6.0 }];
    // Twin: a patch not generated yet lays nothing.
    let missing = engine.inputs(&negative, &dusted, false).expect("readings");
    assert_eq!((&missing.levels, &missing.curves), (&bare.levels, &bare.curves));
    let bright: Vec<f32> = (0..w * h).flat_map(|_| [0.9f32, 0.9, 0.9, 1.0]).collect();
    tiff(&crate::patches::file(&caches, id), w as u32, h as u32, &bright);
    let healed = engine.inputs(&negative, &dusted, false).expect("readings");
    assert_ne!(healed.levels, bare.levels, "the patch reaches the levels' input");
    assert_ne!(healed.curves, bare.curves, "the patch reaches the curves' input");
    let _ = std::fs::remove_dir_all(&root);
}
