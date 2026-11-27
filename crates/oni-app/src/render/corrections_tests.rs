//! The corrections reaching the render: a custom flat-field by its id, a dust patch through its stroke.
use super::*;

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
    let negative = engine.prepare(Decoded { linear: Linear { width: w, height: h, pixels: vec![[0.3, 0.3, 0.3, 1.0]; w * h] }, full_width: w as f64, undeclared: None, seed: [0, 0] }).unwrap();
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
    let negative = engine.prepare(Decoded { linear: Linear { width: w, height: h, pixels: vec![[0.3, 0.3, 0.3, 1.0]; w * h] }, full_width: w as f64, undeclared: None, seed: [0, 0] }).unwrap();
    let mut settings = PipelineSettings::rest(oni_settings::ConversionMode::Positive, true);
    let plain = engine.render(&negative, &settings, None).unwrap();
    let id = Id::parse("0F2B6E4C-9A1D-4E7B-8C3A-5D6E7F809A1B").unwrap();
    let point = |x: f64, y: f64| oni_settings::cg::Point { x, y };
    settings.corrections = vec![oni_settings::frame::Correction { id, stroke: vec![point(0.5, 0.5)], radius: 6.0, gesture: None }];
    let missing = engine.render(&negative, &settings, None).unwrap();
    assert!(missing.pixels.iter().zip(&plain.pixels).all(|(a, b)| (a[1] - b[1]).abs() < 1e-6), "no patch, no change");
    // A bright patch the size of the tile: the frame is 64 × 48, so the tile is the whole frame.
    std::fs::create_dir_all(caches.join("Corrections")).unwrap();
    let out = std::fs::File::create(crate::patches::file(&caches, oni_pipeline::corrections::PatchKey::of(&settings, &settings.corrections[0]))).unwrap();
    let bright: Vec<f32> = (0..w * h).flat_map(|_| [0.9f32, 0.9, 0.9, 1.0]).collect();
    tiff::encoder::TiffEncoder::new(out).unwrap().write_image::<tiff::encoder::colortype::RGBA32Float>(w as u32, h as u32, &bright).unwrap();
    let healed = engine.render(&negative, &settings, None).unwrap();
    let at = |l: &Linear, x: usize, y: usize| l.pixels[y * w + x][1];
    assert!((at(&healed, w / 2, h / 2) - at(&plain, w / 2, h / 2)).abs() > 1e-2, "the stroke's centre takes the patch");
    // Twin: far from the stroke the frame is its own, so a patch laid everywhere would be seen.
    assert!((at(&healed, 2, 2) - at(&plain, 2, 2)).abs() < 1e-5);
    let _ = std::fs::remove_dir_all(&caches);
}
