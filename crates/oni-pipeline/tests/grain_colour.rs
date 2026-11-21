//! The grain's colour and share against their reference, both read on the strength curve's tone, a
//! print notch or a key keeping them, the colour following its line and lifting no tone the eye sees.

mod common;

use common::{V4, graph, judge, noise, whole};
use oni_graph::{Graph, Image, Rect, Stage};
use oni_pipeline::{conversion, grain, scanner, stages};
use oni_render::{GRAIN_NODES, Grain, LUMA, Stage47, cpu, grain_node_light};
use oni_settings::blocks::{GrainColour, GrainCurve};
use oni_settings::cg::Point;
use oni_settings::{ConversionMode, PipelineSettings};

fn digital() -> PipelineSettings {
    PipelineSettings::rest(ConversionMode::Digital, true)
}

fn coloured(colour: [f32; 2]) -> PipelineSettings {
    let mut s = digital();
    (s.grain.amount, s.grain.size) = (1.0, 0.5);
    s.grain.colour = GrainColour { black: colour[0], white: colour[1] };
    s
}

/// A curve of `y` at every tone, or of the tone itself with `None`.
fn even(y: Option<f64>) -> GrainCurve {
    GrainCurve { points: [0.0, 0.25, 0.5, 0.75, 1.0].map(|x| Point::new(x, y.unwrap_or(x))) }
}

/// The tone the strength curve's axis names for a light: its display luminance through the photo's film, then
/// its negative printed at rest on the film's own gains.
fn shown(light: f32, s: &PipelineSettings) -> f32 {
    let stock = oni_pipeline::film::stock(oni_settings::VirtualStock::KodakPortra800);
    let picture = Stage47 { gain: stock.reference.stops.map(f32::exp2), ..conversion::stage47(s) };
    let p = cpu::stage47(cpu::virtual_negative([light, light, light, 1.0], &stock.kernel_at(0.0), &stock.table), &picture);
    (p[0] * LUMA[0] + p[1] * LUMA[1] + p[2] * LUMA[2]).max(0.0).powf(1.0 / 2.2)
}

const GREYS: [f32; 12] = [0.0005, 0.001, 0.002, 0.004, 0.008, 0.016, 0.03, 0.06, 0.12, 0.25, 0.5, 0.9];
const SCAN: Rect = Rect::new(0, 0, 1500, 1000);

#[test]
fn the_coloured_grain_matches_its_reference_on_every_branch() {
    let Some(mut g) = graph("the_coloured_grain_matches_its_reference_on_every_branch") else { return };
    let extent = Rect::new(23, 11, 61, 37);
    // Under the pedestal and past the table's last node, so both its ends and a light of no share are reached.
    let mut pixels = noise(extent.w, extent.h, 33, -0.02, 2.5);
    // A row of a zero channel, one under zero, a neutral grey and a black.
    for (row, p) in [[0.04, 0.0, 0.02, 1.0], [0.04, -0.003, 0.02, 1.0], [0.03, 0.03, 0.03, 1.0], [0.0, 0.0, 0.0, 1.0]].into_iter().enumerate() {
        pixels[row * extent.w as usize..(row + 1) * extent.w as usize].fill(p);
    }
    let input = g.upload_at(extent, &pixels).unwrap();
    let centre = |i: usize| [extent.x as f32 + (i % extent.w as usize) as f32 + 0.5, extent.y as f32 + (i / extent.w as usize) as f32 + 0.5];
    let tones = grain::tones(&Stage47 { gain: [1.0; 3], ..conversion::stage47(&digital()) });
    for (colour, curve) in [([2.0, 0.0], GrainCurve::REST), ([1.5, 0.7], even(Some(1.0))), ([0.0, 2.0], GrainCurve::REST), ([0.0, 0.0], GrainCurve::REST)] {
        let s = PipelineSettings { grain: oni_settings::blocks::Grain { curve, ..coloured(colour).grain }, ..digital() };
        let laid = grain::grain(&s, SCAN, 1.0).unwrap();
        let k = Grain::on([[2.0, 0.0, 0.0], [0.0, 2.0, 0.0]], 1.0, 0.5, 0.4, laid.share).coloured(laid.colour);
        let out = whole(&mut g, &grain::grained(Image::new(&input), &k, 0.0));
        let reference: Vec<V4> = pixels.iter().enumerate().map(|(i, p)| cpu::grain(*p, *p, centre(i), &k)).collect();
        // Twins: the lattice one node over; the line read backwards, where its ends differ.
        let mut over = k;
        over.scan[0][2] += 1.0;
        let shifted: Vec<V4> = pixels.iter().enumerate().map(|(i, p)| cpu::grain(*p, *p, centre(i), &over)).collect();
        judge(&format!("grain coloured {colour:?}, a node over"), &out, &reference, &shifted);
        if colour[0] != colour[1] {
            let backwards = k.coloured(grain::colours(&GrainColour { black: colour[1], white: colour[0] }, &tones));
            let read: Vec<V4> = pixels.iter().enumerate().map(|(i, p)| cpu::grain(*p, *p, centre(i), &backwards)).collect();
            judge(&format!("grain coloured {colour:?}, backwards"), &out, &reference, &read);
        }
    }
}

#[test]
fn the_colour_and_the_share_read_one_tone_the_strength_curve_s() {
    for s in [digital(), PipelineSettings::rest(ConversionMode::Digital, false)] {
        let t = PipelineSettings { grain: oni_settings::blocks::Grain { amount: 0.5, curve: even(None), colour: GrainColour { black: 0.0, white: 1.0 }, ..s.grain }, ..s.clone() };
        let k = grain::grain(&t, SCAN, 1.0).unwrap();
        let (mut gap, mut apart, mut twin) = (0.0f32, 0.0f32, 0.0f32);
        for i in 0..GRAIN_NODES {
            let light = grain_node_light(i);
            gap = gap.max((k.colour[i] - shown(light, &s).min(1.0)).abs());
            apart = apart.max((k.colour[i] - k.share[i]).abs());
            twin = twin.max((light.powf(1.0 / 2.2).min(1.0) - shown(light, &s).min(1.0)).abs());
        }
        println!("the colour's tone is {gap:.2e} off the picture, {apart:.2e} off the share's, a plain gamma {twin:.3}");
        assert!(gap < 1e-4 && apart < 1e-4, "the colour's tone is {gap:.2e} off the picture, {apart:.2e} off the share's");
        // Twin: the light under a plain display gamma, a tenth off in the shadows.
        assert!(twin > 0.1, "the twin is {twin} off");
    }
}

fn luma(p: V4) -> f32 {
    p[0] * LUMA[0] + p[1] * LUMA[1] + p[2] * LUMA[2]
}

/// A skin tone of luminance `light`.
fn skin(light: f32) -> V4 {
    let c = [1.0, 0.64, 0.43];
    let y = luma([c[0], c[1], c[2], 1.0]);
    [c[0] / y * light, c[1] / y * light, c[2] / y * light, 1.0]
}

/// A flat `patch` under `k`, 300 by 200.
fn laid(g: &mut Graph, k: &Grain, patch: V4) -> Vec<V4> {
    let (w, h) = (300u32, 200u32);
    let image = grain::grained(Image::new(&g.upload(w, h, &vec![patch; (w * h) as usize]).unwrap()), k, 0.0);
    whole(g, &image)
}

/// On a flat skin of each light, the colour read off the law, log2 of the power its red over its blue
/// took, over the stops the grain darkened it, at the skin's tone.
fn colour_by_tone(g: &mut Graph, colour: [f32; 2]) -> Vec<(f32, f32)> {
    let mut s = coloured(colour);
    // The whole grain on every light, so each has stops to read the colour against.
    s.grain.curve = even(Some(1.0));
    let k = grain::grain(&s, SCAN, 1.0).unwrap();
    let red_over_blue = |p: V4| f64::from((p[0] / p[2]).log2());
    GREYS
        .iter()
        .map(|&light| {
            let patch = skin(light);
            let (mut sum, mut n) = (0.0f64, 0usize);
            for p in laid(g, &k, patch) {
                let darker = -f64::from((luma(p) / light).log2());
                if darker > 0.05 {
                    sum += (red_over_blue(p) / red_over_blue(patch)).log2() / darker;
                    n += 1;
                }
            }
            (shown(light, &s).min(1.0), (sum / n.max(1) as f64) as f32)
        })
        .collect()
}

/// The largest gap between the measured colour and the line from `colour[0]` at the black to `colour[1]`.
fn off_line(measured: &[(f32, f32)], colour: [f32; 2]) -> f32 {
    measured.iter().map(|(t, r)| (r - (colour[0] + t * (colour[1] - colour[0]))).abs()).fold(0.0, f32::max)
}

#[test]
fn the_colour_follows_its_line_from_the_black_to_a_neutral_white() {
    let Some(mut g) = graph("the_colour_follows_its_line_from_the_black_to_a_neutral_white") else { return };
    let line = [GrainColour::REST.black, GrainColour::REST.white];
    let measured = colour_by_tone(&mut g, line);
    for (t, r) in &measured {
        println!("tone {t:.3}: colour {r:.3}");
    }
    let gap = off_line(&measured, line);
    assert!(gap < 0.08, "the colour is {gap:.3} off its line");
    let white = measured.last().unwrap();
    // The film's shoulder holds its brightest grey under the white: there the colour has fallen most of the way.
    assert!(white.0 > 0.8 && white.1 < 0.3 * measured[0].1, "near the white the colour reads {white:?}");
    // Twin: one colour on every tone.
    let flat = colour_by_tone(&mut g, [1.0, 1.0]);
    let twin = off_line(&flat, line);
    assert!(twin > 0.5, "the twin is {twin:.3} off the line");
}

/// The CIE lightness of a relative luminance.
fn lightness(y: f64) -> f64 {
    if y > 216.0 / 24389.0 { 116.0 * y.cbrt() - 16.0 } else { y * 24389.0 / 27.0 }
}

/// The lightness rise, through the unlit picture, from the monochrome grain to the grain of `colour` on
/// a flat `patch`; `random`, a hue drawn at random each pixel at the line's black, as a coloured noise lays.
fn lift(g: &mut Graph, patch: V4, colour: [f32; 2], random: bool) -> f64 {
    let at = |colour: [f32; 2]| PipelineSettings { grain: oni_settings::blocks::Grain { amount: 0.6, ..coloured(colour).grain }, ..digital() };
    let s = at(colour);
    let plain = laid(g, &grain::grain(&at([0.0, 0.0]), SCAN, 1.0).unwrap(), patch);
    let grained = if random {
        let stops: Vec<f64> = plain.iter().map(|m| f64::from((luma(*m) / luma(patch)).log2())).collect();
        let mean = stops.iter().sum::<f64>() / stops.len() as f64;
        let spread = (stops.iter().map(|a| (a - mean) * (a - mean)).sum::<f64>() / stops.len() as f64).sqrt() as f32;
        let draws = noise(300, 200, 7, 0.0, 1.0);
        plain.iter().zip(draws).map(|(m, u)| {
            let n = [0, 1, 2].map(|c| (u[c] - 0.5) * 12f32.sqrt());
            let d = n.map(|v| (v - (n[0] + n[1] + n[2]) / 3.0) * 1.224_744_9);
            [m[0] * (colour[0] * spread * d[0]).exp2(), m[1] * (colour[0] * spread * d[1]).exp2(), m[2] * (colour[0] * spread * d[2]).exp2(), 1.0]
        }).collect()
    } else {
        laid(g, &grain::grain(&s, SCAN, 1.0).unwrap(), patch)
    };
    let picture = conversion::stage47(&s);
    let shown_y = |out: &[V4]| lightness(out.iter().map(|p| f64::from(luma(cpu::stage47(*p, &picture)))).sum::<f64>() / out.len() as f64);
    shown_y(&grained) - shown_y(&plain)
}

/// A lightness step about at the edge of what the eye sees on a flat area.
const SEEN: f64 = 1.5;

#[test]
fn the_default_colour_lifts_no_grey_and_no_colour_past_a_step_the_eye_sees() {
    let Some(mut g) = graph("the_default_colour_lifts_no_grey_and_no_colour_past_a_step_the_eye_sees") else { return };
    let rest = [GrainColour::REST.black, GrainColour::REST.white];
    let greys: Vec<f64> = GREYS.iter().map(|&v| lift(&mut g, [v, v, v, 1.0], rest, false)).collect();
    println!("lightness rise by grey: {:?}", greys.iter().map(|v| format!("{v:+.1e}")).collect::<Vec<_>>());
    assert!(greys.iter().all(|v| v.abs() < 1e-3), "a grey rises {greys:?}");
    // Twin: a hue drawn at random lifts a grey.
    let random = GREYS.iter().map(|&v| lift(&mut g, [v, v, v, 1.0], rest, true)).fold(0.0f64, f64::max);
    assert!(random > 1e-3, "the twin lifts a grey {random:.2e}");
    let colours = [[0.004, 0.0005, 0.0003, 1.0], [0.0004, 0.0006, 0.004, 1.0], [0.003, 0.0055, 0.0015, 1.0], skin(0.004)];
    let most = colours.iter().map(|&p| lift(&mut g, p, [GrainColour::RANGE.1, 0.0], false).abs()).fold(0.0f64, f64::max);
    println!("a dark colour rises {most:.2} at the top");
    assert!(most < SEEN, "a colour rises {most:.2} in lightness");
    // Twin: a hue drawn at random at the slider's top.
    let over = GREYS.iter().map(|&v| lift(&mut g, [v, v, v, 1.0], [GrainColour::RANGE.1, 0.0], true)).fold(0.0f64, f64::max);
    println!("the twin rises {over:.2}");
    assert!(over > SEEN, "the twin rises {over:.2}");
}

#[test]
fn a_print_notch_keeps_the_grain_s_shares_and_a_curve_notch_moves_them() {
    let mut s = PipelineSettings::rest(ConversionMode::Digital, true);
    s.grain.amount = 0.5;
    let scan = Rect::new(0, 0, 600, 400);
    let k = grain::grain(&s, scan, 1.0).unwrap();
    let notches: [fn(&mut PipelineSettings); 3] = [|t| t.print.exposure += 0.3, |t| t.print.contrast -= 0.2, |t| t.print.shadows += 0.1];
    for notch in notches {
        let mut t = s.clone();
        notch(&mut t);
        assert_eq!(stages::grain(&grain::grain(&t, scan, 1.0).unwrap()).params(), stages::grain(&k).params(), "a print notch moves the grain's pass");
    }
    // Twin: a notch of the curve moves them.
    let mut t = s.clone();
    t.grain.curve.move_point(1, Point::new(0.3, 0.9));
    assert_ne!(grain::grain(&t, scan, 1.0).unwrap().share, k.share);
}

#[test]
fn a_key_notch_on_the_scanner_face_keeps_the_grain_s_shares() {
    let mut s = PipelineSettings::rest(ConversionMode::Digital, true);
    s.grain.amount = 0.5;
    s.face = oni_settings::Face::Scanner;
    let scan = Rect::new(0, 0, 600, 400);
    let machine = scanner::on_oni_machine(&s).unwrap();
    let k = grain::grain(&machine, scan, 1.0).unwrap();
    let mut t = s.clone();
    t.scanner = oni_settings::scanner::ScannerClicks::new(4, 2, -2, 6, 0);
    let keyed = scanner::on_oni_machine(&t).unwrap();
    assert_eq!(stages::grain(&grain::grain(&keyed, scan, 1.0).unwrap()).params(), stages::grain(&k).params(), "a key notch moves the grain's pass");
    // Twin: the tones read through the keyed picture move the shares.
    assert_ne!(grain::shares(&machine.grain.curve, &grain::tones(&conversion::stage47(&keyed))), k.share);
}
