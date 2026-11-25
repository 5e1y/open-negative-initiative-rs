//! Grain size is a grain's width on the film: its field's grains are as wide as it says, every resolution lays
//! the same grain on the picture, and a photo of an earlier format keeps the grain it laid; each with a twin.

use oni_graph::Rect;
use oni_pipeline::grain;
use oni_settings::blocks::Grain as GrainBlock;
use oni_settings::migrate::grain_size::{self, microns};
use oni_settings::migrate::{Refusal, chain::GRAIN_ON_FILM_VERSION};
use oni_settings::{ConversionMode, PipelineSettings};
use serde_json::json;

fn grained(size: f32) -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Digital, true);
    (s.grain.amount, s.grain.size) = (0.5, size);
    s
}

#[test]
fn a_grain_is_as_wide_as_its_size_says() {
    // 30 µm of a 36 mm frame on a scan 6048 pixels long: a grain 5.04 pixels across.
    let k = grain::grain(&grained(30.0), Rect::new(0, 0, 6048, 4032), 1.0).unwrap();
    assert!((2.0 * k.radius - 5.04).abs() < 1e-4, "{}", 2.0 * k.radius);
    // Twin: a size read as a radius lays a grain twice as wide.
    assert!((4.0 * k.radius - 5.04).abs() > 1.0);
}

#[test]
fn every_resolution_lays_the_same_grain_on_the_picture() {
    let s = grained(GrainBlock::SIZE_REST);
    let cells = |w: u32, h: u32| {
        let k = grain::grain(&s, Rect::new(0, 0, w, h), 1.0).unwrap();
        f64::from(w.max(h)) / f64::from(k.radius)
    };
    // 24 and 47 Mpx, a portrait, and a reduced render of the 47: as many cells along the long side.
    let (z, s1r) = (cells(6048, 4032), cells(8368, 5584));
    assert!((z / s1r - 1.0).abs() < 1e-6 && (cells(4032, 6048) / z - 1.0).abs() < 1e-6, "{z} {s1r}");
    // A RAW decoded at half size holds the same frame on half the pixels: as many cells again.
    assert!((cells(3024, 2016) / z - 1.0).abs() < 1e-6, "{} {z}", cells(3024, 2016));
    let reduced = grain::grain(&s, Rect::new(0, 0, 2092, 1396), 0.25).unwrap();
    assert!((8368.0 / f64::from(reduced.radius) / s1r - 1.0).abs() < 1e-6);
    // Its width in pixels follows the resolution: 20 µm of a 36 mm frame.
    assert!((GrainBlock::pixels(20.0, 6048.0) - 3.36).abs() < 1e-9 && (GrainBlock::pixels(20.0, 8368.0) - 4.6489).abs() < 1e-4);
    // Twin: a grain fixed in pixels, as a share of no picture, lays a coarser grain on the 24 Mpx frame.
    let fixed = |w: u32| f64::from(w) / GrainBlock::pixels(20.0, 8368.0);
    assert!((fixed(6048) / fixed(8368) - 1.0).abs() > 0.2);
}

#[test]
fn a_photo_of_an_earlier_format_keeps_the_grain_it_laid() {
    let older = GRAIN_ON_FILM_VERSION - 1;
    for (share, (w, h)) in [(0.0, (6048, 4032)), (0.4, (6048, 4032)), (0.5, (4032, 6048)), (1.0, (5584, 8368)), (0.75, (8368, 5584))] {
        let stored = json!({ "grain": { "amount": 0.5, "size": share } });
        let moved = grain_size::caught_up(older, grained(0.5), Some(&stored), &|_| Ok((f64::from(w), f64::from(h)))).unwrap();
        // The width the former cell laid, on the film: the scan's shape read, the step's law.
        let aspect = f64::from(w) / f64::from(w.max(h));
        assert!((moved.grain.size - microns(share, aspect)).abs() < 1e-4, "{share} on {w}×{h}: {}", moved.grain.size);
        // Twin: a portrait read as wider than tall lays a grain half as coarse again.
        if h > w {
            let flat = grain_size::unfiled(older, grained(0.5), Some(&stored));
            assert!((flat.grain.size / moved.grain.size - 1.0).abs() > 0.4);
        }
    }
    // A grain laid asks the file its shape; a file that cannot say holds the step.
    let laid = json!({ "grain": { "amount": 0.5, "size": 0.5 } });
    assert_eq!(grain_size::caught_up(older, grained(0.5), Some(&laid), &|_| Err(Refusal::Away)), Err(Refusal::Away));
    assert!((microns(0.5, 1.0) - 31.68).abs() < 1e-3);
}
