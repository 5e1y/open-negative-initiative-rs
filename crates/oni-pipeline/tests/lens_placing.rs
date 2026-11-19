//! A lens-corrected digital photo keeps its dust on the scan: a speck lands where the settings' map
//! says on every framing and every scale, the grain stays on the corrected picture, and the optical
//! vignetting lights the scan radially, a patch taking the gain where it lies.

mod common;

use common::{V4, graph};
use oni_graph::{Graph, Image, Rect, Source};
use oni_pipeline::{Inputs, Pipeline, corrections, geometry, grain, stages};
use oni_render::Warp;
use oni_settings::cg::{self, Point};
use oni_settings::frame::{Correction, Geometry};
use oni_settings::lens::LensCorrection;
use oni_settings::{ConversionMode, PipelineSettings};

const SCAN: (u32, u32) = (600, 400);
const GROUND: f32 = 0.2;
const BRIGHT: f32 = 5.0;
/// The speck's centre pixel, off every axis of the scan.
const SPECK: (u32, u32) = (451, 117);

fn size() -> (f64, f64) {
    (f64::from(SCAN.0), f64::from(SCAN.1))
}

/// The ground with a 3 × 3 speck, on a scan `side` times the size.
fn scan(g: &mut Graph, side: u32) -> Source {
    let (w, h) = (SCAN.0 * side, SCAN.1 * side);
    let mut pixels = vec![[GROUND, GROUND, GROUND, 1.0]; (w * h) as usize];
    for dy in 0..3 * side {
        for dx in 0..3 * side {
            pixels[((SPECK.1 * side - side + dy) * w + SPECK.0 * side - side + dx) as usize] = [BRIGHT, BRIGHT, BRIGHT, 1.0];
        }
    }
    g.upload(w, h, &pixels).expect("the scan")
}

fn speck() -> Point {
    Point::new((f64::from(SPECK.0) + 0.5) / size().0, (f64::from(SPECK.1) + 0.5) / size().1)
}

fn whole(g: &mut Graph, image: &Image) -> (Rect, Vec<V4>) {
    let e = image.extent();
    (e, g.render(image, e, None).expect("a render"))
}

/// The brightness-weighted centre of what stands above the ground, in fractions of the image.
fn bright_centre(e: Rect, pixels: &[V4]) -> Point {
    let (mut sum, mut x, mut y) = (0.0, 0.0, 0.0);
    for (i, p) in pixels.iter().enumerate() {
        let v = f64::from(p[1] - GROUND).max(0.0);
        sum += v;
        x += v * ((i % e.w as usize) as f64 + 0.5);
        y += v * ((i / e.w as usize) as f64 + 0.5);
    }
    Point::new(x / sum / f64::from(e.w), y / sum / f64::from(e.h))
}

fn digital(correction: LensCorrection, geometry: Geometry) -> PipelineSettings {
    PipelineSettings { lens_correction: correction, geometry, ..PipelineSettings::rest(ConversionMode::Digital, true) }
}

fn distorted() -> LensCorrection {
    LensCorrection { distortion: -60.0, ..Default::default() }
}

fn framings() -> Vec<(&'static str, Geometry)> {
    let crop = cg::Rect::new(0.15, 0.1, 0.8, 0.85);
    let mut turned = Geometry { crop, ..Geometry::default() };
    turned.rotate(1, size());
    let mut mirrored = Geometry { crop, ..Geometry::default() };
    mirrored.mirror();
    vec![
        ("whole", Geometry::default()),
        ("cropped", Geometry { crop, ..Geometry::default() }),
        ("a quarter turn", turned),
        ("mirrored", mirrored),
        ("3 degrees", Geometry { crop, ..Geometry::default() }.straightened(3.0, size())),
    ]
}

#[test]
fn a_speck_lands_where_the_settings_map_puts_it_under_distortion() {
    let Some(mut g) = graph("a_speck_lands_where_the_settings_map_puts_it_under_distortion") else { return };
    let source = scan(&mut g, 1);
    let mut pipeline = Pipeline::new();
    for (name, framing) in framings() {
        let s = digital(distorted(), framing);
        let framed = pipeline.framed(&mut g, Image::new(&source), &s, 1.0, &Inputs::default()).expect("framed");
        let (e, pixels) = whole(&mut g, &framed);
        let found = bright_centre(e, &pixels);
        let placing = s.placing(size());
        let wanted = placing.on_frame(speck());
        let miss = ((found.x - wanted.x) * f64::from(e.w)).hypot((found.y - wanted.y) * f64::from(e.h));
        println!("{name}: the speck {miss:.3} px from its map");
        assert!(miss < 0.5, "{name}: the speck at {found:?}, the map says {wanted:?}");
        let back = placing.on_scan(wanted);
        assert!((back.x - speck().x).abs() < 1e-9 && (back.y - speck().y).abs() < 1e-9, "{name}: back at {back:?}");
        // Twin: the turns' and the angle's map alone, the lens forgotten, misses it.
        let stale = s.geometry.placing(size()).on_frame(speck());
        let off = ((found.x - stale.x) * f64::from(e.w)).hypot((found.y - stale.y) * f64::from(e.h));
        assert!(off > 3.0, "{name}: the geometry's map alone is {off:.2} px off");
        // The grain stays on the corrected picture: the lens moves none of its arguments.
        let scan_extent = Rect::new(0, 0, SCAN.0, SCAN.1);
        let mut grained = s.clone();
        grained.grain.amount = 0.5;
        let bare = PipelineSettings { lens_correction: LensCorrection::default(), ..grained.clone() };
        assert_eq!(grain::grain(&grained, scan_extent, 1.0), grain::grain(&bare, scan_extent, 1.0), "{name}");
    }
    // A point of the scan the correction pushes off the frame maps outside it.
    let pushed = digital(distorted(), Geometry::default()).placing(size()).on_frame(Point::new(0.999, 0.5));
    assert!(pushed.x > 1.0, "{pushed:?}");
}

#[test]
fn a_preview_puts_the_speck_where_the_full_frame_does() {
    let Some(mut g) = graph("a_preview_puts_the_speck_where_the_full_frame_does") else { return };
    let source = scan(&mut g, 2);
    let mut pipeline = Pipeline::new();
    let s = digital(LensCorrection { distortion: -60.0, red: 40.0, blue: -40.0, vignetting: 0.0 }, Geometry::default());
    let full = (1200.0, 800.0);
    assert_eq!(s.lens(full), s.lens((600.0 / 0.5, 400.0 / 0.5)), "the zoom differs between the scales");
    let at = |g: &mut Graph, pipeline: &mut Pipeline, scale: f64| {
        let input = if scale == 1.0 { Image::new(&source) } else { Image::new(&source).lanczos(scale) };
        let framed = pipeline.framed(g, input, &s, scale, &Inputs::default()).expect("framed");
        let (e, pixels) = whole(g, &framed);
        (e, bright_centre(e, &pixels))
    };
    let (_, sharp) = at(&mut g, &mut pipeline, 1.0);
    let (e, reduced) = at(&mut g, &mut pipeline, 0.5);
    let apart = ((sharp.x - reduced.x) * f64::from(e.w)).hypot((sharp.y - reduced.y) * f64::from(e.h));
    println!("preview against full: {apart:.3} reduced px");
    assert!(apart < 1.0, "the preview's speck is {apart:.2} px off the full frame's");
    // Twin: the radius read in full-resolution pixels on the preview moves the speck.
    let radial = s.lens(full).unwrap();
    let input = Image::new(&source).lanczos(0.5);
    let w = geometry::warped(&s.geometry, &radial, input.extent(), 0.5);
    let in_pixels = stages::Warped { k: Warp { inv_unit2: w.k.inv_unit2 / 4.0, ..w.k }, ..w };
    let (e, pixels) = whole(&mut g, &input.then(in_pixels));
    let moved = bright_centre(e, &pixels);
    let off = ((moved.x - reduced.x) * f64::from(e.w)).hypot((moved.y - reduced.y) * f64::from(e.h));
    assert!(off > 2.0, "the radius in pixels moved the speck {off:.2} px");
}

fn stroke_on(p: Point) -> Correction {
    Correction { radius: 8.0, ..Correction::new(vec![p, Point::new(p.x + 1e-4, p.y)]) }
}

#[test]
fn the_optical_vignetting_lights_the_scan_radially_and_a_patch_takes_its_gain() {
    let Some(mut g) = graph("the_optical_vignetting_lights_the_scan_radially_and_a_patch_takes_its_gain") else { return };
    let (w, h) = SCAN;
    let flat = g.upload(w, h, &vec![[1.0; 4]; (w * h) as usize]).unwrap();
    let mut pipeline = Pipeline::new();
    let s = digital(LensCorrection { vignetting: 100.0, ..Default::default() }, Geometry::default());
    let healed = pipeline.healed(&mut g, Image::new(&flat), &s, 1.0, &Inputs::default()).unwrap();
    let (_, lit) = whole(&mut g, &healed);
    let at = |pixels: &[V4], x: u32, y: u32| pixels[(y * w + x) as usize][1];
    // The corner's centre lies just inside the half-diagonal: a gain a hair under 4.
    let u = (299.5f64.powi(2) + 199.5f64.powi(2)) / (300.0f64.powi(2) + 200.0f64.powi(2));
    assert!((f64::from(at(&lit, 0, 0)) - 4f64.powf(u)).abs() < 1e-4, "the corner reads {}", at(&lit, 0, 0));
    assert!((at(&lit, 299, 199) - 1.0).abs() < 1e-4, "the centre reads {}", at(&lit, 299, 199));
    // As far right of the centre as below it, the same gain: circular in pixels on a 3:2 scan.
    let (right, below) = (at(&lit, 299 + 150, 199), at(&lit, 299, 199 + 150));
    assert!((right - below).abs() < 1e-5, "{right} against {below}");
    // Twin: the creative vignette's ellipse, on the frame's own sides, lights them apart.
    let creative = Image::new(&flat).then(stages::vignette(Rect::new(0, 0, w, h), -2.0, 0.0));
    let (_, elliptic) = whole(&mut g, &creative);
    assert!((at(&elliptic, 299 + 150, 199) - at(&elliptic, 299, 199 + 150)).abs() > 1e-2);

    // A patch over a speck takes the gain of where it lies, as the ground around it.
    let specked = scan(&mut g, 1);
    let c = stroke_on(speck());
    let r = corrections::patch_rect(&c, [0.0, 0.0, size().0, size().1]);
    let patch = g.upload(r[2] as u32, r[3] as u32, &vec![[GROUND, GROUND, GROUND, 1.0]; (r[2] * r[3]) as usize]).unwrap();
    let healed_s = PipelineSettings { corrections: vec![c.clone()], ..s.clone() };
    let inputs = Inputs { patches: [(c.id, &patch)].into_iter().collect(), ..Inputs::default() };
    let healed = pipeline.healed(&mut g, Image::new(&specked), &healed_s, 1.0, &inputs).unwrap();
    let (_, pixels) = whole(&mut g, &healed);
    // The feathered stroke leaves a trace of the speck under its centre, under a hundredth.
    let (gain, healed) = (at(&lit, SPECK.0, SPECK.1), at(&pixels, SPECK.0, SPECK.1));
    assert!((healed / (GROUND * gain) - 1.0).abs() < 1e-2, "the patch reads {healed} for {}", GROUND * gain);
    assert!(healed / GROUND - 1.0 > 0.3, "the patch reads {healed}, ungained");
    // Twin: unpatched, the speck keeps its own light under the gain.
    let bare = pipeline.healed(&mut g, Image::new(&specked), &s, 1.0, &Inputs::default()).unwrap();
    let (_, raw) = whole(&mut g, &bare);
    assert!(at(&raw, SPECK.0, SPECK.1) > BRIGHT, "the speck reads {}", at(&raw, SPECK.0, SPECK.1));
}
