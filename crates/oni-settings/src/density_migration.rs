//! Stages 4 to 6 in two closed forms, the arithmetic the kernel runs and the density-native one,
//! so the migration between them is proved against a known-good oracle rather than asserted.

use crate::axis::pipeline;
use crate::levels::{Levels, LevelsChannel, LevelsSet};
use crate::modes::ConversionMode;

type V3 = [f32; 3];

/// Top of the density axis and the pivot the positive branch re-inverts on.
pub const WINDOW: f32 = pipeline::DMAX;

/// The kernel's `applyLevels`; its span floor is the one place the identity is inexact.
pub fn apply_levels(e: f32, lv: &Levels) -> f32 {
    ((e - lv.black) / (lv.white - lv.black).max(1e-4)).max(0.0).powf(lv.gamma())
}

/// The kernel's `applyWindow`: the three-point pass, then the degree-4 Bezier on its two handles.
pub fn apply_window(e: f32, lv: &Levels) -> f32 {
    let t = apply_levels(e, lv);
    if lv.is_window_neutral() {
        return t;
    }
    let u = t.min(1.0);
    let v = 1.0 - u;
    let curve = lv.shadow_ordinate() * 4.0 * u * v * v * v
        + 0.5 * 6.0 * u * u * v * v
        + lv.highlight_ordinate() * 4.0 * u * u * u * v
        + u * u * u * u;
    curve + (t - u)
}

/// The per-channel window, the only pass whose input changes unit.
pub fn per_channel_window(e: V3, lv: &LevelsSet) -> V3 {
    [apply_window(e[0], &lv.red), apply_window(e[1], &lv.green), apply_window(e[2], &lv.blue)]
}

/// The density-native form: no clamp, a guard under the log, no division.
pub fn new(source: V3, stops: V3, levels: &LevelsSet, mode: ConversionMode, pedestal: f32, guard_floor: f32) -> V3 {
    let d: V3 = std::array::from_fn(|i| -((source[i] + pedestal) * stops[i].exp2()).max(guard_floor).log10());
    let e = if mode.invert_flag() > 0.5 { d } else { d.map(|v| WINDOW - v) };
    per_channel_window(e, levels)
}

/// Only the per-channel sets change unit; a median stored normalised between the ends follows alone.
pub fn migrate(levels: &LevelsSet) -> LevelsSet {
    let mut out = *levels;
    for channel in LevelsChannel::PER_CHANNEL {
        if let Some(lv) = out.get_mut(channel) {
            lv.black *= pipeline::DMAX;
            lv.white *= pipeline::DMAX;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::axis::density_axis;

    /// A literal, so the oracle cannot follow a moving resting ceiling.
    const FROZEN_PEDESTAL: f32 = 0.008;
    const AGREEMENT: f32 = 2e-4;
    const SAMPLES: usize = 4000;

    fn reference_ceiling() -> f32 {
        -FROZEN_PEDESTAL.log10()
    }

    fn frozen_window(e: V3, lv: &LevelsSet) -> V3 {
        [apply_levels(e[0], &lv.red), apply_levels(e[1], &lv.green), apply_levels(e[2], &lv.blue)]
    }

    /// The frozen kernel: transmittance clamped into `[Tmin, 1]`, density divided by `DMAX`.
    fn old(source: V3, stops: V3, levels: &LevelsSet, mode: ConversionMode) -> V3 {
        let d: V3 = std::array::from_fn(|i| {
            -((source[i] + FROZEN_PEDESTAL) * stops[i].exp2()).clamp(FROZEN_PEDESTAL / 8.0, 1.0).log10()
        });
        let e = if mode.invert_flag() > 0.5 {
            d.map(|v| v / pipeline::DMAX)
        } else {
            d.map(|v| (pipeline::DMAX - v) / pipeline::DMAX)
        };
        frozen_window(e, levels)
    }

    /// Swept past both ends of the window and off any round value; offsets decorrelate the channels.
    fn density(index: usize, offset: usize) -> f32 {
        let k = (index + offset) % SAMPLES;
        -0.6 + 4.2 * (k as f32 + 0.5) / SAMPLES as f32
    }

    fn grid_source(index: usize, gain: V3, pedestal: f32) -> (V3, V3) {
        let d = [density(index, 0), density(index, 1373), density(index, 2711)];
        (d, std::array::from_fn(|i| 10f32.powf(-d[i]) / gain[i] - pedestal))
    }

    fn frozen_rest() -> LevelsSet {
        LevelsSet { red: Levels::default(), green: Levels::default(), blue: Levels::default() }
    }

    fn lv(black: f32, white: f32, mid: f32) -> Levels {
        Levels { black, white, mid, ..Levels::default() }
    }

    fn scenes() -> Vec<(&'static str, LevelsSet, ConversionMode, V3)> {
        let mut placed = frozen_rest();
        placed.red = Levels::with_ends(0.0731, 0.8137);
        placed.green = Levels::with_ends(0.1279, 0.7411);
        placed.blue = Levels::with_ends(0.2113, 0.9337);
        let mut gammas = placed;
        gammas.red.mid = 0.3117;
        gammas.green.mid = 0.7213;
        gammas.blue.mid = 0.4409;
        let mut narrow = frozen_rest();
        narrow.red = lv(0.4133, 0.4933, 0.2311);
        narrow.green = lv(0.0117, 0.1017, 0.7911);
        narrow.blue = Levels::with_ends(0.8813, 0.9613);
        vec![
            ("rest", frozen_rest(), ConversionMode::Negative, [0.0; 3]),
            ("rest positive", frozen_rest(), ConversionMode::Positive, [0.0; 3]),
            ("placed", placed, ConversionMode::Negative, [1.5137, 0.0, -2.2531]),
            ("gammas", gammas, ConversionMode::Negative, [-0.7331, 0.4111, 2.9137]),
            ("narrow", narrow, ConversionMode::Negative, [0.1337, -1.7711, 3.0]),
            ("positive placed", placed, ConversionMode::Positive, [-3.0, 0.9131, 1.4477]),
            ("monochrome", gammas, ConversionMode::Monochrome, [-2.1137, 1.3311, 0.0]),
        ]
    }

    #[derive(Default)]
    struct Deviation {
        clipped: f32,
        bare: f32,
        pixels: usize,
    }

    impl Deviation {
        fn note(&mut self, before: f32, after: f32) {
            let gap = (pipeline::clip(before) - pipeline::clip(after)).abs();
            if gap > self.clipped {
                self.clipped = gap;
            }
            let raw = (before - after).abs();
            if raw.is_finite() && raw > self.bare {
                self.bare = raw;
            }
        }
    }

    /// Splits the grid between what the old `Tmax` clamp pinned in positive and the rest.
    fn deviation(preparing: &dyn Fn(&LevelsSet) -> LevelsSet, pedestal: f32, guard_floor: f32, capping: bool) -> (Deviation, Deviation) {
        let mut identity = Deviation::default();
        let mut pinned = Deviation::default();
        for (_, levels, mode, stops) in scenes() {
            let gain = stops.map(f32::exp2);
            let migrated = preparing(&levels);
            let positive = mode.invert_flag() < 0.5;
            for i in 0..SAMPLES {
                let (d, mut source) = grid_source(i, gain, pedestal);
                if capping && positive {
                    for c in 0..3 {
                        if (source[c] + pedestal) * gain[c] > 1.0 {
                            source[c] = 1.0 / gain[c] - pedestal;
                        }
                    }
                }
                let before = old(source, stops, &levels, mode);
                let after = new(source, stops, &migrated, mode, pedestal, guard_floor);
                let clamped = positive && d.iter().cloned().fold(f32::INFINITY, f32::min) < 0.0;
                let target = if clamped { &mut pinned } else { &mut identity };
                for c in 0..3 {
                    target.note(before[c], after[c]);
                }
                target.pixels += 1;
            }
        }
        (identity, pinned)
    }

    fn axis() -> (f32, f32) {
        let ceiling = reference_ceiling();
        (density_axis::pedestal(ceiling), density_axis::log_guard(ceiling))
    }

    #[test]
    fn the_migrated_windows_render_the_frozen_kernel() {
        let (ped, floor) = axis();
        let (identity, pinned) = deviation(&migrate, ped, floor, false);
        assert!(identity.clipped <= AGREEMENT, "post-clip {:e} over {} pixels", identity.clipped, identity.pixels);
        // The one population the `Tmax` clamp pinned is not an identity, by construction.
        assert!(pinned.bare > 0.1, "pinned moved only {}", pinned.bare);
        let (capped_identity, capped_pinned) = deviation(&migrate, ped, floor, true);
        assert!(capped_identity.clipped <= AGREEMENT && capped_pinned.clipped <= AGREEMENT);
        // Twin: the whole grid without the cap refuses.
        assert!(identity.bare.max(pinned.bare) > AGREEMENT);
    }

    #[test]
    fn twin_constants_quoted_to_three_decimals_are_a_different_kernel() {
        let (rounded, _) = deviation(&migrate, 10f32.powf(-2.097), 10f32.powf(-(2.097 + 0.903)), false);
        assert!(rounded.clipped > 50.0 * AGREEMENT, "{:e}", rounded.clipped);
    }

    #[test]
    fn twin_unmigrated_windows_diverge() {
        let (ped, floor) = axis();
        let (unmigrated, _) = deviation(&|l| *l, ped, floor, false);
        assert!(unmigrated.clipped > 0.5, "{}", unmigrated.clipped);
        let extreme = [10f32.powf(0.243) - ped; 3];
        let kept = old(extreme, [0.0; 3], &frozen_rest(), ConversionMode::Positive);
        let lost = new(extreme, [0.0; 3], &frozen_rest(), ConversionMode::Positive, ped, floor);
        assert!(lost[0] - kept[0] > 2.0, "{} against {}", lost[0], kept[0]);
    }

    #[test]
    fn migrate_scales_the_ends_and_moves_no_median() {
        let hand = LevelsSet { red: lv(0.1373, 0.7911, 0.3117), green: lv(0.0211, 0.9137, 0.6413), blue: lv(0.3111, 0.8813, 0.5) };
        let reads = |out: &LevelsSet| {
            LevelsChannel::PER_CHANNEL.iter().all(|&c| {
                let (o, h) = (out.get(c), hand.get(c));
                (o.black - h.black * WINDOW).abs() < 1e-6 && (o.white - h.white * WINDOW).abs() < 1e-6 && o.mid == h.mid
            })
        };
        let after = migrate(&hand);
        assert!(reads(&after));
        let mut further = after;
        further.red.mid *= 0.5;
        assert!(!reads(&further));
    }
}
