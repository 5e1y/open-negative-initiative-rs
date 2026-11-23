//! Print lift: off, whatever Lighting moves, the frame's pick prints on pure black and the paper's white on pure white,
//! the rest showing the reference grey where a plain RAW does, neutral; on, the paper's own ends. Each verdict has a twin.

mod common;
#[path = "paper/frames.rs"]
mod frames;
#[path = "paper/ends.rs"]
mod ends;

use ends::*;
use oni_pipeline::{conversion, paper};
use oni_render::{Paper, Stage47};
use oni_settings::PipelineSettings;

/// Amounts far past any the negative holds: the paper's own white, its curves held at their end.
fn past_the_white(s: &PipelineSettings) -> [f32; 3] {
    span_of(s).map(|a| 20.0 * a)
}

#[test]
fn without_print_lift_the_pick_lies_on_pure_black_and_the_papers_white_on_pure_white_under_every_slider() {
    for (label, s) in frames() {
        let off = moved(&s, |t| t.print.print_lift = false);
        let k = conversion::stage47(&off);
        let (rest, white) = (printed_on(&s, &k)[0], lit_shown(&s, &k, past_the_white(&s)));
        println!("{label}: pure black lays the pick at {rest:?}, pure white the paper's white at {white:?}");
        assert!(rest.iter().all(|v| v.abs() < 1e-6) && white.iter().all(|v| (v - 1.0).abs() < 1e-6), "{label}: {rest:?} {white:?}");
        for (name, f) in SLIDERS {
            let k = conversion::stage47(&moved(&off, f));
            let (pick, white) = (printed_on(&s, &k)[0], lit_shown(&s, &k, past_the_white(&s)));
            assert!(pick.iter().all(|v| v.abs() < 1e-6), "{label}, {name}: {pick:?}");
            // Exposure, Brightness and the filters move no layer off its curve's end there, so the white prints pure.
            assert!(white.iter().all(|v| (v - 1.0).abs() < 1e-6), "{label}, {name}: {white:?}");
        }
        // Twin: the paper's Dmax laid on black, its white kept, leaves the pick above it.
        let p = paper_of(&s);
        let (d, w) = (dmax(&p), p.white);
        let printed = printed_on(&s, &unanchored(conversion::stage47(&s)))[0];
        let on_dmax: [f32; 3] = std::array::from_fn(|c| w[c] * (printed[c] - d[c]) / (w[c] - d[c]));
        println!("{label}: on the Dmax's black the pick would print at {on_dmax:?} (Dmax {d:?})");
        // A hundred times the pick's own bound: Endura Premier lays a negative's bare film 9e-4 over its Dmax.
        assert!(on_dmax.iter().any(|v| *v > 1e-4), "{label}: {on_dmax:?}");
        // Twin: the paper's white held, as with Print lift, prints under 0.87 on Royal Digital; the Frontier's own white
        // is pure white, its fit's curves leaving no density past the table.
        let held = Stage47 { paper: k.paper.map(|p| Paper { white_anchor: p.white, ..p }), ..k };
        let white = lit_shown(&s, &held, past_the_white(&s));
        if label.starts_with("Frontier") {
            assert!(white.iter().all(|v| (v - 1.0).abs() < 1e-4), "{label}: {white:?}");
        } else {
            assert!(white.iter().all(|v| *v < 0.87), "{label}: {white:?}");
        }
    }
}

#[test]
fn print_lift_keeps_the_papers_own_ends() {
    for (label, s) in frames() {
        let k = conversion::stage47(&s);
        let p = k.paper.unwrap();
        let (pick, white) = (printed_on(&s, &k)[0], lit_shown(&s, &k, past_the_white(&s)));
        println!("{label}: Print lift prints the pick at {pick:?}, the paper's white at {white:?}");
        assert!(p.white_anchor == p.white && white == p.white, "{label}: {white:?} {:?}", p.white);
        // Twin: off, the same frame's pick lies on pure black and its white on pure white, 0.14 above on Royal Digital;
        // the Frontier's own white is pure already.
        let off = conversion::stage47(&moved(&s, |t| t.print.print_lift = false));
        assert!(pick.iter().all(|v| *v > 1e-3) && printed_on(&s, &off)[0].iter().all(|v| v.abs() < 1e-6), "{label}: {pick:?}");
        if !label.starts_with("Frontier") {
            assert!((0..3).all(|c| lit_shown(&s, &off, past_the_white(&s))[c] > white[c] + 0.1), "{label}");
        }
    }
}

/// The light `v` holds above no light in stops, its luma read.
fn stops(v: [f32; 3]) -> f32 {
    (0.2627 * v[0] + 0.678 * v[1] + 0.0593 * v[2]).log2()
}

#[test]
fn without_print_lift_the_rest_shows_the_grey_where_a_plain_raw_does_neutral() {
    let grey_print = *paper::GREY_PRINT;
    for (label, s) in frames() {
        let off = moved(&s, |t| t.print.print_lift = false);
        let k = conversion::stage47(&off);
        let grey = printed_on(&off, &k)[2];
        // The paper's own ends at their rest, laid on pure black and pure white by a straight line in light, then by
        // one in density: the measured ways the white could be laid on the enlarger's rest with Print lift.
        let lifted = k_lifted(&s);
        let [_, _, at_rest] = printed_on(&s, &lifted);
        let p = lifted.paper.unwrap();
        let linear: [f32; 3] = std::array::from_fn(|c| (at_rest[c] - p.pick[c]) / (p.white[c] - p.pick[c]));
        let density: [f32; 3] = std::array::from_fn(|c| {
            let (d, dk, dw) = (-at_rest[c].log10(), -p.pick[c].log10(), -p.white[c].log10());
            (10f32.powf(-(d - dw) * dk / (dk - dw)) - p.pick[c]) / (1.0 - p.pick[c])
        });
        let off_by = |v: [f32; 3]| stops(v) - grey_print.log2();
        println!(
            "{label}: the grey at {grey:?} ({:+.4} stop); laid on the rest of Print lift, in light {:+.3} stop, in density {:+.3}",
            off_by(grey),
            off_by(linear),
            off_by(density)
        );
        // Within the thousandth the pick's light laid on black takes off the paper's own neutral.
        assert!((0..3).all(|c| (grey[c] - grey_print).abs() < 1e-3), "{label}: {grey:?}");
        // Twin: either stretch on Print lift's rest moves the grey, lighter by 0.28 and 0.24 stop on Royal Digital,
        // darker by 0.009 on the Frontier, whose own white is pure: past the 0.004 stop the grey's bound allows.
        assert!(off_by(linear).abs() > 5e-3 && off_by(density).abs() > 5e-3, "{label}");
    }
}

/// The frame's print with Print lift, its own ends at its rest.
fn k_lifted(s: &PipelineSettings) -> Stage47 {
    conversion::stage47(&moved(s, |t| t.print.print_lift = true))
}
