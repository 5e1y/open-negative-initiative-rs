//! The paper: each channel's window share laid back on density, the negative's per-channel printing density, exposes the
//! matching layer of a colour paper (red the cyan-forming, green the magenta, blue the yellow) through the enlarger's time
//! and filtration; the layers develop on their curves, the print's dyes are read under D50, then its ends: the frame's
//! pick and the paper's white each on its anchor. Brightness moves the enlarger's light as a time does, faded in from the
//! pick over the deep shadows. Lighting's other sliders and the colour tools act on the negative before it (`negative`).

use super::*;

/// Most wavelengths the print's sums run over: every sample of 380 to 780 nm by 5.
pub const PAPER_BANDS: usize = 81;

/// The nodes of each layer's characteristic curve, on a uniform log exposure axis.
pub const PAPER_CURVE_NODES: usize = 256;

const LOG10_2: f32 = std::f32::consts::LOG10_2;
const LOG2_10: f32 = std::f32::consts::LOG2_10;
const LOG10_E: f32 = std::f32::consts::LOG10_E;

/// The window's shares printed on a paper: how a share is laid back on density, the enlarger's light, the paper's
/// curves, its print seen under D50 in Rec. 2020, and the print's ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paper {
    /// A share `t` of the window is the density `span · t^bend` above the bare film: the frame's mean window span, and
    /// the power that lays the mean window's grey back where its density stands, so the grade on the mean is one.
    pub span: f32,
    pub bend: f32,
    /// What each layer reads of the three densities, by rows summing to one: the identity on an enlarger.
    pub mix: [V3; 3],
    /// Each layer's filter where it absorbs, `10^(−CC/100)`: the enlarger's filtration.
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
    /// Where the pick and the paper's white land: pure black and pure white, or for Print lift where the enlarger's
    /// rest prints the pick and the paper's own white.
    pub anchor: V3,
    pub white_anchor: V3,
    /// A black and white print reads its luma on its three channels.
    pub luma: bool,
}

impl Paper {
    pub fn head(&self) -> [u32; 4] {
        [self.print.len() as u32, 0, 0, 0]
    }

    /// The uniform: the density, the enlarger, Brightness, the print's ends, then the bands and the curves.
    pub fn rows(&self) -> Vec<V4> {
        let flag = |on: bool| if on { 1.0 } else { 0.0 };
        let m = &self.mix;
        let mut rows = vec![
            [self.span, self.bend, self.scale, flag(self.luma)],
            v3(self.filters, 0.0),
            [self.pick_log, self.reach, self.brightness, 0.0],
            [self.curve_low, self.curve_step, 0.0, 0.0],
            v3(m[0], 0.0),
            v3(m[1], 0.0),
            v3(m[2], 0.0),
            v3(self.pick, 0.0),
            v3(self.white, 0.0),
            v3(self.anchor, 0.0),
            v3(self.white_anchor, 0.0),
        ];
        let pad = |n: usize| std::iter::repeat_n([0.0; 4], PAPER_BANDS - n);
        rows.extend(self.print.iter().copied().chain(pad(self.print.len())));
        rows.extend(self.view.iter().map(|w| v3(*w, 0.0)).chain(pad(self.view.len())));
        rows.extend(self.curves.iter().map(|c| v3(*c, 0.0)));
        rows
    }

    /// The density above the bare film each layer reads for the window's `shares`, mixed by its rows; a share under
    /// none reads its power turned over, so the curve passes zero smoothly.
    pub fn density(&self, shares: V3) -> V3 {
        let d = shares.map(|t| self.span * t.signum() * t.abs().powf(self.bend));
        [dot(self.mix[0], d), dot(self.mix[1], d), dot(self.mix[2], d)]
    }

    /// Each layer's log exposure under the negative holding `density`, before Brightness turns it: the light through
    /// the time and the layer's filter, less the density.
    pub fn exposed(&self, density: V3) -> V3 {
        map(density, |c, d| (self.scale * self.filters[c]).ln() * LOG10_E - d)
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

    /// The print of the window's `shares`, a black and white print on its luma; before its ends.
    pub fn printed(&self, shares: V3) -> V3 {
        let light = self.viewed(self.developed(self.brightened(self.exposed(self.density(shares)))));
        if self.luma { [dot(light, LUMA); 3] } else { light }
    }

    /// Each channel laid from the printed pick K on its anchor A and the paper's white W on its anchor T: a straight
    /// line in light through both, under K one onto no light, so the print stays above none.
    pub fn ended(&self, light: V3) -> V3 {
        if self.anchor == self.pick && self.white_anchor == self.white {
            return light;
        }
        map(light, |c, v| {
            let (k, a, w, t) = (self.pick[c], self.anchor[c], self.white[c], self.white_anchor[c]);
            if v <= k { v * a / k } else { a + (t - a) * ((v - k) / (w - k)) }
        })
    }

    /// What the print shows of the window's `shares`: its light between its ends.
    pub fn shown(&self, shares: V3) -> V3 {
        self.ended(self.printed(shares))
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

/// One pixel's window shares printed on the paper, in linear Rec. 2020; alpha kept.
pub fn paper(t: V4, k: &Paper) -> V4 {
    with_alpha(k.shown(rgb(t)), t[3])
}

#[cfg(test)]
mod tests;
