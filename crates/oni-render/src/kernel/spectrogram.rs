//! The spectrogram (`chroma.metal`): hue bands and luminance stops.

use super::*;

/// `spectrogram(src, hueLow, hueHigh, satLow, satHigh, lumaLow, lumaHigh)`: the eight bands' doses,
/// each already divided by its band's authority as `Spectrogram.packed` does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spectrogram {
    pub hue_low: V4,
    pub hue_high: V4,
    pub sat_low: V4,
    pub sat_high: V4,
    pub luma_low: V4,
    pub luma_high: V4,
}

impl Spectrogram {
    pub fn rows(&self) -> Vec<V4> {
        vec![self.hue_low, self.hue_high, self.sat_low, self.sat_high, self.luma_low, self.luma_high]
    }
}

const BAND_CENTRE: [f32; 8] = [0.0, 30.0, 60.0, 120.0, 180.0, 240.0, 270.0, 300.0];

const BAND_SPREAD: f32 = 1.8;

const LUMA_STOPS: f32 = 2.0;

fn band_weight(hue: f32, i: usize) -> f32 {
    let centre = BAND_CENTRE[i];
    let mut delta = hue - centre;
    if delta > 180.0 {
        delta -= 360.0;
    }
    if delta < -180.0 {
        delta += 360.0;
    }
    let neighbour = if delta >= 0.0 { (i + 1) & 7 } else { (i + 7) & 7 };
    let mut span = (BAND_CENTRE[neighbour] - centre).abs();
    if span > 180.0 {
        span = 360.0 - span;
    }
    smoothstep(0.0, 1.0, (1.0 - delta.abs() / (span * BAND_SPREAD)).max(0.0))
}

pub fn spectrogram(s: V4, k: &Spectrogram) -> V4 {
    let src = rgb(s);
    let y = dot(src, LUMA);
    let chroma = map(src, |_, c| c - y);
    let a = dot(chroma, [1.0, -0.5, -0.5]);
    let b = dot(chroma, [0.0, 0.866_025_4, -0.866_025_4]);
    let presence = smoothstep(0.0, 0.10 + 0.25 * y.max(0.0), (a * a + b * b).sqrt());
    if presence <= 0.0 {
        return s;
    }
    let mut hue = b.atan2(a) * 57.295_78;
    if hue < 0.0 {
        hue += 360.0;
    }
    let mut w_low: V4 = std::array::from_fn(|i| band_weight(hue, i));
    let mut w_high: V4 = std::array::from_fn(|i| band_weight(hue, i + 4));
    let total = w_low.iter().sum::<f32>() + w_high.iter().sum::<f32>();
    if total > 0.0 {
        w_low = w_low.map(|w| w / total);
        w_high = w_high.map(|w| w / total);
    }
    let shift = (dot4(w_low, k.hue_low) + dot4(w_high, k.hue_high)) * presence;
    let boost = (dot4(w_low, k.sat_low) + dot4(w_high, k.sat_high)) * presence;
    let lift = (dot4(w_low, k.luma_low) + dot4(w_high, k.luma_high)) * presence;

    let axis = [0.577_350_26_f32; 3];
    let angle = shift * 0.017_453_292;
    let (sn, c) = angle.sin_cos();
    let cross = [
        axis[1] * src[2] - axis[2] * src[1],
        axis[2] * src[0] - axis[0] * src[2],
        axis[0] * src[1] - axis[1] * src[0],
    ];
    let along = dot(axis, src);
    let turned = map(src, |i, v| v * c + cross[i] * sn + axis[i] * along * (1.0 - c));
    let restore = y - dot(turned, LUMA);
    let turned = map(turned, |_, v| v + restore);
    let gain = (1.0 + boost).max(0.0);
    let light = (lift * LUMA_STOPS).exp2();
    with_alpha(map(turned, |_, v| (y + (v - y) * gain) * light), s[3])
}
