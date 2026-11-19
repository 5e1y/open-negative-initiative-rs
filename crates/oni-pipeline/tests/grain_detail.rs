//! A coarse grain takes in the photo's detail finer than a grain: inside its clumps the scene gives way
//! to its blur, between them it stays sharp, a fine grain blurs nothing and a reduced render dissolves
//! what the full one does; each verdict with a twin the check refuses.

mod common;

use common::{V4, graph, judge, noise, whole};
use oni_graph::{Graph, Image, Rect};
use oni_pipeline::{grain, stages};
use oni_render::{GRAIN_NODES, Grain, cpu};
use oni_settings::axis::pipeline::BLUR_FLOOR;
use oni_settings::blocks::{GrainColour, GrainCurve};
use oni_settings::cg::Point;
use oni_settings::{ConversionMode, PipelineSettings};

/// The whole grain at every tone, so a check of the detail reads no curve.
const FLAT: GrainCurve = GrainCurve { points: [Point::new(0.0, 1.0), Point::new(0.25, 1.0), Point::new(0.5, 1.0), Point::new(0.75, 1.0), Point::new(1.0, 1.0)] };
/// The scan the cells are measured on: 6000 pixels wide, so Size 1's cell is four pixels.
const SCAN: (u32, u32) = (6000, 4000);
/// The detail's side, a corner of the scan.
const SIDE: u32 = 384;

fn grainy(amount: f32, size: f32) -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Digital, false).per_pixel_only();
    (s.grain.amount, s.grain.size, s.grain.curve, s.grain.colour) = (amount, size, FLAT, GrainColour { black: 0.0, white: 0.0 });
    s
}

/// The scan at `scale`.
fn scan(scale: f64) -> Rect {
    Rect::new(0, 0, (f64::from(SCAN.0) * scale) as u32, (f64::from(SCAN.1) * scale) as u32)
}

/// One-pixel dark lines every three pixels across and down on a mid grey: detail finer than any grain.
fn lines(side: u32) -> Vec<V4> {
    (0..side * side).map(|i| if (i % side) % 3 == 0 || (i / side) % 3 == 0 { [0.05, 0.05, 0.05, 1.0] } else { [0.18, 0.18, 0.18, 1.0] }).collect()
}

/// `pixels`, `side` square, grained by `s` at `scale`, its clumps taking a blur of `sigma`.
fn laid(g: &mut Graph, pixels: &[V4], side: u32, s: &PipelineSettings, scale: f64, sigma: f64) -> Vec<V4> {
    let k = grain::grain(s, scan(scale), scale).unwrap();
    let image = Image::new(&g.upload(side, side, pixels).unwrap());
    whole(g, &grain::grained(image, &k, sigma))
}

/// The pixels as their grain found them, the grain's own factor read off a flat grey divided out, and
/// that factor's stops, which are the field's.
fn bases(g: &mut Graph, out: &[V4], side: u32, s: &PipelineSettings, scale: f64) -> (Vec<f32>, Vec<f32>) {
    let grey = laid(g, &vec![[0.18, 0.18, 0.18, 1.0]; (side * side) as usize], side, s, scale, 0.0);
    out.iter().zip(&grey).map(|(p, f)| (p[0] / (f[0] / 0.18), (f[0] / 0.18).log2())).unzip()
}

/// The Laplacian's energy on the square root of `v`, `side` wide, over the pixels `kept` keeps.
fn energy(v: &[f32], side: u32, kept: &dyn Fn(usize) -> bool) -> f64 {
    let n = side as usize;
    let at = |x: usize, y: usize| f64::from(v[y * n + x].max(0.0)).sqrt();
    (1..n - 1).flat_map(|y| (1..n - 1).map(move |x| (x, y))).filter(|&(x, y)| kept(y * n + x)).map(|(x, y)| (4.0 * at(x, y) - at(x - 1, y) - at(x + 1, y) - at(x, y - 1) - at(x, y + 1)).powi(2)).sum()
}

/// The detail kept inside the clumps and between them, each a share of the scene's own.
fn detail(g: &mut Graph, s: &PipelineSettings, sigma: f64) -> (f64, f64) {
    let scene = lines(SIDE);
    let out = laid(g, &scene, SIDE, s, 1.0, sigma);
    let (base, stops) = bases(g, &out, SIDE, s, 1.0);
    let spread = (stops.iter().map(|a| f64::from(*a).powi(2)).sum::<f64>() / stops.len() as f64).sqrt() as f32;
    let plain: Vec<f32> = scene.iter().map(|p| p[0]).collect();
    let share = |kept: &dyn Fn(usize) -> bool| energy(&base, SIDE, kept) / energy(&plain, SIDE, kept);
    (share(&|i| stops[i].abs() > spread), share(&|i| stops[i].abs() < 0.5 * spread))
}

#[test]
fn a_coarse_grain_takes_in_the_detail_inside_its_clumps_and_keeps_it_between_them() {
    let Some(mut g) = graph("a_coarse_grain_takes_in_the_detail_inside_its_clumps_and_keeps_it_between_them") else { return };
    let s = grainy(0.8, 1.0);
    let sigma = grain::dissolve(&s, scan(1.0));
    let (inside, between) = detail(&mut g, &s, sigma);
    println!("size 1, amount 0.8, blur {sigma:.2} px: detail {inside:.3} inside the clumps, {between:.3} between them");
    assert!(inside <= 0.25 && between >= 0.95, "{inside} inside, {between} between");
    // Twin: the grain as it was, laying its texture over every detail.
    let (unblurred, _) = detail(&mut g, &s, 0.0);
    assert!(unblurred > 0.25, "the twin keeps {unblurred} inside the clumps");
}

#[test]
fn a_fine_grain_blurs_nothing() {
    let Some(mut g) = graph("a_fine_grain_blurs_nothing") else { return };
    let scene = lines(SIDE);
    for size in [0.0, 0.15] {
        let s = grainy(0.8, size);
        let sigma = grain::dissolve(&s, scan(1.0));
        assert!(sigma <= BLUR_FLOOR, "size {size}: a blur of {sigma}");
        assert_eq!(laid(&mut g, &scene, SIDE, &s, 1.0, sigma), laid(&mut g, &scene, SIDE, &s, 1.0, 0.0), "size {size} moves a pixel");
    }
    let s = grainy(0.8, 0.25);
    let all = |sigma: f64, g: &mut Graph| {
        let out = laid(g, &scene, SIDE, &s, 1.0, sigma);
        let (base, _) = bases(g, &out, SIDE, &s, 1.0);
        energy(&base, SIDE, &|_| true) / energy(&scene.iter().map(|p| p[0]).collect::<Vec<_>>(), SIDE, &|_| true)
    };
    let kept = all(grain::dissolve(&s, scan(1.0)), &mut g);
    // The blur is integrated over each pixel, so even its floor's deviation softens one-pixel lines a little.
    println!("size 0.25: detail {kept:.4}");
    assert!(kept >= 0.97, "size 0.25 keeps {kept}");
    // Twin: a blur of half a cell, which softens the finest grain too.
    let half_cell = 0.5 * f64::from(SCAN.0) * grain::GRAIN_FINEST * grain::GRAIN_COARSEST.powf(0.25);
    let soft = all(half_cell, &mut g);
    println!("size 0.25, the twin: detail {soft:.4}");
    assert!(soft < 0.97, "the twin keeps {soft}");
}

/// A photo's soft detail: value noise at periods of 2 to 16 pixels, each interpolated and as strong as
/// its period, about a mid grey.
fn soft(side: u32) -> Vec<V4> {
    let octaves: Vec<(u32, Vec<V4>)> = [2u32, 4, 8, 16].into_iter().map(|p| (p, noise(side / p + 2, side / p + 2, u64::from(p), -1.0, 1.0))).collect();
    (0..side * side)
        .map(|i| {
            let (x, y) = (i % side, i / side);
            let v = octaves
                .iter()
                .map(|(p, n)| {
                    let (u, w) = (x as f32 / *p as f32, y as f32 / *p as f32);
                    let (i, j, fu, fw) = (u as u32, w as u32, u.fract(), w.fract());
                    let at = |i: u32, j: u32| n[(j * (side / p + 2) + i) as usize][0];
                    let top = at(i, j) * (1.0 - fu) + at(i + 1, j) * fu;
                    let low = at(i, j + 1) * (1.0 - fu) + at(i + 1, j + 1) * fu;
                    (top * (1.0 - fw) + low * fw) * *p as f32 / 16.0
                })
                .sum::<f32>();
            let light = 0.18 * (0.6 * v).exp2();
            [light, light, light, 1.0]
        })
        .collect()
}

/// `v`'s `n`×`n` means, `side` wide.
fn box_means(v: &[V4], side: u32, n: u32) -> Vec<V4> {
    let m = side / n;
    (0..m * m)
        .map(|i| {
            let (bx, by) = (i % m, i / m);
            let sum = (0..n * n).map(|j| v[((by * n + j / n) * side + bx * n + j % n) as usize][0]).sum::<f32>();
            let mean = sum / (n * n) as f32;
            [mean, mean, mean, 1.0]
        })
        .collect()
}

/// The mean and the 99th percentile of the gap in stops between two renders.
fn stops_apart(a: &[V4], b: &[V4]) -> (f32, f32) {
    let mut gaps: Vec<f32> = a.iter().zip(b).map(|(p, q)| (p[0] / q[0]).log2().abs()).collect();
    gaps.sort_by(f32::total_cmp);
    (gaps.iter().sum::<f32>() / gaps.len() as f32, gaps[gaps.len() * 99 / 100])
}

#[test]
fn a_reduced_render_dissolves_what_the_full_one_does() {
    let Some(mut g) = graph("a_reduced_render_dissolves_what_the_full_one_does") else { return };
    let s = grainy(0.8, 1.0);
    let scene = soft(SIDE);
    let full = laid(&mut g, &scene, SIDE, &s, 1.0, grain::dissolve(&s, scan(1.0)));
    // The grain alone already parts a reduced render from the full one's means on a textured scene, as
    // the mean of a product is not the product of means: the blur may add at most 0.002 / 0.01 stop to it.
    let unblurred = laid(&mut g, &scene, SIDE, &s, 1.0, 0.0);
    for (scale, n) in [(0.5, 2u32), (0.25, 4)] {
        let reduced_scene = box_means(&scene, SIDE, n);
        let at = |g: &mut Graph, sigma: f64| laid(g, &reduced_scene, SIDE / n, &s, scale, sigma);
        let (mean, p99) = stops_apart(&at(&mut g, grain::dissolve(&s, scan(scale))), &box_means(&full, SIDE, n));
        let (was_mean, was_p99) = stops_apart(&at(&mut g, 0.0), &box_means(&unblurred, SIDE, n));
        let within = |m: f32, p: f32| m - was_mean < 0.002 && p - was_p99 < 0.01;
        println!("at {scale}: {mean:.4} / {p99:.4} stops from the full render's means, the grain alone {was_mean:.4} / {was_p99:.4}");
        assert!(within(mean, p99), "at {scale}: {mean} / {p99}");
        // Twin: the full render's blur in the reduced render's pixels.
        let (twin_mean, twin_p99) = stops_apart(&at(&mut g, grain::dissolve(&s, scan(1.0))), &box_means(&full, SIDE, n));
        println!("at {scale}: the twin {twin_mean:.4} / {twin_p99:.4}");
        assert!(!within(twin_mean, twin_p99), "at {scale}: the twin passes at {twin_mean} / {twin_p99}");
    }
}

#[test]
fn the_seeded_dissolving_grain_matches_its_reference_on_every_branch() {
    let Some(mut g) = graph("the_seeded_dissolving_grain_matches_its_reference_on_every_branch") else { return };
    let extent = Rect::new(17, 9, 64, 48);
    let mut pixels = noise(extent.w, extent.h, 31, 0.02, 1.2);
    let blurred = noise(extent.w, extent.h, 32, 0.02, 1.2);
    // Past both ends of the table, on none and under it.
    for (i, v) in [0.0, -0.3, 1e-6, 2.0, 0.6].into_iter().enumerate() {
        pixels[i * 101] = [v, v, v, 1.0];
    }
    let (input, aux) = (g.upload_at(extent, &pixels).unwrap(), g.upload_at(extent, &blurred).unwrap());
    let centre = |i: usize| [extent.x as f32 + (i % extent.w as usize) as f32 + 0.5, extent.y as f32 + (i / extent.w as usize) as f32 + 0.5];
    let mut share = [1.0; GRAIN_NODES];
    share.iter_mut().enumerate().for_each(|(i, v)| *v = (i as f32 * 0.29).sin().abs());
    // Cells of a pixel and a half, so the gate is shut, opening and open over the region.
    let k = Grain::on([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], 1.5, 1.0, 0.4, share).coloured([1.5; GRAIN_NODES]).dissolving(0.8).seeded([0x2545_f491, 0x9e37_79b9]);
    let out = whole(&mut g, &Image::new(&input).then_reading(stages::grain(&k), &[Image::new(&aux)]));
    let reference: Vec<V4> = pixels.iter().enumerate().map(|(i, p)| cpu::grain(*p, blurred[i], centre(i), &k)).collect();
    // Twins: the seed ignored, and the blur ignored.
    let unseeded: Vec<V4> = pixels.iter().enumerate().map(|(i, p)| cpu::grain(*p, blurred[i], centre(i), &k.seeded([0, 0]))).collect();
    judge("seeded grain", &out, &reference, &unseeded);
    let sharp: Vec<V4> = pixels.iter().enumerate().map(|(i, p)| cpu::grain(*p, *p, centre(i), &k)).collect();
    judge("dissolving grain", &out, &reference, &sharp);
}

