//! Every stage the pipeline adds to the render graph, on the GPU, against oni-render's scalar
//! reference; each verdict has a twin, a reference deliberately wrong, which the check must refuse.

use oni_graph::{Graph, Image, Rect, Source, Stage};
use oni_pipeline::stages::{self, Blur, DetailColumns, DetailRows, Oriented, Patched, Resample, Stroke};
use oni_pipeline::{Pipeline, Request, conversion, finishing, geometry, oni_look, scanner};
use oni_render::cpu::{self, Grid};
use oni_render::{
    ColorCube, Detail, ExposureAdjust, FlatField, Gaussian, Orient, ScannerEncode, ScannerOutput, Texture, TextureCoeff,
    Vignette,
};
use oni_settings::cg::Point;
use oni_settings::curves::{Curve, CurveSet};
use oni_settings::frame::{FlatFieldMask, Geometry};
use oni_settings::modes::{Face, ScannerMachine};
use oni_settings::scanner::ScannerClicks;
use oni_settings::spectrogram::Spectrogram;
use oni_settings::{ConversionMode, PipelineSettings};

type V4 = [f32; 4];

/// Largest error accepted, as |gpu - cpu| / (1 + |cpu|): lavapipe's `exp2` and `log2` sit near 1e-6.
const TOLERANCE: f32 = 1e-4;

fn graph(check: &str) -> Option<Graph> {
    Graph::new().map_err(|e| println!("SKIPPED {check}: {e}")).ok()
}

/// Deterministic noise in `low..high`, alpha one.
fn noise(width: u32, height: u32, seed: u64, low: f32, high: f32) -> Vec<V4> {
    let mut state = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
    let mut next = move || {
        state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        low + ((state >> 40) as f32 / (1u64 << 24) as f32) * (high - low)
    };
    (0..width * height).map(|_| [next(), next(), next(), 1.0]).collect()
}

/// Largest relative error; a NaN on one side only is infinite.
fn error(a: &[V4], b: &[V4]) -> f32 {
    assert_eq!(a.len(), b.len(), "buffers of different sizes");
    assert!(!a.is_empty(), "nothing to compare");
    a.iter()
        .zip(b)
        .flat_map(|(p, q)| {
            (0..4).map(move |c| {
                if p[c] == q[c] {
                    return 0.0;
                }
                let e = (p[c] - q[c]).abs() / (1.0 + q[c].abs());
                if e.is_nan() { f32::INFINITY } else { e }
            })
        })
        .fold(0.0, f32::max)
}

/// The GPU agrees with the reference and refuses the twin.
fn judge(name: &str, gpu: &[V4], reference: &[V4], twin: &[V4]) {
    let (e, t) = (error(gpu, reference), error(gpu, twin));
    println!("{name}: error {e:.2e}, twin {t:.2e}");
    assert!(e < TOLERANCE, "{name}: the GPU is {e:.2e} off its reference");
    assert!(t > TOLERANCE, "{name}: the twin passes at {t:.2e}, so the check sees nothing");
}

fn whole(g: &mut Graph, image: &Image) -> Vec<V4> {
    g.render(image, image.extent(), None).expect("the stage renders")
}

fn mapped(pixels: &[V4], f: impl Fn(V4) -> V4) -> Vec<V4> {
    pixels.iter().map(|&p| f(p)).collect()
}

/// A per-pixel stage on `input` against `f`, refusing `twin`.
fn per_pixel(g: &mut Graph, name: &str, input: &Source, pixels: &[V4], stage: impl Stage + 'static, f: impl Fn(V4) -> V4, twin: impl Fn(V4) -> V4) {
    let out = whole(g, &Image::new(input).then(stage));
    judge(name, &out, &mapped(pixels, f), &mapped(pixels, twin));
}

fn swapped(p: V4) -> V4 {
    [p[2], p[1], p[0], p[3]]
}

fn negative() -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.print.exposure = 0.3;
    s.print.contrast = 0.2;
    s.density = 0.3;
    s
}

#[test]
fn per_pixel_kernels_match_their_references() {
    let Some(mut g) = graph("per_pixel_kernels_match_their_references") else { return };
    let (w, h) = (37, 23);
    let pixels = noise(w, h, 1, -0.2, 1.6);
    let input = g.upload(w, h, &pixels).unwrap();
    // Moments of a neighbourhood, a mean and a mean square above its square.
    let moments: Vec<V4> = noise(w, h, 14, 0.0, 1.0).iter().map(|p| [p[0], p[0] * p[0] + 0.05 * p[1], 0.0, 1.0]).collect();
    let transmitted = noise(w, h, 2, 0.002, 0.9);
    let film = g.upload(w, h, &transmitted).unwrap();

    let k = conversion::stage47(&negative());
    let shifted = oni_render::Stage47 { gain: [k.gain[0], k.gain[1] * 1.01, k.gain[2]], ..k };
    per_pixel(&mut g, "stage47", &film, &transmitted, stages::stage47(&k), |p| cpu::stage47(p, &k), |p| cpu::stage47(p, &shifted));
    per_pixel(&mut g, "stage47, channels swapped", &film, &transmitted, stages::stage47(&k), |p| cpu::stage47(p, &k), |p| swapped(cpu::stage47(p, &k)));
    let mono = conversion::stage47(&PipelineSettings::rest(ConversionMode::Monochrome, true));
    let positive = conversion::stage47(&PipelineSettings::rest(ConversionMode::Positive, true));
    per_pixel(&mut g, "stage47, monochrome", &film, &transmitted, stages::stage47(&mono), |p| cpu::stage47(p, &mono), |p| cpu::stage47(p, &positive));

    let ev = |ev| ExposureAdjust { ev };
    per_pixel(&mut g, "exposure", &input, &pixels, stages::exposure(0.7), |p| cpu::exposure_adjust(p, &ev(0.7)), |p| cpu::exposure_adjust(p, &ev(0.72)));
    per_pixel(&mut g, "output clip", &input, &pixels, stages::output_clip(), cpu::output_clip, |p| p);

    let sauce = finishing::saturation(&finishing::sauce(1.0)).unwrap();
    let less = finishing::saturation(&finishing::sauce(0.9)).unwrap();
    per_pixel(&mut g, "saturation", &input, &pixels, stages::saturation(&sauce), |p| cpu::saturation_curve(p, &sauce), |p| cpu::saturation_curve(p, &less));

    let mut mixer = Spectrogram::default();
    mixer.saturation[5] = 0.5;
    mixer.hue[1] = 0.4;
    mixer.luma[3] = -0.3;
    let spectrogram = finishing::spectrogram(&mixer).unwrap();
    mixer.luma[3] = -0.25;
    let near = finishing::spectrogram(&mixer).unwrap();
    per_pixel(&mut g, "spectrogram", &input, &pixels, stages::spectrogram(&spectrogram), |p| cpu::spectrogram(p, &spectrogram), |p| cpu::spectrogram(p, &near));

    let l = oni_settings::axis::pipeline::LUMA_WEIGHTS;
    let luma = |p: V4| {
        let y = l[0] * p[0] + l[1] * p[1] + l[2] * p[2];
        [y, y, y, p[3]]
    };
    per_pixel(&mut g, "luma only", &input, &pixels, stages::luma_only(), luma, |p| luma(swapped(p)));

    let positive_luma = mapped(&pixels, luma);
    let luma_source = g.upload(w, h, &positive_luma).unwrap();
    per_pixel(&mut g, "texture moments", &luma_source, &positive_luma, stages::texture_moments(), cpu::texture_moments, |p| cpu::texture_moments(p.map(|v| v * 1.01)));
    let moment_source = g.upload(w, h, &moments).unwrap();
    let coeff = |relative| TextureCoeff { relative };
    per_pixel(&mut g, "texture coefficients", &moment_source, &moments, stages::texture_coeff(&coeff(0.125)), |p| cpu::texture_coeff(p, &coeff(0.125)), |p| cpu::texture_coeff(p, &coeff(0.13)));

    let mut frontier = negative();
    frontier.face = Face::Scanner;
    frontier.scanner_machine = ScannerMachine::Frontier;
    frontier.scanner = ScannerClicks::new(1, -2, 3, 0, 0);
    let d = scanner::density(&frontier);
    let steeper = oni_render::ScannerDensity { scale: d.scale * 1.01, ..d };
    per_pixel(&mut g, "scanner density", &film, &transmitted, stages::scanner_density(&d), |p| cpu::scanner_density(p, &d), |p| cpu::scanner_density(p, &steeper));
    let root = |root| ScannerEncode { root };
    per_pixel(&mut g, "scanner encode", &input, &pixels, stages::scanner_encode(oni_look::ROOT), |p| cpu::scanner_encode(p, &root(oni_look::ROOT)), |p| cpu::scanner_encode(p, &root(2.1)));
}

#[test]
fn two_input_kernels_read_their_auxiliary_images_at_the_same_pixel() {
    let Some(mut g) = graph("two_input_kernels_read_their_auxiliary_images_at_the_same_pixel") else { return };
    let (w, h) = (29, 19);
    let (pa, pb, pc) = (noise(w, h, 3, 0.0, 1.2), noise(w, h, 4, 0.3, 1.0), noise(w, h, 5, 0.0, 1.0));
    let (a, b, c) = (g.upload(w, h, &pa).unwrap(), g.upload(w, h, &pb).unwrap(), g.upload(w, h, &pc).unwrap());
    // A neighbour's value, the twin of an auxiliary image read one pixel off.
    let beside = |p: &[V4], i: usize| p[if (i + 1).is_multiple_of(w as usize) { i } else { i + 1 }];
    let each = |f: &dyn Fn(usize) -> V4| (0..pa.len()).map(f).collect::<Vec<_>>();

    let k = FlatField { amount: 0.8, exponent: [0.75, 1.0, 1.5], per_channel: 0.0 };
    let out = whole(&mut g, &Image::new(&a).then_reading(stages::flat_field(&k), &[Image::new(&b)]));
    judge("flat-field", &out, &each(&|i| cpu::flat_field(pa[i], pb[i], &k)), &each(&|i| cpu::flat_field(pa[i], beside(&pb, i), &k)));

    let gain = 1.3;
    let out = whole(&mut g, &Image::new(&a).then_reading(stages::texture(gain), &[Image::new(&b), Image::new(&c)]));
    let t = Texture { gain };
    judge("texture", &out, &each(&|i| cpu::texture(pa[i], pb[i], pc[i], &t)), &each(&|i| cpu::texture(pa[i], pc[i], pb[i], &t)));

    let k = ScannerOutput { warmth_share: 0.6, lift: scanner::LIFT, contrast: 0.3 };
    let out = whole(&mut g, &Image::new(&c).then_reading(stages::scanner_output(k.warmth_share, k.lift, k.contrast), &[Image::new(&b)]));
    judge("scanner output", &out, &each(&|i| cpu::scanner_output(pc[i], pb[i], &k)), &each(&|i| cpu::scanner_output(pc[i], beside(&pb, i), &k)));

    // A patch on part of the frame, through its mask; the frame stays itself outside it.
    let tile = Rect::new(7, 4, 13, 9);
    let patch_pixels = noise(tile.w, tile.h, 6, 0.0, 1.0);
    let mask_pixels = noise(tile.w, tile.h, 7, 0.0, 1.0);
    let patch = g.upload_at(tile, &patch_pixels).unwrap();
    let mask = g.upload_at(tile, &mask_pixels).unwrap();
    let out = whole(&mut g, &Image::new(&a).then_reading(Patched { patch: tile, mask: tile }, &[Image::new(&patch), Image::new(&mask)]));
    let blended = |invert: bool| {
        each(&|i| {
            let (x, y) = ((i % w as usize) as i32, (i / w as usize) as i32);
            if x < tile.x || y < tile.y || x >= tile.right() || y >= tile.bottom() {
                return pa[i];
            }
            let j = ((y - tile.y) as u32 * tile.w + (x - tile.x) as u32) as usize;
            let m = if invert { mask_pixels[j].map(|v| 1.0 - v) } else { mask_pixels[j] };
            cpu::blend_with_mask(patch_pixels[j], pa[i], m)
        })
    };
    judge("patch through its mask", &out, &blended(false), &blended(true));
}

#[test]
fn tables_are_read_whole_whatever_region_is_asked() {
    let Some(mut g) = graph("tables_are_read_whole_whatever_region_is_asked") else { return };
    let extent = Rect::new(300, 200, 31, 17);
    let pixels = noise(extent.w, extent.h, 8, -0.1, 1.1);
    let input = g.upload_at(extent, &pixels).unwrap();
    let region = Rect::new(310, 205, 12, 9);
    let cut = |p: &[V4]| -> Vec<V4> {
        (region.y..region.bottom())
            .flat_map(|y| (region.x..region.right()).map(move |x| ((y - extent.y) as u32 * extent.w + (x - extent.x) as u32) as usize))
            .map(|i| p[i])
            .collect()
    };

    let set = CurveSet { contrast: 0.4, red: Curve { points: vec![Point::new(0.0, 0.1), Point::new(1.0, 1.0)] }, ..CurveSet::default() };
    let k = finishing::curves(&set).unwrap();
    let less = finishing::curves(&CurveSet { contrast: 0.38, ..set.clone() }).unwrap();
    let n = k.rgb_lut.len() as u32;
    let (rgb, luma) = (g.upload(n, 1, &k.rgb_lut).unwrap(), g.upload(n, 1, &k.luma_lut).unwrap());
    let image = Image::new(&input).then_reading(stages::curves(&k), &[Image::new(&rgb), Image::new(&luma)]);
    let out = g.render(&image, region, None).unwrap();
    judge("curves, a region away from the table", &out, &cut(&mapped(&pixels, |p| cpu::curves(p, &k))), &cut(&mapped(&pixels, |p| cpu::curves(p, &less))));

    let cubes: [(&str, &ColorCube); 3] =
        [("Frontier", &scanner::MACHINE.render), ("warmth", &scanner::MACHINE.warmth_plus), ("ONI look", &oni_look::CUBE)];
    for (name, cube) in cubes {
        let n = cube.cube_dimension as u32;
        let table = g.upload(n * n, n, &cube.cube_data).unwrap();
        let image = Image::new(&input).then_reading(stages::color_cube(n as usize), &[Image::new(&table)]);
        let out = g.render(&image, region, None).unwrap();
        let reference = cut(&mapped(&pixels, |p| cpu::color_cube(p, cube)));
        let twin = cut(&mapped(&pixels, |p| cpu::color_cube(swapped(p), cube)));
        judge(&format!("{name} cube, {n} nodes"), &out, &reference, &twin);
    }
}

#[test]
fn the_vignette_centres_on_its_extent() {
    let Some(mut g) = graph("the_vignette_centres_on_its_extent") else { return };
    let extent = Rect::new(40, 10, 51, 33);
    let pixels = noise(extent.w, extent.h, 9, 0.0, 1.4);
    let input = g.upload_at(extent, &pixels).unwrap();
    let picture = oni_render::Image::new(extent.w as usize, extent.h as usize, pixels.clone());
    for (stops, hunt) in [(-1.2, 1.0), (0.8, 0.0)] {
        let out = whole(&mut g, &Image::new(&input).then(stages::vignette(extent, stops, hunt)));
        let k = |stops| Vignette::on_extent([0.0; 2], picture.width, picture.height, stops, hunt);
        let reference = cpu::vignette(&picture, &k(stops)).pixels;
        let twin = cpu::vignette(&picture, &k(stops * 1.05)).pixels;
        judge(&format!("vignette {stops} stops, hunt {hunt}"), &out, &reference, &twin);
    }
}

#[test]
fn detail_matches_its_reference_whole_and_on_a_region() {
    let Some(mut g) = graph("detail_matches_its_reference_whole_and_on_a_region") else { return };
    let (w, h) = (61, 47);
    let pixels = noise(w, h, 10, 0.0, 1.3);
    let input = g.upload(w, h, &pixels).unwrap();
    let picture = oni_render::Image::new(w as usize, h as usize, pixels);
    let cases = [
        ("joint, haloed", Detail::dosed(Some((3.0, 0.6)), Some((1.6, 0.8, 0.5))).unwrap()),
        ("chroma alone", Detail::dosed(Some((5.0, 0.4)), None).unwrap()),
        ("sharpening alone", Detail::dosed(None, Some((2.4, 1.2, 0.0))).unwrap()),
    ];
    for (name, k) in cases {
        let image = Image::new(&input);
        let rows = image.clone().then(DetailRows(k));
        let image = image.then_reading(DetailColumns(k), &[rows]);
        let out = whole(&mut g, &image);
        let reference = cpu::detail(&picture, &k).pixels;
        let twin = cpu::detail(&picture, &Detail { force: k.force * 0.9, amount: k.amount * 1.05, ..k }).pixels;
        judge(&format!("detail, {name}"), &out, &reference, &twin);
        let region = Rect::new(9, 13, 20, 11);
        let part = g.render(&image, region, None).unwrap();
        let cut = |p: &[V4], r: Rect| -> Vec<V4> {
            (r.y..r.bottom()).flat_map(|y| (r.x..r.right()).map(move |x| p[(y as u32 * w + x as u32) as usize])).collect()
        };
        judge(&format!("detail, {name}, a region"), &part, &cut(&reference, region), &cut(&reference, Rect::new(10, 13, 20, 11)));
    }
}

#[test]
fn the_gaussian_matches_on_repeated_and_transparent_edges() {
    let Some(mut g) = graph("the_gaussian_matches_on_repeated_and_transparent_edges") else { return };
    let extent = Rect::new(-5, 3, 43, 29);
    let pixels = noise(extent.w, extent.h, 11, 0.0, 1.0);
    let input = g.upload_at(extent, &pixels).unwrap();
    let grid = Grid::new(extent.x, extent.y, extent.w as usize, extent.h as usize, pixels);
    for transparent in [false, true] {
        let both = |sigma: f32| {
            let k = |vertical| Gaussian { sigma, vertical, transparent };
            cpu::gaussian(&cpu::gaussian(&grid, &k(false)), &k(true)).pixels
        };
        let out = whole(&mut g, &stages::blurred(Image::new(&input), 2.3, transparent));
        judge(&format!("gaussian, transparent {transparent}"), &out, &both(2.3), &both(2.4));
    }
    let k = Gaussian { sigma: 1.7, vertical: true, transparent: false };
    let out = whole(&mut g, &Image::new(&input).then(Blur(k)));
    let across = cpu::gaussian(&grid, &Gaussian { vertical: false, ..k }).pixels;
    judge("gaussian, one vertical pass", &out, &cpu::gaussian(&grid, &k).pixels, &across);
}

#[test]
fn rotation_turns_and_strokes_match() {
    let Some(mut g) = graph("rotation_turns_and_strokes_match") else { return };
    let upright = Rect::new(0, 0, 41, 27);
    let pixels = noise(upright.w, upright.h, 12, 0.0, 1.0);
    let input = g.upload_at(upright, &pixels).unwrap();
    let grid = Grid::new(0, 0, upright.w as usize, upright.h as usize, pixels);
    let out_of = |r: Rect| (r.x, r.y, r.w as usize, r.h as usize);

    let (k, rect) = geometry::rotation(7.0, upright);
    let (tilted, _) = geometry::rotation(7.2, upright);
    let out = whole(&mut g, &Image::new(&input).then(Resample { k, extent: rect }));
    judge("rotation by 7 degrees", &out, &cpu::affine(&grid, out_of(rect), &k).pixels, &cpu::affine(&grid, out_of(rect), &tilted).pixels);

    for (turns, mirrored) in [(1, false), (2, true), (3, false)] {
        let geometry = Geometry { quarter_turns: turns, mirrored, ..Geometry::default() };
        let (k, rect) = geometry::orientation(&geometry, upright);
        let out = whole(&mut g, &Image::new(&input).then(Oriented { k, extent: rect }));
        let reference = cpu::orient(&grid, out_of(rect), &k).pixels;
        let twin = cpu::orient(&grid, out_of(rect), &Orient { b: [k.b[0] + 1, k.b[1]], ..k }).pixels;
        judge(&format!("{turns} quarter turns, mirrored {mirrored}"), &out, &reference, &twin);
    }

    let points = [[5.5f32, 6.0], [30.2, 18.7], [12.0, 25.0]];
    let row: Vec<V4> = points.iter().map(|p| [p[0], p[1], 0.0, 1.0]).collect();
    let row = g.upload(row.len() as u32, 1, &row).unwrap();
    let extent = Rect::new(0, 0, 40, 30);
    let mask = oni_render::StrokeMask { radius: 3.2, count: points.len() };
    let out = whole(&mut g, &Image::new(&row).then(Stroke { k: mask, extent }));
    judge("stroke mask", &out, &cpu::stroke_mask(&points, out_of(extent), 3.2).pixels, &cpu::stroke_mask(&points, out_of(extent), 3.4).pixels);
}

/// Every settings family the pipeline branches on, each stage on.
fn families() -> Vec<(&'static str, PipelineSettings)> {
    let mut pro = negative();
    pro.capture_stops = 0.4;
    pro.flat_field.mask = FlatFieldMask::ValoiEasy35V1;
    pro.flat_field.amount = 1.0;
    pro.geometry = Geometry { quarter_turns: 1, angle: 2.5, crop: oni_settings::cg::Rect { x: 0.1, y: 0.05, width: 0.8, height: 0.85 }, ..Geometry::default() };
    pro.manual_curves = CurveSet { contrast: 0.3, ..CurveSet::default() };
    pro.spectrogram.saturation[2] = 0.4;
    pro.saturation = finishing::sauce(0.5);
    pro.vignette.stops = -0.6;
    pro.texture.amount = 0.8;
    pro.secret_sauce = 1.2;
    pro.print.black_point = 0.05;
    let mut joint = pro.clone();
    joint.texture.amount = 0.0;
    let mut mono = pro.clone();
    mono.mode = ConversionMode::Monochrome;
    let mut positive = pro.clone();
    positive.mode = ConversionMode::Positive;
    let mut frontier = pro.clone();
    frontier.face = Face::Scanner;
    frontier.scanner_machine = ScannerMachine::Frontier;
    frontier.scanner = ScannerClicks::new(1, 2, -1, 0, 3);
    frontier.scanner_finish.contrast = 0.2;
    let mut oni = frontier.clone();
    oni.scanner_machine = ScannerMachine::Oni;
    oni.scanner = ScannerClicks::new(0, 1, 0, -2, -2);
    vec![("Pro", pro), ("Pro, texture resting", joint), ("monochrome", mono), ("positive", positive), ("Frontier", frontier), ("ONI machine", oni)]
}

#[test]
fn every_family_renders_finite_and_a_region_is_the_cut_of_the_whole() {
    let Some(mut g) = graph("every_family_renders_finite_and_a_region_is_the_cut_of_the_whole") else { return };
    let (w, h) = (1600, 1066);
    // A density ramp across the frame under a grain, so some of it lands in any window.
    let grain = noise(w, h, 13, 0.9, 1.1);
    let ramp: Vec<V4> = grain
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let (x, y) = ((i as u32 % w) as f32 / w as f32, (i as u32 / w) as f32 / h as f32);
            let t = |c: usize, tilt: f32| 10f32.powf(-3.0 * (x + tilt * y) / (1.0 + tilt)) * n[c];
            [t(0, 0.1), t(1, 0.3), t(2, 0.5), 1.0]
        })
        .collect();
    let source = g.upload(w, h, &ramp).unwrap();
    let mut pipeline = Pipeline::new();
    let mut means = Vec::new();
    for (name, settings) in families() {
        // Half of 1600 px: Texture's wide radius, 0.0008 of the long side, clears its 0.6 px floor.
        let request = Request { scale: 0.5, full_width: Some(f64::from(w)), ..Request::default() };
        let image = pipeline.image(&mut g, &source, &settings, &request, &Default::default()).unwrap();
        let extent = image.extent();
        let all = whole(&mut g, &image);
        assert!(all.iter().all(|p| p.iter().all(|v| v.is_finite())), "{name}: a pixel is not finite");
        let mean = all.iter().fold([0.0f32; 3], |m, p| [m[0] + p[0], m[1] + p[1], m[2] + p[2]]).map(|v| v / all.len() as f32);
        println!("{name}: mean {mean:?}, first {:?}", all[0]);
        if means.len() < 2 {
            means.push(all.clone());
        }
        assert!(all.iter().any(|p| p[0] > 0.02 && p[0] < 0.98), "{name}: the frame is flat");
        let region = Rect::new(extent.x + 9, extent.y + 7, extent.w / 2, extent.h / 2);
        let part = g.render(&image, region, None).unwrap();
        let cut = |r: Rect| -> Vec<V4> {
            (r.y..r.bottom())
                .flat_map(|y| (r.x..r.right()).map(move |x| ((y - extent.y) as u32 * extent.w + (x - extent.x) as u32) as usize))
                .map(|i| all[i])
                .collect()
        };
        let moved = Rect::new(region.x + 1, region.y, region.w, region.h);
        judge(&format!("{name} {}x{}, a region against the whole", extent.w, extent.h), &part, &cut(region), &cut(moved));
    }
    // Texture moves the frame: the family with it resting is a different picture.
    let moved = error(&means[0], &means[1]);
    println!("Texture against its rest: {moved:.2e}");
    assert!(moved > 1e-2, "Texture changed nothing: {moved:.2e}");
}
