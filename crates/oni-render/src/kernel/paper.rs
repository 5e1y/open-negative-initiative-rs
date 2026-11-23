//! The paper: the enlarger's light through the negative's dyes exposes a colour paper's layers, which develop on their
//! curves; the print's dyes are read under D50, then its ends: the frame's pick on its anchor, the paper's white held.
//! Brightness moves the enlarger's light as a time does, faded in from the pick over the deep shadows: the blacks stay,
//! the mids move. Lighting's other sliders and the colour tools act on the negative before it (`negative`).

use super::*;

/// Most wavelengths either sum runs over: every sample of 380 to 780 nm by 5.
pub const PAPER_BANDS: usize = 81;

/// The nodes of each layer's characteristic curve, on a uniform log exposure axis.
pub const PAPER_CURVE_NODES: usize = 256;

/// The paper's raw exposure floor in its log, as its curves read a layer no light reached.
pub const PAPER_EXPOSURE_FLOOR: f32 = 1e-10;

const LOG2_10: f32 = std::f32::consts::LOG2_10;
const LOG10_2: f32 = std::f32::consts::LOG10_2;
const LOG10_E: f32 = std::f32::consts::LOG10_E;

/// A negative printed on a paper: how its pixel is read as dye amounts, the enlarger's light, the paper's curves, its
/// print seen under D50 in Rec. 2020, and the print's ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paper {
    /// The input read as density above the bare film: `−log10((s + pedestal)·gain) − black`, held at the guard.
    pub gain: V3,
    pub pedestal: f32,
    pub log_guard: f32,
    pub black: V3,
    /// Per wavelength the negative passes: its three dyes' density at unit amount, then its base's.
    pub negative: &'static [V4],
    /// Per wavelength the negative passes: the lamp's light read by each of the paper's three layers.
    pub exposure: &'static [V3],
    /// Per wavelength the negative passes: the cyan, magenta and yellow dichroic filters' transmittance.
    pub dichroic: &'static [V3],
    /// Each dichroic filter's transmittance where it absorbs, `10^(−CC/100)`: the enlarger's filtration.
    pub filters: V3,
    /// The print's exposure as a factor of the light: the enlarger's time.
    pub scale: f32,
    /// Brightness, in stops of the enlarger's light; the pick's mean log exposure; the reach, how far from it
    /// toward the light a tone's mean log exposure lies when Brightness moves it as far as Exposure would.
    pub brightness: f32,
    pub pick_log: f32,
    pub reach: f32,
    /// Each layer's density along the log exposure from `curve_low` by `curve_step`, held at either end.
    pub curves: &'static [V3; PAPER_CURVE_NODES],
    pub curve_low: f32,
    pub curve_step: f32,
    /// Per wavelength the print reflects: its three dyes' density at unit amount, then its base's.
    pub print: &'static [V4],
    /// Per wavelength the print reflects: D50 read by the 1931 observer, adapted to D65, in Rec. 2020.
    pub view: &'static [V3],
    /// The print of the frame's pick without Lighting's points, and the paper's own white: the ends the print's light is
    /// read between.
    pub pick: V3,
    pub white: V3,
    /// Where the pick lands: pure black, or where the enlarger's rest prints it for Paper blacks.
    pub anchor: V3,
    /// A black and white print reads its luma on its three channels.
    pub luma: bool,
}

impl Paper {
    pub fn head(&self) -> [u32; 4] {
        [self.negative.len() as u32, self.print.len() as u32, 0, 0]
    }

    /// The uniform: the reading, the enlarger, Brightness, the print's ends, then the bands and the curves.
    pub fn rows(&self) -> Vec<V4> {
        let flag = |on: bool| if on { 1.0 } else { 0.0 };
        let mut rows = vec![
            v3(self.gain, self.pedestal),
            v3(self.black, self.log_guard),
            v3(self.filters, self.scale),
            [self.pick_log, self.reach, 0.0, 0.0],
            [self.curve_low, self.curve_step, self.brightness, flag(self.luma)],
            v3(self.pick, 0.0),
            v3(self.white, 0.0),
            v3(self.anchor, 0.0),
        ];
        let pad = |n: usize| std::iter::repeat_n([0.0; 4], PAPER_BANDS - n);
        rows.extend(self.negative.iter().copied().chain(pad(self.negative.len())));
        rows.extend(self.exposure.iter().map(|w| v3(*w, 0.0)).chain(pad(self.exposure.len())));
        rows.extend(self.dichroic.iter().map(|w| v3(*w, 0.0)).chain(pad(self.dichroic.len())));
        rows.extend(self.print.iter().copied().chain(pad(self.print.len())));
        rows.extend(self.view.iter().map(|w| v3(*w, 0.0)).chain(pad(self.view.len())));
        rows.extend(self.curves.iter().map(|c| v3(*c, 0.0)));
        rows
    }

    /// The dye amounts above the bare film a pixel holds, as the paper reads it.
    pub fn amounts(&self, s: V4) -> V3 {
        map(rgb(s), |c, v| -((v + self.pedestal) * self.gain[c]).max(self.log_guard).ln() * LOG10_E - self.black[c])
    }

    /// Each layer's log exposure under the negative holding `amounts`, before the grade turns it.
    pub fn exposed(&self, amounts: V3) -> V3 {
        let mut raw = [0.0f32; 3];
        for ((dye, weight), filter) in self.negative.iter().zip(self.exposure).zip(self.dichroic) {
            let dimmed = map(*filter, |c, f| 1.0 - (1.0 - f) * (1.0 - self.filters[c]));
            let passed = (-LOG2_10 * (dot(amounts, rgb(*dye)) + dye[3])).exp2() * dimmed[0] * dimmed[1] * dimmed[2];
            raw = map(raw, |c, v| v + passed * weight[c]);
        }
        map(raw, |_, v| (v.max(0.0) * self.scale + PAPER_EXPOSURE_FLOOR).ln() * LOG10_E)
    }

    /// How far the mean of `log` lies under the pick's `pick_log`, toward a lighter print.
    pub fn toward_light(&self, pick_log: f32, log: V3) -> f32 {
        pick_log - (log[0] + log[1] + log[2]) / 3.0
    }

    /// Each layer's log exposure moved by Brightness as Exposure moves it, by the same amount on the three, as far as
    /// its mean lies from the pick's toward the light: `lifted`.
    pub fn brightened(&self, log: V3) -> V3 {
        if self.brightness == 0.0 {
            return log;
        }
        let x = self.toward_light(self.pick_log, log);
        let moved = lifted(x, self.brightness * LOG10_2, self.reach) - x;
        log.map(|v| v - moved)
    }

    /// Each layer's density at log exposure `log`.
    pub fn developed(&self, log: V3) -> V3 {
        map(log, |c, v| {
            let f = clamp((v - self.curve_low) / self.curve_step, 0.0, (PAPER_CURVE_NODES - 1) as f32);
            let i = f.floor().min((PAPER_CURVE_NODES - 2) as f32);
            let at = i as usize;
            mix(self.curves[at][c], self.curves[at + 1][c], f - i)
        })
    }

    /// The print holding `density` of each dye, seen under D50, in linear Rec. 2020.
    pub fn viewed(&self, density: V3) -> V3 {
        let mut out = [0.0f32; 3];
        for (dye, weight) in self.print.iter().zip(self.view) {
            let t = (-LOG2_10 * (dot(density, rgb(*dye)) + dye[3])).exp2();
            out = map(out, |c, v| v + t * weight[c]);
        }
        out
    }

    /// The print of a negative holding `amounts`, a black and white print on its luma; before its ends.
    pub fn printed(&self, amounts: V3) -> V3 {
        let light = self.viewed(self.developed(self.brightened(self.exposed(amounts))));
        if self.luma { [dot(light, LUMA); 3] } else { light }
    }

    /// Each channel laid from the printed pick K on the anchor A, the white W held: a straight line in light through
    /// both, under K one onto no light, so the print stays above none.
    pub fn ended(&self, light: V3) -> V3 {
        if self.anchor == self.pick {
            return light;
        }
        map(light, |c, v| {
            let (k, a, w) = (self.pick[c], self.anchor[c], self.white[c]);
            if v <= k { v * a / k } else { a + (w - a) * ((v - k) / (w - k)) }
        })
    }

    /// What the print shows of a negative holding `amounts`: its light between its ends.
    pub fn shown(&self, amounts: V3) -> V3 {
        self.ended(self.printed(amounts))
    }
}

/// The share of Brightness's shift a tone takes `x` of log exposure from the pick toward the light: none at the pick,
/// the whole from `width` on, a smoothstep between so the print's slope never jumps.
pub fn share(x: f32, width: f32) -> f32 {
    let t = clamp(x / width, 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The fade of a `lift` in log exposure: its `reach`, never under √stops stops, so every slope stays within
/// 1 + 1.5·√stops and its inverse, 4 and ¼ at ±4: a layer up to ¾ slower than its tone's mean never turns back.
pub fn fade(lift: f32, reach: f32) -> f32 {
    reach.max((lift.abs() * LOG10_2).sqrt())
}

/// `x` moved by `lift` times its share; a fall is that brightening's inverse, found by halving, so no tone passes
/// another and −b undoes +b.
pub fn lifted(x: f32, lift: f32, reach: f32) -> f32 {
    let width = fade(lift, reach);
    if lift >= 0.0 {
        return x + lift * share(x, width);
    }
    let fall = -lift;
    if x <= 0.0 {
        return x;
    }
    if x >= width + fall {
        return x - fall;
    }
    let (mut low, mut high) = ((x - fall).max(0.0), x.min(width));
    for _ in 0..24 {
        let mid = 0.5 * (low + high);
        if mid + fall * share(mid, width) < x { low = mid } else { high = mid }
    }
    0.5 * (low + high)
}

/// One negative pixel printed on the paper, in linear Rec. 2020; alpha kept.
pub fn paper(s: V4, k: &Paper) -> V4 {
    with_alpha(k.shown(k.amounts(s)), s[3])
}

#[cfg(test)]
mod tests;
