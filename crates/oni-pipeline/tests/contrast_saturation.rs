//! Contrast's colour: the whole render lays the saturation curve the Saturation Curve's plot shows, on ONI's print and
//! on a paper, and nothing at Contrast's rest.

mod common;
use common::{TOLERANCE, V4, error, graph, noise, whole};
use oni_graph::{Graph, Image, Source};
use oni_pipeline::{Pipeline, Request, conversion, finishing, stages};
use oni_render::cpu;
use oni_settings::axis::pipeline::luma;
use oni_settings::curves::{Curve, SaturationCurve};
use oni_settings::{ConversionMode, PipelineSettings};

/// Flat patches, so the spatial stages leave their centres as they are.
const PATCH: u32 = 9;
const ACROSS: u32 = 12;
const DOWN: u32 = 8;

/// Each patch's scan: a grey from thin to dense with a colour pushed off it, one per patch.
fn patches() -> Vec<V4> {
    let tilts = noise(ACROSS, DOWN, 41, -0.12, 0.12);
    (0..ACROSS * DOWN)
        .map(|i| {
            let d = 0.15 + 2.4 * i as f32 / (ACROSS * DOWN - 1) as f32;
            let t = tilts[i as usize];
            [10f32.powf(-(d + t[0])), 10f32.powf(-(d + t[1])), 10f32.powf(-(d + t[2])), 1.0]
        })
        .collect()
}

fn upload(g: &mut Graph, scan: &[V4]) -> Source {
    let (w, h) = (ACROSS * PATCH, DOWN * PATCH);
    let pixels: Vec<V4> = (0..w * h).map(|i| scan[((i / w / PATCH) * ACROSS + (i % w) / PATCH) as usize]).collect();
    g.upload(w, h, &pixels).unwrap()
}

/// The render's patch centres.
fn centres(g: &mut Graph, pipeline: &mut Pipeline, source: &Source, s: &PipelineSettings) -> Vec<V4> {
    let request = Request { full_width: Some(f64::from(ACROSS * PATCH)), ..Request::default() };
    let image = pipeline.image(g, source, s, &request, &Default::default()).unwrap();
    let out = whole(g, &image);
    let w = ACROSS * PATCH;
    (0..ACROSS * DOWN).map(|i| out[((i / ACROSS * PATCH + PATCH / 2) * w + i % ACROSS * PATCH + PATCH / 2) as usize]).collect()
}

/// The colour `shown` lays on a pixel, read off the plot's own spline: each curve twice its value at the pixel's tone.
fn laid(p: V4, shown: &[SaturationCurve]) -> V4 {
    let y = luma([p[0], p[1], p[2]]);
    let x = f64::from(y.max(0.0).powf(1.0 / 2.2).clamp(0.0, 1.0));
    let f = shown.iter().map(|c| (2.0 * Curve { points: c.points.clone() }.evaluated().value(x)).max(0.0)).product::<f64>() as f32;
    [y + f * (p[0] - y), y + f * (p[1] - y), y + f * (p[2] - y), p[3]]
}

/// Stage 4-7's scalar reference, then `shown`'s colour and the output clip: the render where every other stage rests.
fn expected(scan: &[V4], s: &PipelineSettings, shown: &[SaturationCurve]) -> Vec<V4> {
    let k = conversion::stage47(s);
    scan.iter().map(|p| cpu::output_clip(laid(cpu::stage47(*p, &k), shown))).collect()
}

fn settled(paper: bool, contrast: f32) -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.print_paper = if paper { Some(Default::default()) } else { None };
    (s.sharpen.amount, s.chroma.force, s.texture.amount, s.secret_sauce) = (0.0, 0.0, 0.0, 0.0);
    s.print.contrast = contrast;
    s
}

#[test]
fn the_render_lays_contrasts_saturation_as_the_plot_shows_it() {
    let Some(mut g) = graph("the_render_lays_contrasts_saturation_as_the_plot_shows_it") else { return };
    let scan = patches();
    let source = upload(&mut g, &scan);
    let mut pipeline = Pipeline::new();
    for paper in [false, true] {
        let rest = settled(paper, 0.0);
        assert_eq!(finishing::contrast_curve(&rest), SaturationCurve::default(), "Contrast at rest lays no curve");
        let at_rest = error(&centres(&mut g, &mut pipeline, &source, &rest), &expected(&scan, &rest, &[]));
        println!("paper {paper}, Contrast at rest: {at_rest:.2e} off the reference");
        assert!(at_rest < TOLERANCE, "{paper}: {at_rest}");
        for travel in [-0.3, 0.3] {
            let s = settled(paper, travel);
            let shown = finishing::print_saturation(&s);
            assert!(!shown[1].is_neutral(), "{paper} {travel}: Contrast lays a curve");
            let render = centres(&mut g, &mut pipeline, &source, &s);
            let e = error(&render, &expected(&scan, &s, &shown));
            // Twin: the curve not applied.
            let bare = error(&render, &expected(&scan, &s, &[]));
            println!("paper {paper}, Contrast {travel:+}: {e:.2e} off the shown curve, {bare:.2e} off the print without it");
            assert!(e < TOLERANCE, "{paper} {travel}: {e}");
            assert!(bare > 1e-2, "{paper} {travel}: the check sees no curve ({bare})");
        }
    }
}

#[test]
fn contrasts_curve_takes_the_colour_its_slope_gives() {
    // Steeper mids lose colour and softer ones gain it, the black untouched.
    for (travel, mids_below) in [(0.3, true), (-0.3, false)] {
        let c = finishing::contrast_curve(&settled(false, travel));
        assert_eq!(c.points[0].y, 0.5, "{travel}: the black keeps its colour");
        assert_eq!(c.points[2].y < 0.45, mids_below, "{travel}: {:?}", c.points);
        assert_eq!(c.points[2].y > 0.55, !mids_below, "{travel}: {:?}", c.points);
    }
    // A black and white print and a bypassed one lay none.
    let mut mono = PipelineSettings::rest(ConversionMode::Monochrome, true);
    mono.print.contrast = 0.3;
    assert_eq!(finishing::print_saturation(&mono), [SaturationCurve::default(), SaturationCurve::default()]);
    let mut bypassed = settled(false, 0.3);
    bypassed.print.bypassed = true;
    assert!(finishing::contrast_curve(&bypassed).is_neutral());
    // Twin: the same Contrast on colour lays one.
    assert!(!finishing::contrast_curve(&settled(false, 0.3)).is_neutral());
}

#[test]
fn contrasts_curve_on_the_gpu_is_its_reference() {
    let Some(mut g) = graph("contrasts_curve_on_the_gpu_is_its_reference") else { return };
    let (w, h) = (64, 48);
    let pixels = noise(w, h, 52, 0.0, 1.0);
    let input = g.upload(w, h, &pixels).unwrap();
    for travel in [-0.3, 0.3] {
        let k = finishing::saturation(&finishing::contrast_curve(&settled(false, travel))).unwrap();
        let out = whole(&mut g, &Image::new(&input).then(stages::saturation(&k)));
        let reference: Vec<V4> = pixels.iter().map(|p| cpu::saturation_curve(*p, &k)).collect();
        let (e, twin) = (error(&out, &reference), error(&out, &pixels));
        println!("Contrast {travel:+}'s curve: GPU {e:.2e} off its reference; twin, the curve not applied, {twin:.2e}");
        assert!(e < TOLERANCE, "{travel}: {e}");
        assert!(twin > 4e-2, "{travel}: {twin}");
    }
}
