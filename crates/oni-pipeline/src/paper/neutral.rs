//! The neutral paper the enlarger prints on with Print on paper off: no paper's data, each layer exposed by its own dye
//! of the negative alone and printing its own channel alone, its curve ONI's print at rest, the same on the three.

use super::printing::{GREY_PRINT, bisected};
use super::*;
use oni_settings::levels::LevelsChannel;
use oni_settings::tone::Print;
use std::sync::OnceLock;

/// The neutral paper's deepest black as light, a density of 5: ONI's print lays none at its black, past any a screen shows.
pub const NEUTRAL_DEEPEST: f32 = 1e-5;

fn unit(c: usize) -> V3 {
    std::array::from_fn(|k| if k == c { 1.0 } else { 0.0 })
}

/// Dye `c` alone at unit density, on no base.
fn dye(c: usize) -> V4 {
    let [r, g, b] = unit(c);
    [r, g, b, 0.0]
}

/// ONI's print at rest as a characteristic curve, its log exposure in units of the frame's range: the range spans the
/// print from black to white as a window spans it, the reference grey at 0, where its share prints a plain RAW's grey.
fn sheet_of() -> Sheet {
    let rest = Print { exposure: crate::conversion::NEGATIVE_PRINT_STOPS, ..Print::default() };
    let light = |t: f32| rest.output(t, LevelsChannel::Linked);
    let grey = bisected(0.0, 1.0, *GREY_PRINT, light);
    let last = (PAPER_CURVE_NODES - 1) as f32;
    // More light prints darker.
    let share = |i: usize| 1.0 - i as f32 / last;
    Sheet {
        curves: std::array::from_fn(|i| [-light(share(i)).max(NEUTRAL_DEEPEST).log10(); 3]),
        curve_low: grey - 1.0,
        curve_step: 1.0 / last,
        print: (0..3).map(dye).collect(),
        view: (0..3).map(unit).collect(),
    }
}

/// The neutral paper, baked at first use; its axis scales with the frame's range.
pub fn sheet() -> &'static Sheet {
    static SHEET: OnceLock<Sheet> = OnceLock::new();
    SHEET.get_or_init(sheet_of)
}

/// The enlarger on the neutral paper: one band a dye, each layer reading its own behind the filter that holds it back;
/// a black and white negative's silver the mean of its three readings on each.
fn enlarging(silver: bool) -> Enlarger {
    let band = |c: usize| if silver { [1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0, 0.0] } else { dye(c) };
    Enlarger {
        negative: (0..3).map(band).collect(),
        exposure: (0..3).map(unit).collect(),
        dichroic: (0..3).map(|c| unit(c).map(|v| 1.0 - v)).collect(),
    }
}

/// The neutral paper's enlarger, baked at first use.
pub fn enlarger(silver: bool) -> &'static Enlarger {
    static TABLES: [OnceLock<Enlarger>; 2] = [const { OnceLock::new() }; 2];
    TABLES[usize::from(silver)].get_or_init(|| enlarging(silver))
}
