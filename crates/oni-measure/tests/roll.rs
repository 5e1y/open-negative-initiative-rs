//! `RollReference.selfCheck`'s combining claims: the roll outvotes one bad frame, a picked base is
//! every channel's black, gains and medians move as the tails do; each claim beside its twin.

use oni_core::balance::{Suggestion, written};
use oni_measure::auto_levels::{Balance, shift};
use oni_measure::roll::{Measured, combined};
use oni_pipeline::conversion::{colour_factor, decompression_weights};
use oni_settings::axis::density_axis;
use oni_settings::frame::RollReference;
use oni_settings::{ConversionMode, Id, PipelineSettings};

const NEGATIVE: ConversionMode = ConversionMode::Negative;

fn rest() -> [Suggestion; 3] {
    [Suggestion { black: 0.0, white: 1.0, mid: 0.5, shadows: 0.0, highlights: 0.0 }; 3]
}

fn measured(stops: [f32; 3], tails: [[f32; 2]; 3], median: Option<[f32; 3]>) -> Measured {
    Measured { path: String::new(), balance: Balance { stops, levels: rest(), shift: [0.0; 3] }, tails, median }
}

fn frame(white: f32, stops: [f32; 3]) -> Measured {
    measured(stops, [[0.10, white], [0.12, white], [0.15, white]], None)
}

#[test]
fn one_leaked_frame_of_four_is_outvoted_and_two_frames_drop_nothing() {
    let z = [0.0; 3];
    let leaked = combined(&[frame(0.61, z), frame(0.66, z), frame(0.64, z), frame(0.97, z)], NEGATIVE, None).unwrap();
    assert_eq!(leaked.white[0], 0.66, "the leak leaves the white on the next frame");
    // Twin: under three frames nothing is dropped, so the same leak sets it.
    assert_eq!(combined(&[frame(0.61, z), frame(0.97, z)], NEGATIVE, None).unwrap().white[0], 0.97);
}

#[test]
fn a_picked_base_is_every_channel_s_black_and_reaches_the_roll_s_gains() {
    let dark = |lows: [f32; 3], stops: [f32; 3]| measured(stops, [0, 1, 2].map(|c| [lows[c], 0.8]), None);
    let lows = [[0.12, 0.17, 0.16], [0.10, 0.12, 0.19], [0.03, 0.30, 0.30], [0.20, 0.22, 0.24]];
    let frames: Vec<Measured> = lows.iter().map(|l| dark(*l, [0.0; 3])).collect();
    let picked = [0.14, 0.09, 0.13];
    let bare = combined(&frames, NEGATIVE, None).unwrap();
    let based = combined(&frames, NEGATIVE, Some(picked)).unwrap();
    assert!(based.black == picked && based.base == Some(picked) && based.white == bare.white);
    // Twin: without it the roll's blacks stand off the film's own zero.
    assert!(bare.black != picked && bare.base.is_none(), "{:?}", bare.black);
    assert_eq!(bare.black, [0.10, 0.17, 0.19], "each black on the second darkest frame");

    let gained: Vec<Measured> = lows.iter().map(|l| dark(*l, [0.5, 0.0, -0.3])).collect();
    let landed = combined(&gained, NEGATIVE, Some(picked)).unwrap().black;
    let expected = [picked[0] + shift(0.5, NEGATIVE), picked[1], picked[2] + shift(-0.3, NEGATIVE)];
    assert!((0..3).all(|c| (landed[c] - expected[c]).abs() < 1e-6), "{landed:?} against {expected:?}");
    // Twin: the base left at no gain would miss the roll's gains by a whole shift.
    assert!((landed[0] - picked[0]).abs() > 0.01);
}

#[test]
fn the_roll_s_gains_are_each_channel_s_median_and_the_tails_move_with_them() {
    let moved = combined(
        &[frame(0.70, [0.2, 0.0, -0.4]), frame(0.50, [0.6, 0.0, 0.0]), frame(0.80, [1.0, 0.0, 0.2])],
        NEGATIVE,
        None,
    )
    .unwrap();
    let expected = 0.70 + shift(0.6 - 0.2, NEGATIVE);
    assert_eq!(moved.stops, [0.6, 0.0, 0.0]);
    assert_eq!(moved.white[0], expected, "the tail moves to {expected}");
    // Twin: left where its own gains read it, the tail would stay at 0.70.
    assert_ne!(moved.white[0], 0.70);
}

#[test]
fn the_roll_s_median_is_the_middle_frame_s_moved_and_one_frame_without_its_own_gives_none() {
    let tails = [[0.1, 0.8]; 3];
    let centres = [
        measured([0.2, 0.0, 0.1], tails, Some([0.41, 0.43, 0.47])),
        measured([0.4, 0.0, -0.2], tails, Some([0.37, 0.39, 0.44])),
        measured([0.3, 0.0, 0.0], tails, Some([0.45, 0.42, 0.49])),
    ];
    let roll = combined(&centres, NEGATIVE, None).unwrap();
    let wanted = [0, 1, 2].map(|c| {
        let mut moved: Vec<f32> =
            centres.iter().map(|f| f.median.unwrap()[c] + shift(roll.stops[c] - f.balance.stops[c], NEGATIVE)).collect();
        moved.sort_by(|a, b| a.total_cmp(b));
        moved[1]
    });
    let centre = roll.median.unwrap();
    assert!((0..3).all(|c| (centre[c] - wanted[c]).abs() < 1e-6), "{centre:?} against {wanted:?}");
    // Twin: one frame without its own leaves the roll without a median rather than a skewed one.
    let mut short = centres[..2].to_vec();
    short.push(measured([0.0; 3], tails, None));
    assert!(combined(&short, NEGATIVE, None).unwrap().median.is_none());
}

#[test]
fn a_frame_on_the_roll_s_window_carries_its_colour_whole_and_a_narrower_one_by_its_span() {
    let reference = RollReference { id: Id::new(), stops: [0.0; 3], black: [0.19, 0.23, 0.31], white: [0.53, 0.58, 0.67], base: None, median: None };
    let base = PipelineSettings { separation: 0.0, density_ceiling: density_axis::MOSAIC_REST, ..PipelineSettings::default() };
    let mut on_roll = written(&reference, &base);
    on_roll.separation = 0.0;
    assert_eq!(decompression_weights(&on_roll), (1.0, 0.0), "the windows' colour whole, the film's none");
    let mut narrowed = on_roll.clone();
    for w in [&mut narrowed.levels.red, &mut narrowed.levels.green, &mut narrowed.levels.blue] {
        w.white -= 0.37 * (w.white - w.black);
    }
    let factor = colour_factor(&narrowed);
    assert!((factor - 0.63).abs() < 1e-5, "narrowed to 63 % of the span, the factor is {factor}");
    // Twin: off the roll the same narrowed frame carries no factor at all.
    let mut off = narrowed.clone();
    off.roll_reference = None;
    assert_eq!(colour_factor(&off), 1.0);
}
