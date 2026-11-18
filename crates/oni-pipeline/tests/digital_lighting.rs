//! A digital photo's Lighting acts on its levels and curves: its picture renders with Lighting and
//! Saturation rested, the curves read that picture, then the print lays the points, the tone, Hunt and
//! Saturation on what they draw. A film keeps Lighting in its fused kernel, its graph untouched.

mod common;

use common::{error, graph, judge, noise, whole};
use oni_graph::{Graph, Image, Source};
use oni_pipeline::{Inputs, Pipeline, Request, conversion, finishing};
use oni_render::{Stage47, cpu};
use oni_settings::cg::Point;
use oni_settings::levels::Levels;
use oni_settings::tone::color_density;
use oni_settings::{ConversionMode, PipelineSettings};

type V4 = [f32; 4];

/// Every Lighting value the print carries, off its rest.
fn lit(mut s: PipelineSettings) -> PipelineSettings {
    s.print.exposure += 0.6;
    s.print.contrast = 0.2;
    s.print.shadows = 0.1;
    s.print.black_point = 0.03;
    s.print.white_point = 0.05;
    s.density = 0.2;
    s
}

fn curved(mut s: PipelineSettings) -> PipelineSettings {
    s.manual_curves.linked.add(Point::new(0.3, 0.2));
    s.manual_curves.linked.add(Point::new(0.7, 0.85));
    s.picture_levels.red.set_white(0.85);
    s
}

fn digital() -> PipelineSettings {
    PipelineSettings::rest(ConversionMode::Digital, true)
}

/// The fused kernel Lighting rode in before it followed the curves: the points reaching the windows.
fn former(s: &PipelineSettings) -> Stage47 {
    let lighting = conversion::lighting(s).expect("Lighting moved");
    let reached = conversion::reaching(&s.print, &conversion::placed_levels(s), None, false);
    let lanes = |l: Levels| [l.black, l.white, l.gamma()];
    Stage47 { red: lanes(reached.red), green: lanes(reached.green), blue: lanes(reached.blue), ..lighting }
}

fn sweep() -> Vec<V4> {
    (0..=600).map(|i| i as f32 / 400.0).map(|x| [x, x * 0.8 + 0.01, (x * 1.1).min(1.5), 1.0]).collect()
}

/// With nothing between them, the picture then its Lighting is the former fused kernel on any light.
#[test]
fn the_picture_then_its_lighting_is_the_former_kernel_on_any_light() {
    let mut worst: f32 = 0.0;
    for black_point in [0.03, -0.1] {
        let mut s = lit(digital());
        s.print.black_point = black_point;
        let (head, tail, fused) = (conversion::stage47(&s), conversion::lighting(&s).expect("lit"), former(&s));
        let split: Vec<V4> = sweep().iter().map(|&p| cpu::print(cpu::stage47(p, &head), &tail)).collect();
        let whole: Vec<V4> = sweep().iter().map(|&p| cpu::stage47(p, &fused)).collect();
        worst = worst.max(error(&split, &whole));
        // Twin: a curve between them, as the order was, renders elsewhere.
        let bend = |p: V4| [p[0].max(0.0).powf(0.7), p[1].max(0.0).powf(0.7), p[2].max(0.0).powf(0.7), p[3]];
        let after: Vec<V4> = sweep().iter().map(|&p| cpu::print(bend(cpu::stage47(p, &head)), &tail)).collect();
        let before: Vec<V4> = sweep().iter().map(|&p| bend(cpu::stage47(p, &fused))).collect();
        assert!(error(&after, &before) > 1e-2, "black point {black_point}: the order shows no difference");
    }
    println!("picture then Lighting against the former kernel: {worst:.2e}");
    assert!(worst < 1e-5, "{worst:.2e}");
}

/// The frame through the real chain against the scalar composition: the picture, its levels and
/// curves, then Lighting and the points' saturation.
#[test]
fn a_digital_photos_lighting_acts_on_its_levels_and_curves() {
    let Some(mut g) = graph("a_digital_photos_lighting_acts_on_its_levels_and_curves") else { return };
    let (w, h) = (48, 32);
    let pixels = noise(w, h, 9, 0.0, 1.3);
    let scan = g.upload(w, h, &pixels).unwrap();
    let s = lit(curved(digital())).per_pixel_only();
    let request = Request { measuring: true, ..Request::default() };
    let image = Pipeline::new().image(&mut g, &scan, &s, &request, &Inputs::default()).expect("the recipe builds");
    let out = whole(&mut g, &image);
    let tables = finishing::curves(&s.manual_curves, Some(&s.picture_levels)).expect("tables");
    let points = finishing::saturation(&finishing::points_curve(&s.print, s.mode)).expect("the points' saturation");
    let (head, tail) = (conversion::stage47(&s), conversion::lighting(&s).expect("lit"));
    let now = |p: V4| cpu::saturation_curve(cpu::print(cpu::curves(cpu::stage47(p, &head), &tables), &tail), &points);
    // Twin: the former order, the curves on the lit picture.
    let fused = former(&s);
    let was = |p: V4| cpu::saturation_curve(cpu::curves(cpu::stage47(p, &fused), &tables), &points);
    judge("Lighting after the levels and curves", &out, &pixels.iter().map(|&p| now(p)).collect::<Vec<_>>(), &pixels.iter().map(|&p| was(p)).collect::<Vec<_>>());
}

fn seen(g: &mut Graph, scan: &Source, s: &PipelineSettings) -> (u64, usize, Vec<V4>) {
    let image: Image = Pipeline::new().image(g, scan, s, &Request::default(), &Inputs::default()).expect("the recipe builds");
    (image.key(), image.stages(), whole(g, &image))
}

/// Lighting at rest lays no stage, its picture the frame's own print; a film keeps Lighting in its
/// fused kernel whatever its curves.
#[test]
fn lighting_at_rest_lays_no_stage_and_a_film_keeps_its_kernel() {
    for s in [digital(), curved(digital())] {
        assert!(conversion::lighting(&s).is_none());
        assert_eq!((s.unlit().print, s.unlit().density), (s.print, s.density), "the picture renders the frame's own print");
    }
    for mode in [ConversionMode::Negative, ConversionMode::Positive, ConversionMode::Monochrome] {
        let s = lit(curved(PipelineSettings::rest(mode, true)));
        assert!(conversion::lighting(&s).is_none(), "{mode:?}");
        let k = conversion::stage47(&s);
        let density = if s.mode.is_monochrome() { 1.0 } else { color_density::factor(0.2) };
        assert_eq!((k.print_stops, k.print_contrast, k.density), (s.print.exposure, 0.2, density), "{mode:?}");
    }
    // Twin: on a digital photo the same Lighting leaves the picture and rides after it.
    let s = lit(curved(digital()));
    let k = conversion::stage47(&s);
    assert_eq!((k.print_stops, k.print_contrast, k.density), (0.0, 0.0, 1.0));
    assert!(conversion::lighting(&s).is_some_and(|l| l.print_stops == s.print.exposure && l.red != k.red));
}

/// With its curves at rest, a digital photo's Lighting rides in its picture's pass; after its tables it
/// takes finishing's first, a pass of its own when that one is empty.
#[test]
fn with_curves_at_rest_the_lighting_rides_in_the_pictures_pass() {
    let Some(mut g) = graph("with_curves_at_rest_the_lighting_rides_in_the_pictures_pass") else { return };
    let scan = g.upload(16, 8, &noise(16, 8, 3, 0.0, 1.2)).unwrap();
    let exposed = |mut s: PipelineSettings| {
        s.print.exposure = 0.5;
        s
    };
    let (_, rest, _) = seen(&mut g, &scan, &digital());
    let (_, bright, _) = seen(&mut g, &scan, &exposed(digital()));
    assert_eq!(bright, rest, "Lighting fused into the picture's pass");
    let (_, curves, _) = seen(&mut g, &scan, &curved(digital()));
    let (_, both, _) = seen(&mut g, &scan, &exposed(curved(digital())));
    // Twin: after the tables the same Lighting is a pass of its own.
    assert_eq!(both, curves + 1);
}
