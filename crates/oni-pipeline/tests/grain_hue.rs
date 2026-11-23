//! The grain's colour is the photo's own: no hue drawn at random, a grey kept grey, the shadows' colour
//! deepened within a bound, a reduced render reading the full one's; each verdict with a twin.

mod common;

use common::{V4, graph, noise, whole};
use oni_graph::{Graph, Image, Rect};
use oni_pipeline::{conversion, grain};
use oni_render::{GRAIN_LOW_STOPS, Grain, LUMA, Stage47, cpu};
use oni_settings::migrate::grain_size::microns;
use oni_settings::blocks::GrainColour;
use oni_settings::{ConversionMode, PipelineSettings};

const SCAN: Rect = Rect::new(0, 0, 1500, 1000);
const WHITE: (f64, f64) = (0.197_83, 0.468_32);
const COLOURS: [(&str, [f32; 3]); 5] = [("skin", [1.0, 0.64, 0.43]), ("sky", [0.35, 0.55, 1.0]), ("deep blue", [0.1, 0.15, 1.0]), ("red", [1.0, 0.12, 0.08]), ("foliage", [0.3, 0.55, 0.15])];

/// A digital photo's grain at Amount's top, its curve at rest, its colour `colour`.
fn grained(colour: GrainColour) -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Digital, true);
    (s.grain.amount, s.grain.size, s.grain.colour) = (1.0, microns(0.5, 1.0), colour);
    s
}

fn luma(p: V4) -> f32 {
    p[0] * LUMA[0] + p[1] * LUMA[1] + p[2] * LUMA[2]
}

/// `c` at luminance `light`.
fn patch(c: [f32; 3], light: f32) -> V4 {
    let y = luma([c[0], c[1], c[2], 1.0]);
    [c[0] / y * light, c[1] / y * light, c[2] / y * light, 1.0]
}

/// The light whose tone, as the strength curve's axis names it, is `tone`: through the photo's film, then its
/// negative printed at rest on the film's own gains.
fn light_at(tone: f32, s: &PipelineSettings) -> f32 {
    let stock = oni_pipeline::film::stock(oni_settings::VirtualStock::of("kodak-portra-800"));
    let picture = Stage47 { gain: stock.reference.stops.map(f32::exp2), ..conversion::stage47(s) };
    let shown = |y: f32| luma(cpu::stage47(cpu::virtual_negative([y, y, y, 1.0], &stock.kernel_at(0.0), &stock.table), &picture)).max(0.0).powf(1.0 / 2.2);
    let (mut lo, mut hi) = (GRAIN_LOW_STOPS, 1.0f32);
    for _ in 0..40 {
        let mid = (lo + hi) / 2.0;
        if shown(mid.exp2()) < tone { lo = mid } else { hi = mid }
    }
    lo.exp2()
}

/// A flat `p` under `k`, `side` square.
fn laid(g: &mut Graph, k: &Grain, p: V4, side: u32) -> Vec<V4> {
    let image = grain::grained(Image::new(&g.upload(side, side, &vec![p; (side * side) as usize]).unwrap()), k, 0.0);
    whole(g, &image)
}

/// Rec. 2020 linear to CIE u'v'.
fn uv(p: V4) -> (f64, f64) {
    let [r, g, b] = [p[0], p[1], p[2]].map(|v| f64::from(v.max(0.0)));
    let x = 0.636_958 * r + 0.144_617 * g + 0.168_881 * b;
    let y = 0.2627 * r + 0.677_998 * g + 0.059_302 * b;
    let z = 0.028_073 * g + 1.060_985 * b;
    let d = x + 15.0 * y + 3.0 * z;
    (4.0 * x / d, 9.0 * y / d)
}

/// How far each pixel's colour strays across its hue line in u'v' (root mean square), how widely its hue
/// spreads in degrees, and its mean distance from the white over the patch's own.
fn measure(plain: V4, out: &[V4]) -> (f64, f64, f64) {
    let (u0, v0) = uv(plain);
    let (hu, hv) = (u0 - WHITE.0, v0 - WHITE.1);
    let c0 = hu.hypot(hv);
    let (mut across, mut sin, mut cos, mut chroma) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for p in out {
        let (u, v) = uv(*p);
        let (du, dv) = (u - u0, v - v0);
        across += if c0 > 1e-4 { (-du * hv + dv * hu) / c0 } else { du.hypot(dv) }.powi(2);
        let turn = (v - WHITE.1).atan2(u - WHITE.0) - hv.atan2(hu);
        (sin, cos) = (sin + turn.sin(), cos + turn.cos());
        chroma += (u - WHITE.0).hypot(v - WHITE.1);
    }
    let n = out.len() as f64;
    let hue = (-2.0 * (sin.hypot(cos) / n).ln()).sqrt().to_degrees();
    ((across / n).sqrt(), if c0 > 1e-4 { hue } else { 0.0 }, if c0 > 1e-4 { chroma / n / c0 } else { chroma / n })
}

/// The monochrome grain `mono` of `p` given a hue drawn at random each pixel, `colour` times its stops' spread.
fn random_hue(mono: &[V4], p: V4, colour: f32) -> Vec<V4> {
    let stops: Vec<f64> = mono.iter().map(|m| f64::from((luma(*m) / luma(p)).log2())).collect();
    let mean = stops.iter().sum::<f64>() / stops.len() as f64;
    let spread = (stops.iter().map(|a| (a - mean) * (a - mean)).sum::<f64>() / stops.len() as f64).sqrt() as f32;
    let draws = noise(mono.len() as u32, 1, 11, 0.0, 1.0);
    mono.iter().zip(draws).map(|(m, u)| {
        let n = [0, 1, 2].map(|c| (u[c] - 0.5) * 12f32.sqrt());
        let d = n.map(|v| (v - (n[0] + n[1] + n[2]) / 3.0) * 1.224_744_9);
        [m[0] * (colour * spread * d[0]).exp2(), m[1] * (colour * spread * d[1]).exp2(), m[2] * (colour * spread * d[2]).exp2(), m[3]]
    }).collect()
}

/// The colour line at `tone`.
fn colour_at(c: GrainColour, tone: f32) -> f32 {
    c.black + tone * (c.white - c.black)
}

#[test]
fn the_grain_draws_no_hue_and_leaves_a_grey_grey() {
    let Some(mut g) = graph("the_grain_draws_no_hue_and_leaves_a_grey_grey") else { return };
    let s = grained(GrainColour::REST);
    let (k, mono) = (grain::grain(&s, SCAN, 1.0).unwrap(), grain::grain(&grained(GrainColour { black: 0.0, white: 0.0 }), SCAN, 1.0).unwrap());
    let (mut across, mut hue, mut grey, mut twin) = (0.0f64, 0.0f64, 0.0f64, (0.0f64, 0.0f64));
    for tone in [0.08f32, 0.2, 0.45] {
        let light = light_at(tone, &s);
        let neutral = [light, light, light, 1.0];
        grey = grey.max(measure(neutral, &laid(&mut g, &k, neutral, 200)).0);
        for (name, c) in COLOURS {
            let p = patch(c, light);
            let m = measure(p, &laid(&mut g, &k, p, 200));
            println!("tone {tone}, {name}: across {:.4}, hue {:.2}°, chroma ×{:.3}", m.0, m.1, m.2);
            (across, hue) = (across.max(m.0), hue.max(m.1));
            // Twin: a hue drawn at random, the line's colour times the light's spread.
            let r = measure(p, &random_hue(&laid(&mut g, &mono, p, 200), p, colour_at(GrainColour::REST, tone)));
            twin = (twin.0.max(r.0), twin.1.max(r.1));
        }
    }
    println!("worst across {across:.4}, hue {hue:.2}°, grey {grey:.2e}; the twin {:.4}, {:.1}°", twin.0, twin.1);
    // The deepening's own drift, as straight lines in the channels' ratios bend in u'v': a quarter of a
    // random hue's stray, a sixteenth of its spread.
    assert!(across < 0.015 && hue < 4.0 && grey < 1e-5, "across {across:.4}, hue {hue:.2}°, grey {grey:.2e}");
    assert!(twin.0 > 0.04 && twin.1 > 30.0, "the twin passes at {twin:?}");
}

/// Skin's mean chroma in the shadows under the grain of `colour`, over its own; `symmetric` greys the
/// lighter grains as much as the law deepens the darker.
fn shadow_chroma(g: &mut Graph, colour: GrainColour, c: [f32; 3], symmetric: bool) -> f64 {
    let s = grained(colour);
    let tone = 0.2;
    let p = patch(c, light_at(tone, &s));
    if !symmetric {
        return measure(p, &laid(g, &grain::grain(&s, SCAN, 1.0).unwrap(), p, 200)).2;
    }
    let mono = laid(g, &grain::grain(&grained(GrainColour { black: 0.0, white: 0.0 }), SCAN, 1.0).unwrap(), p, 200);
    let y = luma(p);
    let out: Vec<V4> = mono.iter().map(|m| {
        let a = (luma(*m) / y).log2();
        let r = [0, 1, 2].map(|i| (p[i] / y).powf((-colour_at(colour, tone) * a).exp2()));
        let q = luma([r[0], r[1], r[2], 1.0]);
        [r[0] * luma(*m) / q, r[1] * luma(*m) / q, r[2] * luma(*m) / q, 1.0]
    }).collect();
    measure(p, &out).2
}

#[test]
fn the_shadows_colour_deepens_within_a_bound() {
    let Some(mut g) = graph("the_shadows_colour_deepens_within_a_bound") else { return };
    let skin = COLOURS[0].1;
    let rest = shadow_chroma(&mut g, GrainColour::REST, skin, false);
    let top = shadow_chroma(&mut g, GrainColour { black: GrainColour::RANGE.1, white: 0.0 }, skin, false);
    println!("skin's chroma in the shadows ×{rest:.3} at rest, ×{top:.3} at the top");
    assert!((1.05..1.35).contains(&rest) && top < 1.5, "×{rest:.3} at rest, ×{top:.3} at the top");
    // Twin: the symmetric law, its greyed lighter grains outweighing on a red.
    let twin = shadow_chroma(&mut g, GrainColour::REST, COLOURS[3].1, true);
    println!("the twin's red ×{twin:.3}");
    assert!(twin < 1.0, "the twin's red is ×{twin:.3}");
}

#[test]
fn a_reduced_render_deepens_the_colour_about_as_the_full_one_averaged() {
    let Some(mut g) = graph("a_reduced_render_deepens_the_colour_about_as_the_full_one_averaged") else { return };
    // Size 1 on a 3000-pixel scan, as the grain's own reduced check: under the cap to a quarter.
    let mut s = grained(GrainColour::REST);
    s.grain.size = microns(1.0, 1.0);
    let p = patch(COLOURS[0].1, light_at(0.2, &s));
    let at = |scale: f64| grain::grain(&s, Rect::new(0, 0, (3000.0 * scale) as u32, (2000.0 * scale) as u32), scale).unwrap();
    let full = laid(&mut g, &at(1.0), p, 800);
    let means: Vec<V4> = (0..200 * 200)
        .map(|i| {
            let block = (0..16).map(|j| full[(i / 200 * 4 + j / 4) * 800 + i % 200 * 4 + j % 4]);
            block.fold([0.0f32; 4], |m, q| [m[0] + q[0] / 16.0, m[1] + q[1] / 16.0, m[2] + q[2] / 16.0, 1.0])
        })
        .collect();
    let (whole, averaged) = (measure(p, &full).2, measure(p, &means).2);
    let quarter = measure(p, &laid(&mut g, &at(0.25), p, 200)).2;
    // Twin: a quarter laid without its colour.
    let plain = measure(p, &laid(&mut g, &at(0.25).coloured([0.0; oni_render::GRAIN_NODES]), p, 200)).2;
    println!("chroma ×{whole:.3} in full, ×{averaged:.3} its 4×4 means, ×{quarter:.3} at a quarter, the twin ×{plain:.3}");
    // A mean of the darkening is not the darkening of the mean: the quarter deepens a little less.
    assert!((quarter / averaged - 1.0).abs() < 0.05, "a quarter's ×{quarter:.3} against ×{averaged:.3}");
    assert!((plain / averaged - 1.0).abs() > 0.1, "the twin passes at ×{plain:.3}");
}
