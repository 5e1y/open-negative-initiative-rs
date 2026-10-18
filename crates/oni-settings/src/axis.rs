//! The density axis and the window a measurement is graduated on: the constants every rest value
//! and migration is written in.

use crate::modes::ConversionMode;
use std::ops::RangeInclusive;

/// The pipeline's own constants.
pub mod pipeline {
    use super::density_axis;

    /// Top of the density axis: the positive branch's pivot and the levels' resting white.
    pub const DMAX: f32 = 3.0;
    pub const LUMA_WEIGHTS: [f32; 3] = [0.2627, 0.6780, 0.0593];
    /// Largest radius at which a Gaussian blur returns its input bit for bit.
    pub const BLUR_FLOOR: f64 = 0.1614;
    /// Long side of the preview copy.
    pub const PREVIEW_SIDE: f64 = 2000.0;

    pub fn tpedestal() -> f32 {
        density_axis::pedestal(density_axis::base())
    }

    pub fn tmin() -> f32 {
        density_axis::log_guard(density_axis::base())
    }

    /// The output clip, the only place information is bounded.
    pub fn clip(e: f32) -> f32 {
        e.clamp(0.0, 1.0)
    }

    /// Largest full-resolution radius a fitted preview leaves inert.
    pub fn preview_visible_radius() -> f64 {
        BLUR_FLOOR * crate::codec::RADIUS_REFERENCE_SIDE / PREVIEW_SIDE
    }

    pub fn luma(rgb: [f32; 3]) -> f32 {
        rgb[0] * LUMA_WEIGHTS[0] + rgb[1] * LUMA_WEIGHTS[1] + rgb[2] * LUMA_WEIGHTS[2]
    }
}

/// Top of the density axis, dialled per photograph: the pedestal lifting a decode off zero and the
/// guard under it both come from it.
pub mod density_axis {
    use super::*;

    /// How far under the ceiling the guard sits, in density: `log10(8)`.
    pub fn guard_offset() -> f32 {
        8f64.log10() as f32
    }

    pub fn pedestal(ceiling: f32) -> f32 {
        10f32.powf(-ceiling)
    }

    /// An eighth of the pedestal, exact in binary so −3 stops of gain reach it.
    pub fn log_guard(ceiling: f32) -> f32 {
        pedestal(ceiling) / 8.0
    }

    const HIGHEST: f32 = 2.6;

    /// A scanner file's range: its floor is where the guard meets the resting white.
    pub fn range() -> RangeInclusive<f32> {
        (pipeline::DMAX - guard_offset())..=HIGHEST
    }

    pub fn mosaic_range() -> RangeInclusive<f32> {
        *range().start()..=4.0
    }

    pub fn range_for(mosaic: bool) -> RangeInclusive<f32> {
        if mosaic { mosaic_range() } else { range() }
    }

    /// Rest of every file the RAW engine does not read, on the range's floor.
    pub fn base() -> f32 {
        *range().start()
    }

    /// A mosaic negative's rest: a pedestal of 3.2·10⁻⁴, at the veil a copy stand's stray light leaves.
    pub const MOSAIC_REST: f32 = 3.5;

    /// A positive rests on `base` whatever read it: past a ceiling of 3 its shadows leave the window.
    pub fn rest(mosaic: bool, mode: ConversionMode) -> f32 {
        if mosaic && mode != ConversionMode::Positive { MOSAIC_REST } else { base() }
    }

    pub fn carried(ceiling: f32, mosaic: bool, from: ConversionMode, to: ConversionMode) -> f32 {
        if ceiling == rest(mosaic, from) { rest(mosaic, to) } else { ceiling }
    }

    /// True where raising the ceiling brightens the render, the opposite of stage 4's sense.
    pub fn brightens(mode: ConversionMode) -> bool {
        mode.invert_flag() > 0.5
    }
}

/// The window of the density axis a measurement is counted on, mapped onto 0…1.
pub mod graduation {
    use super::*;
    use crate::levels::Levels;

    /// How far under zero the window reaches.
    pub const OVERSHOOT: f32 = 0.35;

    /// The densest value the axis can express, tied to the guard at the resting ceiling.
    pub fn top() -> f32 {
        -pipeline::tmin().log10()
    }

    pub fn span() -> f32 {
        top() + OVERSHOOT
    }

    /// Mirrors with the kernel's `invertFlag`: the positive branch re-inverts on `DMAX`.
    pub fn window(mode: ConversionMode) -> RangeInclusive<f32> {
        if mode.invert_flag() > 0.5 {
            (-OVERSHOOT)..=top()
        } else {
            (pipeline::DMAX - top())..=(pipeline::DMAX + OVERSHOOT)
        }
    }

    pub fn levels(mode: ConversionMode) -> Levels {
        let w = window(mode);
        Levels { black: *w.start(), white: *w.end(), ..Levels::default() }
    }

    pub fn fraction(value: f32, mode: ConversionMode) -> f32 {
        (value - window(mode).start()) / span()
    }

    pub fn value(fraction: f32, mode: ConversionMode) -> f32 {
        window(mode).start() + fraction * span()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lowest density a test film holds, which the window has to reach in both modes.
    const TEST_FILM_LOW: f32 = -0.243;

    #[test]
    fn the_graduation_reaches_under_the_lowest_density_a_test_film_holds() {
        let negative = graduation::window(ConversionMode::Negative);
        assert!(negative.contains(&TEST_FILM_LOW));
        assert!(graduation::OVERSHOOT / -TEST_FILM_LOW > 1.44, "the overshoot clears that film by 44 %");
        // Stage 6 mirrors the axis on DMAX in positive and stretches nothing.
        let positive = graduation::window(ConversionMode::Positive);
        assert!(positive.contains(&(pipeline::DMAX - TEST_FILM_LOW)));
        for window in [&negative, &positive] {
            assert!((window.end() - window.start() - graduation::span()).abs() < 1e-6);
        }
        for mode in [ConversionMode::Negative, ConversionMode::Positive] {
            for f in [0.0, 0.37, 1.0] {
                assert!((graduation::fraction(graduation::value(f, mode), mode) - f).abs() < 1e-6);
            }
        }
        // Twin: a window starting at zero density puts that film's lowest value under bin 0.
        assert!((TEST_FILM_LOW - 0.0) / graduation::top() < 0.0 && graduation::fraction(TEST_FILM_LOW, ConversionMode::Negative) > 0.0);
    }
}
