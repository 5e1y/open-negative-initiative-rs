//! The halation on a film's layers against its reference: its glow fed by each layer's light weighted by how near the
//! saturation it stands, so a mid tone barely moves, a halo that follows its source's light with no step, Portra 800's
//! glow past a lamp, the same at every scale, a region's the whole's, absent at rest; each verdict with a twin.

mod common;

use common::{V4, graph, judge, mapped, whole};
use oni_graph::{Graph, Image, Rect, Source, Stage};
use oni_pipeline::stages::{self, Blur};
use oni_pipeline::{Pipeline, Request, film, halation};
use oni_render::cpu::{self, Grid};
use oni_render::{Gaussian, Halation};
use oni_settings::{ConversionMode, PipelineSettings, VirtualStock};

/// The dose at Halation's top, and the one photographers' medium glow takes.
const TOP: f32 = oni_settings::blocks::Halation::AMOUNT_RANGE.1;
const MEDIUM: f32 = 0.31;
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

/// `pixels`, `w` by `h` at the origin, blurred by a Gaussian of `sigma` as the graph blurs them.
fn blurred(pixels: &[V4], w: usize, h: usize, sigma: f32) -> Vec<V4> {
    let gaussian = |vertical| Gaussian { sigma, vertical, transparent: false };
    cpu::gaussian(&cpu::gaussian(&Grid::new(0, 0, w, h, pixels.to_vec()), &gaussian(false)), &gaussian(true)).pixels
}

/// `pixels` with the halation `k` of deviation `sigma`, as its reference lays it.
fn glowing(pixels: &[V4], w: usize, h: usize, sigma: f32, k: &Halation) -> Vec<V4> {
    let glow = blurred(&mapped(pixels, |e| cpu::halation_source(e, k)), w, h, sigma);
    pixels.iter().zip(&glow).map(|(e, b)| cpu::halation(*e, *b, k)).collect()
}

#[test]
fn the_halation_matches_its_reference_and_adds_the_glow_of_the_brightest_light() {
    let Some(mut g) = graph("the_halation_matches_its_reference_and_adds_the_glow_of_the_brightest_light") else { return };
    let extent = Rect::new(0, 0, 61, 43);
    let pixels = common::noise(extent.w, extent.h, 31, 0.0, 3.0);
    let input = g.upload_at(extent, &pixels).unwrap();
    let sigma = 3.2;
    let (w, h) = (extent.w as usize, extent.h as usize);
    // A colour film's three layers, then a silver film's one, each layer its own saturation.
    for k in [Halation::on(0.4, false, [1.2, 1.4, 0.9]), Halation::on(0.07, true, [1.39; 3])] {
        let glow = stages::blurred(Image::new(&input).then(stages::halation_source(&k)), f64::from(sigma), false);
        let out = whole(&mut g, &Image::new(&input).then_reading(stages::halation(&k), &[glow]));
        let reference = glowing(&pixels, w, h, sigma, &k);
        // Twin: every light feeding the glow whole, as a linear halation would.
        let linear = blurred(&pixels, w, h, sigma);
        judge("halation", &out, &reference, &pixels.iter().zip(&linear).map(|(e, b)| cpu::halation(*e, *b, &k)).collect::<Vec<_>>());
        // Twin: the other film's layers.
        let other = Halation::on(k.dose[0], k.dose[2] == 0.0, k.saturation);
        judge("halation, the other film's layers", &out, &reference, &glowing(&pixels, w, h, sigma, &other));
    }

    // A lamp well inside its frame: red and green glow by their dose of its light, blue never moves.
    let source = lamp(&mut g, 1200, 100.0);
    let s = haloed(MEDIUM, 125.0);
    let scan = Rect::new(0, 0, 1200, 1200);
    let before = whole(&mut g, &Image::new(&source));
    let after = whole(&mut g, &halation::halated(Image::new(&source), &s, scan));
    let k = halation::kernel(&s);
    let sum = |p: &[V4], c: usize| p.iter().map(|v| f64::from(v[c])).sum::<f64>();
    let fed = |c: usize| before.iter().map(|v| f64::from(cpu::halation_source(*v, &k)[c])).sum::<f64>();
    for c in 0..2 {
        let (was, is) = (sum(&before, c), sum(&after, c));
        assert!(((is - was) / (f64::from(k.dose[c]) * fed(c)) - 1.0).abs() < 1e-3, "channel {c}: {was} became {is}");
    }
    assert!(before.iter().zip(&after).all(|(a, b)| a[2] == b[2]), "the blue layer never glows");
    let halo = ring(1200, 102.0, 120.0);
    let (red, green) = (halo.iter().map(|i| after[*i][0] - before[*i][0]).sum::<f32>(), halo.iter().map(|i| after[*i][1] - before[*i][1]).sum::<f32>());
    assert!(red > 0.0 && green > 0.0 && green < 0.5 * red, "a red glow, a little green: {red} {green}");
    // Twin: a glow taken from the light it spreads, as a halation keeping the frame's light would lay it, adds none.
    assert!((sum(&after, 0) / sum(&before, 0) - 1.0) > 1e-2, "the glow adds no light");
}

#[test]
fn a_mid_tone_barely_moves_and_a_clipped_light_glows_its_whole_dose() {
    let Some(mut g) = graph("a_mid_tone_barely_moves_and_a_clipped_light_glows_its_whole_dose") else { return };
    let side = 96u32;
    let white = film::saturation();
    // The scene's own grey and the sensor's saturation, flat over the frame: the neutral film's layers.
    let raised = |g: &mut Graph, light: f32| {
        let source = g.upload(side, side, &vec![[light, light, light, 1.0]; (side * side) as usize]).unwrap();
        let out = whole(g, &halation::halated(Image::new(&source), &haloed(TOP, 200.0), Rect::new(0, 0, side, side)));
        out[(side * side / 2 + side / 2) as usize][0] / light - 1.0
    };
    let (grey, clipped) = (raised(&mut g, 0.184), raised(&mut g, white));
    println!("under the top dose a mid grey's red rises {:.4}, a clipped light's {clipped:.4}", grey);
    assert!((grey - TOP * (0.184 / white).powi(2)).abs() < 2e-4, "a mid grey rises by the dose over its share of the saturation squared: {grey}");
    assert!(grey < 0.02, "a mid tone barely moves: {grey}");
    assert!((clipped - TOP).abs() < 1e-4, "a clipped light glows its whole dose: {clipped}");
    // Twin: a glow fed by every light whole moves the grey as much as the lamp.
    assert!(clipped > 50.0 * grey);
}

#[test]
fn a_point_sources_halo_follows_its_light_with_no_step() {
    let Some(mut g) = graph("a_point_sources_halo_follows_its_light_with_no_step") else { return };
    let side = 400;
    let s = haloed(TOP, WIDE * 4.0 / 1.6);
    let scan = Rect::new(0, 0, side, side);
    let halo = ring(side, 9.0, 14.0);
    let white = film::saturation();
    // The glow a source of each light lays on its surround, over the light it feeds: from the shadows to past white.
    let lights = [0.05f32, 0.3, 0.59, 0.61, 1.2, 4.0, 16.0];
    let shares: Vec<f32> = lights
        .iter()
        .map(|light| {
            let source = lamp_of(&mut g, side, 6.0, *light);
            let (before, after) = (whole(&mut g, &Image::new(&source)), whole(&mut g, &halation::halated(Image::new(&source), &s, scan)));
            // The dark ground's own glow, even over the frame, laid aside: the lamp feeds its light over the ground's.
            let feeds = |v: f32| v * (v / white).min(1.0).powi(2);
            let (ground, fed) = (TOP * feeds(0.02), feeds(*light) - feeds(0.02));
            (halo.iter().map(|i| after[*i][0] - before[*i][0]).sum::<f32>() / halo.len() as f32 - ground) / fed
        })
        .collect();
    let spread = |v: &[f32]| (v.iter().copied().fold(f32::MIN, f32::max) - v.iter().copied().fold(f32::MAX, f32::min)) / v.iter().copied().fold(f32::MIN, f32::max);
    println!("the halo over the light its source feeds, from {} to {}: {shares:?}", lights[0], lights[6]);
    assert!(shares[0] > 1e-3, "the source glows: {}", shares[0]);
    assert!(spread(&shares) < 5e-3, "the halo follows the light within {}", spread(&shares));
    // Twin: a glow fed by the light past a threshold alone, none under it and a step at it.
    let former: Vec<f32> = lights.iter().zip(&shares).map(|(light, share)| share * (light - 0.6).max(0.0) / light).collect();
    assert!(spread(&former) > 0.5, "the twin's halo follows the light too, so the check sees nothing");
}

#[test]
fn portra_800s_glow_past_a_clipped_lamp_falls_as_measured() {
    let Some(mut g) = graph("portra_800s_glow_past_a_clipped_lamp_falls_as_measured") else { return };
    // A frame 3600 pixels long, ten micrometres a pixel; a clipped lamp over its left half, a dark ground past it.
    let (w, h) = (3600u32, 32u32);
    let white = film::saturation();
    let grey = 0.184;
    let pixels: Vec<V4> = (0..w * h).map(|i| if i % w < w / 2 { [white, white, white, 1.0] } else { [0.04 * grey, 0.04 * grey, 0.04 * grey, 1.0] }).collect();
    let source = g.upload(w, h, &pixels).unwrap();
    let excess = |g: &mut Graph, size: f32| -> Vec<f32> {
        let out = whole(g, &halation::halated(Image::new(&source), &haloed(MEDIUM, size), Rect::new(0, 0, w, h)));
        let row = (h / 2 * w) as usize;
        [15u32, 30, 45].iter().map(|d| (out[row + (w / 2 + d) as usize][0] - pixels[row + (w / 2 + d) as usize][0]) / grey).collect()
    };
    let at = excess(&mut g, 200.0);
    println!("the red past a clipped lamp, in shares of a mid grey's exposure, at 150, 300 and 450 µm: {at:?}");
    for (got, want) in at.iter().zip([0.51, 0.15, 0.02]) {
        assert!((got - want).abs() < 0.03, "{got} against {want}");
    }
    // Twin: the former width, 125 µm, falls far faster.
    assert!((excess(&mut g, 125.0)[0] - 0.51).abs() > 0.1);
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
        let k = halation::kernel(&s);
        let wide = stages::blurred(reduced.clone().then(stages::halation_source(&k)), halation::deviation(&s, full_scan), false);
        let twin = whole(&mut g, &reduced.then_reading(stages::halation(&k), &[wide]));
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
    let k = halation::kernel(&s);
    let gaussian = |vertical| Gaussian { sigma: sigma as f32, vertical, transparent: false };
    let glow = Image::new(&source).then(stages::halation_source(&k)).then(Marginless(Blur(gaussian(false)))).then(Marginless(Blur(gaussian(true))));
    let twin = Image::new(&source).then_reading(stages::halation(&k), &[glow]);
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
        let rest = haloed_on(stock, 0.0, 200.0);
        let wide = haloed_on(stock, 0.0, 380.0);
        let (a, b) = (image(&rest, false), image(&wide, false));
        assert_eq!((a.key(), a.stages()), (b.key(), b.stages()), "{stock:?}: a size at amount 0 lays no stage");
        let faint = haloed_on(stock, 0.001, 200.0);
        assert_ne!(image(&faint, false).key(), a.key(), "{stock:?}, twin: the faintest halation is a stage");
        assert_eq!(image(&faint, true).key(), image(&rest, true).key(), "{stock:?}: a measure reads no halation");
    }
    // A film mode renders no halation its panel hides, its value kept; the Digital one above is its twin.
    let negative = PipelineSettings::rest(ConversionMode::Negative, false);
    let held = PipelineSettings { halation: haloed(0.01, 200.0).halation, ..negative.clone() };
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
    let n = side as usize;
    let exposed = mapped(&pixels, |p| cpu::virtual_exposure(p, &k, &stock.table));
    let reference = mapped(&glowing(&exposed, n, n, sigma, &dose), |e| stock.scanned(cpu::virtual_development(e, &k, &stock.table)));
    // Twin: the glow laid on the scene's own channels, before the film reads them.
    let on_scene = mapped(&glowing(&pixels, n, n, sigma, &dose), |p| stock.scanned(cpu::virtual_negative(p, &k, &stock.table)));
    judge("halation on the layers", &out, &reference, &on_scene);
    // Twin: the glow laid on the developed negative, past its curve.
    let on_negative = mapped(&glowing(&mapped(&exposed, |e| cpu::virtual_development(e, &k, &stock.table)), n, n, sigma, &dose), |p| stock.scanned(p));
    judge("halation before the curve", &out, &reference, &on_negative);
}
