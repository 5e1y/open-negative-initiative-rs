//! The paper's ends: whatever Lighting moves, the frame's pick prints on pure black, or where the enlarger's rest prints
//! it with Paper blacks; the whites stay free, the points move the ends, Brightness runs between the anchored pick and
//! the paper's white, the print continuous and monotone. Each verdict has a twin.

mod common;
#[path = "paper/frames.rs"]
mod frames;

use common::V4;
use frames::*;
use oni_pipeline::{conversion, paper};
use oni_render::{PAPER_CURVE_NODES, Paper, Stage47, cpu};
use oni_settings::tone::Print;
use oni_settings::{PaperStock, PipelineSettings};

fn frames() -> [(&'static str, PipelineSettings); 4] {
    let frontier = PipelineSettings { print_paper: Some(PaperStock::FujifilmFrontierSp3000), ..prepared_negative() };
    [("negative", prepared_negative()), ("digital", digital()), ("black and white", mono()), ("Frontier negative", frontier)]
}

type Move = (&'static str, fn(&mut PipelineSettings));

/// Every Lighting slider but the points, each at a visible travel either way.
const SLIDERS: [Move; 22] = [
    ("Exposure -1", |s| s.print.exposure -= 1.0),
    ("Exposure +1", |s| s.print.exposure += 1.0),
    ("Contrast -0.3", |s| s.print.contrast = -0.3),
    ("Contrast +0.3", |s| s.print.contrast = 0.3),
    ("Temperature -1", |s| s.print.set_filters(Print::TEMPERATURE_AXIS.map(|a| -a))),
    ("Temperature +1", |s| s.print.set_filters(Print::TEMPERATURE_AXIS)),
    ("Tint -1", |s| s.print.set_filters(Print::TINT_AXIS.map(|a| -a))),
    ("Tint +1", |s| s.print.set_filters(Print::TINT_AXIS)),
    ("red filter +0.3", |s| s.print.set_filters([0.3, 0.0, 0.0])),
    ("Shadows -0.2", |s| s.print.shadows = -0.2),
    ("Shadows +0.2", |s| s.print.shadows = 0.2),
    ("Highlights -0.2", |s| s.print.highlights = -0.2),
    ("Highlights +0.2", |s| s.print.highlights = 0.2),
    ("channel contrast -0.2", |s| s.print.channel_contrast = [-0.2, 0.0, 0.2]),
    ("channel contrast +0.2", |s| s.print.channel_contrast = [0.2, 0.0, -0.2]),
    ("channel mids", |s| s.print.channel_mid = [0.2, -0.2, 0.0]),
    ("Saturation -0.5", |s| s.density = -0.5),
    ("Saturation +0.5", |s| s.density = 0.5),
    ("Brightness -1", |s| s.print.brightness = -1.0),
    ("Brightness +1", |s| s.print.brightness = 1.0),
    ("Exposure +1, Contrast +0.3, Temperature +1", |s| {
        (s.print.exposure, s.print.contrast) = (s.print.exposure + 1.0, 0.3);
        s.print.set_filters(Print::TEMPERATURE_AXIS);
    }),
    ("Exposure -1, Saturation +0.5, Brightness +1", |s| (s.print.exposure, s.density, s.print.brightness) = (s.print.exposure - 1.0, 0.5, 1.0)),
];

fn moved(s: &PipelineSettings, f: impl Fn(&mut PipelineSettings)) -> PipelineSettings {
    let mut t = s.clone();
    f(&mut t);
    t
}

fn paper_of(s: &PipelineSettings) -> Paper {
    conversion::stage47(s).paper.expect("a paper")
}

/// The frame's anchors printed on `k`: its pick, its densest read and its reference grey.
fn printed_on(s: &PipelineSettings, k: &Stage47) -> [[f32; 3]; 3] {
    anchor_scans(s).map(|q| rgb(cpu::stage47(q, k)))
}

/// The same print with no ends: the paper's light at its colour density, as before the pick was anchored.
fn unanchored(k: Stage47) -> Stage47 {
    Stage47 { paper: k.paper.map(|p| Paper { anchor: p.pick, brightness: 0.0, ..p }), ..k }
}

/// The paper's own Dmax as its print shows it, at its colour density.
fn dmax(p: &Paper) -> [f32; 3] {
    let v = p.viewed(p.curves[PAPER_CURVE_NODES - 1]);
    p.coloured(if p.luma { [0.2627 * v[0] + 0.678 * v[1] + 0.0593 * v[2]; 3] } else { v })
}

fn rgb(v: V4) -> [f32; 3] {
    [v[0], v[1], v[2]]
}

fn worst(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|c| (a[c] - b[c]).abs()).fold(0.0, f32::max)
}

#[test]
fn every_lighting_slider_keeps_the_pick_where_the_rest_prints_it() {
    for (label, s) in frames() {
        let rest = printed_on(&s, &conversion::stage47(&s))[0];
        let mut lifted = 0;
        for (name, f) in SLIDERS {
            let t = moved(&s, f);
            let k = conversion::stage47(&t);
            let (now, before) = (printed_on(&s, &k)[0], printed_on(&s, &unanchored(k))[0]);
            let (held, lift) = (worst(now, rest), worst(before, rest));
            println!("{label}, {name}: the pick {now:?} from {rest:?} (off {held:.1e}); unanchored {before:?} (off {lift:.1e})");
            assert!(held < 1e-6, "{label}, {name}: {now:?} {rest:?}");
            lifted += usize::from(lift > 1e-4);
        }
        // Twin: unanchored, the time, the filters, the grade and the colour density lift or sink the pick (on black and
        // white the time and the grade); the zones and the mids lay no density on the bare film, so they leave it too.
        println!("{label}: {lifted} moves of {} lift the unanchored pick", SLIDERS.len());
        assert!(lifted >= if label == "black and white" { 6 } else { 14 }, "{label}: {lifted}");
    }
}

#[test]
fn without_paper_blacks_the_pick_lies_on_pure_black_under_every_slider() {
    for (label, s) in frames() {
        let on = moved(&s, |t| t.print.paper_blacks = false);
        let rest = printed_on(&s, &conversion::stage47(&on))[0];
        println!("{label}: pure black lays the pick at {rest:?}");
        assert!(rest.iter().all(|v| v.abs() < 1e-6), "{label}: {rest:?}");
        for (name, f) in SLIDERS {
            let pick = printed_on(&s, &conversion::stage47(&moved(&on, f)))[0];
            assert!(pick.iter().all(|v| v.abs() < 1e-6), "{label}, {name}: {pick:?}");
        }
        // Twin: the paper's Dmax laid on black, its white kept, leaves the pick above it.
        let p = paper_of(&s);
        let (d, w) = (dmax(&p), p.white);
        let printed = printed_on(&s, &unanchored(conversion::stage47(&s)))[0];
        let on_dmax: [f32; 3] = std::array::from_fn(|c| w[c] * (printed[c] - d[c]) / (w[c] - d[c]));
        println!("{label}: on the Dmax's black the pick would print at {on_dmax:?} (Dmax {d:?})");
        assert!(on_dmax.iter().any(|v| *v > 1e-3), "{label}: {on_dmax:?}");
    }
}

#[test]
fn the_whites_stay_free() {
    let mut seen = 0;
    for (label, s) in frames() {
        let [_, rest_white, _] = printed_on(&s, &conversion::stage47(&s));
        for stops in [-1.0f32, 1.0] {
            let k = conversion::stage47(&moved(&s, |t| t.print.exposure += stops));
            let (white, free) = (printed_on(&s, &k)[1], printed_on(&s, &unanchored(k))[1]);
            let (moved_by, free_by, apart) = (worst(white, rest_white), worst(free, rest_white), worst(white, free));
            println!("{label}, Exposure {stops:+}: the densest read {white:?} from {rest_white:?} (moved {moved_by:.1e}), unanchored {free:?} ({free_by:.1e}), apart {apart:.1e}");
            // The densest read moves as the enlarger moves it, unanchored, within a hundredth of that move.
            assert!(apart <= 0.01 * free_by + 2e-5, "{label}, {stops}: {apart} {free_by}");
            // Twin: a white anchored too stays where the rest prints it, off the free move by all of it, where it moves.
            if free_by > 1e-3 {
                assert!(worst(rest_white, free) > 0.01 * free_by + 2e-5, "{label}, {stops}");
                seen += 1;
            }
        }
    }
    // Black and white's and a digital photo's densest read sit at the paper's white either way, the negatives' darken.
    assert!(seen >= 3, "{seen}");
}

#[test]
fn brightness_runs_between_the_anchored_pick_and_the_papers_white() {
    for (label, s) in frames() {
        let [rest_pick, _, rest_grey] = printed_on(&s, &conversion::stage47(&s));
        for (brightness, pure) in [(-1.0f32, false), (1.0, false), (0.5, true), (-1.0, true)] {
            let t = moved(&s, |u| (u.print.brightness, u.print.paper_blacks) = (brightness, !pure));
            let k = conversion::stage47(&t);
            let p = k.paper.unwrap();
            let [pick, _, grey] = printed_on(&s, &k);
            let black = if pure { [0.0; 3] } else { rest_pick };
            assert!(worst(pick, black) < 1e-6 && worst(p.ended(p.white), p.white) < 1e-6, "{label}, {brightness}: ends held");
            let g = (Print::EXPOSURE_GEAR * brightness).exp2();
            let odds = |u: f32| g * u / (1.0 + (g - 1.0) * u);
            let law: [f32; 3] = std::array::from_fn(|c| black[c] + (p.white[c] - black[c]) * odds((rest_grey[c] - p.pick[c]) / (p.white[c] - p.pick[c])));
            let off = worst(grey, law);
            println!("{label}, Brightness {brightness:+}{}: the grey {:.4} to {:.4}, within {off:.1e} of its curve", if pure { ", pure black" } else { "" }, rest_grey[1], grey[1]);
            assert!(off < 2e-6, "{label}, {brightness}: {grey:?} {law:?}");
            // Twin: the curve run between the paper's Dmax and white misses the grey.
            let d = dmax(&p);
            let floor = if pure { [0.0; 3] } else { d };
            let on_dmax: [f32; 3] = std::array::from_fn(|c| floor[c] + (p.white[c] - floor[c]) * odds((rest_grey[c] - d[c]) / (p.white[c] - d[c])));
            println!("{label}, Brightness {brightness:+}: run between the Dmax and the white, the grey would miss by {:.1e}", worst(grey, on_dmax));
            assert!(worst(grey, on_dmax) > 1e-4, "{label}, {brightness}: {on_dmax:?}");
        }
    }
}

#[test]
fn the_points_still_move_the_ends() {
    for (label, s) in frames() {
        let [rest_pick, rest_white, _] = printed_on(&s, &conversion::stage47(&s));
        for (name, black, white) in [("black point +0.05", 0.05f32, 0.0f32), ("black point -0.05", -0.05, 0.0), ("white point +0.05", 0.0, 0.05)] {
            let t = moved(&s, |u| (u.print.black_point, u.print.white_point) = (black, white));
            let k = conversion::stage47(&t);
            let [pick, densest, _] = printed_on(&s, &k);
            let (p, at_rest) = (k.paper.unwrap(), paper_of(&s));
            let (high, rest_high) = (p.shown(p.span.map(|a| a * 0.8)), at_rest.shown(at_rest.span.map(|a| a * 0.8)));
            let (sank, lightened) = (worst(pick, rest_pick), worst(high, rest_high));
            println!("{label}, {name}: the pick {pick:?} from {rest_pick:?}, the densest read {densest:?} from {rest_white:?}, 0.8 of the range {high:?} from {rest_high:?}");
            if black != 0.0 {
                // A black point crushes the pick below where the rest prints it; a negative one lifts it.
                assert!(sank > 1e-4 && (0..3).all(|c| (pick[c] < rest_pick[c]) == (black > 0.0)), "{label}, {name}");
                // Twin: the pick anchored through the points stays where the rest prints it.
                let through = Paper { pick: p.coloured(p.printed(paper::anchors(&t, k.dyes.as_ref())[0])), ..p };
                let held = rgb(cpu::stage47(anchor_scans(&s)[0], &Stage47 { paper: Some(through), ..k }));
                assert!(worst(held, rest_pick) < 1e-6, "{label}, {name}: {held:?}");
            } else {
                // A white point lightens the upper range toward the paper's white, the pick held.
                assert!(sank < 1e-6 && lightened > 1e-4 && (0..3).all(|c| high[c] > rest_high[c] && densest[c] >= rest_white[c]), "{label}, {name}");
            }
        }
    }
}

#[test]
fn the_print_is_continuous_and_monotone_from_past_the_pick_to_past_the_white() {
    let settings: [Move; 4] = [
        ("at rest", |_| {}),
        ("pure black", |s| s.print.paper_blacks = false),
        ("Exposure +2, Brightness -1", |s| (s.print.exposure, s.print.brightness) = (s.print.exposure + 2.0, -1.0)),
        ("Exposure -1, Contrast +0.3, pure black, Brightness +1", |s| {
            (s.print.exposure, s.print.contrast, s.print.paper_blacks, s.print.brightness) = (s.print.exposure - 1.0, 0.3, false, 1.0);
        }),
    ];
    for (label, s) in frames() {
        for (name, f) in settings {
            let p = paper_of(&moved(&s, f));
            let print = |u: f32| p.shown(p.span.map(|a| a * u));
            let (coarse, fine) = (steps(&print, 2000), steps(&print, 4000));
            let (falls_by, lowest) = (fall(&print, 4000), least(&print, 4000));
            println!("{label}, {name}: the largest step {coarse:.2e} over 2000 tones, {fine:.2e} over 4000; its largest fall {falls_by:.1e}, its least light {lowest:.1e}");
            assert!(fine < 0.6 * coarse, "{label}, {name}");
            // Monotone and at or above black, but for f32's step near the white times Brightness's slope there; the
            // Frontier's fit falls 6.5e-6 near its white at rest and prints its Dmax a hair under no light.
            let (falls, floor) = if label.starts_with("Frontier") { (3e-5, -2e-5) } else { (1e-6, 0.0) };
            assert!(falls_by < falls && lowest >= floor, "{label}, {name}");
            // Twin: a jump of a hundredth past the middle keeps its step whatever the sampling, so the check refuses it.
            let jump = |u: f32| print(u).map(|v| if u > 0.5 { v + 0.01 } else { v });
            assert!(steps(&jump, 4000) >= 0.6 * steps(&jump, 2000), "{label}, {name}");
            // Twin: a fall of a hundredth is refused.
            let dip = |u: f32| print(u).map(|v| if u > 0.5 { v - 0.01 } else { v });
            assert!(fall(&dip, 4000) > 1e-3, "{label}, {name}");
        }
    }
}

/// The tones of `print` across the negative's range and past both its ends, `n` of them.
fn tones(print: &dyn Fn(f32) -> [f32; 3], n: usize) -> Vec<[f32; 3]> {
    (0..=n).map(|i| print(-1.0 + 3.0 * i as f32 / n as f32)).collect()
}

/// The largest rise of `print` between two neighbours of `n` tones.
fn steps(print: &dyn Fn(f32) -> [f32; 3], n: usize) -> f32 {
    tones(print, n).windows(2).flat_map(|w| (0..3).map(move |c| w[1][c] - w[0][c])).fold(0.0, f32::max)
}

/// The largest fall of `print` between two neighbours of `n` tones.
fn fall(print: &dyn Fn(f32) -> [f32; 3], n: usize) -> f32 {
    tones(print, n).windows(2).flat_map(|w| (0..3).map(move |c| w[0][c] - w[1][c])).fold(0.0, f32::max)
}

/// The least light of `print` over `n` tones.
fn least(print: &dyn Fn(f32) -> [f32; 3], n: usize) -> f32 {
    tones(print, n).iter().flatten().copied().fold(f32::INFINITY, f32::min)
}
