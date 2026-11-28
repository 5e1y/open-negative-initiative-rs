//! The paper, a look at the chain's end: each layer's dye amount read off its curve at its channel's window share, the
//! dyes' Status A crosstalk over the D-min, each density's light on its own channel through the paper's primaries, then
//! its ends: the frame's pick and the paper's white each on its anchor. Every curve is baked in the paper's file
//! (`scripts/paper-looks.py`); Brightness moves the light as a time does, faded in from the pick over the deep shadows.
//! Lighting's other sliders and the colour tools act on the negative before it (`negative`).

use super::*;

/// The nodes of each layer's curve, on a uniform axis of the window's share.
pub const PAPER_CURVE_NODES: usize = 256;

const LOG10_2: f32 = std::f32::consts::LOG10_2;
const LOG2_10: f32 = std::f32::consts::LOG2_10;

/// The window's shares printed on a paper: its curves, its dyes and their crosstalk, the light's primaries, Brightness
/// and the print's ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paper {
    /// Each layer's dye amount over the paper's D-min along the window's share from `curve_low` by `curve_step`, held at
    /// either end.
    pub curves: [V3; PAPER_CURVE_NODES],
    pub curve_low: f32,
    pub curve_step: f32,
    /// The paper's D-min, and what each channel reads of each dye's amount, a row a channel: its Status A crosstalk.
    pub floor: V3,
    pub crosstalk: [V3; 3],
    /// Each density's light in the working space, a row a channel: the identity where each absorbs its own alone.
    pub primaries: [V3; 3],
    /// The log exposure the window's span lays on the paper, Brightness's unit; Brightness, in stops of the light; the
    /// pick's mean share; the reach, how far from it toward the light a tone's mean lies, in log exposure, when
    /// Brightness moves it as far as Exposure would.
    pub latitude: f32,
    pub brightness: f32,
    pub pick_share: f32,
    pub reach: f32,
    /// The print of the frame's pick without Lighting's points, and the paper's own white: the ends the print's light is
    /// read between.
    pub pick: V3,
    pub white: V3,
    /// Where the pick and the paper's white land: pure black and pure white, or for Print lift where the paper's rest
    /// prints the pick and the paper's own white.
    pub anchor: V3,
    pub white_anchor: V3,
    /// A black and white print reads its luma on its three channels.
    pub luma: bool,
}

impl Paper {
    /// The uniform: Brightness, the axis, the primaries, the print's ends, the dyes, then the curves.
    pub fn rows(&self) -> Vec<V4> {
        let flag = |on: bool| if on { 1.0 } else { 0.0 };
        let m = &self.primaries;
        let mut rows = vec![
            [self.latitude, self.pick_share, self.reach, self.brightness],
            [self.curve_low, self.curve_step, flag(self.luma), 0.0],
            v3(m[0], 0.0),
            v3(m[1], 0.0),
            v3(m[2], 0.0),
            v3(self.pick, 0.0),
            v3(self.white, 0.0),
            v3(self.anchor, 0.0),
            v3(self.white_anchor, 0.0),
            v3(self.floor, 0.0),
            v3(self.crosstalk[0], 0.0),
            v3(self.crosstalk[1], 0.0),
            v3(self.crosstalk[2], 0.0),
        ];
        rows.extend(self.curves.iter().map(|c| v3(*c, 0.0)));
        rows
    }

    /// How far, in log exposure, the mean of `shares` lies past the pick's `pick_share`, toward a lighter print.
    pub fn toward_light(&self, pick_share: f32, shares: V3) -> f32 {
        self.latitude * ((shares[0] + shares[1] + shares[2]) / 3.0 - pick_share)
    }

    /// Each share moved by Brightness as Exposure moves the light, by the same amount on the three, as far as its mean
    /// lies from the pick's toward the light: `lifted`.
    pub fn brightened(&self, shares: V3) -> V3 {
        if self.brightness == 0.0 {
            return shares;
        }
        let x = self.toward_light(self.pick_share, shares);
        let moved = lifted(x, self.brightness * LOG10_2, self.reach) - x;
        shares.map(|t| t + moved / self.latitude)
    }

    /// Each layer's dye amount over the D-min at the window's `shares`.
    pub fn amounts(&self, shares: V3) -> V3 {
        map(shares, |c, t| {
            let f = clamp((t - self.curve_low) / self.curve_step, 0.0, (PAPER_CURVE_NODES - 1) as f32);
            let i = f.floor().min((PAPER_CURVE_NODES - 2) as f32);
            let at = i as usize;
            mix(self.curves[at][c], self.curves[at + 1][c], f - i)
        })
    }

    /// Each channel's density over the paper's D-min holding the dyes' `amounts`.
    pub fn dyed(&self, amounts: V3) -> V3 {
        map(self.floor, |c, f| f + dot(self.crosstalk[c], amounts))
    }

    /// The print holding each channel's `density`, in linear Rec. 2020: each density's light through the primaries.
    pub fn viewed(&self, density: V3) -> V3 {
        let light = viewed(density);
        [dot(self.primaries[0], light), dot(self.primaries[1], light), dot(self.primaries[2], light)]
    }

    /// The print of the window's `shares`, a black and white print on its luma; before its ends.
    pub fn printed(&self, shares: V3) -> V3 {
        let light = self.viewed(self.dyed(self.amounts(self.brightened(shares))));
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

/// The light a print holding `density` of each pure dye reflects, in linear Rec. 2020.
pub fn viewed(density: V3) -> V3 {
    density.map(|d| (-LOG2_10 * d).exp2())
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
