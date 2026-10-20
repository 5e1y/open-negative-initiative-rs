//! `Thumbnails.criterionChecks`: a thumbnail runs opposite to its negative, carries the settings and
//! an image, on a thin and a dense manufactured negative; a flat plate is refused.

use crate::display::{Params, Screen};
use crate::render::{Decoded, Device, Engine, Linear, encoded};
use crate::thumbnails::SIDE;
use oni_settings::PipelineSettings;

type V3 = [f32; 3];

/// One 8-bit step, doubled: anything under is the encoder, not a setting.
const NOISE_FLOOR: f32 = 2.0 / 255.0;
/// Below this the render carries no image.
const CODE_FLOOR: usize = 4;

/// A scene-linear negative: transmittance ramps across x, modulated down y, blue most attenuated.
fn negative(range: (f32, f32)) -> Decoded {
    let (w, h) = (320usize, 240usize);
    let pixels = (0..w * h)
        .map(|i| {
            let (along, down) = ((i % w) as f32 / (w - 1) as f32, (i / w) as f32 / (h - 1) as f32);
            let t = range.0 + (range.1 - range.0) * along * (0.75 + 0.25 * down);
            [t.min(0.999), (t * 0.88).min(0.999), (t * 0.74).min(0.999), 1.0]
        })
        .collect();
    Decoded { linear: Linear { width: w, height: h, pixels }, full_width: w as f64, undeclared: None }
}

/// One display-encoded sample per 8 × 8 block.
fn samples(l: &Linear) -> Vec<V3> {
    let rgba = encoded(l, &Params::new(Screen::Srgb, l.width as u32, l.height as u32, 0), 1);
    let mut out = Vec::new();
    for y in (0..l.height).step_by(8) {
        for x in (0..l.width).step_by(8) {
            let p = (y * l.width + x) * 4;
            out.push([0, 1, 2].map(|c| f32::from(rgba[p + c]) / 255.0));
        }
    }
    out
}

fn moved_share(a: &[V3], b: &[V3]) -> f64 {
    let moved = a.iter().zip(b).filter(|(p, q)| (0..3).any(|c| (p[c] - q[c]).abs() > NOISE_FLOOR)).count();
    moved as f64 / a.len() as f64
}

/// Pearson correlation of green: it carries no level, so a correct inversion runs against the negative.
fn correlation(a: &[V3], b: &[V3]) -> f64 {
    let n = a.len() as f64;
    let (ma, mb) = (a.iter().map(|v| f64::from(v[1])).sum::<f64>() / n, b.iter().map(|v| f64::from(v[1])).sum::<f64>() / n);
    let (mut cov, mut va, mut vb) = (0.0, 0.0, 0.0);
    for (p, q) in a.iter().zip(b) {
        let (da, db) = (f64::from(p[1]) - ma, f64::from(q[1]) - mb);
        (cov, va, vb) = (cov + da * db, va + da * da, vb + db * db);
    }
    if va > 0.0 && vb > 0.0 { cov / (va * vb).sqrt() } else { 0.0 }
}

fn occupied_codes(rgb: &[V3]) -> usize {
    rgb.iter().map(|v| (v[1].clamp(0.0, 1.0) * 255.0).round() as u8).collect::<std::collections::HashSet<_>>().len()
}

fn edited() -> PipelineSettings {
    let mut s = PipelineSettings { chroma_decompression: 1.0, ..PipelineSettings::default() };
    s.gains.stops = [2.2, 0.4, -0.6];
    s.print.shadows = -0.4;
    s
}

/// Which of the four criteria a negative's thumbnail meets: bounded, inverted, edited, an image.
fn criteria(engine: &mut Engine, decoded: Decoded) -> [bool; 4] {
    let against = samples(&decoded.linear);
    let prepared = engine.prepare(decoded).expect("the negative uploads");
    let mut thumb = |s: &PipelineSettings| engine.render(&prepared, &s.per_pixel_only(), Some(SIDE)).expect("the thumbnail renders");
    let (neutral, adjusted) = (thumb(&PipelineSettings::default()), thumb(&edited()));
    let bounded = neutral.width.max(neutral.height) <= SIDE + 1;
    let (neutral, adjusted) = (samples(&neutral), samples(&adjusted));
    assert_eq!(neutral.len(), against.len(), "the thumbnail overlays its negative");
    [bounded, correlation(&against, &neutral) < -0.9, moved_share(&neutral, &adjusted) > 0.5, occupied_codes(&neutral) > CODE_FLOOR]
}

#[test]
fn a_thumbnail_runs_opposite_to_its_negative_on_a_thin_and_a_dense_one_and_a_plate_is_refused() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the criterion is not exercised");
        return;
    };
    let mut engine = Engine::on(&device);
    for (name, range) in [("thin", (0.80, 0.995)), ("dense", (0.22, 0.34))] {
        let met = criteria(&mut engine, negative(range));
        assert_eq!(met, [true; 4], "{name} negative: bounded, inverted, edited, an image");
    }
    // The correlation discriminates: the negative against itself, a forgotten inversion, is refused.
    let thin = samples(&negative((0.80, 0.995)).linear);
    assert!(correlation(&thin, &thin) > 0.9);
    // Twin: a source rendering as a flat plate fails at least one criterion.
    let plate = criteria(&mut engine, negative((0.9, 0.9)));
    assert!(plate.iter().any(|m| !m), "a flat plate passes every criterion: {plate:?}");
}

/// `Thumbnails.selfCheck(source:)`: the same four criteria on a frame of the reference roll; the
/// inversion's bar is frame-dependent, DSC_6440 reading −0.86 against Swift's −0.90.
#[test]
fn a_thumbnail_of_the_reference_roll_runs_opposite_to_its_negative() {
    let path = oni_fixtures::folder(oni_fixtures::RAW_DIR).join("DSC_6432.NEF");
    let Ok(device) = Device::open() else { return eprintln!("no adapter: the criterion is not exercised") };
    let Ok(decoded) = crate::render::decode_alone(&path, Some(SIDE as f64)) else { return eprintln!("SKIPPED: no {}", path.display()) };
    let against = samples(&decoded.linear);
    let met = criteria(&mut Engine::on(&device), decoded);
    assert_eq!(met, [true; 4], "bounded, inverted, edited, an image");
    // Twin: the negative against itself, a forgotten inversion, is refused.
    assert!(correlation(&against, &against) > 0.9);
}
