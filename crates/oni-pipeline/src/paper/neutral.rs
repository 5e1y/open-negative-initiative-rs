//! The neutral paper the enlarger prints on with Print on paper off: no paper's data, each layer exposed by its own
//! channel and printing its own channel alone, its curve ONI's print at rest, the same on the three.

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
    Sheet::new(
        std::array::from_fn(|i| [-light(share(i)).max(NEUTRAL_DEEPEST).log10(); 3]),
        grey - 1.0,
        1.0 / last,
        (0..3).map(dye).collect(),
        (0..3).map(unit).collect(),
        OWN_LAYERS,
    )
}

/// The neutral paper, baked at first use; its axis scales with the frame's range.
pub fn sheet() -> &'static Sheet {
    static SHEET: OnceLock<Sheet> = OnceLock::new();
    SHEET.get_or_init(sheet_of)
}
