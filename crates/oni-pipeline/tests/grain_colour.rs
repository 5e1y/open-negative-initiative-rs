//! The grain's colour against its reference, read along the unlit picture's tone, following its line from
//! the black to the white, and lifting no tone past a step the eye sees; each verdict with a twin.

mod common;

use common::{V4, graph, judge, noise, whole};
use oni_graph::{Graph, Image, Rect};
use oni_pipeline::{Inputs, Pipeline, Request, conversion, grain, stages};
use oni_render::{Grain, cpu};
use oni_settings::blocks::GrainColour;
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

/// The picture's display value of a light, its green channel encoded as the screen shows it.
fn shown(light: f32, s: &PipelineSettings) -> f32 {
    cpu::stage47([light, light, light, 1.0], &conversion::stage47(s))[1].max(0.0).powf(1.0 / 2.2)
}

const GREYS: [f32; 12] = [0.0005, 0.001, 0.002, 0.004, 0.008, 0.016, 0.03, 0.06, 0.12, 0.25, 0.5, 0.9];

#[test]
fn the_coloured_grain_matches_its_reference_on_every_branch() {
    let Some(mut g) = graph("the_coloured_grain_matches_its_reference_on_every_branch") else { return };
    let extent = Rect::new(23, 11, 61, 37);
    // Under the pedestal and past the white too, so the tone's guard, floor and ceiling are all reached.
    let pixels = noise(extent.w, extent.h, 33, -0.02, 1.2);
    let input = g.upload_at(extent, &pixels).unwrap();
    let centre = |i: usize| [extent.x as f32 + (i % extent.w as usize) as f32 + 0.5, extent.y as f32 + (i / extent.w as usize) as f32 + 0.5];
    let tone = grain::tone(&digital());
    for colour in [[2.0, 0.0], [1.5, 0.7], [0.0, 2.0], [0.0, 0.0]] {
        let k = Grain::on([[2.0, 0.0, 0.0], [0.0, 2.0, 0.0]], 1.0, 0.5, 0.4).coloured(colour, tone);
        let out = whole(&mut g, &Image::new(&input).then(stages::grain(&k)));
        let reference: Vec<V4> = pixels.iter().enumerate().map(|(i, p)| cpu::grain(*p, centre(i), &k)).collect();
        // Twins: the lattice one node over; the line read backwards, where its ends differ.
        let mut over = k;
        over.scan[0][2] += 1.0;
        let shifted: Vec<V4> = pixels.iter().enumerate().map(|(i, p)| cpu::grain(*p, centre(i), &over)).collect();
        judge(&format!("grain coloured {colour:?}, a node over"), &out, &reference, &shifted);
        if colour[0] != colour[1] {
            let backwards = k.coloured([colour[1], colour[0]], tone);
            let read: Vec<V4> = pixels.iter().enumerate().map(|(i, p)| cpu::grain(*p, centre(i), &backwards)).collect();
            judge(&format!("grain coloured {colour:?}, backwards"), &out, &reference, &read);
        }
    }
}

#[test]
fn the_tone_is_the_unlit_pictures_display_value() {
    for s in [digital(), PipelineSettings::rest(ConversionMode::Digital, false)] {
        let tone = grain::tone(&s);
        let (mut gap, mut twin) = (0.0f32, 0.0f32);
        for grey in GREYS {
            let read = cpu::grain_tone([grey; 3], &tone);
            gap = gap.max((read - shown(grey, &s)).abs());
            twin = twin.max((grey.powf(1.0 / 2.2) - shown(grey, &s)).abs());
        }
        println!("the tone is {gap:.2e} off the picture, a plain gamma {twin:.3}");
        assert!(gap < 1e-4, "the tone is {gap:.2e} off the picture");
        // Twin: the light under a plain display gamma, a tenth off in the shadows.
        assert!(twin > 0.1, "the twin is {twin} off");
    }
}

/// On a flat grey of each light, the colour's spread over the light's, both in stops, at the grey's tone.
fn colour_over_light(g: &mut Graph, colour: [f32; 2]) -> Vec<(f32, f32)> {
    let s = coloured(colour);
    let k = grain::grain(&s, Rect::new(0, 0, 1500, 1000), 1.0).unwrap();
    let (w, h) = (300u32, 200u32);
    GREYS
        .iter()
        .map(|&grey| {
            let image = Image::new(&g.upload(w, h, &vec![[grey, grey, grey, 1.0]; (w * h) as usize]).unwrap()).then(stages::grain(&k));
            let (mut light, mut colour) = (Vec::new(), Vec::new());
            for p in whole(g, &image) {
                let x = [0, 1, 2].map(|c| f64::from((p[c] / grey).log2()));
                let mean = x.iter().sum::<f64>() / 3.0;
                light.push(mean);
                colour.extend(x.map(|v| v - mean));
            }
            let spread = |v: &[f64]| {
                let m = v.iter().sum::<f64>() / v.len() as f64;
                (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / v.len() as f64).sqrt()
            };
            let ratio = spread(&colour) / spread(&light);
            (cpu::grain_tone([grey; 3], &grain::tone(&s)), ratio as f32)
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
    let measured = colour_over_light(&mut g, line);
    for (t, r) in &measured {
        println!("tone {t:.3}: colour {r:.3} times the light");
    }
    let gap = off_line(&measured, line);
    assert!(gap < 0.08, "the colour is {gap:.3} off its line");
    let white = measured.last().unwrap();
    assert!(white.0 > 0.95 && white.1 < 0.1, "near the white the colour reads {white:?}");
    // Twin: one colour on every tone.
    let flat = colour_over_light(&mut g, [1.0, 1.0]);
    let twin = off_line(&flat, line);
    assert!(twin > 0.5, "the twin is {twin:.3} off the line");
}

/// The CIE lightness of a relative luminance.
fn lightness(y: f64) -> f64 {
    if y > 216.0 / 24389.0 { 116.0 * y.cbrt() - 16.0 } else { y * 24389.0 / 27.0 }
}

/// Each grey band's lightness rise from the monochrome grain to the grain of `colour`, the whole chain at rest.
fn lifts(g: &mut Graph, colour: [f32; 2]) -> Vec<f64> {
    let (band, h) = (250u32, 120u32);
    let w = band * GREYS.len() as u32;
    let pixels: Vec<V4> = (0..w * h).map(|i| [GREYS[((i % w) / band) as usize]; 3]).map(|v| [v[0], v[1], v[2], 1.0]).collect();
    let source = g.upload(w, h, &pixels).unwrap();
    let mut pipeline = Pipeline::new();
    let request = Request { full_width: Some(f64::from(w)), ..Request::default() };
    let mut render = |g: &mut Graph, s: &PipelineSettings| {
        let image = pipeline.image(g, &source, s, &request, &Inputs::default()).unwrap();
        g.render(&image, image.extent(), None).unwrap()
    };
    let at = |colour| PipelineSettings { grain: oni_settings::blocks::Grain { amount: 0.6, ..coloured(colour).grain }, ..digital() };
    let (plain, grained) = (render(g, &at([0.0, 0.0])), render(g, &at(colour)));
    let mean_y = |out: &[V4], b: u32| {
        let px = (8..h - 8).flat_map(|y| (b * band + 8..(b + 1) * band - 8).map(move |x| (y * w + x) as usize));
        let (sum, n) = px.fold((0.0f64, 0usize), |(sum, n), i| (sum + f64::from(0.2627 * out[i][0] + 0.6780 * out[i][1] + 0.0593 * out[i][2]), n + 1));
        sum / n as f64
    };
    (0..GREYS.len() as u32).map(|b| lightness(mean_y(&grained, b)) - lightness(mean_y(&plain, b))).collect()
}

/// A lightness step about at the edge of what the eye sees on a flat area.
const SEEN: f64 = 1.5;

#[test]
fn the_default_colour_lifts_no_tone_past_a_step_the_eye_sees() {
    let Some(mut g) = graph("the_default_colour_lifts_no_tone_past_a_step_the_eye_sees") else { return };
    let rise = lifts(&mut g, [GrainColour::REST.black, GrainColour::REST.white]);
    println!("lightness rise by band: {:?}", rise.iter().map(|v| format!("{v:+.2}")).collect::<Vec<_>>());
    let most = rise.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    assert!(most < SEEN, "a band rises {most:.2} in lightness");
    // Twin: the slider's top at the black.
    let over = lifts(&mut g, [GrainColour::RANGE.1, 0.0]).iter().fold(0.0f64, |m, v| m.max(v.abs()));
    println!("the twin rises {over:.2}");
    assert!(over > SEEN, "the twin rises {over:.2}");
}
