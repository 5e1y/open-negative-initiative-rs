//! The paper's ends: whatever Lighting moves, the frame's pick prints where the rest lays it, the whites stay free, the
//! points move the ends, Brightness holds the pick, barely moves the deep shadows and moves the mids as the enlarger's
//! time does, the print continuous and monotone; pure ends are `print_lift`'s. Each verdict has a twin.

mod common;
#[path = "paper/frames.rs"]
mod frames;
#[path = "paper/ends.rs"]
mod ends;

use ends::*;
use frames::*;
use oni_pipeline::{conversion, paper};
use oni_pipeline::paper::BRIGHTNESS_REACH_LIGHT;
use oni_render::{Paper, Stage47, cpu, fade};
use oni_settings::PipelineSettings;

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
        // On the negative every move leaves the bare film at its fog; twin: an eighth of the enlarger's time lifts it,
        // unanchored, by a tenth of its light at least.
        let timed = |p: Paper| unanchored(Stage47 { paper: Some(Paper { scale: p.scale / 8.0, ..p }), ..conversion::stage47(&s) });
        let most = rest.iter().copied().fold(0.0f32, f32::max);
        assert!(lifted == 0 && worst(printed_on(&s, &timed(paper_of(&s)))[0], rest) > 0.1 * most, "{label}: {lifted} of {} moves lift the pick", SLIDERS.len());
    }
}

#[test]
fn the_whites_stay_free() {
    let mut seen = 0;
    for (label, s) in frames() {
        let [_, rest_white, _] = printed_on(&s, &conversion::stage47(&s));
        for travel in [-0.3f32, 0.3] {
            let k = conversion::stage47(&moved(&s, |t| t.print.contrast = travel));
            let (white, free) = (printed_on(&s, &k)[1], printed_on(&s, &unanchored(k))[1]);
            let (moved_by, free_by, apart) = (worst(white, rest_white), worst(free, rest_white), worst(white, free));
            println!("{label}, Contrast {travel:+}: the densest read {white:?} from {rest_white:?} (moved {moved_by:.1e}), unanchored {free:?} ({free_by:.1e}), apart {apart:.1e}");
            // The densest read moves as the enlarger moves it, unanchored, within a hundredth of that move.
            assert!(apart <= 0.01 * free_by + 2e-5, "{label}, {travel}: {apart} {free_by}");
            // Twin: a white anchored too stays where the rest prints it, off the free move by all of it, where it moves.
            if free_by > 1e-3 {
                assert!(worst(rest_white, free) > 0.01 * free_by + 2e-5, "{label}, {travel}");
                seen += 1;
            }
        }
    }
    // Black and white's and a digital photo's densest read sit at the paper's white either way, the negatives' darken.
    assert!(seen >= 3, "{seen}");
}

/// The frame's neutral from its pick (0) through its reference grey (1), as the window's shares.
fn neutral(s: &PipelineSettings, t: f32) -> [f32; 3] {
    let [pick, _, grey] = anchor_shares(s, &conversion::stage47(s));
    std::array::from_fn(|c| pick[c] + t * (grey[c] - pick[c]))
}

/// How far a tone's mean log exposure lies from the pick's toward the light on `p`.
fn under_pick(p: &Paper, shares: [f32; 3]) -> f32 {
    p.toward_light(p.pick_log, p.exposed(p.density(shares)))
}

/// The light `p` prints for `shares` above its pick, its pick laid on black and its white the paper's.
fn on_black(p: &Paper, shares: [f32; 3]) -> f32 {
    let luma = |v: [f32; 3]| 0.2627 * v[0] + 0.678 * v[1] + 0.0593 * v[2];
    let (k, w) = (luma(p.pick), luma(p.white));
    (luma(p.printed(shares)) - k) / (w - k) * w
}

/// What `p` shows of `shares` with the tone's mean log exposure from the pick moved by `f` in Brightness's place.
fn shown_moved(p: &Paper, shares: [f32; 3], f: impl Fn(f32) -> f32) -> [f32; 3] {
    let log = p.exposed(p.density(shares));
    let x = p.toward_light(p.pick_log, log);
    let moved = f(x) - x;
    let light = p.viewed(p.developed(log.map(|v| v - moved)));
    let light = if p.luma { [0.2627 * light[0] + 0.678 * light[1] + 0.0593 * light[2]; 3] } else { light };
    p.ended(light)
}

/// CIE L* of a print's light, its luma read on D65's white.
fn lstar(v: [f32; 3]) -> f32 {
    let y = 0.2627 * v[0] + 0.678 * v[1] + 0.0593 * v[2];
    if y > 0.008856 { 116.0 * y.cbrt() - 16.0 } else { 903.3 * y }
}

#[test]
fn brightnesss_reach_lies_where_the_neutral_prints_l_25_over_the_pick_whatever_lighting_does() {
    for (label, s) in frames() {
        for (name, f) in SLIDERS {
            let t = moved(&s, |u| {
                f(u);
                u.print.brightness = 0.5;
            });
            let (k, p) = (conversion::stage47(&t), paper_of(&t));
            let [pick, densest, _] = anchor_shares(&t, &k);
            let lit = |x: f32| -> [f32; 3] { std::array::from_fn(|c| pick[c] + x * (densest[c] - pick[c])) };
            // The neutral's tone, pick to the window's white as Lighting develops them, whose exposure lies at the reach
            // prints its light.
            let (mut low, mut high) = (0.0f32, 1.0f32);
            for _ in 0..60 {
                let mid = 0.5 * (low + high);
                if under_pick(&p, lit(mid)) < p.reach { low = mid } else { high = mid }
            }
            // Read at rest: Brightness itself moves that tone.
            let light = on_black(&Paper { brightness: 0.0, ..p }, lit(low));
            assert!(p.reach > 0.0 && (light / BRIGHTNESS_REACH_LIGHT - 1.0).abs() < 1e-3, "{label}, {name}: reach {} at {light}", p.reach);
            // The anchor leaves the reach where it is: it is read on the print's own range.
            assert_eq!(paper::ended(&p, pick, densest, 1.0, p.white, None, 0.5).reach, p.reach, "{label}, {name}");
        }
        let p = paper_of(&moved(&s, |u| u.print.brightness = 0.5));
        println!("{label}: the reach {:.3} of log exposure under the pick", p.reach);
        // Twin: the reach's light read on the print as shown with Print lift, its pick on its anchor, lies elsewhere.
        let k = conversion::stage47(&s);
        let [pick, densest, _] = anchor_shares(&s, &k);
        let toward = |x: f32| -> [f32; 3] { std::array::from_fn(|c| pick[c] + x * (densest[c] - pick[c])) };
        let (mut low, mut high) = (0.0f32, 1.0f32);
        for _ in 0..60 {
            let mid = 0.5 * (low + high);
            if under_pick(&p, toward(mid)) < p.reach { low = mid } else { high = mid }
        }
        let luma = |v: [f32; 3]| 0.2627 * v[0] + 0.678 * v[1] + 0.0593 * v[2];
        let shown = Paper { anchor: p.pick.map(|v| v + 0.01), brightness: 0.0, ..p }.shown(toward(low));
        assert!((luma(shown) / BRIGHTNESS_REACH_LIGHT - 1.0).abs() > 0.01, "{label}");
    }
}

#[test]
fn brightness_holds_the_pick_spares_the_deep_shadows_and_moves_the_mids_as_exposure_does() {
    let (mut followed, mut deep_seen, mut greys) = (0, 0, 0);
    for (label, s) in frames() {
        for (brightness, print_lift) in [(-4.0f32, true), (-1.0, true), (-0.5, false), (0.5, true), (1.0, false), (4.0, true)] {
            let t = moved(&s, |u| (u.print.brightness, u.print.print_lift) = (brightness, print_lift));
            let p = paper_of(&t);
            let at_rest = Paper { brightness: 0.0, ..p };
            // The enlarger's time, as many stops: Exposure's move on the print.
            let exposure = Paper { scale: p.scale * (-brightness).exp2(), brightness: 0.0, ..p };
            // The pick and the tones beyond it print as at rest, to the bit.
            let mut beyond = 0;
            for i in 0..=100 {
                let a = neutral(&s, -0.5 + 0.5 * i as f32 / 100.0);
                if under_pick(&p, a) <= 0.0 {
                    beyond += 1;
                    assert_eq!(p.shown(a), at_rest.shown(a), "{label}, {brightness}");
                }
            }
            assert!(beyond > 20, "{label}, {brightness}: {beyond}");
            // Tones 0.5 to 3 L* over the pick move 0.4 of Exposure's move at most brightening, 0.8 darkening (Exposure
            // crushes them on black), and less than a straight fade over the same width.
            let width = fade(brightness * std::f32::consts::LOG10_2, p.reach);
            let straight = |a: [f32; 3]| {
                let lift = brightness * std::f32::consts::LOG10_2;
                shown_moved(&p, a, |x| if lift >= 0.0 { x + lift * (x / width).clamp(0.0, 1.0) } else { x + lift * (x / (width + lift.abs())).clamp(0.0, 1.0) })
            };
            let (mut worst_share, mut straight_share) = (0.0f32, 0.0f32);
            let black = lstar(at_rest.shown(neutral(&s, 0.0)));
            for i in 1..=1600 {
                let a = neutral(&s, i as f32 / 1600.0);
                let was = lstar(at_rest.shown(a));
                if !(0.5..=3.0).contains(&(was - black)) {
                    continue;
                }
                let by_exposure = lstar(exposure.shown(a)) - was;
                if by_exposure.abs() < 0.2 {
                    continue;
                }
                deep_seen += 1;
                // The twin's print is the kernel's own when it moves the tone as Brightness does.
                let own = shown_moved(&p, a, |x| oni_render::lifted(x, brightness * std::f32::consts::LOG10_2, p.reach));
                assert!(worst(own, p.shown(a)) < 1e-6, "{label}, {brightness}");
                worst_share = worst_share.max((lstar(p.shown(a)) - was) / by_exposure);
                straight_share = straight_share.max((lstar(straight(a)) - was) / by_exposure);
            }
            println!("{label}, Brightness {brightness:+}: the deep shadows take {worst_share:.2} of Exposure's move at most; a straight fade {straight_share:.2}");
            // Below a straight fade by 0.02 at least, 0.4 of Exposure's move at most brightening and 0.8 darkening. The
            // Frontier's fit lays its pick at a density of 4.2, its first L* over it far up its shoulder: it is not weighed.
            let bound = if brightness > 0.0 { 0.4 } else { 0.8 };
            let weighed = label != "Frontier negative";
            assert!(!weighed || (worst_share < bound && worst_share < straight_share - 0.02), "{label}, {brightness}: {worst_share} {straight_share}");
            // Past the reach and what a darkening folds under it, a light grey, twice the reference grey's share over the
            // pick, prints within 1.5 L* of Exposure's own print, the gap its moved pick lays; nearer the pick, no darker
            // than that, held off the black Exposure crushes it on.
            let lift = brightness.abs() * std::f32::consts::LOG10_2;
            let light_grey = neutral(&s, 2.0);
            let (grey, wanted) = (lstar(p.shown(light_grey)), lstar(exposure.shown(light_grey)));
            println!("{label}, Brightness {brightness:+}: the light grey at L* {grey:.2}, Exposure's {wanted:.2}");
            if under_pick(&p, light_grey) >= width + lift {
                greys += 1;
                assert!((grey - wanted).abs() < 1.5, "{label}, {brightness}: {grey} {wanted}");
            } else {
                assert!(grey > wanted - 1.5, "{label}, {brightness}: {grey} {wanted}");
            }
            // Past the reach and what a darkening folds under it, each tone prints as Exposure's light would, the ends
            // kept.
            let twice = Paper { scale: p.scale * (-2.0 * brightness).exp2(), brightness: 0.0, ..p };
            for i in 0..=40 {
                let a = neutral(&s, 0.5 + 1.5 * i as f32 / 40.0);
                if under_pick(&p, a) < width + lift {
                    continue;
                }
                followed += 1;
                let (now, want) = (p.shown(a), exposure.shown(a));
                assert!(worst(now, want) < 2e-5 * want.iter().copied().fold(0.0, f32::max) + 1e-7, "{label}, {brightness}: {now:?} {want:?}");
                // Twin: Exposure at twice the stops misses wherever the print is not held where the paper's white
                // lands or at its black.
                let off = worst(now, twice.shown(a));
                assert!(off > 1e-4 || worst(now, p.white_anchor) < 1e-3 || now[1] < 1e-3, "{label}, {brightness}: {off}");
            }
        }
    }
    println!("{followed} tones past the reach checked against Exposure, {greys} greys, {deep_seen} deep shadows");
    assert!(followed > 200 && greys >= 10 && deep_seen > 100, "{followed} {greys} {deep_seen}");
}

#[test]
fn the_points_still_move_the_ends() {
    for (label, s) in frames() {
        let [rest_pick, rest_white, _] = printed_on(&s, &conversion::stage47(&s));
        for (name, black, white) in [("black point +0.05", 0.05f32, 0.0f32), ("black point -0.05", -0.05, 0.0), ("white point +0.05", 0.0, 0.05)] {
            let t = moved(&s, |u| (u.print.black_point, u.print.white_point) = (black, white));
            let k = conversion::stage47(&t);
            let [pick, densest, _] = printed_on(&s, &k);
            let (p, span) = (k.paper.unwrap(), span_of(&s));
            // The upper range under the white: Endura Premier prints black and white's 0.7 of the range on its white.
            let (high, rest_high) = (lit_shown(&s, &k, span.map(|a| a * 0.6)), lit_shown(&s, &conversion::stage47(&s), span.map(|a| a * 0.6)));
            let (sank, lightened) = (worst(pick, rest_pick), worst(high, rest_high));
            // The Frontier's pick lies at its fit's 0.9 Dmax, a density of 4.2: a move of its light is read on its own scale.
            let pick_scale = rest_pick.iter().copied().fold(0.0f32, f32::max);
            println!("{label}, {name}: the pick {pick:?} from {rest_pick:?}, the densest read {densest:?} from {rest_white:?}, 0.6 of the range {high:?} from {rest_high:?}");
            if black != 0.0 {
                // A black point crushes the pick below where the rest prints it; a negative one lifts it.
                assert!(sank > 0.01 * pick_scale && (0..3).all(|c| (pick[c] < rest_pick[c]) == (black > 0.0)), "{label}, {name}");
                // Twin: the pick anchored through the points stays where the rest prints it.
                let through = Paper { pick: p.printed(paper::shares_of(&t, &k, k.negative.unwrap().toned([0.0; 3]))), ..p };
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
    let settings: [Move; 6] = [
        ("at rest", |_| {}),
        ("pure ends", |s| s.print.print_lift = false),
        ("Brightness -4", |s| s.print.brightness = -4.0),
        ("Brightness +4, pure ends", |s| (s.print.brightness, s.print.print_lift) = (4.0, false)),
        ("Exposure +2, Brightness -1", |s| (s.print.exposure, s.print.brightness) = (s.print.exposure + 2.0, -1.0)),
        ("Exposure -1, Contrast +0.3, pure ends, Brightness +1", |s| {
            (s.print.exposure, s.print.contrast, s.print.print_lift, s.print.brightness) = (s.print.exposure - 1.0, 0.3, false, 1.0);
        }),
    ];
    for (label, s) in frames() {
        for (name, f) in settings {
            let (k, span) = (conversion::stage47(&moved(&s, f)), span_of(&s));
            let print = |u: f32| lit_shown(&s, &k, span.map(|a| a * u));
            let (coarse, fine) = (steps(&print, 2000), steps(&print, 4000));
            let (falls_by, lowest) = (fall(&print, 4000), least(&print, 4000));
            println!("{label}, {name}: the largest step {coarse:.2e} over 2000 tones, {fine:.2e} over 4000; its largest fall {falls_by:.1e}, its least light {lowest:.1e}");
            assert!(fine < 0.6 * coarse, "{label}, {name}");
            // Monotone and at or above black, but for f32's step near the white times Brightness's slope there; the
            // Frontier's fit falls 6.5e-6 near its white at rest and prints its Dmax a hair under no light. Each pure dye
            // shows its layer alone: Exposure +2 leaves the digital photo's blue layer 60 % slower than its tone's mean
            // by the pick, where Brightness -1's one shift on the three (`fade`) turns it back by 2.0e-4.
            let (falls, floor) = match label {
                "Frontier negative" => (3e-5, -2e-5),
                "digital" => (3e-4, 0.0),
                _ => (1e-6, 0.0),
            };
            assert!(falls_by < falls && lowest >= floor, "{label}, {name}");
            // Twin: a jump of a tenth past the middle keeps its step whatever the sampling, so the check refuses it.
            let jump = |u: f32| print(u).map(|v| if u > 0.5 { v + 0.1 } else { v });
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



