//! A digital photo's film effects in named levels, the same as the film files' (owner's call): few states, each a rest
//! a film can take, so the scan each lays can be made before the hand asks for it.

use crate::virtual_film::VirtualStock;
use crate::PipelineSettings;

/// Grain's amounts: none, then the five the films' files hold, the survey's intensities 1 to 5 over `grain::GRAIN_TOP`.
pub const GRAIN_AMOUNT: [f32; 6] = [0.0, 0.11, 0.1719, 0.2545, 0.3783, 0.5708];
/// Grain's widths on the film, in micrometres, the survey's sizes 1 to 5.
pub const GRAIN_SIZE: [f32; 5] = [19.0, 24.0, 30.0, 38.0, 48.0];
/// Halation's amounts: none, low, medium and high, as the films' files name them.
pub const HALATION_AMOUNT: [f32; 4] = [0.0, 0.10, 0.31, 0.92];
/// Halation's widths on the film, in micrometres, the files' own in the middle.
pub const HALATION_SIZE: [f32; 3] = [100.0, 200.0, 400.0];
/// Bloom on a film whose sheet gives no MTF: none, then three fixed widths in micrometres.
pub const BLOOM_UNSHEETED: [f32; 4] = [0.0, 2.5, 3.4, 4.5];
/// Bloom on a film whose sheet gives its MTF, as shares of that width: none, the sheet's, half again and twice.
pub const BLOOM_SHARES: [f32; 4] = [0.0, 1.0, 1.5, 2.0];
/// Shooting exposure's notches, in stops: a third of a stop from two under to two over.
pub const EXPOSURE_THIRDS: i32 = 6;

/// Bloom's widths on `stock`: its sheet's own and its multiples where its file gives one, else the fixed ones.
pub fn bloom(stock: VirtualStock) -> [f32; 4] {
    let sheet = stock.effects().bloom.size;
    if sheet > 0.0 { BLOOM_SHARES.map(|k| k * sheet) } else { BLOOM_UNSHEETED }
}

/// Shooting exposure's notches, from two stops under to two over.
pub fn exposure() -> Vec<f32> {
    (-EXPOSURE_THIRDS..=EXPOSURE_THIRDS).map(|i| i as f32 / 3.0).collect()
}

/// The index of the level nearest `value`.
pub fn nearest(levels: &[f32], value: f32) -> usize {
    (0..levels.len()).min_by(|&a, &b| (levels[a] - value).abs().total_cmp(&(levels[b] - value).abs())).unwrap_or(0)
}

/// `levels`' value nearest `value`.
pub fn snapped(levels: &[f32], value: f32) -> f32 {
    levels[nearest(levels, value)]
}

/// `s` with every film effect and its shooting exposure on its nearest level; a photo on no virtual film as it is.
pub fn on_levels(s: &PipelineSettings) -> PipelineSettings {
    let Some(film) = s.film() else { return s.clone() };
    let mut out = s.clone();
    out.grain.amount = snapped(&GRAIN_AMOUNT, s.grain.amount);
    out.grain.size = snapped(&GRAIN_SIZE, s.grain.size);
    out.halation.amount = snapped(&HALATION_AMOUNT, s.halation.amount);
    out.halation.size = snapped(&HALATION_SIZE, s.halation.size);
    out.bloom.size = snapped(&bloom(film.stock), s.bloom.size);
    if let Some(f) = out.virtual_film.as_mut() {
        f.exposure = snapped(&exposure(), f.exposure);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConversionMode;

    #[test]
    fn every_films_rests_lie_on_the_levels_and_a_value_off_them_snaps_to_the_nearest() {
        for stock in VirtualStock::all() {
            let picked = PipelineSettings::rest(ConversionMode::Digital, true).picking(stock);
            assert_eq!(on_levels(&picked), picked, "{stock:?}'s rests lie on the levels");
        }
        let mut off = PipelineSettings::rest(ConversionMode::Digital, true).picking(VirtualStock::of("kodak-portra-800"));
        (off.grain.amount, off.halation.size) = (0.3, 260.0);
        off.virtual_film.as_mut().unwrap().exposure = 0.4;
        let on = on_levels(&off);
        assert_eq!((on.grain.amount, on.halation.size, on.film().unwrap().exposure), (0.2545, 200.0, 1.0 / 3.0));
        // Twin: a value already on a level stays where it is.
        assert_eq!(on_levels(&on), on);
        assert_ne!(on, off);
    }
}
