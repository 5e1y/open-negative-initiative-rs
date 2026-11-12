//! A dust stroke lives on the scan: stage 0's map finds a scan point on any framing, and a patch
//! laid there stays on its speck whatever the crop, the turns, the mirrors or the angle.

mod common;

use common::{V4, graph};
use oni_graph::{Graph, Image, Source};
use oni_pipeline::{Inputs, Pipeline, corrections, geometry};
use oni_settings::cg::{Point, Rect};
use oni_settings::frame::{Correction, Geometry};
use oni_settings::{ConversionMode, PipelineSettings};

/// The scan, and its one speck's pixel.
const SCAN: (u32, u32) = (60, 40);
const SPECK: (u32, u32) = (37, 13);
const GROUND: f32 = 0.2;
const BRIGHT: f32 = 5.0;

fn scan(g: &mut Graph) -> Source {
    let mut pixels = vec![[GROUND, GROUND, GROUND, 1.0]; (SCAN.0 * SCAN.1) as usize];
    pixels[(SPECK.1 * SCAN.0 + SPECK.0) as usize] = [BRIGHT, BRIGHT, BRIGHT, 1.0];
    g.upload(SCAN.0, SCAN.1, &pixels).expect("the scan")
}

fn size() -> (f64, f64) {
    (f64::from(SCAN.0), f64::from(SCAN.1))
}

/// The speck's centre in fractions of the scan.
fn speck() -> Point {
    Point::new((f64::from(SPECK.0) + 0.5) / size().0, (f64::from(SPECK.1) + 0.5) / size().1)
}

fn cropped() -> Geometry {
    Geometry { crop: Rect::new(0.2, 0.2, 0.6, 0.6), ..Geometry::default() }
}

/// Every framing a frame meets: turns, mirrors, a moved crop, an angle, the Crop tab's and Prepare's.
fn framings() -> Vec<(&'static str, Geometry)> {
    let mut turned = cropped();
    turned.rotate(1, size());
    let mut mirrored = cropped();
    mirrored.mirror();
    let mut upside = cropped();
    upside.mirror_vertically();
    let mut half = Geometry { quarter_turns: 2, ..cropped() };
    half.rotate(1, size());
    let tilted = cropped().straightened(3.0, size());
    let mut uncropped = tilted;
    (uncropped.crop, uncropped.angle) = (Rect::UNIT, 0.0);
    vec![
        ("cropped", cropped()),
        ("crop moved", Geometry { crop: Rect::new(0.3, 0.05, 0.7, 0.9), ..Geometry::default() }),
        ("a quarter turn", turned),
        ("three quarter turns", half),
        ("mirrored", mirrored),
        ("mirrored vertically", upside),
        ("3 degrees", tilted),
        ("the Crop tab", tilted.without_crop()),
        ("Prepare's whole scan", uncropped),
    ]
}

fn whole(g: &mut Graph, image: &Image) -> (oni_graph::Rect, Vec<V4>) {
    let e = image.extent();
    (e, g.render(image, e, None).expect("a render"))
}

/// The brightness-weighted centre of what stands above the ground, in the image's own pixels.
fn bright_centre(e: oni_graph::Rect, pixels: &[V4]) -> (f64, f64) {
    let (mut sum, mut x, mut y) = (0.0, 0.0, 0.0);
    for (i, p) in pixels.iter().enumerate() {
        let v = f64::from(p[1] - GROUND).max(0.0);
        sum += v;
        x += v * ((i % e.w as usize) as f64 + 0.5);
        y += v * ((i / e.w as usize) as f64 + 0.5);
    }
    (x / sum, y / sum)
}

#[test]
fn a_scan_point_lands_where_stage_0_puts_its_pixel_and_comes_back() {
    let Some(mut g) = graph("a_scan_point_lands_where_stage_0_puts_its_pixel_and_comes_back") else { return };
    let source = scan(&mut g);
    for (name, framing) in framings() {
        let (e, pixels) = whole(&mut g, &geometry::apply(Image::new(&source), &framing, 1.0));
        let placing = framing.placing(size());
        let at = placing.on_frame(speck());
        let wanted = (at.x * f64::from(e.w), at.y * f64::from(e.h));
        let found = bright_centre(e, &pixels);
        let reach = if framing.angle == 0.0 { 0.5 } else { 1.0 };
        assert!((found.0 - wanted.0).abs() < reach && (found.1 - wanted.1).abs() < reach, "{name}: the speck at {found:?}, the map says {wanted:?}");
        let back = placing.on_scan(at);
        assert!((back.x - speck().x).abs() < 1e-9 && (back.y - speck().y).abs() < 1e-9, "{name}: back at {back:?}");
        // Twin: the scan's fraction read on the framed frame misses the speck.
        if name == "a quarter turn" {
            let stale = (speck().x * f64::from(e.w), speck().y * f64::from(e.h));
            assert!((stale.0 - found.0).hypot(stale.1 - found.1) > 5.0, "the twin found the speck at {stale:?}");
        }
    }
}

fn stroke_on(p: Point) -> Correction {
    Correction { radius: 6.0, ..Correction::new(vec![p, Point::new(p.x + 1e-4, p.y)]) }
}

/// A patch of the ground's value, the size the scan's stroke asks.
fn patch(g: &mut Graph, c: &Correction, extent: (f64, f64)) -> Source {
    let r = corrections::patch_rect(c, [0.0, 0.0, extent.0, extent.1]);
    g.upload(r[2] as u32, r[3] as u32, &vec![[GROUND, GROUND, GROUND, 1.0]; (r[2] * r[3]) as usize]).expect("a patch")
}

/// The framed frame's green at a point in its fractions.
fn read_at(g: &mut Graph, image: &Image, at: Point) -> f32 {
    let (e, pixels) = whole(g, image);
    let (x, y) = ((at.x * f64::from(e.w)) as usize, (at.y * f64::from(e.h)) as usize);
    pixels[y * e.w as usize + x][1]
}

#[test]
fn a_patch_painted_on_one_framing_heals_its_speck_on_every_other() {
    let Some(mut g) = graph("a_patch_painted_on_one_framing_heals_its_speck_on_every_other") else { return };
    let source = scan(&mut g);
    let mut pipeline = Pipeline::new();
    let c = stroke_on(speck());
    let laid = patch(&mut g, &c, size());
    for (name, framing) in framings() {
        let mut s = PipelineSettings::rest(ConversionMode::Positive, true);
        (s.geometry, s.corrections) = (framing, vec![c.clone()]);
        let inputs = Inputs { patches: [(c.id, &laid)].into_iter().collect(), custom_flat_field: None };
        let framed = pipeline.framed(&mut g, Image::new(&source), &s, 1.0, &inputs).expect("framed");
        let at = framing.placing(size()).on_frame(speck());
        let healed = read_at(&mut g, &framed, at);
        assert!(healed < 0.5, "{name}: the speck reads {healed}");
    }
    // Twin: a stroke kept in the fractions of the frame it was painted on, laid after stage 0 on
    // another framing, leaves the speck bright.
    let painted = cropped().placing(size()).on_frame(speck());
    let (_, moved) = &framings()[1];
    let after = geometry::apply(Image::new(&source), moved, 1.0);
    let e = after.extent();
    let old = stroke_on(painted);
    let stale = patch(&mut g, &old, (f64::from(e.w), f64::from(e.h)));
    let flat = PipelineSettings { corrections: vec![old.clone()], ..PipelineSettings::rest(ConversionMode::Positive, true) };
    let inputs = Inputs { patches: [(old.id, &stale)].into_iter().collect(), custom_flat_field: None };
    let framed = pipeline.framed(&mut g, after, &flat, 1.0, &inputs).expect("framed");
    let bright = read_at(&mut g, &framed, moved.placing(size()).on_frame(speck()));
    assert!(bright > 1.0, "the twin healed the speck: {bright}");
}
