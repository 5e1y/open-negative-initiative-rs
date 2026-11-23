//! The grain against its reference, held by its scan pixel, averaged at reduced scales, in stops, its
//! share following its curve, absent at rest and laid under the print; each verdict with a twin the check refuses.

mod common;

use common::{V4, graph, judge, noise, whole};
use oni_graph::{Graph, Image, Rect};
use oni_pipeline::film::GREY;
use oni_pipeline::{Pipeline, Request, conversion, geometry, grain, stages};
use oni_render::{GRAIN_NODES, Grain, LUMA, cpu};
use oni_settings::migrate::grain_size::microns;
use oni_settings::blocks::{GrainColour, GrainCurve};
use oni_settings::cg::Point;
use oni_settings::frame::Geometry;
use oni_settings::{ConversionMode, LegacyPrint, PipelineSettings};

const IDENTITY: [[f32; 3]; 2] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];

fn scaled(scale: f32) -> [[f32; 3]; 2] {
    [[1.0 / scale, 0.0, 0.0], [0.0, 1.0 / scale, 0.0]]
}

/// The whole grain at every tone, so a check of the texture reads no curve.
const FLAT: GrainCurve = GrainCurve { points: [Point::new(0.0, 1.0), Point::new(0.25, 1.0), Point::new(0.5, 1.0), Point::new(0.75, 1.0), Point::new(1.0, 1.0)] };

fn grainy(amount: f32, size: f32) -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Digital, false).per_pixel_only();
    s.grain.amount = amount;
    s.grain.size = microns(f64::from(size), 1.0);
    s.grain.curve = FLAT;
    s
}

/// The rest curve's share on a digital photo's picture at rest.
fn resting_shares() -> [f32; GRAIN_NODES] {
    grain::shares(&GrainCurve::REST, &grain::tones(&conversion::stage47(&PipelineSettings::rest(ConversionMode::Digital, false))))
}

fn flat(g: &mut Graph, w: u32, h: u32, value: V4) -> Image {
    Image::new(&g.upload(w, h, &vec![value; (w * h) as usize]).unwrap())
}

/// Each pixel's grain in stops, on a flat input of `grey`.
fn stops(out: &[V4], grey: f32) -> Vec<f32> {
    out.iter().map(|p| (p[0] / grey).log2()).collect()
}

fn spread(v: &[f32]) -> f32 {
    let mean = v.iter().sum::<f32>() / v.len() as f32;
    (v.iter().map(|x| (x - mean) * (x - mean)).sum::<f32>() / v.len() as f32).sqrt()
}

/// The `n`×`n` means of a field `w` wide.
fn box_means(v: &[f32], w: usize, n: usize) -> Vec<f32> {
    let h = v.len() / w;
    let mut out = Vec::new();
    for by in 0..h / n {
        for bx in 0..w / n {
            let sum: f32 = (0..n).flat_map(|y| (0..n).map(move |x| (bx * n + x, by * n + y))).map(|(x, y)| v[y * w + x]).sum();
            out.push(sum / (n * n) as f32);
        }
    }
    out
}

fn largest_gap(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f32::max)
}

#[test]
fn the_grain_matches_its_reference_on_every_branch() {
    let Some(mut g) = graph("the_grain_matches_its_reference_on_every_branch") else { return };
    let extent = Rect::new(23, 11, 61, 37);
    let mut pixels = noise(extent.w, extent.h, 21, 0.02, 1.2);
    // Past both ends of the table, on none and under it: every branch of the share.
    for (i, v) in [0.0, -0.3, 1e-6, 2.0, 0.6, 0.08].into_iter().enumerate() {
        pixels[i * 97] = [v, v, v, 1.0];
    }
    let input = g.upload_at(extent, &pixels).unwrap();
    let centre = |i: usize| [extent.x as f32 + (i % extent.w as usize) as f32 + 0.5, extent.y as f32 + (i / extent.w as usize) as f32 + 0.5];
    let scan = Rect::new(0, 0, 200, 120);
    let narrowed = |m: [[f64; 3]; 2]| m.map(|r| r.map(|v| v as f32));
    let turned = narrowed(geometry::scan_from(&Geometry { quarter_turns: 1, mirrored: true, ..Geometry::default() }, scan, 1.0));
    let angled = narrowed(geometry::scan_from(&Geometry { quarter_turns: 3, angle: 7.0, ..Geometry::default() }, scan, 0.5));
    // A quarter of a cell, two cells, past the cap, turned and mirrored, turned and angled.
    let cases = [
        ("a footprint under a cell", IDENTITY, 4.0, 1.0),
        ("a footprint of two cells", scaled(0.5), 1.0, 0.5),
        ("a footprint past the cap", scaled(0.2), 1.0, 0.2),
        ("turned and mirrored", turned, 1.5, 1.0),
        ("angled, at half", angled, 1.2, 0.5),
    ];
    let share = resting_shares();
    for (name, m, cell, scale) in cases {
        let k = Grain::on(m, cell, scale, 0.4, share);
        // Twin: the lattice one node over, a seed off by one.
        let mut over = k;
        over.scan[0][2] += cell;
        let out = whole(&mut g, &grain::grained(Image::new(&input), &k, 0.0));
        let reference: Vec<V4> = pixels.iter().enumerate().map(|(i, p)| cpu::grain(*p, *p, centre(i), &k)).collect();
        let twin: Vec<V4> = pixels.iter().enumerate().map(|(i, p)| cpu::grain(*p, *p, centre(i), &over)).collect();
        judge(&format!("grain, {name}"), &out, &reference, &twin);
        // Twin: the whole grain at every tone.
        let whole_share = Grain { share: [1.0; GRAIN_NODES], ..k };
        let flat: Vec<V4> = pixels.iter().enumerate().map(|(i, p)| cpu::grain(*p, *p, centre(i), &whole_share)).collect();
        judge(&format!("grain by tone, {name}"), &out, &reference, &flat);
    }
    // As the chain lays them: the grain on the scene, then the print.
    let k = Grain::on(scaled(0.5), 1.0, 0.5, 0.4, [1.0; GRAIN_NODES]);
    let print = conversion::stage47(&PipelineSettings::rest(ConversionMode::Digital, false));
    let out = whole(&mut g, &stages::converted(grain::grained(Image::new(&input), &k, 0.0), &print));
    let reference: Vec<V4> = pixels.iter().enumerate().map(|(i, p)| cpu::stage47(cpu::grain(*p, *p, centre(i), &k), &print)).collect();
    // Twin: the grain on the print.
    let after: Vec<V4> = pixels.iter().enumerate().map(|(i, p)| { let q = cpu::stage47(*p, &print); cpu::grain(q, q, centre(i), &k) }).collect();
    judge("grain, then the print", &out, &reference, &after);
}

#[test]
fn a_scan_pixel_keeps_its_grain_under_a_region_a_crop_and_a_turn() {
    let Some(mut g) = graph("a_scan_pixel_keeps_its_grain_under_a_region_a_crop_and_a_turn") else { return };
    // 1500 pixels wide at size 0.5: half-pixel cells, two to a pixel's footprint.
    let (w, h) = (1500u32, 1000u32);
    let source = g.upload(w, h, &vec![[0.18, 0.18, 0.18, 1.0]; (w * h) as usize]).unwrap();
    let mut pipeline = Pipeline::new();
    let s = grainy(1.0, 0.5);
    let mut render = |g: &mut Graph, s: &PipelineSettings, region: Option<Rect>| -> (Rect, Vec<V4>) {
        let image = pipeline.image(g, &source, s, &Request::default(), &Default::default()).unwrap();
        let region = region.unwrap_or(image.extent());
        (region, g.render(&image, region, None).unwrap())
    };
    let (_, all) = render(&mut g, &s, None);
    let at = |x: i32, y: i32| all[(y as u32 * w + x as u32) as usize][0];
    let cut = |r: Rect, dx: i32| -> Vec<f32> { (r.y..r.bottom()).flat_map(|y| (r.x..r.right()).map(move |x| (x + dx, y))).map(|(x, y)| at(x, y)).collect() };
    let reds = |p: &[V4]| -> Vec<f32> { p.iter().map(|p| p[0]).collect() };

    // A pan: the region renders what the whole holds there, bit for bit; twin: a pixel over.
    let region = Rect::new(301, 207, 128, 96);
    let (_, part) = render(&mut g, &s, Some(region));
    assert_eq!(largest_gap(&reds(&part), &cut(region, 0)), 0.0, "a region's grain is the whole's");
    assert!(largest_gap(&reds(&part), &cut(region, 1)) > 1e-3, "twin: the grain changes pixel to pixel");

    // A crop keeps every pixel's coordinate: the grain stays where it was.
    let mut cropped = s.clone();
    cropped.geometry.crop = oni_settings::cg::Rect::new(0.2, 0.25, 0.5, 0.5);
    let (frame, inside) = render(&mut g, &cropped, None);
    assert_eq!(frame, Rect::new(300, 250, 750, 500));
    assert_eq!(largest_gap(&reds(&inside), &cut(frame, 0)), 0.0, "a crop moves no grain");

    // A turn and a mirror take each scan pixel's grain with it; twin: a grain laid on the output's grid.
    let scan = Rect::new(0, 0, w, h);
    for turn in [Geometry { quarter_turns: 1, ..Geometry::default() }, Geometry { quarter_turns: 3, mirrored: true, ..Geometry::default() }] {
        let (k, out) = geometry::orientation(&turn, scan);
        let back = |p: [i32; 2]| [k.a[0] * p[0] + k.a[1] * p[1] + k.b[0], k.a[2] * p[0] + k.a[3] * p[1] + k.b[1]];
        let followed = |turned: &[V4], plain: &dyn Fn(i32, i32) -> f32| -> f32 {
            (0..turned.len())
                .map(|i| {
                    let q = back([(i as u32 % out.w) as i32, (i as u32 / out.w) as i32]);
                    (turned[i][0] - plain(q[0], q[1])).abs()
                })
                .fold(0.0, f32::max)
        };
        let mut t = s.clone();
        t.geometry = turn;
        let (_, turned) = render(&mut g, &t, None);
        let gap = followed(&turned, &at);
        assert!(gap < 1e-6, "{turn:?}: a scan pixel's grain moved by {gap:.2e}");
        let on_output = Grain { scan: IDENTITY, ..grain::grain(&s, scan, 1.0).unwrap() };
        let image = flat(&mut g, w, h, [1.0; 4]);
        let plain = whole(&mut g, &grain::grained(image.clone(), &on_output, 0.0));
        let turned = whole(&mut g, &grain::grained(geometry::apply(image, &turn, None, 1.0), &on_output, 0.0));
        let twin = followed(&turned, &|x, y| plain[(y as u32 * w + x as u32) as usize][0]);
        assert!(twin > 1e-2, "{turn:?}: the twin follows at {twin:.2e}, so the check sees nothing");
    }
}

#[test]
fn a_reduced_render_reads_the_full_grain_averaged() {
    let Some(mut g) = graph("a_reduced_render_reads_the_full_grain_averaged") else { return };
    // Size 1 on a 3000-pixel scan: two-pixel cells, so the footprint stays under the cap to a quarter.
    let s = grainy(1.0, 1.0);
    let at = |g: &mut Graph, scale: f64, w: u32, h: u32, k: Option<Grain>| -> Vec<f32> {
        let k = k.unwrap_or_else(|| grain::grain(&s, Rect::new(0, 0, (3000.0 * scale) as u32, (2000.0 * scale) as u32), scale).unwrap());
        // A dark grey, where the colour is strongest and leaves the grey grey, so the red reads the field.
        let image = flat(g, w, h, [0.05, 0.05, 0.05, 1.0]);
        stops(&whole(g, &grain::grained(image, &k, 0.0)), 0.05)
    };
    let full = at(&mut g, 1.0, 800, 800, None);
    let full_spread = spread(&full);
    for (scale, n) in [(0.5, 2usize), (0.25, 4)] {
        let side = 800 / n as u32;
        let reduced = at(&mut g, scale, side, side, None);
        let gap = largest_gap(&reduced, &box_means(&full, 800, n));
        println!("at {scale}: {gap:.2e} from the full grain's means");
        assert!(gap < 1e-4, "at {scale}: {gap:.2e}");
        // Twin: the field read at each pixel's centre alone.
        let point = Grain { reach: 1e-3, ..grain::grain(&s, Rect::new(0, 0, (3000.0 * scale) as u32, (2000.0 * scale) as u32), scale).unwrap() };
        let twin = largest_gap(&at(&mut g, scale, side, side, Some(point)), &box_means(&full, 800, n));
        assert!(twin > 1e-2, "at {scale}: the twin passes at {twin:.2e}");
    }
    // Past the cap the mean is the cap's, scaled to the whole footprint's spread.
    for (scale, n) in [(0.1, 10usize), (0.05, 20)] {
        let side = 800 / n as u32;
        let k = grain::grain(&s, Rect::new(0, 0, (3000.0 * scale) as u32, (2000.0 * scale) as u32), scale).unwrap();
        let means = spread(&box_means(&full, 800, n));
        let reduced = spread(&at(&mut g, scale, side, side, Some(k)));
        println!("at {scale}: spread {reduced:.4} against the full grain's means {means:.4}, full {full_spread:.4}");
        assert!((reduced / means - 1.0).abs() < 0.15, "at {scale}: {reduced} against {means}");
        // Twin: the cap's mean at the full grain's stops, unscaled to the whole footprint's spread.
        let unscaled = Grain { stops: grain::grain(&s, Rect::new(0, 0, 3000, 2000), 1.0).unwrap().stops, ..k };
        let twin = spread(&at(&mut g, scale, side, side, Some(unscaled)));
        assert!((twin / means - 1.0).abs() > 0.3, "at {scale}: the twin passes at {twin} against {means}");
    }
}

#[test]
fn the_grain_is_a_texture_in_stops_its_spread_by_tone_its_curve_s() {
    let Some(mut g) = graph("the_grain_is_a_texture_in_stops_its_spread_by_tone_its_curve_s") else { return };
    let scan = Rect::new(0, 0, 1500, 1000);
    let mut mono = grainy(1.0, 0.5);
    mono.grain.colour = GrainColour { black: 0.0, white: 0.0 };
    let k = grain::grain(&mono, scan, 1.0).unwrap();
    let rest = PipelineSettings { grain: oni_settings::blocks::Grain { curve: GrainCurve::REST, ..mono.grain }, ..mono.clone() };
    let by_tone = grain::grain(&rest, scan, 1.0).unwrap();
    let picture = conversion::stage47(&rest);
    let strength = GrainCurve::REST.strength();
    let (w, h) = (200, 150);
    let mut whole_spreads = Vec::new();
    let mut misses = Vec::new();
    for grey in [0.02f32, 0.18, 0.8] {
        let image = flat(&mut g, w, h, [grey, grey, grey, 1.0]);
        let whole_grain = spread(&stops(&whole(&mut g, &grain::grained(image.clone(), &k, 0.0)), grey));
        let curved = spread(&stops(&whole(&mut g, &grain::grained(image, &by_tone, 0.0)), grey));
        // A digital photo's light crosses its film before its tone is read.
        let stock = oni_pipeline::film::stock(oni_settings::VirtualStock::of("kodak-portra-800"));
        let shown = cpu::stage47(cpu::virtual_negative([grey, grey, grey, 1.0], &stock.kernel_at(0.0), &stock.table), &picture);
        let tone = (shown[0] * LUMA[0] + shown[1] * LUMA[1] + shown[2] * LUMA[2]).powf(1.0 / 2.2);
        let want = strength.at(f64::from(tone)) as f32;
        println!("{grey}: tone {tone:.3}, share {want:.3}, spread {curved:.4} against {whole_grain:.4}");
        let miss = |ratio: f32| (ratio - want).abs() > 0.1 * want.max(0.1);
        assert!(!miss(curved / whole_grain), "{grey}: the curve's share {want} against a spread {}", curved / whole_grain);
        // Twin: the whole grain at every tone misses the curve somewhere.
        misses.push(miss(1.0));
        whole_spreads.push(whole_grain);
    }
    assert!(misses.iter().any(|&m| m), "the twin passes, so the check sees nothing");
    // The whole grain is a texture in stops, the same on every tone.
    for s in &whole_spreads {
        assert!((s / whole_spreads[1] - 1.0).abs() < 0.1, "{s} stops against {} on a middle grey", whole_spreads[1]);
    }
    // Twin: the middle grey's grain added in light, the same on every tone, is no texture in stops.
    let image = flat(&mut g, w, h, [0.18, 0.18, 0.18, 1.0]);
    let middle = whole(&mut g, &grain::grained(image, &k, 0.0));
    let added: Vec<V4> = middle.iter().map(|p| [0.8 + p[0] - 0.18; 4]).collect();
    let light = spread(&stops(&added, 0.8));
    assert!((light / whole_spreads[1] - 1.0).abs() > 0.5, "the twin passes: {light} against {}", whole_spreads[1]);
    // Monochrome at no colour: every channel moves by one factor, alpha kept.
    let colour = [0.3, 0.12, 0.05, 1.0];
    let parted = |g: &mut Graph, k: &Grain| -> f32 {
        let image = grain::grained(flat(g, w, h, colour), k, 0.0);
        let out = whole(g, &image);
        assert!(out.iter().all(|p| p[3] == 1.0));
        out.iter().map(|p| [p[0] / colour[0], p[1] / colour[1], p[2] / colour[2]]).map(|f| (f[0] - f[1]).abs().max((f[1] - f[2]).abs())).fold(0.0, f32::max)
    };
    assert!(parted(&mut g, &k) < 1e-5);
    // Twin: the default colour parts the channels.
    let coloured = grain::grain(&grainy(1.0, 0.5), Rect::new(0, 0, 1500, 1000), 1.0).unwrap();
    assert!(parted(&mut g, &coloured) > 1e-2);
}

#[test]
fn off_or_measured_the_chain_holds_no_grain() {
    let Some(mut g) = graph("off_or_measured_the_chain_holds_no_grain") else { return };
    let (w, h) = (300, 200);
    let source = g.upload(w, h, &noise(w, h, 5, 0.05, 0.9)).unwrap();
    let mut pipeline = Pipeline::new();
    let mut image = |s: &PipelineSettings, measuring: bool| {
        let request = Request { measuring, ..Request::default() };
        pipeline.image(&mut g, &source, s, &request, &Default::default()).unwrap()
    };
    let rest = PipelineSettings::rest(ConversionMode::Digital, false);
    let rest = PipelineSettings { grain: oni_settings::blocks::Grain { amount: 0.0, ..rest.grain }, ..rest };
    let mut coarse = rest.clone();
    coarse.grain.size = microns(0.9, 1.0);
    coarse.grain.colour = GrainColour { black: 0.5, white: 1.5 };
    let (a, b) = (image(&rest, false), image(&coarse, false));
    assert_eq!((a.key(), a.stages()), (b.key(), b.stages()), "a size or a colour at amount 0 lays no stage");
    let mut grained = rest.clone();
    grained.grain.amount = 0.01;
    assert_ne!(image(&grained, false).key(), a.key(), "twin: the faintest grain is a stage");
    let mut bent = coarse.clone();
    bent.grain.curve.move_point(2, Point::new(0.5, 0.2));
    assert_eq!(image(&bent, false).key(), a.key(), "a curve at amount 0 lays no stage");
    let mut none = grained.clone();
    none.grain.curve = GrainCurve { points: FLAT.points.map(|p| Point::new(p.x, 0.0)) };
    assert_eq!(image(&none, false).key(), a.key(), "a curve of no share anywhere lays no stage");
    assert_eq!(image(&grained, true).key(), image(&rest, true).key(), "a measure reads no grain");
    // A film mode renders no grain its panel hides, its value kept; the Digital grain above is its twin.
    let negative = PipelineSettings::rest(ConversionMode::Negative, false);
    let held = PipelineSettings { grain: grained.grain, ..negative.clone() };
    assert_eq!(image(&held, false).key(), image(&negative, false).key(), "a negative lays no grain");
    let (rendered, again) = (whole(&mut g, &a), whole(&mut g, &b));
    assert_eq!(rendered, again);
}

/// On the whole grain at every tone: the rest curve gives the printed twin's light no share at all.
#[test]
fn the_grain_lies_under_the_print_so_contrast_steepens_it() {
    let Some(mut g) = graph("the_grain_lies_under_the_print_so_contrast_steepens_it") else { return };
    let (w, h) = (1500u32, 1000u32);
    let grey = 0.18;
    let source = g.upload(w, h, &vec![[grey, grey, grey, 1.0]; (w * h) as usize]).unwrap();
    let mut pipeline = Pipeline::new();
    let region = Rect::new(400, 300, 200, 200);
    let s = grainy(0.6, 0.5);
    let k = grain::grain(&s, Rect::new(0, 0, w, h), 1.0).unwrap();
    // Each render's grain in stops of its output, at two contrasts.
    let mut steepening = |after: bool| -> f32 {
        let mut at = |c: f32| {
            let mut t = s.clone();
            t.print.contrast = c;
            if after {
                t.grain.amount = 0.0;
            }
            let image = pipeline.image(&mut g, &source, &t, &Request::default(), &Default::default()).unwrap();
            let image = if after { grain::grained(image, &k, 0.0) } else { image };
            let out = g.render(&image, region, None).unwrap();
            let mid = out.iter().map(|p| p[1]).sum::<f32>() / out.len() as f32;
            println!("contrast {c}: mid {mid}");
            assert!(mid > 0.1 && mid < 0.9, "the grey prints mid-tone: {mid}");
            spread(&out.iter().map(|p| p[1].log2()).collect::<Vec<_>>())
        };
        at(0.5) / at(-0.5)
    };
    let under = steepening(false);
    println!("contrast steepens the grain {under:.3} times");
    assert!(under > 1.15, "the print's contrast leaves the grain at {under:.3}");
    // Twin: a grain laid on the print, which the contrast never reaches.
    let over = steepening(true);
    assert!(over < 1.15, "the twin is steepened {over:.3} times, so the check sees nothing");
}

#[test]
fn a_print_notch_through_the_frontier_reruns_no_grain() {
    let Some(mut g) = graph("a_print_notch_through_the_frontier_reruns_no_grain") else { return };
    let (w, h) = (300u32, 200u32);
    let source = g.upload(w, h, &noise(w, h, 9, 0.05, 0.9)).unwrap();
    let mut pipeline = Pipeline::new();
    let mut image = |g: &mut Graph, s: &PipelineSettings| pipeline.image(g, &source, s, &Request::default(), &Default::default()).unwrap();
    let grained = PipelineSettings::rest(ConversionMode::Digital, false);
    let rest = PipelineSettings { grain: oni_settings::blocks::Grain { amount: 0.0, ..grained.grain }, ..grained.clone() };
    assert_eq!(image(&mut g, &grained).stages(), image(&mut g, &rest).stages() + 1, "the grain is a pass of its own");
    let legacy = Some(LegacyPrint::default());
    let (scanner, keyed) = (PipelineSettings { legacy_print: legacy, ..rest.clone() }, PipelineSettings { legacy_print: legacy, ..grained.clone() });
    assert_eq!(image(&mut g, &keyed).stages(), image(&mut g, &scanner).stages() + 1, "a legacy print lays the grain");
    // Each setting's notches through one frontier cache, or none: the passes and pixels the last one ran.
    let mut notches = |g: &mut Graph, s: &PipelineSettings, cached: bool, notch: &dyn Fn(&mut PipelineSettings, f32)| -> (u64, u64) {
        let mut cache = oni_graph::Cache::frontier();
        let mut ran = (0, 0);
        for step in 0..3 {
            let mut t = s.clone();
            notch(&mut t, step as f32 * 0.01);
            let image = image(g, &t);
            let before = g.stats();
            g.materialise_cached(&image, image.extent(), cached.then_some(&mut cache)).unwrap();
            let after = g.stats();
            ran = (after.dispatches - before.dispatches, after.pixels - before.pixels);
        }
        ran
    };
    let exposure = |t: &mut PipelineSettings, d: f32| t.print.exposure += d;
    let (plain, with_grain) = (notches(&mut g, &rest, true, &exposure), notches(&mut g, &grained, true, &exposure));
    assert_eq!(with_grain, plain, "an exposure notch reruns the grain");
    // Twins: with no cache the notch writes more; a grain notch reruns the grain's pass as well as the print's.
    assert!(notches(&mut g, &grained, false, &exposure).1 > with_grain.1, "the cache keeps nothing");
    let amount = |t: &mut PipelineSettings, d: f32| t.grain.amount += d;
    // A grain notch reruns its own pass, the film's that reads it and the dye separation's that reads the film.
    assert_eq!(notches(&mut g, &grained, true, &amount).0, with_grain.0 + 3, "a grain notch skips its own pass");
    let colour = |t: &mut PipelineSettings, d: f32| t.grain.colour.black += d;
    assert_eq!(notches(&mut g, &grained, true, &colour).0, with_grain.0 + 3, "a colour notch skips the grain's pass");
    // A legacy print's key is a gain the grain's tone never reads.
    let key = |t: &mut PipelineSettings, d: f32| t.legacy_print = Some(LegacyPrint { d: (d * 200.0) as i64, ..LegacyPrint::default() });
    assert_eq!(notches(&mut g, &keyed, true, &key), notches(&mut g, &scanner, true, &key), "a key notch reruns the grain");
}

/// A digital photo shot on the virtual film keeps its grain where a photo's grain lies, fullest in the low mids (a stop
/// under the grey on Endura Premier) and gone at the white: its scene's light crosses the film before the tone is read.
/// Twin: read straight through stage 4-7, as if the scene were the scan, the shares turn over.
#[test]
fn a_filmed_photo_keeps_its_grain_in_the_shadows() {
    let mut s = PipelineSettings::rest(ConversionMode::Digital, true);
    s.grain.amount = 0.5;
    let k = grain::grain(&s, Rect::new(0, 0, 6000, 4000), 1.0).expect("a grain");
    let node = |y: f32| (0..GRAIN_NODES).min_by(|a, b| (oni_render::grain_node_light(*a).log2() - y.log2()).abs().total_cmp(&(oni_render::grain_node_light(*b).log2() - y.log2()).abs())).unwrap();
    let (shadow, highlight) = (k.share[node(GREY / 2.0)], k.share[node(0.9)]);
    println!("filmed: shadow {shadow:.3}, highlight {highlight:.3}");
    assert!(shadow > 0.6 && shadow > 2.0 * highlight, "shadow {shadow}, highlight {highlight}");
    let through = grain::shares(&s.grain.curve, &grain::tones(&oni_render::Stage47 { gain: [1.0; 3], ..conversion::stage47(&s) }));
    assert!(through[node(0.03)] < through[node(0.9)], "twin: the scene read as the scan turns the shares over");
}
