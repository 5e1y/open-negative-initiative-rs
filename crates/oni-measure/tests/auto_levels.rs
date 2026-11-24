//! `AutoLevels.selfCheck`: the suggestion's threshold, the alignment, the placed handles, the two
//! steps on simulated densities, each with its twin.

use oni_core::balance::{self as core_balance, Suggestion};
use oni_measure::Histogram;
use oni_measure::auto_levels::*;
use oni_measure::roll;
use oni_settings::axis::{graduation, pipeline};
use oni_settings::blocks::Gains;
use oni_settings::migrate::density;
use oni_settings::levels::{Levels, LevelsSet};
use oni_settings::{ConversionMode, PipelineSettings};

const NEG: ConversionMode = ConversionMode::Negative;

/// The share a synthetic histogram skips at each end; a reading of the app skips nothing.
const DEFAULT_THRESHOLD: f32 = 0.001;

fn block(from: usize, to: usize, strays: f32) -> Vec<f32> {
    let mut counts = vec![0.0f32; 1024];
    for c in &mut counts[from..=to] {
        *c = 100.0;
    }
    counts[0] += strays;
    counts[1023] += strays;
    counts
}

#[test]
fn the_threshold_skips_isolated_pixels_and_bites_when_raised() {
    let raw = suggest_counts(&block(300, 700, 1.0), 0.0).unwrap();
    assert_eq!((raw.black, raw.white), (0.0, 1.0), "zero threshold reaches the isolated pixels");
    let cleaned = suggest_counts(&block(300, 700, 1.0), DEFAULT_THRESHOLD).unwrap();
    let (eb, ew) = (300.0 / 1023.0, 700.0 / 1023.0);
    assert!((cleaned.black - eb).abs() < 0.01 && (cleaned.white - ew).abs() < 0.01);
    // Twin: at 5 % the threshold bites into the block.
    let biting = suggest_counts(&block(300, 700, 1.0), 0.05).unwrap();
    assert!(biting.black > eb && biting.white < ew);
    let weak: Vec<f32> = block(300, 700, 1.0).iter().map(|v| v * 0.01).collect();
    assert_eq!(suggest_counts(&weak, DEFAULT_THRESHOLD), Some(cleaned), "a channel less populated gives the same suggestion");
    assert_eq!(suggest_counts(&[], 0.0), None);
    assert_eq!(suggest_counts(&[0.0; 1024], 0.0), None);
    let mut spike = vec![0.0f32; 1024];
    spike[512] = 1000.0;
    assert_eq!(suggest_counts(&spike, 0.0), None, "a flat channel suggests no violent stretch");
    if let Some(d) = suggest_counts(&block(300, 700, 1.0), 0.9) {
        assert!(d.white > d.black);
    }
    for i in 0..=25 {
        let t = i as f32 * 0.002;
        if let Some(s) = suggest_counts(&block(200, 800, 1.0), t) {
            assert!(s.white > s.black && s.black >= 0.0 && s.white <= 1.0);
        }
    }
}

fn lobe(centre: i64, tail: Option<(usize, usize)>, tail_mass: f32) -> Vec<f32> {
    let spread = 60i64;
    let mut counts = vec![0.0f32; 1024];
    for bin in (centre - spread).max(0)..=(centre + spread).min(1023) {
        let d = (bin - centre) as f32 / spread as f32;
        counts[bin as usize] = (1.0 - d * d).max(0.0) * 1000.0;
    }
    for bin in (centre - spread * 3).max(0)..=(centre - spread * 2).max(0) {
        counts[bin as usize] += 300.0;
    }
    if let Some((a, b)) = tail {
        for c in &mut counts[a..=b] {
            *c += tail_mass;
        }
    }
    counts
}

fn scene(shifts: [i64; 3], tails: [Option<(usize, usize)>; 3], tail_mass: f32) -> Histogram {
    let ch: Vec<Vec<f32>> = (0..3).map(|c| lobe(500 - shifts[c], tails[c], tail_mass)).collect();
    Histogram { rgb: (0..1024).map(|i| [ch[0][i], ch[1][i], ch[2][i]]).collect(), luma: ch[1].clone(), ends: None }
}

#[test]
fn a_known_shift_comes_back_and_classic_moves_no_median_or_white() {
    let known = scene([80, 0, -50], [None; 3], 0.0);
    let placed = placement(&known, DEFAULT_THRESHOLD).unwrap();
    let anchor = suggest(&known, REFERENCE, DEFAULT_THRESHOLD).unwrap();
    let (er, eb) = (anchor.black - 80.0 / 1023.0, anchor.black + 50.0 / 1023.0);
    assert!((placed[0].black - er).abs() < 2e-3 && (placed[1].black - anchor.black).abs() < 1e-6 && (placed[2].black - eb).abs() < 2e-3);
    let spans: Vec<f32> = placed.iter().map(|p| p.white - p.black).collect();
    assert!(spans.iter().cloned().fold(f32::MIN, f32::max) - spans.iter().cloned().fold(f32::MAX, f32::min) < 1e-6);

    let cast = scene([40, 0, -20], [None, None, Some((700, 1000))], 12.0);
    let placed = placement(&cast, DEFAULT_THRESHOLD).unwrap();
    assert!(placed.iter().all(|p| p.mid == 0.5 && p.shadows == 0.25 && p.highlights == 0.75), "the two added points at rest");
    assert_eq!(placement(&Histogram::default(), 0.0), None);
    assert_eq!(median(&[0.0; 1024]), None);
}

#[test]
fn placing_writes_all_five_handles_on_the_mode_s_own_window() {
    let window = graduation::window(NEG);
    let whole = core_balance::placed(&Suggestion { mid: 0.62, ..Suggestion::ends(0.0, 1.0) }, NEG);
    assert!(whole.black == *window.start() && whole.white == *window.end() && whole.mid == 0.62);
    // Twin: written as measured, the black would sit at 0 and the white at 1.
    assert!(whole.black != 0.0 && whole.white != 1.0);
    let reset = core_balance::placed(&Suggestion::ends(0.05, 0.9), NEG);
    assert!(reset.mid == 0.5 && reset.shadows == 0.25 && reset.highlights == 0.75);
    let carried = core_balance::placed(&Suggestion { black: 0.05, white: 0.9, mid: 0.62, shadows: 0.37, highlights: 0.58 }, NEG);
    assert!(carried.shadows == 0.37 && carried.highlights == 0.58);
    let positive = core_balance::placed(&Suggestion::ends(0.0, 1.0), ConversionMode::Positive);
    let pw = graduation::window(ConversionMode::Positive);
    assert!(positive.black == *pw.start() && positive.white == *pw.end() && positive.black != whole.black);
}

/// The measuring path's closed form over densities the frame really holds, binned onto the window.
fn simulate(densities: &[Vec<f32>; 3], stops: [f32; 3]) -> Histogram {
    let g = graduation::levels(NEG);
    let measuring = LevelsSet { red: g, green: g, blue: g };
    let mut counts = vec![[0.0f32; 3]; 1024];
    for (c, list) in densities.iter().enumerate() {
        for &d in list {
            let t = 10f32.powf(-d) - pipeline::tpedestal();
            let m = density::new([t; 3], [stops[c]; 3], &measuring, NEG, pipeline::tpedestal(), pipeline::tmin());
            let bin = (m[0].min(1.0) * 1023.0).round() as i64;
            counts[bin.clamp(0, 1023) as usize][c] += 1.0;
        }
    }
    Histogram { rgb: counts.clone(), luma: counts.iter().map(|b| b[1]).collect(), ends: None }
}

/// The histogram with each channel's outermost populated bins as its tails, as a reading carries them.
fn tailed(mut h: Histogram) -> Histogram {
    let tail = |c: usize| {
        let lit: Vec<usize> = (0..h.rgb.len()).filter(|&i| h.rgb[i][c] > 0.0).collect();
        [lit[0] as f32 / 1023.0, lit[lit.len() - 1] as f32 / 1023.0]
    };
    h.ends = Some((0..3).map(tail).collect());
    h
}

fn cloud(centre: f32, spread: f32, count: usize) -> Vec<f32> {
    (0..count).map(|i| centre + ((i as f32 + 0.5) / count as f32 - 0.5) * spread).collect()
}

fn shifted(base: &[f32], stops: f32) -> Vec<f32> {
    base.iter().map(|d| d - stops * 2f32.log10()).collect()
}

fn piled(h: &Histogram, c: usize) -> f32 {
    let counts = h.channel(c);
    let total: f32 = counts.iter().sum();
    if total > 0.0 { (counts[0] + counts[1023]) / total } else { 0.0 }
}

fn outcomes(densities: &[Vec<f32>; 3], stops: [f32; 3], levels: &[Levels; 3]) -> Vec<f32> {
    let frame = simulate(densities, stops);
    (0..3)
        .filter_map(|c| {
            let bin = median(&frame.channel(c))?;
            let set = levels[c];
            let e = graduation::value(bin / 1023.0, NEG);
            let n = ((e - set.black) / (set.white - set.black).max(1e-4)).max(0.0);
            Some(n.powf(set.gamma()))
        })
        .collect()
}

fn disagreement(v: &[f32]) -> f32 {
    v.iter().cloned().fold(f32::MIN, f32::max) - v.iter().cloned().fold(f32::MAX, f32::min)
}

fn handles(points: &[Suggestion; 3]) -> [Levels; 3] {
    points.map(|p| core_balance::placed(&p, NEG))
}

#[test]
fn step_one_recentres_on_green_and_the_gain_sense_follows_the_mode() {
    let green = cloud(1.2, 1.2, 4000);
    let sane = [green.clone(), green.clone(), shifted(&green, -1.5)];
    let r = recentred(&simulate(&sane, [0.0; 3]), [0.0; 3], NEG, DEFAULT_THRESHOLD).unwrap();
    assert!((r[2] - 1.5).abs() < 0.03 && r[0].abs() < 0.03 && r[1] == 0.0, "{r:?}");
    let round = stops_for_shift(shift(1.5, NEG), NEG);
    assert!((round - 1.5).abs() < 1e-4 && (shift(1.5, ConversionMode::Positive) + shift(1.5, NEG)).abs() < 1e-6 && shift(1.5, NEG) < 0.0);
}

#[test]
fn two_steps_recover_a_channel_piled_on_the_window_s_top_and_change_nothing_on_a_sane_one() {
    let spread = 1.2f32;
    let green = cloud(1.2, spread, 4000);
    let sane = [green.clone(), green.clone(), shifted(&green, -1.5)];
    let broken = [green.clone(), green.clone(), shifted(&green, -5.0)];
    let broken_frame = simulate(&broken, [0.0; 3]);
    let broken_pile = piled(&broken_frame, 2);
    let one_bin = 1.0 / 1023.0;

    let quantiles = |d: &[Vec<f32>; 3], c: usize, stops: [f32; 3]| -> Vec<f32> {
        let counts = simulate(d, stops).channel(c);
        [0.1, 0.5, 0.9].iter().filter_map(|&f| quantile(&counts, f).map(|q| q / 1023.0)).collect()
    };
    let travelled = |d: &[Vec<f32>; 3], c: usize, asked: f32| -> f32 {
        let mut moved = [0.0; 3];
        moved[c] = asked;
        let owed = shift(asked, NEG);
        quantiles(d, c, [0.0; 3]).iter().zip(quantiles(d, c, moved)).map(|(a, b)| (b - a - owed).abs()).fold(0.0, f32::max)
    };
    assert!(travelled(&sane, 2, 1.5) < 2.0 * one_bin, "a gain translates a channel inside the window exactly");
    // Twin: on the channel crossing the window's top the same gain is no translation.
    assert!(travelled(&broken, 2, 3.0) > 20.0 * one_bin);

    let at_rest = PipelineSettings { mode: NEG, ..PipelineSettings::default() };
    for (name, raws, frame) in [("sane", &sane, simulate(&sane, [0.0; 3])), ("broken", &broken, broken_frame.clone())] {
        let one = placement(&frame, DEFAULT_THRESHOLD).unwrap();
        let stops = roll::measured("", &at_rest, |s| tailed(simulate(raws, s))).unwrap().stops;
        let two = placement(&simulate(raws, stops), DEFAULT_THRESHOLD).unwrap();
        let before = disagreement(&outcomes(raws, [0.0; 3], &handles(&one)));
        let after = disagreement(&outcomes(raws, stops, &handles(&two)));
        if name == "broken" {
            let noise = 3.0 * one_bin * graduation::span() / spread;
            assert!(after < noise && after < before / 10.0, "broken: {before} to {after}");
            assert_eq!(one[2].white, 1.0, "one step reads blue's white off the pile");
            let expected = raws[2].iter().filter(|&&d| d > graduation::top()).count() as f32 / raws[2].len() as f32;
            let recovered = piled(&simulate(raws, stops), 2);
            assert!((broken_pile - expected).abs() < 0.01 && recovered < broken_pile / 10.0);
            let own = median(&frame.channel(2)).unwrap();
            let anchor = median(&frame.channel(1)).unwrap();
            let asked = stops_for_shift((anchor - own) * one_bin, NEG);
            assert!((asked - 5.0).abs() < 0.03 && stops[2] == Gains::RANGE.1, "truncated, not refused: {asked}");
        } else {
            assert!((after - before).abs() < 5e-3, "step one changes nothing on a sane scene");
            let sane_pile: Vec<f32> = (0..3).map(|c| piled(&simulate(raws, stops), c)).collect();
            assert!((0..3).all(|c| sane_pile[c] <= piled(&frame, c) + 1e-3));
        }
    }
}

#[test]
fn the_bound_holds_recentring_to_the_room_between_the_tail_and_the_window() {
    let green = cloud(1.2, 1.2, 4000);
    let body = cloud(2.0, 0.8, 4000);
    let (from, to, share) = (-0.30f32, -0.15f32, 0.08f32);
    let count = ((body.len() as f32 * share / (1.0 - share)).round() as usize).max(1);
    let tailed: Vec<f32> = body.iter().cloned().chain((0..count).map(|i| from + (i as f32 + 0.5) / count as f32 * (to - from))).collect();
    let brink = [tailed, green.clone(), green];
    let frame = simulate(&brink, [0.0; 3]);
    let held = recentred(&frame, [0.0; 3], NEG, DEFAULT_THRESHOLD).unwrap();
    let ends = suggest(&frame, 0, DEFAULT_THRESHOLD).unwrap();
    let own = median(&frame.channel(0)).unwrap();
    let anchor = median(&frame.channel(1)).unwrap();
    let unbounded = [stops_for_shift((anchor - own) / 1023.0, NEG), 0.0, 0.0];
    let after = piled(&simulate(&brink, held), 0);
    let without = piled(&simulate(&brink, unbounded), 0);
    let room = stops_for_shift(-ends.black, NEG);
    assert!((held[0] - room).abs() < 1e-4 && held[0] < unbounded[0], "held at {}, room {room}", held[0]);
    // Twin: without the bound the whole tail leaves the window.
    assert!(without > after + share / 2.0 && after < share / 10.0);
}
