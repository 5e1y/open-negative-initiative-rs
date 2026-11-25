//! The halation on a film's layers against its reference: an even exposure kept, a halo that follows its source's light
//! with no step, the same at every scale, a region's the whole's, absent at rest; each verdict with a twin it refuses.

mod common;

use common::{V4, graph, judge, mapped, whole};
use oni_graph::{Graph, Image, Rect, Source, Stage};
use oni_pipeline::stages::{self, Blur};
use oni_pipeline::{Pipeline, Request, film, halation};
use oni_render::cpu::{self, Grid};
use oni_render::{Gaussian, Halation};
use oni_settings::{ConversionMode, PipelineSettings, VirtualStock};

/// The dose at Halation's top.
const TOP: f32 = oni_settings::blocks::Halation::AMOUNT_RANGE.1;
/// A glow 6.4 pixels wide on a scan 1600 long.
const WIDE: f32 = 144.0;

/// A digital photo on `stock` with no grain and no neighbourhood stage, its halation of `amount` and `size`.
fn haloed_on(stock: VirtualStock, amount: f32, size: f32) -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Digital, false).on_stock(stock).per_pixel_only();
    s.grain.amount = 0.0;
    s.bloom.size = 0.0;
    s.halation.amount = amount;
    s.halation.size = size;
    s
}

fn haloed(amount: f32, size: f32) -> PipelineSettings {
    haloed_on(VirtualStock::NEUTRAL, amount, size)
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

/// The dose added on top of the exposure, as a halation that kept no even exposure would lay it.
fn added(e: V4, b: V4, k: &Halation) -> V4 {
    [e[0] + k.dose[0] * b[0], e[1] + k.dose[1] * b[1], e[2] + k.dose[2] * b[2], e[3]]
}

#[test]
fn the_halation_matches_its_reference_and_moves_light_without_adding_any() {
    let Some(mut g) = graph("the_halation_matches_its_reference_and_moves_light_without_adding_any") else { return };
    let extent = Rect::new(17, 9, 61, 43);
    let pixels = common::noise(extent.w, extent.h, 31, 0.0, 3.0);
    let input = g.upload_at(extent, &pixels).unwrap();
    let sigma = 3.2;
    let gaussian = |vertical| Gaussian { sigma, vertical, transparent: false };
    let source = Grid::new(extent.x, extent.y, extent.w as usize, extent.h as usize, pixels.clone());
    let blur = cpu::gaussian(&cpu::gaussian(&source, &gaussian(false)), &gaussian(true)).pixels;
    let laid = |k: &Halation, f: &dyn Fn(V4, V4, &Halation) -> V4| -> Vec<V4> { pixels.iter().zip(&blur).map(|(p, b)| f(*p, *b, k)).collect() };
    // A colour film's three layers, then a silver film's one.
    for k in [Halation::on(0.2, false), Halation::on(0.07, true)] {
        let glow = stages::blurred(Image::new(&input), f64::from(sigma), false);
        let out = whole(&mut g, &Image::new(&input).then_reading(stages::halation(&k), &[glow]));
        let reference = laid(&k, &cpu::halation);
        judge("halation", &out, &reference, &laid(&k, &added));
        // Twin: the other kind of film's layers.
        let other = Halation::on(k.dose[0], k.dose[2] == 0.0);
        judge("halation, the other film's layers", &out, &reference, &laid(&other, &cpu::halation));
    }

    // A lamp well inside its frame: the red and green it glows with leave the lamp, blue never moves.
    let source = lamp(&mut g, 1200, 100.0);
    let s = haloed(TOP, 125.0);
    let scan = Rect::new(0, 0, 1200, 1200);
    let before = whole(&mut g, &Image::new(&source));
    let after = whole(&mut g, &halation::halated(Image::new(&source), &s, scan));
    let sum = |p: &[V4], c: usize| p.iter().map(|v| f64::from(v[c])).sum::<f64>();
    for c in 0..3 {
        let (was, is) = (sum(&before, c), sum(&after, c));
        assert!((is / was - 1.0).abs() < 1e-4, "channel {c}: {was} became {is}");
    }
    assert!(before.iter().zip(&after).all(|(a, b)| a[2] == b[2]), "the blue layer never glows");
    let halo = ring(1200, 102.0, 120.0);
    let (red, green) = (halo.iter().map(|i| after[*i][0] - before[*i][0]).sum::<f32>(), halo.iter().map(|i| after[*i][1] - before[*i][1]).sum::<f32>());
    assert!(red > 0.0 && green > 0.0 && green < 0.5 * red, "a red glow, a little green: {red} {green}");
    // Twin: the dose added on top of the lamp's own light brightens the frame by it.
    let top: Vec<V4> = after.iter().map(|p| [p[0] * (1.0 + TOP), p[1], p[2], p[3]]).collect();
    assert!(sum(&top, 0) / sum(&before, 0) - 1.0 > 1e-2, "the twin's light is not counted");
}

#[test]
fn an_even_exposure_keeps_its_tone_and_hue_under_any_dose() {
    // On the kernel: an even exposure's blur is itself, and every layer stays where its sheet measured it.
    let even = [0.02f32, 0.5, 3.0, 1.0];
    for k in [Halation::on(TOP, false), Halation::on(TOP, true)] {
        let out = cpu::halation(even, even, &k);
        assert!((0..3).all(|c| (out[c] / even[c] - 1.0).abs() < 3e-7), "{out:?}");
        // Twin: the dose on top moves the red layer by the dose, a cast on every flat tone.
        assert!((added(even, even, &k)[0] / even[0] - 1.0 - TOP).abs() < 1e-6);
    }
    // Through the whole chain, on every kind of film: a flat patch prints as with no halation.
    let Some(mut g) = graph("an_even_exposure_keeps_its_tone_and_hue_under_any_dose") else { return };
    let side = 96;
    let flat = g.upload(side, side, &vec![[0.3, 0.2, 0.1, 1.0]; (side * side) as usize]).unwrap();
    let lit = lamp(&mut g, side, 12.0);
    let mut pipeline = Pipeline::new();
    let stocks = [VirtualStock::NEUTRAL, VirtualStock::of("kodak-portra-800"), VirtualStock::of("kodak-ektachrome-e100"), VirtualStock::of("foma-fomapan-100")];
    for stock in stocks {
        let mut render = |source: &Source, amount: f32| {
            let image = pipeline.image(&mut g, source, &haloed_on(stock, amount, 250.0), &Request::default(), &Default::default()).unwrap();
            whole(&mut g, &image)
        };
        let worst = |a: &[V4], b: &[V4]| a.iter().zip(b).map(|(p, q)| (0..3).map(|c| (p[c] - q[c]).abs()).fold(0.0, f32::max)).fold(0.0, f32::max);
        let off = worst(&render(&flat, TOP), &render(&flat, 0.0));
        // Twin: a frame with a lamp in it moves under the same dose.
        let seen = worst(&render(&lit, TOP), &render(&lit, 0.0));
        println!("{stock:?}: a flat patch moves {off:.2e} under the top dose, a lamp's frame {seen:.2e}");
        assert!(off < 1e-4, "{stock:?}: a flat patch moved by {off}");
        assert!(seen > 1e-2, "{stock:?}: the lamp's frame moved by {seen} alone, so the check sees nothing");
    }
}

#[test]
fn a_point_sources_halo_follows_its_light_with_no_step() {
    let Some(mut g) = graph("a_point_sources_halo_follows_its_light_with_no_step") else { return };
    let side = 400;
    let s = haloed(TOP, WIDE * 4.0 / 1.6);
    let scan = Rect::new(0, 0, side, side);
    let halo = ring(side, 9.0, 14.0);
    // The glow a source of each light lays on its surround, over that light: from the shadows to far past white.
    let lights = [0.05f32, 0.3, 0.59, 0.61, 1.2, 4.0, 16.0];
    let shares: Vec<f32> = lights
        .iter()
        .map(|light| {
            let source = lamp_of(&mut g, side, 6.0, *light);
            let after = whole(&mut g, &halation::halated(Image::new(&source), &s, scan));
            halo.iter().map(|i| after[*i][0] - 0.02).sum::<f32>() / halo.len() as f32 / (light - 0.02)
        })
        .collect();
    let spread = |v: &[f32]| (v.iter().copied().fold(f32::MIN, f32::max) - v.iter().copied().fold(f32::MAX, f32::min)) / v.iter().copied().fold(f32::MIN, f32::max);
    println!("the halo over its source's light, from {} to {}: {shares:?}", lights[0], lights[6]);
    assert!(shares[0] > 1e-3, "the source glows: {}", shares[0]);
    assert!(spread(&shares) < 1e-3, "the halo follows the light within {}", spread(&shares));
    // Twin: the light past a threshold alone, none under it and a step at it.
    let former: Vec<f32> = lights.iter().zip(&shares).map(|(light, share)| share * (light - 0.6).max(0.0) / light).collect();
    assert!(spread(&former) > 0.5, "the twin's halo follows the light too, so the check sees nothing");
}

#[test]
fn a_reduced_render_glows_as_the_full_one_reduced() {
    let Some(mut g) = graph("a_reduced_render_glows_as_the_full_one_reduced") else { return };
    let side = 1600;
    let source = lamp(&mut g, side, 160.0);
    let s = haloed(TOP, WIDE);
    let full_scan = Rect::new(0, 0, side, side);
    // The glow alone, the frame with its halation less the frame without: the reduction's own ringing leaves it.
    let plain = whole(&mut g, &Image::new(&source));
    let full: Vec<f32> = whole(&mut g, &halation::halated(Image::new(&source), &s, full_scan)).iter().zip(&plain).map(|(a, b)| a[0] - b[0]).collect();
    for n in [2u32, 4] {
        let reduced = Image::new(&source).lanczos(1.0 / f64::from(n));
        let scan = reduced.extent();
        let small = side / n;
        assert_eq!(scan, Rect::new(0, 0, small, small));
        let means: Vec<f32> = (0..small * small)
            .map(|i| {
                let (bx, by) = (i % small, i / small);
                (0..n * n).map(|j| full[((by * n + j / n) * side + bx * n + j % n) as usize]).sum::<f32>() / (n * n) as f32
            })
            .collect();
        // The halo outside the lamp, clear of its edge.
        let halo = ring(small, 170.0 / n as f32, 200.0 / n as f32);
        let peak = halo.iter().map(|i| means[*i]).fold(0.0, f32::max);
        let unlit = whole(&mut g, &reduced);
        let gap = |out: &[V4]| halo.iter().map(|i| (out[*i][0] - unlit[*i][0] - means[*i]).abs()).fold(0.0, f32::max) / peak;
        let here = whole(&mut g, &halation::halated(reduced.clone(), &s, scan));
        let off = gap(&here);
        println!("at 1/{n}: the glow {off:.2e} of its peak {peak:.3} from the full one's means");
        assert!(off < 0.1, "at 1/{n}: {off:.3} of the glow's peak");
        // Twin: the full resolution's deviation laid on the reduced pixels, a glow n times as wide.
        let wide = stages::blurred(reduced.clone(), halation::deviation(&s, full_scan), false);
        let twin = whole(&mut g, &reduced.then_reading(stages::halation(&halation::kernel(&s)), &[wide]));
        assert!(gap(&twin) > 0.2, "at 1/{n}: the twin passes at {:.3}", gap(&twin));
    }
}

#[test]
fn a_region_glows_as_the_whole_frame_does_there() {
    let Some(mut g) = graph("a_region_glows_as_the_whole_frame_does_there") else { return };
    let side = 400;
    let source = lamp(&mut g, side, 40.0);
    let s = haloed(TOP, WIDE);
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
    let glow = Image::new(&source).then(Marginless(Blur(k(false)))).then(Marginless(Blur(k(true))));
    let twin = Image::new(&source).then_reading(stages::halation(&halation::kernel(&s)), &[glow]);
    let part = g.render(&twin, region, None).unwrap();
    let all = whole(&mut g, &twin);
    let gap = part.iter().zip(cut(&all)).map(|(a, b)| (a[0] - b[0]).abs()).fold(0.0, f32::max);
    assert!(gap > 2e-4, "the twin's region matches at {gap:.2e}, so the check sees nothing");
}

#[test]
fn at_rest_or_measured_the_chain_holds_no_halation() {
    let Some(mut g) = graph("at_rest_or_measured_the_chain_holds_no_halation") else { return };
    let source = lamp(&mut g, 200, 20.0);
    let mut pipeline = Pipeline::new();
    let mut image = |s: &PipelineSettings, measuring: bool| pipeline.image(&mut g, &source, s, &Request { measuring, ..Request::default() }, &Default::default()).unwrap();
    for stock in [VirtualStock::NEUTRAL, VirtualStock::of("kodak-portra-800")] {
        let rest = haloed_on(stock, 0.0, 125.0);
        let wide = haloed_on(stock, 0.0, 240.0);
        let (a, b) = (image(&rest, false), image(&wide, false));
        assert_eq!((a.key(), a.stages()), (b.key(), b.stages()), "{stock:?}: a size at amount 0 lays no stage");
        let faint = haloed_on(stock, 0.001, 125.0);
        assert_ne!(image(&faint, false).key(), a.key(), "{stock:?}, twin: the faintest halation is a stage");
        assert_eq!(image(&faint, true).key(), image(&rest, true).key(), "{stock:?}: a measure reads no halation");
    }
    // A film mode renders no halation its panel hides, its value kept; the Digital one above is its twin.
    let negative = PipelineSettings::rest(ConversionMode::Negative, false);
    let held = PipelineSettings { halation: haloed(0.01, 125.0).halation, ..negative.clone() };
    assert_eq!(image(&held, false).key(), image(&negative, false).key(), "a negative lays no halation");
}

#[test]
fn on_a_sheets_film_the_halation_lies_on_its_layers_exposures_before_their_curve() {
    let Some(mut g) = graph("on_a_sheets_film_the_halation_lies_on_its_layers_exposures_before_their_curve") else { return };
    let side = 240u32;
    let c = side as f32 / 2.0;
    // A warm lamp on a cold ground: the film's layers take neither as the scene's channels hold it.
    let pixels: Vec<V4> = (0..side * side)
        .map(|i| {
            let (x, y) = ((i % side) as f32 + 0.5 - c, (i / side) as f32 + 0.5 - c);
            if x * x + y * y < 400.0 { [4.0, 3.0, 1.0, 1.0] } else { [0.02, 0.05, 0.1, 1.0] }
        })
        .collect();
    let source = g.upload(side, side, &pixels).unwrap();
    let scan = Rect::new(0, 0, side, side);
    let stock_key = VirtualStock::of("kodak-portra-800");
    let s = haloed_on(stock_key, TOP, 250.0);
    let filmed = Pipeline::new().filmed(&mut g, Image::new(&source), &s, Some(scan)).unwrap();
    let out = whole(&mut g, &filmed);
    let stock = film::stock(stock_key);
    let (k, dose) = (stock.kernel_at(0.0), halation::kernel(&s));
    let sigma = halation::deviation(&s, scan) as f32;
    let blurred = |of: &[V4]| {
        let gaussian = |vertical| Gaussian { sigma, vertical, transparent: false };
        cpu::gaussian(&cpu::gaussian(&Grid::new(0, 0, side as usize, side as usize, of.to_vec()), &gaussian(false)), &gaussian(true)).pixels
    };
    let glowing = |of: &[V4]| -> Vec<V4> { of.iter().zip(&blurred(of)).map(|(e, b)| cpu::halation(*e, *b, &dose)).collect() };
    let exposed = mapped(&pixels, |p| cpu::virtual_exposure(p, &k, &stock.table));
    let reference = mapped(&glowing(&exposed), |e| stock.scanned(cpu::virtual_development(e, &k, &stock.table)));
    // Twin: the glow laid on the scene's own channels, before the film reads them.
    let on_scene = mapped(&glowing(&pixels), |p| stock.scanned(cpu::virtual_negative(p, &k, &stock.table)));
    judge("halation on the layers", &out, &reference, &on_scene);
    // Twin: the glow laid on the developed negative, past its curve.
    let on_negative = mapped(&glowing(&mapped(&exposed, |e| cpu::virtual_development(e, &k, &stock.table))), |n| stock.scanned(n));
    judge("halation before the curve", &out, &reference, &on_negative);
}
