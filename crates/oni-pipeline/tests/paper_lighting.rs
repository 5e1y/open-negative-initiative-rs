//! Lighting on the paper in density: Exposure a stop of the enlarger's light, ungeared; Temperature and Tint its filtration,
//! 30.1 CC a stop, the grey's luma held and the ends left where the paper prints them; Contrast the paper's grade about
//! the grey; no colour compensation on the paper; ONI's print to the bit. Each verdict has a twin.

mod common;
#[path = "paper/frames.rs"]
mod frames;

use common::{V4, noise};
use frames::*;
use oni_pipeline::paper::{self, CC_PER_STOP, GRADE_BASE};
use oni_pipeline::{conversion, film, finishing};
use oni_render::{Paper, Stage47, cpu};
use oni_settings::tone::Print;
use oni_settings::{PipelineSettings, VirtualStock};

/// The frames Lighting is read on.
fn frames() -> [(&'static str, PipelineSettings); 3] {
    [("negative", prepared_negative()), ("digital", digital()), ("black and white", mono())]
}

fn moved(s: &PipelineSettings, f: impl Fn(&mut Print)) -> PipelineSettings {
    let mut t = s.clone();
    f(&mut t.print);
    t
}

fn paper_of(s: &PipelineSettings) -> Paper {
    conversion::stage47(s).paper.expect("a paper")
}

fn luma(v: [f32; 3]) -> f32 {
    0.2627 * v[0] + 0.678 * v[1] + 0.0593 * v[2]
}

#[test]
fn exposure_is_a_stop_of_the_enlargers_light_ungeared() {
    for (label, s) in frames() {
        let rest = paper_of(&s);
        let anchors = paper::anchors(&s, conversion::stage47(&s).dyes.as_ref());
        let [black, _, grey] = lit(&s, |_| {});
        for stops in [-1.0f32, 1.0, 2.5] {
            let t = moved(&s, |p| p.exposure += stops);
            let p = paper_of(&t);
            let shift: Vec<f32> = anchors.iter().flat_map(|a| (0..3).map(move |c| (a, c))).map(|(a, c)| p.exposed(*a)[c] - rest.exposed(*a)[c]).collect();
            let off = shift.iter().map(|d| (d + stops * std::f32::consts::LOG10_2).abs()).fold(0.0f32, f32::max);
            let [b, _, g] = lit(&s, |u| u.print.exposure += stops);
            let (lifted, was, dmax) = (luma(rgb(b)), luma(rgb(black)), luma(p.dmax));
            let (step, now, rest_grey) = (shift[0], luma(rgb(g)), luma(rgb(grey)));
            println!("{label}, Exposure {stops:+}: log exposure {step:+.5} (off a stop's by {off:.1e}), black {lifted:.5} from {was:.5} (Dmax {dmax:.5}), grey {now:.4} from {rest_grey:.4}");
            // A stop on the slider is a stop less light on the paper, every layer, every tone.
            assert!(off < 2e-6, "{label}, {stops}: {shift:?}");
            // Brightening lifts the base off the paper's Dmax; darkening takes it toward the Dmax, never past.
            if stops > 0.0 {
                assert!(luma(rgb(b)) > luma(rgb(black)) * 1.2 && luma(rgb(g)) > luma(rgb(grey)), "{label}, {stops}");
            } else {
                assert!(luma(rgb(b)) < luma(rgb(black)) && (0..3).all(|c| b[c] >= p.dmax[c] - 1e-6), "{label}, {stops}");
            }
            // Twin: ONI's gear, twice the stops, moves the paper two stops.
            let geared = Paper { scale: rest.scale * (-Print::EXPOSURE_GEAR * stops).exp2(), ..rest };
            assert!((geared.exposed(anchors[2])[1] - p.exposed(anchors[2])[1]).abs() > 0.29, "{label}, {stops}");
        }
        // Twin: ONI's window, laid on each print's own base and densest read, holds the base on black either way, so the
        // lift the check reads is gone.
        let windowed = |[b, _, w]: [V4; 3]| luma(std::array::from_fn(|c| (b[c] - b[c]) / (w[c] - b[c])));
        let (up, at_rest) = (windowed(lit(&s, |u| u.print.exposure += 1.0)), windowed(lit(&s, |_| {})));
        assert!(up <= at_rest * 1.2 + 1e-9, "{label}: {up} {at_rest}");
    }
}

fn rgb(v: V4) -> [f32; 3] {
    [v[0], v[1], v[2]]
}

/// Temperature and Tint at half and full travel either way, as the filters they write.
fn warmths() -> Vec<(String, [f32; 3])> {
    let mut out = Vec::new();
    for (name, axis, range) in [("Temperature", Print::TEMPERATURE_AXIS, Print::TEMPERATURE_RANGE.1), ("Tint", Print::TINT_AXIS, Print::TINT_RANGE.1)] {
        for share in [-1.0, -0.5, 0.5, 1.0] {
            out.push((format!("{name} {share:+}"), axis.map(|a| a * share * range)));
        }
    }
    out
}

#[test]
fn temperature_and_tint_move_the_enlargers_filtration_and_hold_the_greys_luma() {
    let s = prepared_negative();
    let rest = conversion::printing(&s).unwrap();
    let [rest_black, rest_white, rest_grey] = lit(&s, |_| {});
    for (label, f) in warmths() {
        let t = moved(&s, |p| p.set_filters(f));
        let now = conversion::printing(&t).unwrap();
        let expected: [f32; 3] = std::array::from_fn(|c| rest.filtration[c] + CC_PER_STOP * f[c]);
        let least = expected.iter().copied().fold(f32::INFINITY, f32::min);
        let off = (0..3).map(|c| (now.filtration[c] - (expected[c] - least)).abs()).fold(0.0f32, f32::max);
        let [black, white, grey] = lit(&s, |u| u.print.set_filters(f));
        println!("{label}: filtration {:?} from {:?}; grey {grey:?} (luma {:.6} from {:.6}); base {black:?}, white {white:?}", now.filtration, rest.filtration, luma(rgb(grey)), luma(rgb(rest_grey)));
        assert!(off < 1e-4 && now.filtration.contains(&0.0), "{label}: 30.1 CC a stop, the least at none");
        assert!((luma(rgb(grey)) / luma(rgb(rest_grey)) - 1.0).abs() < 1e-5, "{label}: the grey's luma held");
        // Toned the filters' way: warmer red against blue, a magenta tint green below red and blue.
        let axes = |v: V4| ((v[0] / v[2]).ln(), ((v[0] * v[2]).sqrt() / v[1]).ln());
        let ((warmth, tint), (was_warmth, was_tint)) = (axes(grey), axes(rest_grey));
        let (dt, dn) = (f[0] - f[2], (f[0] + f[2]) / 2.0 - f[1]);
        assert!(if dt != 0.0 { (warmth - was_warmth) / dt > 0.05 } else { (tint - was_tint) / dn > 0.05 }, "{label}: {grey:?}");
        // The ends move with the filtration, as on an enlarger: the paper's base and the densest read tinted too.
        let moved_end = |a: V4, b: V4| ((a[0] / a[2]).ln() - (b[0] / b[2]).ln()).abs() + ((a[1] / a[0]).ln() - (b[1] / b[0]).ln()).abs();
        assert!(moved_end(white, rest_white) > 1e-3 || moved_end(black, rest_black) > 1e-3, "{label}");
        // Twin: a window re-anchoring each print's densest read on white leaves no tint there for the check to read.
        let anchored = |w: V4, b: V4| -> V4 { std::array::from_fn(|c| if c < 3 { (w[c] - b[c]) / (w[c] - b[c]) } else { 1.0 }) };
        assert!(moved_end(anchored(white, black), anchored(rest_white, rest_black)) < 1e-6, "{label}");
    }
    // Twin: the filters' stop read on ONI's print curve leaves the enlarger at rest.
    let onis = PipelineSettings { print_paper: None, ..moved(&s, |p| p.set_filters(Print::TEMPERATURE_AXIS)) };
    assert!(conversion::printing(&onis).is_none() && conversion::stage47(&onis).own_stops == Print::TEMPERATURE_AXIS);
}

#[test]
fn contrast_is_the_papers_grade_about_the_grey() {
    for (label, s) in frames() {
        let rest = paper_of(&s);
        let anchors = paper::anchors(&s, conversion::stage47(&s).dyes.as_ref());
        for travel in [-0.3f32, 0.3] {
            let p = paper_of(&moved(&s, |u| u.contrast = travel));
            let (grey, base) = (p.exposed(anchors[2]), p.exposed(anchors[0]));
            let (g, b) = (p.graded(grey), p.graded(base));
            let slope: Vec<f32> = (0..3).map(|c| (g[c] - b[c]) / (grey[c] - base[c])).collect();
            println!("{label}, Contrast {travel:+}: the grade {slope:?}, 4^C {:.5}; the grey's log exposure {g:?} from {:?}", GRADE_BASE.powf(travel), rest.exposed(anchors[2]));
            assert!(slope.iter().all(|k| (k - GRADE_BASE.powf(travel)).abs() < 1e-4), "{label}, {travel}");
            assert!((0..3).all(|c| (g[c] - rest.exposed(anchors[2])[c]).abs() < 1e-6), "{label}, {travel}: about the grey");
            // Twin: the paper's own slope moves nothing.
            assert!((0..3).all(|c| (rest.graded(base)[c] - b[c]).abs() > 0.01), "{label}, {travel}");
        }
    }
}

#[test]
fn the_paper_lays_no_colour_compensation() {
    let s = prepared_negative();
    let moves: [(&str, fn(&mut Print)); 6] = [
        ("Exposure +1", |p| p.exposure += 1.0),
        ("Contrast +0.3", |p| p.contrast = 0.3),
        ("Contrast -0.3", |p| p.contrast = -0.3),
        ("Shadows +0.2", |p| p.shadows = 0.2),
        ("Highlights -0.2", |p| p.highlights = -0.2),
        ("points +0.05", |p| (p.black_point, p.white_point) = (0.05, 0.05)),
    ];
    for (label, f) in moves {
        let t = moved(&s, f);
        let k = conversion::stage47(&t);
        let curves = finishing::print_saturation(&t);
        assert!(k.hunt == 0.0 && k.print_on == 0.0 && curves.iter().all(|c| c.is_neutral()), "{label}: no Hunt, no saturation curve");
        // Twin: ONI's print lays Hunt and, for Contrast and the points, a curve.
        let onis = PipelineSettings { print_paper: None, ..t.clone() };
        let ko = conversion::stage47(&onis);
        let laid = finishing::print_saturation(&onis).iter().any(|c| !c.is_neutral());
        assert!(ko.hunt == 1.0 && (laid || !label.starts_with('C') && !label.starts_with('p')), "{label}");
    }
}

/// ONI's print on every setting the paper's checks move, Contrast at rest, its outputs' bits folded by FNV-1a.
fn onis_print_bits(twin: impl Fn(Stage47) -> Stage47) -> u64 {
    let stock = film::stock(VirtualStock::KodakPortra800);
    let mut pixels: Vec<V4> = noise(23, 17, 5, 0.0, 1.5).iter().map(|p| cpu::virtual_negative(*p, &stock.kernel, &stock.table)).collect();
    pixels.extend(noise(23, 17, 6, 0.0, 1.0));
    let off = |s: PipelineSettings| PipelineSettings { print_paper: None, ..s };
    let mut lit = off(prepared_negative());
    (lit.print.exposure, lit.print.shadows, lit.print.highlights) = (1.75, 0.1, -0.1);
    lit.print.set_filters([0.2, -0.1, 0.0]);
    (lit.print.black_point, lit.print.white_point, lit.print.channel_contrast, lit.print.channel_mid) = (0.03, 0.05, [0.1, 0.0, -0.1], [0.0, 0.1, 0.0]);
    lit.density = 0.3;
    let mut settings = vec![off(prepared_negative()), off(digital()), off(mono()), lit];
    for stops in [-0.5, 0.5] {
        let mut t = off(prepared_negative());
        t.print.exposure += stops;
        settings.push(t);
    }
    let mut warmer = off(prepared_negative());
    warmer.print.set_filters(Print::TEMPERATURE_AXIS);
    settings.push(warmer);
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for s in &settings {
        let k = twin(conversion::stage47(s));
        assert!(k.paper.is_none());
        for p in &pixels {
            for v in cpu::stage47(*p, &k) {
                for b in v.to_bits().to_le_bytes() {
                    hash = (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
                }
            }
        }
    }
    hash
}

#[test]
fn onis_print_is_unchanged_to_the_bit() {
    // ONI's print's outputs on these settings, Contrast at rest, as before the paper left its curve.
    const BEFORE: u64 = 0x3374_6d46_e618_f2e6;
    let bits = onis_print_bits(|k| k);
    println!("ONI's print folds to {bits:#018x}");
    assert_eq!(bits, BEFORE);
    // Twin: Contrast a notch off its rest moves it.
    assert_ne!(onis_print_bits(|k| Stage47 { print_contrast: k.print_contrast + 0.05, ..k }), BEFORE);
}
