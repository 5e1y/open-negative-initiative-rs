//! The neutral paper the enlarger prints on with Print on paper off: a paper as any other, its curve ONI's print at rest,
//! the same on the three layers.

use super::printing::{GREY_PRINT, bisected};
use super::*;
use oni_settings::levels::LevelsChannel;
use oni_settings::tone::Print;
use std::sync::OnceLock;

/// The neutral paper's deepest black as light, a density of 5: ONI's print lays none at its black, past any a screen shows.
pub const NEUTRAL_DEEPEST: f32 = 1e-5;

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
        OWN_LAYERS,
        PURE_DYES,
    )
}

/// The neutral paper, baked at first use; its axis scales with the frame's range.
pub fn sheet() -> &'static Sheet {
    static SHEET: OnceLock<Sheet> = OnceLock::new();
    SHEET.get_or_init(sheet_of)
}
