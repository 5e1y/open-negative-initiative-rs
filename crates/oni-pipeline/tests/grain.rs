//! The grain against its reference on every branch, a reduced render's the full one averaged, its strength at rest a real
//! Portra 800 scan's, absent at rest or measured, and its cost on a whole screen; each verdict with a twin it refuses.

mod common;

use common::{V4, graph, judge, noise, whole};
use oni_graph::{Graph, Image, Rect};
use oni_pipeline::{Pipeline, Request, geometry, grain};
use oni_render::cpu::{self, Grid};
use oni_render::{Grain, GrainedNegative};
use oni_settings::frame::Geometry;
use oni_settings::{ConversionMode, PipelineSettings, VirtualStock};
use std::time::Instant;

const IDENTITY: [[f32; 3]; 2] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
const NEGATIVE: GrainedNegative = GrainedNegative { bare: [0.5003, 0.2503, 0.1003], veil: 0.0003, window: [0.8, 1.0, 1.2] };

fn scaled(scale: f32) -> [[f32; 3]; 2] {
    [[1.0 / scale, 0.0, 0.0], [0.0, 1.0 / scale, 0.0]]
}

fn grainy(amount: f32) -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Digital, false).per_pixel_only();
    s.grain.amount = amount;
    s
}

fn with_amount(s: &PipelineSettings, amount: f32) -> PipelineSettings {
    let mut out = s.clone();
    out.grain.amount = amount;
    out
}

fn flat(g: &mut Graph, w: u32, h: u32, value: V4) -> Image {
    Image::new(&g.upload(w, h, &vec![value; (w * h) as usize]).unwrap())
}

/// The mean of each tile's deviation, `tile` pixels a side, of `v` laid `w` wide.
fn tiled(v: &[f32], w: usize, tile: usize) -> f32 {
    let (cols, rows) = (w / tile, v.len() / w / tile);
    let mut sum = 0.0;
    for t in 0..cols * rows {
        let (tx, ty) = (t % cols * tile, t / cols * tile);
        let cell: Vec<f32> = (0..tile * tile).map(|i| v[(ty + i / tile) * w + tx + i % tile]).collect();
        let mean = cell.iter().sum::<f32>() / cell.len() as f32;
        sum += (cell.iter().map(|x| (x - mean) * (x - mean)).sum::<f32>() / cell.len() as f32).sqrt();
    }
    sum / (cols * rows) as f32
}

/// The `n`×`n` means of a field `w` wide.
fn box_means(v: &[f32], w: usize, n: usize) -> Vec<f32> {
    let (cols, rows) = (w / n, v.len() / w / n);
    (0..cols * rows).map(|i| (0..n * n).map(|j| v[(i / cols * n + j / n) * w + i % cols * n + j % n]).sum::<f32>() / (n * n) as f32).collect()
}

#[test]
fn the_grain_matches_its_reference_on_every_branch() {
    let Some(mut g) = graph("the_grain_matches_its_reference_on_every_branch") else { return };
    let extent = Rect::new(23, 11, 61, 37);
    let mut pixels = noise(extent.w, extent.h, 21, 0.02, 1.2);
    for (i, v) in [0.0, -0.3, 1e-6, 2.0, 0.6, 0.08].into_iter().enumerate() {
        pixels[i * 97] = [v, v, v, 1.0];
    }
    let input = g.upload_at(extent, &pixels).unwrap();
    let grid = Grid::new(extent.x, extent.y, extent.w as usize, extent.h as usize, pixels.clone());
    let centre = |i: usize| [extent.x as f32 + (i % extent.w as usize) as f32 + 0.5, extent.y as f32 + (i / extent.w as usize) as f32 + 0.5];
    let scan = Rect::new(0, 0, 200, 120);
    let narrowed = |m: [[f64; 3]; 2]| m.map(|r| r.map(|v| v as f32));
    let turned = narrowed(geometry::scan_from(&Geometry { quarter_turns: 1, mirrored: true, ..Geometry::default() }, scan, 1.0));
    let angled = narrowed(geometry::scan_from(&Geometry { quarter_turns: 3, angle: 7.0, ..Geometry::default() }, scan, 0.5));
    let gaze = grain::gaze(VirtualStock::of("kodak-portra-800"));
    let cases = [
        ("a grain over many pixels, on a negative", Grain::on(IDENTITY, 4.0, 1.0, [0.03, 0.04, 0.05], gaze).on_negative(NEGATIVE)),
        ("two pixels of the scan to one, on a positive", Grain::on(scaled(0.5), 2.0, 2.0, [0.03; 3], gaze)),
        ("past the cells' bound, on silver", Grain::on(scaled(0.2), 1.5, 5.0, [0.05; 3], gaze).on_silver().on_negative(NEGATIVE)),
        ("turned and mirrored", Grain::on(turned, 3.0, 1.0, [0.04; 3], gaze).on_negative(NEGATIVE)),
        ("angled, at half", Grain::on(angled, 2.5, 2.0, [0.04; 3], gaze)),
    ];
    for (name, k) in cases {
        let k = k.seeded([0x2545_f491, 7]);
        let out = whole(&mut g, &grain::grained(Image::new(&input), &k));
        let reference: Vec<V4> = (0..pixels.len()).map(|i| cpu::grain(&grid, centre(i), &k)).collect();
        // Twin: another photo's seed.
        let twin: Vec<V4> = (0..pixels.len()).map(|i| cpu::grain(&grid, centre(i), &k.seeded([0x2545_f492, 7]))).collect();
        judge(&format!("grain, {name}"), &out, &reference, &twin);
    }
}

#[test]
fn a_reduced_render_reads_the_full_grain_averaged() {
    let Some(mut g) = graph("a_reduced_render_reads_the_full_grain_averaged") else { return };
    let (w, h) = (2048u32, 1536u32);
    let source = g.upload(w, h, &vec![[0.18, 0.18, 0.18, 1.0]; (w * h) as usize]).unwrap();
    let mut pipeline = Pipeline::new();
    let s = PipelineSettings { grain: oni_settings::blocks::Grain { amount: 0.6, size: 60.0 }, ..grainy(0.6) };
    // The pure inversion shows each share as its screen code: the grain read in the window's shares.
    let mut render = |g: &mut Graph, scale: f64| -> (Vec<f32>, usize) {
        let request = Request { scale, full_width: Some(f64::from(w)), ..Request::default() };
        let image = pipeline.image(g, &source, &s, &request, &Default::default()).unwrap();
        (whole(g, &image).iter().map(|p| cpu::screen_code(p[1])).collect(), image.extent().w as usize)
    };
    let (full, fw) = render(&mut g, 1.0);
    let k = grain::grain(&s, Rect::new(0, 0, w, h), 1.0).unwrap();
    let n_of = k.negative.unwrap();
    let film = oni_pipeline::film::shot(s.film().unwrap().stock, [0.18, 0.18, 0.18, 1.0], 0.0);
    let flat_grid = Grid::new(0, 0, 128, 128, vec![film; 128 * 128]);
    for n in [2usize, 4] {
        let (reduced, rw) = render(&mut g, 1.0 / n as f64);
        let (want, got) = (tiled(&box_means(&full, fw, n), fw / n, 16), tiled(&reduced, rw, 16));
        // Twin: each reduced pixel reading the full grain at its centre, its clouds unwidened.
        let at_reduced = grain::grain(&s, Rect::new(0, 0, w / n as u32, h / n as u32), 1.0 / n as f64).unwrap();
        let point = Grain { cloud: k.cloud, deviation: k.deviation, ..at_reduced };
        let share = |v: f32| -((v + n_of.veil) / n_of.bare[1]).log10() / n_of.window[1];
        let sampled: Vec<f32> = (0..128 * 128).map(|i| share(cpu::grain(&flat_grid, [(i % 128) as f32 + 0.5, (i / 128) as f32 + 0.5], &point)[1])).collect();
        let twin = tiled(&sampled, 128, 16);
        println!("at 1/{n}: the grain spreads {got:.5} where the full one's means spread {want:.5}; read at each centre {twin:.5}");
        assert!((got / want - 1.0).abs() < 0.15, "at 1/{n}: {got} against {want}");
        assert!((twin / want - 1.0).abs() > 0.15, "at 1/{n}: the twin spreads {twin} against {want}");
    }
}

#[test]
fn a_photo_on_portra_800_grains_as_a_real_portra_800_scan_did() {
    // A 47 Mpx frame's grid, the scan's: green's share of its window in 32-pixel tiles, on the film the scan was of, at its
    // own grain, read by the scanner.
    let s = PipelineSettings::rest(ConversionMode::Digital, true).picking(VirtualStock::of("kodak-portra-800"));
    let k = grain::grain(&s, Rect::new(0, 0, 8368, 5584), 1.0).expect("a photo opens grained");
    let n = k.negative.expect("on its negative");
    let side = 256usize;
    let grey = oni_pipeline::film::shot(s.film().unwrap().stock, [0.184, 0.184, 0.184, 1.0], 0.0);
    let grid = Grid::new(0, 0, side, side, vec![grey; side * side]);
    let share = |k: &Grain| -> Vec<f32> {
        (0..side * side).map(|i| cpu::grain(&grid, [(i % side) as f32 + 0.5, (i / side) as f32 + 0.5], k)[1]).map(|v| -((v + n.veil) / n.bare[1]).log10() / n.window[1]).collect()
    };
    let sd = tiled(&share(&k), side, 32);
    println!("Portra 800 at its rest, 30 µm on a 47 Mpx grid: {sd:.5} of green's window in 32-pixel tiles; each grain's deviation {:.4}", k.deviation[1]);
    assert!((sd / 0.014 - 1.0).abs() < 0.08, "{sd} against the scan's 0.014");
    // Twin: each layer carrying the whole Amount, as no scan shows it, lays a third more.
    let carried = Grain { deviation: k.deviation.map(|d| d / grain::READ_ALONE), ..k };
    let unread = tiled(&share(&carried), side, 32);
    assert!((unread / 0.014 - 1.0).abs() > 0.08, "twin: {unread}");
    // Twin: Amount's top on the same film lays four times the scan's grain.
    let top = grain::grain(&with_amount(&s, 1.0), Rect::new(0, 0, 8368, 5584), 1.0).unwrap();
    assert!((tiled(&share(&top), side, 32) / sd - 4.0).abs() < 0.2);
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
    let rest = grainy(0.0);
    let mut coarse = rest.clone();
    coarse.grain.size = 55.0;
    let (a, b) = (image(&rest, false), image(&coarse, false));
    assert_eq!((a.key(), a.stages()), (b.key(), b.stages()), "a size at amount 0 lays no stage");
    let grained = grainy(0.01);
    assert_ne!(image(&grained, false).key(), a.key(), "twin: the faintest grain is a stage");
    assert_eq!(image(&grained, false).stages(), a.stages() + 1, "the grain is one pass");
    assert_eq!(image(&grained, true).key(), image(&rest, true).key(), "a measure reads no grain");
    // A film mode renders no grain its panel hides, its value kept; the Digital grain above is its twin.
    let negative = PipelineSettings::rest(ConversionMode::Negative, false);
    let held = PipelineSettings { grain: grained.grain, ..negative.clone() };
    assert_eq!(image(&held, false).key(), image(&negative, false).key(), "a negative lays no grain");
}

/// The grain's pass over a whole screen's render, 3456 by 2234, at full resolution and at a fit's footprint.
#[test]
fn the_grain_over_a_whole_screen_is_timed() {
    let Some(mut g) = graph("the_grain_over_a_whole_screen_is_timed") else { return };
    let (w, h) = (3456u32, 2234u32);
    let film = flat(&mut g, w, h, [0.3, 0.2, 0.1, 1.0]);
    let time = |g: &mut Graph, image: &Image| {
        (0..4).map(|_| {
            let start = Instant::now();
            g.render(image, image.extent(), None).unwrap();
            start.elapsed().as_secs_f64() * 1000.0
        }).skip(1).fold(f64::INFINITY, f64::min)
    };
    let bare = time(&mut g, &film.clone().then(oni_pipeline::stages::exposure(0.0)));
    for (name, radius, footprint) in [("a 24 Mpx scan at 100 %", 2.52, 1.0), ("a 47 Mpx scan fitted", 3.49, 2.42)] {
        let k = Grain::on(scaled(1.0 / footprint), radius, footprint, [0.025, 0.03, 0.03], grain::gaze(VirtualStock::of("kodak-portra-800"))).on_negative(NEGATIVE);
        let ms = time(&mut g, &grain::grained(film.clone(), &k)) - bare;
        println!("the grain over a whole screen, {name}: {ms:.1} ms");
        assert!(ms < 250.0, "{name}: {ms:.1} ms");
    }
}

#[test]
fn a_notch_on_the_grain_s_amount_or_the_film_keeps_its_field_and_one_on_its_size_draws_it_again() {
    let Some(mut g) = graph("a_notch_on_the_grain_s_amount_or_the_film_keeps_its_field_and_one_on_its_size_draws_it_again") else { return };
    let (w, h) = (400u32, 300u32);
    let source = g.upload(w, h, &noise(w, h, 5, 0.02, 1.2)).unwrap();
    let mut pipeline = Pipeline::new();
    let request = Request { full_width: Some(f64::from(w)), ..Request::default() };
    let mut cache = oni_graph::Cache::frontier();
    let mut notch = |g: &mut Graph, s: &PipelineSettings| {
        let image = pipeline.image(g, &source, s, &request, &Default::default()).unwrap();
        let before = g.stats().pixels;
        let made = g.materialise_cached(&image, image.extent(), Some(&mut cache)).unwrap();
        let ran = g.stats().pixels - before;
        let fresh = g.render(&image, image.extent(), None).unwrap();
        assert_eq!(g.render(&Image::new(&made), image.extent(), None).unwrap(), fresh, "a cached notch differs from a fresh render");
        ran
    };
    let s = grainy(0.4);
    let mut lit = s.clone();
    lit.virtual_film = Some(oni_settings::VirtualFilm { exposure: 0.3, ..s.film().unwrap() });
    let frame = u64::from(w * h);
    notch(&mut g, &s);
    notch(&mut g, &with_amount(&s, 0.45));
    let amount = notch(&mut g, &with_amount(&s, 0.5));
    let film = notch(&mut g, &with_amount(&lit, 0.5));
    // Twin: a notch on the grain's size moves its lattice, its field drawn again.
    let mut sized = with_amount(&lit, 0.5);
    sized.grain.size += 5.0;
    let size = notch(&mut g, &sized);
    println!("pixels run: an Amount notch {amount}, a film notch {film}, a size notch {size}, a frame {frame}");
    assert!(size >= amount + frame && size >= film + frame, "a size notch draws the field again: {size} against {amount} and {film}");
}
