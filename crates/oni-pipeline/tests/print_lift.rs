//! Print lift: off, whatever Lighting moves, the frame's pick prints on pure black, and the window's white on pure white
//! as Exposure leaves it; on, the paper's own ends. Each verdict has a twin.

mod common;
#[path = "paper/frames.rs"]
mod frames;
#[path = "paper/ends.rs"]
mod ends;

use ends::*;
use oni_pipeline::conversion;
use oni_render::{Paper, Shown, Stage47};
use oni_settings::PipelineSettings;

/// Amounts far past any the negative holds: the paper's own white, its curves held at their end.
fn past_the_white(s: &PipelineSettings) -> [f32; 3] {
    span_of(s).map(|a| 20.0 * a)
}

#[test]
fn without_print_lift_the_pick_lies_on_pure_black_and_the_windows_white_on_pure_white() {
    for (label, s) in frames() {
        let off = moved(&s, |t| t.print.print_lift = false);
        let k = conversion::stage47(&off);
        let (rest, white) = (printed_on(&s, &k)[0], printed_on(&s, &k)[1]);
        println!("{label}: pure black lays the pick at {rest:?}, pure white the window's white at {white:?}");
        assert!(rest.iter().all(|v| v.abs() < 1e-6) && white.iter().all(|v| (v - 1.0).abs() < 1e-5), "{label}: {rest:?} {white:?}");
        for (name, f) in SLIDERS {
            let k = conversion::stage47(&moved(&off, f));
            let pick = printed_on(&s, &k)[0];
            assert!(pick.iter().all(|v| v.abs() < 1e-6), "{label}, {name}: {pick:?}");
        }
        // Exposure holds the window's white on pure white too.
        for exposure in [-1.0f32, 1.0] {
            let k = conversion::stage47(&moved(&off, |t| t.print.exposure = exposure));
            let white = printed_on(&s, &k)[1];
            assert!(white.iter().all(|v| (v - 1.0).abs() < 1e-4), "{label}, Exposure {exposure}: {white:?}");
        }
        // Twin: a data sheet's own white, past the window's, prints past pure white; the Frontier's window ends on its
        // table's white.
        let paper_white = lit_shown(&s, &k, past_the_white(&s));
        println!("{label}: the paper's own white at {paper_white:?}");
        let sheet = s.paper().is_some_and(|p| !p.is_machine());
        assert!(!sheet || paper_white.iter().all(|v| *v > 1.01), "{label}: {paper_white:?}");
        // Twin: the window's white held where the paper prints it, as with Print lift, prints under pure white.
        let held = Stage47 { shown: k.paper().map_or(k.shown, |p| Shown::Paper(Paper { white_anchor: p.white, ..p })), ..k };
        assert!(printed_on(&s, &held)[1].iter().all(|v| *v < 0.99), "{label}");
    }
}

#[test]
fn print_lift_keeps_the_papers_own_ends() {
    for (label, s) in frames() {
        let k = conversion::stage47(&s);
        let p = k.paper().unwrap();
        let (pick, white) = (printed_on(&s, &k)[0], printed_on(&s, &k)[1]);
        println!("{label}: Print lift prints the pick at {pick:?}, the window's white at {white:?}");
        let off_white = (0..3).map(|c| (white[c] - p.white[c]).abs()).fold(0.0f32, f32::max);
        assert!(p.white_anchor == p.white && off_white < 1e-5, "{label}: {white:?} {:?}", p.white);
        // Twin: off, the same frame's pick lies on pure black and its window's white on pure white, above the paper's own.
        let off = conversion::stage47(&moved(&s, |t| t.print.print_lift = false));
        assert!(pick.iter().all(|v| *v > 0.0) && printed_on(&s, &off)[0].iter().all(|v| v.abs() < 1e-6), "{label}: {pick:?}");
        // A data sheet's own white lies under pure white; the Frontier's is its table's, near it.
        let sheet = s.paper().is_some_and(|p| !p.is_machine());
        assert!(!sheet || (0..3).all(|c| printed_on(&s, &off)[1][c] > white[c] + 0.05), "{label}");
    }
}
