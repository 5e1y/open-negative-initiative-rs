//! Lighting's tones on a paper, in density on the negative before the enlarger: Black point and White point move the
//! density read as bare film and as the densest, Shadows and Highlights lay density by tone, each channel's mids and
//! contrast its own. Each verdict has a twin.

mod common;
#[path = "paper/frames.rs"]
mod frames;

use frames::*;
use oni_pipeline::conversion;
use oni_pipeline::negative::GRADE_BASE;
use oni_render::{Negative, Paper};
use oni_settings::frame::RollReference;
use oni_settings::tone::Print;
use oni_settings::{Id, PipelineSettings};

/// The prepared negative with its windows' mids at rest, as roll #8's frames have them.
fn negative() -> PipelineSettings {
    let mut s = prepared_negative();
    for w in [&mut s.levels.red, &mut s.levels.green, &mut s.levels.blue] {
        w.mid = 0.5;
    }
    s
}

/// The negative on a measured roll: its reference's ends as roll #8's.
fn on_roll() -> PipelineSettings {
    let mut s = negative();
    let (black, white) = ([0.362, 0.348, 0.319], [0.716, 0.754, 0.850]);
    s.roll_reference = Some(RollReference { id: Id::new(), stops: s.gains.stops, black, white, base: None, median: None });
    s
}

/// The frames, a measured roll and none: the negative on its roll, the same off a roll and a digital photo.
fn frames() -> [(&'static str, PipelineSettings); 3] {
    [("on its roll", on_roll()), ("off a roll", negative()), ("digital", digital())]
}

/// Lighting's points the checks move: the pair, each alone, one channel's own white and another's own black.
fn moves() -> Vec<(&'static str, fn(&mut Print))> {
    vec![
        ("Black point +0.05", |p| p.black_point += 0.05),
        ("White point +0.05", |p| p.white_point += 0.05),
        ("both +0.05", |p| (p.black_point, p.white_point) = (p.black_point + 0.05, p.white_point + 0.05)),
        ("both +0.15", |p| (p.black_point, p.white_point) = (p.black_point + 0.15, p.white_point + 0.15)),
        ("red's white +0.05", |p| p.channel_white[0] += 0.05),
        ("blue's black +0.05", |p| p.channel_black[2] += 0.05),
    ]
}

fn moved(s: &PipelineSettings, f: impl Fn(&mut Print)) -> PipelineSettings {
    let mut t = s.clone();
    f(&mut t.print);
    t
}

fn paper_of(s: &PipelineSettings) -> Paper {
    conversion::stage47(s).paper().expect("a paper")
}

/// The window's shares of a density `above` each channel's bare film, as the frame's window reads it.
fn sh(s: &PipelineSettings, above: [f32; 3]) -> [f32; 3] {
    oni_pipeline::paper::shares_of(s, &conversion::stage47(s), above)
}

fn negative_of(s: &PipelineSettings) -> Negative {
    conversion::stage47(s).negative.expect("Lighting on the negative")
}

/// The negative's range on each dye, as a moved Lighting reads it.
fn span_of(s: &PipelineSettings) -> [f32; 3] {
    negative_of(&moved(s, |p| p.shadows = 1e-3)).span
}

fn apart(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|c| (a[c] - b[c]).abs() / (1e-3 + b[c].abs())).fold(0.0, f32::max)
}

#[test]
fn the_points_move_the_density_read_as_bare_film_and_as_the_densest() {
    for (frame, s) in frames() {
        let rest = paper_of(&s);
        let span = span_of(&s);
        let (base, densest) = (rest.printed(sh(&s, [0.0; 3])), rest.printed(sh(&s, span)));
        for (label, f) in moves() {
            let (p, n) = (paper_of(&moved(&s, f)), negative_of(&moved(&s, f)));
            let (black, white) = (n.points_black, n.points_white);
            // Each channel's density at its black point prints as the bare film printed at rest, its white point's as the densest.
            let at_black: [f32; 3] = std::array::from_fn(|c| black[c] * span[c]);
            let at_white: [f32; 3] = std::array::from_fn(|c| (1.0 - white[c]) * span[c]);
            let (b, w) = (apart(p.printed(sh(&s, n.toned(at_black))), base), apart(p.printed(sh(&s, n.toned(at_white))), densest));
            // The paper's light is read where it is exposed: the densest prints near the paper's white, its log exposure apart.
            let exposure = |q: &Paper, a: [f32; 3]| q.exposed(q.density(sh(&s, n.toned(a))));
            let rest_at = |a: [f32; 3]| rest.exposed(rest.density(sh(&s, a)));
            let gap = |x: [f32; 3], y: [f32; 3]| (0..3).map(|c| (x[c] - y[c]).abs()).fold(0.0f32, f32::max);
            let (eb, ew) = (gap(exposure(&p, at_black), rest_at([0.0; 3])), gap(exposure(&p, at_white), rest_at(span)));
            println!("{frame}, {label}: points {black:?} {white:?}; their densities print within {b:.1e} and {w:.1e} of the rest's ends, exposed within {eb:.1e} and {ew:.1e}");
            assert!(b < 1e-5 && w < 1e-5 && eb < 1e-5 && ew < 1e-5, "{frame}, {label}");
            // Twin: the rest's paper exposes the same densities elsewhere.
            assert!(gap(rest_at(at_black), rest_at([0.0; 3])).max(gap(rest_at(at_white), rest_at(span))) > 1e-2, "{frame}, {label}");
        }
        // A global pair moves the three channels alike; a channel's own its own alone.
        let red = negative_of(&moved(&s, |p| p.channel_white[0] += 0.05));
        assert!(red.points_white[0] > 0.04 && red.points_white[1] == 0.0 && red.points_white[2] == 0.0, "{frame}");
    }
}

#[test]
fn shadows_and_highlights_lay_density_by_tone_the_ends_held() {
    for (frame, s) in frames() {
        let rest = paper_of(&s);
        let span = span_of(&s);
        let plain = |u: f32| rest.printed(sh(&s, span.map(|a| a * u)));
        let luma = |v: [f32; 3]| 0.2627 * v[0] + 0.678 * v[1] + 0.0593 * v[2];
        for (label, zones) in [("Shadows +0.2", [0.2, 0.0]), ("Highlights +0.2", [0.0, 0.2]), ("Shadows -0.25", [-0.25, 0.0]), ("Highlights -0.4", [0.0, -0.4])] {
            let t = moved(&s, |q| (q.shadows, q.highlights) = (zones[0], zones[1]));
            let (p, n) = (paper_of(&t), negative_of(&t));
            let along = |u: f32| p.printed(sh(&s, n.toned(span.map(|a| a * u))));
            let ends = apart(along(0.0), plain(0.0)).max(apart(along(1.0), plain(1.0)));
            // The density laid, as a share of the range, at a quarter and at three quarters of it.
            let laid = |u: f32| n.toned(span.map(|a| a * u))[1] / span[1] - u;
            let (low, high) = (laid(0.25), laid(0.75));
            let printed = |u: f32| luma(along(u)) / luma(plain(u));
            println!("{frame}, {label}: ends within {ends:.1e}; density laid {low:+.4} at a quarter, {high:+.4} at three quarters; the print x{:.3}, x{:.3}", printed(0.25), printed(0.75));
            assert!(ends < 1e-6, "{frame}, {label}");
            let (near, far, at) = if zones[0] != 0.0 { (low, high, 0.25) } else { (high, low, 0.75) };
            let sense = zones[0] + zones[1];
            assert!(near * sense > 0.0 && near.abs() > 2.0 * far.abs() && (printed(at) - 1.0) * sense > 0.0, "{frame}, {label}: {near} {far}");
            // One density by the mean tone: a coloured amount moves along the negative's range, each dye by the same share.
            let u = [0.2f32, 0.3, 0.5];
            let toned = n.toned(std::array::from_fn(|c| span[c] * u[c]));
            let shares: Vec<f32> = (0..3).map(|c| toned[c] / span[c] - u[c]).collect();
            assert!(shares.iter().all(|v| (v - shares[0]).abs() < 1e-6 && v.abs() > 1e-3), "{frame}, {label}: {shares:?}");
            // Twin: each channel toned by its own tone moves the three apart, a colour laid by the zones.
            let own: Vec<f32> = (0..3).map(|c| n.toned(span.map(|a| a * u[c]))[c] / span[c] - u[c]).collect();
            assert!((own[0] - own[2]).abs() > 1e-3, "{frame}, {label}: {own:?}");
        }
    }
}

#[test]
fn each_channels_mids_and_contrast_are_its_own_in_density() {
    for (frame, s) in frames() {
        let rest = paper_of(&s);
        let span = span_of(&s);
        let t = moved(&s, |q| q.channel_mid = [0.2, 0.0, 0.0]);
        let (p, n) = (paper_of(&t), negative_of(&t));
        let mid = span.map(|a| a * 0.5);
        let (lit, was) = (p.printed(sh(&s, n.toned(mid))), rest.printed(sh(&s, mid)));
        println!("{frame}, red mid +0.2: the middle {lit:?} from {was:?}");
        let held = |a: [f32; 3]| apart(p.printed(sh(&s, n.toned(a))), rest.printed(sh(&s, a))) < 1e-6;
        assert!(lit[0] > was[0] * 1.05 && held([0.0; 3]) && held(span), "{frame}");
        // Twin: the same mid on green moves green, not red.
        let green = negative_of(&moved(&s, |q| q.channel_mid = [0.0, 0.2, 0.0])).toned(mid);
        assert!((green[0] - mid[0]).abs() < 1e-7 && green[1] > mid[1], "{frame}");
        let c = negative_of(&moved(&s, |q| (q.contrast, q.channel_contrast) = (0.1, [0.2, 0.0, -0.1])));
        let expected = [0.2f32, 0.0, -0.1].map(|t| GRADE_BASE.powf(t));
        println!("{frame}, contrast 0.1 and red +0.2, blue -0.1: slopes {:?}, each layer's own {:?}", c.slope, c.own_slope);
        assert!((0..3).all(|i| (c.own_slope[i] - expected[i]).abs() < 1e-6 && (c.slope[i] - GRADE_BASE.powf(0.1)).abs() < 1e-6), "{frame}");
        // Twin: Contrast alone grades the three layers alike.
        assert!(negative_of(&moved(&s, |q| q.contrast = 0.1)).slope.iter().all(|v| (v - GRADE_BASE.powf(0.1)).abs() < 1e-6));
    }
    // Black and white reads no channel's own.
    let m = negative_of(&moved(&mono(), |q| (q.channel_mid, q.channel_contrast, q.shadows) = ([0.2; 3], [0.2; 3], 0.1)));
    assert!(m.mids == [0.0; 3] && m.own_slope == [1.0; 3]);
}
