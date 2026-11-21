//! The halation against its reference, its light moved and never added, the same at every scale, a
//! region's the whole's, absent at rest and laid under the print; each verdict with a twin it refuses.

mod common;

use common::{V4, graph, judge, noise, whole};
use oni_graph::{Graph, Image, Rect, Source, Stage};
use oni_pipeline::stages::{self, Before, Blur, BlurOf};
use oni_pipeline::{Pipeline, Request, halation};
use oni_render::cpu::{self, Grid};
use oni_render::{Gaussian, HALATION_THRESHOLD, Halation};
use oni_settings::{ConversionMode, PipelineSettings};

fn haloed(amount: f32, size: f32) -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Digital, false).per_pixel_only();
    s.halation.amount = amount;
    s.halation.size = size;
    s
}

/// A dark frame `side` square with a light source of `radius` at its centre.
fn lamp(g: &mut Graph, side: u32, radius: f32) -> Source {
    lamp_of(g, side, radius, 4.0)
}

/// `lamp` with its source of `light`.
fn lamp_of(g: &mut Graph, side: u32, radius: f32, light: f32) -> Source {
    let c = side as f32 / 2.0;
    let pixels: Vec<V4> = (0..side * side)
        .map(|i| {
            let (x, y) = ((i % side) as f32 + 0.5 - c, (i / side) as f32 + 0.5 - c);
            if x * x + y * y < radius * radius { [light, light, light, 1.0] } else { [0.02, 0.02, 0.02, 1.0] }
        })
        .collect();
    g.upload(side, side, &pixels).unwrap()
}

/// The pixels of a `side`-square render between `inner` and `outer` from its centre, by index.
fn ring(side: u32, inner: f32, outer: f32) -> Vec<usize> {
    let c = side as f32 / 2.0;
    (0..(side * side) as usize)
        .filter(|i| {
            let (x, y) = ((*i as u32 % side) as f32 + 0.5 - c, (*i as u32 / side) as f32 + 0.5 - c);
            let r = (x * x + y * y).sqrt();
            r >= inner && r < outer
        })
        .collect()
}

/// A stage that reads no margin around its output, as a blur that forgot its reach would.
struct Marginless<S: Stage>(S);

impl<S: Stage> Stage for Marginless<S> {
    fn name(&self) -> &'static str {
        "marginless"
    }
    fn shader(&self) -> &'static str {
        self.0.shader()
    }
    fn params(&self) -> Vec<u8> {
        self.0.params()
    }
}

#[test]
fn the_halation_matches_its_reference_and_moves_light_without_adding_any() {
    let Some(mut g) = graph("the_halation_matches_its_reference_and_moves_light_without_adding_any") else { return };
    let extent = Rect::new(17, 9, 61, 43);
    // Shadows to light sources: both sides of the threshold on every channel.
    let pixels = noise(extent.w, extent.h, 31, 0.0, 3.0);
    let input = g.upload_at(extent, &pixels).unwrap();
    let sigma = 3.2;
    let gaussian = |vertical| Gaussian { sigma, vertical, transparent: false };
    let blurred = |pre: &dyn Fn(V4) -> V4| {
        let source = Grid::new(extent.x, extent.y, extent.w as usize, extent.h as usize, pixels.iter().map(|p| pre(*p)).collect());
        cpu::gaussian(&cpu::gaussian(&source, &gaussian(false)), &gaussian(true)).pixels
    };
    let glowing = |k: &Halation, glow: &[V4]| -> Vec<V4> { glow.iter().zip(&pixels).map(|(b, p)| cpu::halation(*p, *b, k)).collect() };
    for threshold in [HALATION_THRESHOLD, 0.3] {
        let k = Halation { amount: 0.7, threshold };
        let glow = stages::blurred_of(Image::new(&input), f64::from(sigma), Before::Halation(threshold));
        let out = whole(&mut g, &Image::new(&input).then_reading(stages::halation(&k), &[glow]));
        let reference = glowing(&k, &blurred(&|p| cpu::halation_source(p, threshold)));
        // Twin: the glow of the whole picture, not of its light past the threshold.
        judge(&format!("halation past {threshold}"), &out, &reference, &glowing(&k, &blurred(&|p| p)));
        if threshold != HALATION_THRESHOLD {
            // Twin: the rest's threshold, as a blur or a pass deaf to the sensitivity would read it.
            let deaf = Halation { threshold: HALATION_THRESHOLD, ..k };
            judge(&format!("halation past {threshold}, its rest"), &out, &reference, &glowing(&deaf, &blurred(&|p| cpu::halation_source(p, HALATION_THRESHOLD))));
        }
    }

    // A lamp well inside its frame: the red and green it glows with leave the lamp, blue never moves.
    let source = lamp(&mut g, 1200, 100.0);
    let s = haloed(1.0, 1.0);
    let scan = Rect::new(0, 0, 1200, 1200);
    let before = whole(&mut g, &Image::new(&source));
    let after = whole(&mut g, &halation::halated(Image::new(&source), &s, scan));
    let sum = |p: &[V4], c: usize| p.iter().map(|v| f64::from(v[c])).sum::<f64>();
    for c in 0..3 {
        let (was, is) = (sum(&before, c), sum(&after, c));
        assert!((is / was - 1.0).abs() < 1e-4, "channel {c}: {was} became {is}");
    }
    assert!(before.iter().zip(&after).all(|(a, b)| a[2] == b[2]), "blue never glows");
    let halo = ring(1200, 102.0, 120.0);
    let (red, green) = (halo.iter().map(|i| after[*i][0] - before[*i][0]).sum::<f32>(), halo.iter().map(|i| after[*i][1] - before[*i][1]).sum::<f32>());
    assert!(red > 0.0 && green > 0.0 && green < 0.5 * red, "a red glow, a little green: {red} {green}");
    // Twin: the glow added on top of the lamp's own light brightens the frame.
    let added: Vec<V4> = before.iter().zip(&after).map(|(a, b)| [b[0] + cpu::halation_source(*a, HALATION_THRESHOLD)[0], b[1], b[2], b[3]]).collect();
    assert!(sum(&added, 0) / sum(&before, 0) - 1.0 > 1e-2, "the twin's light is not counted");
}

#[test]
fn a_reduced_render_glows_as_the_full_one_reduced() {
    let Some(mut g) = graph("a_reduced_render_glows_as_the_full_one_reduced") else { return };
    let side = 1600;
    let source = lamp(&mut g, side, 160.0);
    let s = haloed(1.0, 1.0);
    let full_scan = Rect::new(0, 0, side, side);
    let full = whole(&mut g, &halation::halated(Image::new(&source), &s, full_scan));
    for n in [2u32, 4] {
        let reduced = Image::new(&source).lanczos(1.0 / f64::from(n));
        let scan = reduced.extent();
        let small = side / n;
        assert_eq!(scan, Rect::new(0, 0, small, small));
        let means: Vec<f32> = (0..small * small)
            .map(|i| {
                let (bx, by) = (i % small, i / small);
                (0..n * n).map(|j| full[((by * n + j / n) * side + bx * n + j % n) as usize][0]).sum::<f32>() / (n * n) as f32
            })
            .collect();
        // The halo outside the lamp, clear of the reduction's ringing at its edge.
        let halo = ring(small, 170.0 / n as f32, 200.0 / n as f32);
        let peak = halo.iter().map(|i| means[*i] - 0.02).fold(0.0, f32::max);
        let gap = |out: &[V4]| halo.iter().map(|i| (out[*i][0] - means[*i]).abs()).fold(0.0, f32::max) / peak;
        let here = whole(&mut g, &halation::halated(reduced.clone(), &s, scan));
        let off = gap(&here);
        println!("at 1/{n}: the glow {off:.2e} of its peak {peak:.3} from the full one's means");
        assert!(off < 0.08, "at 1/{n}: {off:.3} of the glow's peak");
        // Twin: the full resolution's deviation laid on the reduced pixels, a glow n times as wide.
        let wide = stages::blurred_of(reduced.clone(), halation::deviation(&s, full_scan), Before::Halation(HALATION_THRESHOLD));
        let twin = whole(&mut g, &reduced.then_reading(stages::halation(&Halation { amount: 1.0, threshold: HALATION_THRESHOLD }), &[wide]));
        assert!(gap(&twin) > 0.2, "at 1/{n}: the twin passes at {:.3}", gap(&twin));
    }
}

#[test]
fn a_region_glows_as_the_whole_frame_does_there() {
    let Some(mut g) = graph("a_region_glows_as_the_whole_frame_does_there") else { return };
    let side = 400;
    let source = lamp(&mut g, side, 40.0);
    let s = haloed(1.0, 1.0);
    let sigma = halation::deviation(&s, Rect::new(0, 0, side, side));
    // A region beside the lamp, the lamp outside it but inside the blur's reach, its top row in the glow.
    let region = Rect::new(243, 196, 60, 40);
    assert!(f64::from(region.x) - 240.0 < 3.5 * sigma, "the lamp lies within the glow's reach: {sigma}");
    let cut = |all: &[V4]| -> Vec<V4> { (region.y..region.bottom()).flat_map(|y| (region.x..region.right()).map(move |x| all[(y as u32 * side + x as u32) as usize])).collect() };
    let image = halation::halated(Image::new(&source), &s, Rect::new(0, 0, side, side));
    let (all, part) = (whole(&mut g, &image), g.render(&image, region, None).unwrap());
    assert_eq!(part, cut(&all), "a region's glow is the whole frame's");
    assert!(part.iter().any(|p| p[0] > 0.03), "the region holds glow");
    // Twin: a blur reading no margin misses the rows above; in a graph of its own, region first, so no
    // texture another render filled stands in for the margin.
    let Some(mut g) = graph("a_region_glows_as_the_whole_frame_does_there, its twin") else { return };
    let source = lamp(&mut g, side, 40.0);
    let k = |vertical| Gaussian { sigma: sigma as f32, vertical, transparent: false };
    let glow = Image::new(&source).then(Marginless(BlurOf(k(false), Before::Halation(HALATION_THRESHOLD)))).then(Marginless(Blur(k(true))));
    let twin = Image::new(&source).then_reading(stages::halation(&Halation { amount: 1.0, threshold: HALATION_THRESHOLD }), &[glow]);
    let part = g.render(&twin, region, None).unwrap();
    let all = whole(&mut g, &twin);
    let gap = part.iter().zip(cut(&all)).map(|(a, b)| (a[0] - b[0]).abs()).fold(0.0, f32::max);
    assert!(gap > 1e-3, "the twin's region matches at {gap:.2e}, so the check sees nothing");
}

#[test]
fn at_rest_or_measured_the_chain_holds_no_halation() {
    let Some(mut g) = graph("at_rest_or_measured_the_chain_holds_no_halation") else { return };
    let source = lamp(&mut g, 200, 20.0);
    let mut pipeline = Pipeline::new();
    let mut image = |s: &PipelineSettings, measuring: bool| pipeline.image(&mut g, &source, s, &Request { measuring, ..Request::default() }, &Default::default()).unwrap();
    let rest = PipelineSettings::rest(ConversionMode::Digital, false);
    let mut wide = rest.clone();
    wide.halation.size = 0.9;
    let (a, b) = (image(&rest, false), image(&wide, false));
    assert_eq!((a.key(), a.stages()), (b.key(), b.stages()), "a size at amount 0 lays no stage");
    let mut sensitive = rest.clone();
    sensitive.halation.sensitivity = 1.5;
    let c = image(&sensitive, false);
    assert_eq!((c.key(), c.stages()), (a.key(), a.stages()), "a sensitivity at amount 0 lays no stage");
    let mut faint = rest.clone();
    faint.halation.amount = 0.01;
    assert_ne!(image(&faint, false).key(), a.key(), "twin: the faintest halation is a stage");
    let keener = PipelineSettings { halation: oni_settings::blocks::Halation { sensitivity: 0.01, ..faint.halation }, ..faint.clone() };
    assert_ne!(image(&keener, false).key(), image(&faint, false).key(), "a sensitivity moves the halation's stages");
    assert_eq!(image(&faint, true).key(), image(&rest, true).key(), "a measure reads no halation");
    // A film mode renders no halation its panel hides, its value kept; the Digital one above is its twin.
    let negative = PipelineSettings::rest(ConversionMode::Negative, false);
    let held = PipelineSettings { halation: faint.halation, ..negative.clone() };
    assert_eq!(image(&held, false).key(), image(&negative, false).key(), "a negative lays no halation");
}

#[test]
fn the_halation_lies_under_the_print_so_its_exposure_reaches_the_glow() {
    let Some(mut g) = graph("the_halation_lies_under_the_print_so_its_exposure_reaches_the_glow") else { return };
    let side = 1200;
    // A lamp the film's shoulder still prints past white at the print's rest.
    let source = lamp_of(&mut g, side, 100.0, 16.0);
    let mut pipeline = Pipeline::new();
    let s = haloed(1.0, 1.0);
    let halo = ring(side, 102.0, 120.0);
    // The glow's red in the print, at a print exposure: the lamp's surround with it less without it.
    let mut glow = |after: bool, exposure: f32| -> f32 {
        let mut render = |amount: f32| {
            let mut t = s.clone();
            t.print.exposure = exposure;
            t.halation.amount = if after { 0.0 } else { amount };
            let image = pipeline.image(&mut g, &source, &t, &Request::default(), &Default::default()).unwrap();
            let image = if after && amount > 0.0 { halation::halated(image, &s, Rect::new(0, 0, side, side)) } else { image };
            whole(&mut g, &image)
        };
        let (with, without) = (render(1.0), render(0.0));
        halo.iter().map(|i| with[*i][0] - without[*i][0]).sum::<f32>() / halo.len() as f32
    };
    // From the print's rest, where the lamp prints past white, two stops up.
    let rest = oni_settings::tone::Print::resting_exposure(s.mode);
    let under = glow(false, rest + 2.0) / glow(false, rest);
    println!("two stops of print exposure raise the glow {under:.3} times");
    assert!(under > 1.1, "the print's exposure leaves the glow at {under:.3}");
    // Twin: a halation laid on the print, its lamp clipped at white whatever the exposure.
    let over = glow(true, rest + 2.0) / glow(true, rest);
    println!("the twin's glow rises {over:.3} times");
    assert!(over < 1.1, "the twin's glow rises {over:.3} times, so the check sees nothing");
}

#[test]
fn the_sensitivity_at_rest_is_the_threshold_the_halation_always_had() {
    let rest = PipelineSettings::rest(ConversionMode::Digital, false);
    assert_eq!(halation::threshold(&rest).to_bits(), HALATION_THRESHOLD.to_bits(), "a frame at rest glows as before");
    let mut keen = rest.clone();
    keen.halation.sensitivity = 1.0;
    assert_eq!(halation::threshold(&keen), HALATION_THRESHOLD / 2.0, "a stop more sensitive halves the threshold");
    // Twin: the faintest sensitivity is another threshold.
    keen.halation.sensitivity = 0.01;
    assert_ne!(halation::threshold(&keen).to_bits(), HALATION_THRESHOLD.to_bits());
}
