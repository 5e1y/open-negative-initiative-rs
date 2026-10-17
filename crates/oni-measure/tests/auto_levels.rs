//! `AutoLevels.selfCheck`: the suggestion's threshold, the alignment, the placed handles, the two
//! steps on simulated densities and a selection's two strategies, each with its twin.

use oni_core::balance::{self as core_balance, Strategy, Suggestion};
use oni_measure::Histogram;
use oni_measure::auto_levels::*;
use oni_settings::axis::{graduation, pipeline};
use oni_settings::blocks::Gains;
use oni_settings::cg::{Point, Rect};
use oni_settings::density_migration;
use oni_settings::levels::{Levels, LevelsChannel, LevelsSet};
use oni_settings::{ConversionMode, PipelineSettings};

const NEG: ConversionMode = ConversionMode::Negative;

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
            let m = density_migration::new([t; 3], [stops[c]; 3], &measuring, NEG, pipeline::tpedestal(), pipeline::tmin());
            let bin = (m[0].min(1.0) * 1023.0).round() as i64;
            counts[bin.clamp(0, 1023) as usize][c] += 1.0;
        }
    }
    Histogram { rgb: counts.clone(), luma: counts.iter().map(|b| b[1]).collect(), ends: None }
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

    for (name, raws, frame) in [("sane", &sane, simulate(&sane, [0.0; 3])), ("broken", &broken, broken_frame.clone())] {
        let one = placement(&frame, DEFAULT_THRESHOLD).unwrap();
        let two = two_steps(&frame, [0.0; 3], NEG, DEFAULT_THRESHOLD, |s| simulate(raws, s)).unwrap();
        let before = disagreement(&outcomes(raws, [0.0; 3], &handles(&one)));
        let after = disagreement(&outcomes(raws, two.stops, &handles(&two.levels)));
        if name == "broken" {
            let noise = 3.0 * one_bin * graduation::span() / spread;
            assert!(after < noise && after < before / 10.0, "broken: {before} to {after}");
            assert_eq!(one[2].white, 1.0, "one step reads blue's white off the pile");
            let expected = raws[2].iter().filter(|&&d| d > graduation::top()).count() as f32 / raws[2].len() as f32;
            let recovered = piled(&simulate(raws, two.stops), 2);
            assert!((broken_pile - expected).abs() < 0.01 && recovered < broken_pile / 10.0);
            let own = median(&frame.channel(2)).unwrap();
            let anchor = median(&frame.channel(1)).unwrap();
            let asked = stops_for_shift((anchor - own) * one_bin, NEG);
            assert!((asked - 5.0).abs() < 0.03 && two.stops[2] == Gains::RANGE.1, "truncated, not refused: {asked}");
        } else {
            assert!((after - before).abs() < 5e-3, "step one changes nothing on a sane scene");
            let sane_pile: Vec<f32> = (0..3).map(|c| piled(&simulate(raws, two.stops), c)).collect();
            assert!((0..3).all(|c| sane_pile[c] <= piled(&frame, c) + 1e-3));
            let single = handles(&one);
            let mapped: Vec<Levels> = (0..3).map(|c| two.shown(c, NEG)).collect();
            let density = one_bin * graduation::span();
            let drift = (0..3).map(|c| (mapped[c].black - single[c].black).abs().max((mapped[c].white - single[c].white).abs())).fold(0.0, f32::max);
            assert!(drift < 2.0 * density, "mapped back, the markers land on a single step's");
            // Twin: without the mapping they are out by step one's own 1.5 stops.
            let unread = handles(&two.levels);
            let unmapped = (0..3).map(|c| (unread[c].black - single[c].black).abs().max((unread[c].white - single[c].white).abs())).fold(0.0, f32::max);
            let owed = shift(1.5, NEG).abs() * graduation::span();
            assert!((unmapped - owed).abs() < 2.0 * density);
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

fn run(frames: &[(&str, PipelineSettings)], strategy: Strategy, scenes: &dyn Fn(&str) -> Option<[Vec<f32>; 3]>) -> Result<Vec<(String, PipelineSettings)>, (String, usize)> {
    let measure = |path: &str, s: &PipelineSettings| -> Option<Balance> {
        let raws = scenes(path)?;
        let stops = s.gains.stops;
        two_steps(&simulate(&raws, stops), stops, NEG, DEFAULT_THRESHOLD, |st| simulate(&raws, st))
    };
    let mut landed = Vec::new();
    match strategy {
        Strategy::Each => {
            for (path, s) in frames {
                let b = measure(path, s).ok_or((path.to_string(), landed.len()))?;
                landed.push((path.to_string(), applied(&b, s, None)));
            }
        }
        Strategy::Average => {
            let measured: Vec<Balance> = frames.iter().map(|(p, s)| measure(p, s).ok_or((p.to_string(), 0))).collect::<Result<_, _>>()?;
            let n = measured.len() as f32;
            let levels = [0, 1, 2].map(|c| mean(&measured.iter().map(|b| b.levels[c]).collect::<Vec<_>>()).unwrap());
            let sum = |f: fn(&Balance) -> [f32; 3]| measured.iter().fold([0.0; 3], |a, b| [0, 1, 2].map(|c| a[c] + f(b)[c])).map(|v| v / n);
            let shared = Balance { stops: sum(|b| b.stops), levels, shift: sum(|b| b.shift) };
            for (path, s) in frames {
                landed.push((path.to_string(), applied(&shared, s, None)));
            }
        }
    }
    Ok(landed)
}

#[test]
fn a_selection_balances_each_or_on_the_mean_and_an_unreadable_frame_stops_it() {
    let green = cloud(1.2, 1.2, 4000);
    let first = [green.clone(), green.clone(), shifted(&green, -1.5)];
    let second = [shifted(&green, -1.0), green.clone(), green.clone()];
    let scenes = move |p: &str| match p {
        "A" => Some(first.clone()),
        "B" => Some(second.clone()),
        _ => None,
    };
    let mut other = PipelineSettings::default();
    other.geometry.crop = Rect::new(0.05, 0.1, 0.8, 0.7);
    other.manual_curves.linked.points = vec![Point::new(0.0, 0.0), Point::new(0.3, 0.5), Point::new(1.0, 1.0)];
    let roll = [("A", PipelineSettings::default()), ("B", other.clone())];
    let each = run(&roll, Strategy::Each, &scenes).unwrap();
    let averaged = run(&roll, Strategy::Average, &scenes).unwrap();
    let (ea, eb, aa, ab) = (&each[0].1, &each[1].1, &averaged[0].1, &averaged[1].1);
    assert_ne!(ea.gains.stops, eb.gains.stops, "one per image: each view has its own gains");
    let middle = [0, 1, 2].map(|c| (ea.gains.stops[c] + eb.gains.stops[c]) / 2.0);
    let same_levels = LevelsChannel::PER_CHANNEL.iter().all(|&c| aa.levels.get(c) == ab.levels.get(c));
    assert!(aa.gains == ab.gains && same_levels && (0..3).all(|c| (aa.gains.stops[c] - middle[c]).abs() < 1e-5));
    // Twin: the mean is neither view's own correction.
    assert!(aa != ea && ab != eb);
    assert!(ab.geometry == other.geometry && eb.geometry == other.geometry && ab.manual_curves == other.manual_curves && eb.manual_curves == other.manual_curves);

    let hole = [("A", PipelineSettings::default()), ("introuvable", PipelineSettings::default()), ("B", PipelineSettings::default())];
    assert_eq!(run(&hole, Strategy::Each, &scenes).unwrap_err(), ("introuvable".to_string(), 1), "only the frame before was written");
    // Twin: averaging reads every frame first, so none is written.
    assert_eq!(run(&hole, Strategy::Average, &scenes).unwrap_err().1, 0);
}
