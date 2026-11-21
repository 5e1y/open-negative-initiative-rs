//! The gains' temperature and tint, read and moved across the three gains.

use oni_settings::blocks::Gains;

/// Unit moves of the gains' temperature and tint, each leaving the other and the mean where they are.
pub(super) const GAIN_TEMPERATURE_AXIS: [f32; 3] = [1.0, 0.0, -1.0];
pub(super) const GAIN_TINT_AXIS: [f32; 3] = [-1.0 / 3.0, 2.0 / 3.0, -1.0 / 3.0];

pub fn gain_temperature(g: [f32; 3]) -> f32 {
    (g[0] - g[2]) / 2.0
}

pub fn gain_tint(g: [f32; 3]) -> f32 {
    g[1] - (g[0] + g[2]) / 2.0
}

pub(super) fn shift_gains(g: &mut Gains, axis: [f32; 3], by: f32) {
    for (stop, a) in g.stops.iter_mut().zip(axis) {
        *stop = (*stop + by * a).clamp(Gains::RANGE.0, Gains::RANGE.1);
    }
}
