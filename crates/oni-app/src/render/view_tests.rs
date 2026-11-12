//! The view at 100 %: the full frame's pixels one for one on the canvas, its size read on its own grid.
use super::*;
use oni_pipeline::geometry;
use oni_settings::frame::Geometry;

/// A turned frame cut off the preview's grid shows its full render at 100 % unstretched.
#[test]
fn at_a_hundred_percent_a_cut_frame_shows_its_full_pixels_one_for_one() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the view at 100 % is not exercised");
        return;
    };
    let mut engine = Engine::on(&device);
    let (fw, fh) = (1600usize, 1000usize);
    let gradient = |w: usize, h: usize| (0..w * h).map(|i| [0.2 + 0.5 * (i % w) as f32 / w as f32, 0.3, 0.4, 1.0]).collect::<Vec<_>>();
    let negative = engine.prepare(Decoded { linear: Linear { width: 400, height: 250, pixels: gradient(400, 250) }, full_width: fw as f64, undeclared: None }).unwrap();
    let full = engine.full(Decoded { linear: Linear { width: fw, height: fh, pixels: gradient(fw, fh) }, full_width: fw as f64, undeclared: None }).unwrap();
    let mut settings = PipelineSettings::rest(oni_settings::ConversionMode::Positive, true);
    // Turned, the left edge falls at 101 full pixels, 25.25 on the quarter preview, cut there a column wider.
    settings.geometry = Geometry { quarter_turns: 1, crop: oni_settings::cg::Rect::new(0.1003, 0.0, 0.8997, 1.0), ..Geometry::default() };
    let hundred = crate::viewport::Viewport { scale: Some(1.0), offset: (0.0, 0.0) };
    let view = View { viewport: hundred, size: (600.0, 400.0), full: Some(full), coarse: false };
    let (source, placed) = engine.resident(&negative, &settings, Some(&view)).unwrap();
    let placed = placed.unwrap();
    let cut = geometry::extent(&settings.geometry, Rect::new(0, 0, fw as u32, fh as u32), 1.0);
    assert!(placed.at_full_resolution);
    assert_eq!(placed.full, (f64::from(cut.w), f64::from(cut.h)), "the full frame's size, as stage 0 cuts it");
    let e = source.extent();
    assert_eq!((placed.width, placed.height), (f64::from(e.w), f64::from(e.h)), "each texel on one drawable pixel");
    // Twin: the size read off the preview's cut and scaled up stretches the full render past 1:1.
    let guessed = geometry::extent(&settings.geometry, Rect::new(0, 0, 400, 250), 0.25);
    assert_ne!(f64::from(guessed.w) * 4.0, placed.full.0);
    assert!((f64::from(guessed.w) * 4.0 - placed.full.0).abs() >= 1.0);
}
