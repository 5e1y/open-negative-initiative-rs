//! Lighting on the negative: a pixel's density above the bare film read back to each layer's log exposure through its
//! stock's development, couplers included, Contrast, the colour tools and the filters laid there and the film developed
//! again, as a shot exposed otherwise; then Exposure on its tone share, the points, zones and mids, and that density
//! coded on the bare film's own as the light the window reads.

use super::*;
use super::saturation_curve::SaturationCurve;
use super::spectrogram::Spectrogram;
use super::stage47::{midded, quartered};

/// The nodes of each layer's development, on a uniform log exposure axis.
pub const NEGATIVE_CURVE_NODES: usize = 256;

/// Newton's steps through the couplers: their pull is under 0.8 of a layer's own slope, so four bring every exposure
/// within 1e-5 log of its root.
pub const NEGATIVE_STEPS: usize = 4;

const LOG2_10: f32 = std::f32::consts::LOG2_10;
const LOG10_E: f32 = std::f32::consts::LOG10_E;
const LOG10_2: f32 = std::f32::consts::LOG10_2;

/// The nodes of the print's lightness of a grey, from `LIGHTNESS_LOW` to `LIGHTNESS_HIGH` log of exposure about the
/// reference grey's, past any film's latitude; held at either end.
pub const LIGHTNESS_NODES: usize = 128;
pub const LIGHTNESS_LOW: f32 = -4.0;
pub const LIGHTNESS_HIGH: f32 = 4.0;

/// The chroma, in log exposure, over which a colour comes into its hue's band: a grey has no hue to move.
pub const PRESENCE: f32 = 0.1;

/// A band's luma pull at its track's end, in stops of the scene's light: a gain, never `1 + x`, which reaches black.
pub const LUMA_STOPS: f32 = 2.0;

/// A stock's development, each curve rising at every node so it reads back.
#[derive(Clone, Debug, PartialEq)]
pub struct Development {
    /// Each layer's density as a grey exposing the three layers alike develops it.
    pub normal: [V3; NEGATIVE_CURVE_NODES],
    /// Each layer's density at its log exposure less the couplers' pull: `normal` where every layer reads a grey.
    pub after: [V3; NEGATIVE_CURVE_NODES],
    pub log_low: f32,
    pub log_step: f32,
    /// Each layer's densities where its toe starts and its shoulder ends reading exposure: under and over them a density
    /// tells no exposure apart.
    pub foot: V3,
    pub top: V3,
    /// The DIR couplers: the row's density lowers the column's log exposure.
    pub couplers: [V3; 3],
}

impl Development {
    /// Channel `c` of `table` at `log` and its slope: held at its first node under it, as no exposure develops under the
    /// fog, straight on past its last.
    fn at(&self, table: &[V3; NEGATIVE_CURVE_NODES], log: f32, c: usize) -> (f32, f32) {
        let f = (log - self.log_low) / self.log_step;
        if f < 0.0 {
            return (table[0][c], 0.0);
        }
        let i = f.floor().min((NEGATIVE_CURVE_NODES - 2) as f32);
        let at = i as usize;
        let (a, b) = (table[at][c], table[at + 1][c]);
        (mix(a, b, f - i), (b - a) / self.log_step)
    }

    fn back(&self, table: &[V3; NEGATIVE_CURVE_NODES], density: f32, c: usize) -> f32 {
        let (mut low, mut high) = (0usize, NEGATIVE_CURVE_NODES - 1);
        while high - low > 1 {
            let mid = (low + high) / 2;
            if table[mid][c] <= density { low = mid } else { high = mid }
        }
        let (a, b) = (table[low][c], table[low + 1][c]);
        self.log_low + (low as f32 + (density - a) / (b - a)) * self.log_step
    }

    /// The couplers' pull on each layer and each layer's own pull's slope.
    fn pulled(&self, log: V3) -> (V3, V3) {
        let n: [(f32, f32); 3] = std::array::from_fn(|c| self.at(&self.normal, log[c], c));
        let density = n.map(|v| v.0);
        let pull = std::array::from_fn(|m| (0..3).map(|d| density[d] * self.couplers[d][m]).sum());
        (pull, n.map(|v| v.1))
    }

    /// The node at the speed's log exposure, 0, where a grey exposes the three layers alike.
    pub fn at_speed(&self) -> usize {
        clamp((-self.log_low / self.log_step).round(), 0.0, (NEGATIVE_CURVE_NODES - 1) as f32) as usize
    }

    /// Each layer's log exposure less the couplers' pull: what it develops on.
    pub fn inhibited(&self, log: V3) -> V3 {
        let (pull, _) = self.pulled(log);
        map(log, |c, e| e - pull[c])
    }

    /// Each layer's density developed on its log exposure less the couplers' pull.
    pub fn developed_on(&self, inhibited: V3) -> V3 {
        map(inhibited, |c, x| self.at(&self.after, x, c).0)
    }

    /// Each layer's density developed from its log exposure.
    pub fn developed(&self, log: V3) -> V3 {
        self.developed_on(self.inhibited(log))
    }

    /// The log exposure at which a grey develops each layer's density from what it develops on: equal on the three
    /// layers where they print a neutral.
    pub fn as_grey(&self, inhibited: V3) -> V3 {
        map(inhibited, |c, x| self.back(&self.normal, self.at(&self.after, x, c).0, c))
    }

    /// What each layer develops on to hold the density a grey develops at `log`.
    pub fn from_grey(&self, log: V3) -> V3 {
        map(log, |c, e| self.back(&self.after, self.at(&self.normal, e, c).0, c))
    }

    /// Each layer's log exposure that develops `density`: the couplers' pull solved by Newton's steps.
    pub fn exposures(&self, density: V3) -> V3 {
        let inhibited = map(density, |c, d| self.back(&self.after, d, c));
        if self.couplers == [[0.0; 3]; 3] {
            return inhibited;
        }
        let (pull, _) = self.pulled(inhibited);
        let mut log = map(inhibited, |c, x| x + pull[c]);
        for _ in 0..NEGATIVE_STEPS {
            let (pull, slope) = self.pulled(log);
            let miss = map(log, |c, e| e - pull[c] - inhibited[c]);
            let j: [V3; 3] = std::array::from_fn(|m| std::array::from_fn(|k| f32::from(m == k) - slope[k] * self.couplers[k][m]));
            log = map(log, |c, e| e - solved(&j, miss)[c]);
        }
        log
    }
}

/// `m⁻¹ v` by cofactors; the couplers' pull keeps `m` near the identity, never singular.
fn solved(m: &[V3; 3], v: V3) -> V3 {
    let [[a, b, c], [d, e, f], [g, h, i]] = *m;
    let (ca, cb, cc) = (e * i - f * h, f * g - d * i, d * h - e * g);
    let det = a * ca + b * cb + c * cc;
    let inverse = [[ca, c * h - b * i, b * f - c * e], [cb, a * i - c * g, c * d - a * f], [cc, b * g - a * h, a * e - b * d]];
    map([dot(inverse[0], v), dot(inverse[1], v), dot(inverse[2], v)], |_, x| x / det)
}

/// A frame's negative under Lighting: how a pixel is read as density above the bare film, its development, the moves
/// in log exposure and the tones on its density.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Negative {
    /// The input read as density above the bare film: `−log10((s + pedestal)·gain) − black`, held at the guard.
    pub gain: V3,
    pub pedestal: f32,
    pub log_guard: f32,
    pub black: V3,
    /// 1 on a negative; −1 on a positive, whose density falls as its exposure rises.
    pub sense: f32,
    pub development: &'static Development,
    /// Each layer's density over the development's own: the frame's development of that layer, which puts its reference
    /// grey on the stock's neutral.
    pub scale: V3,
    /// Each layer's log exposure turned about `pivot` by `slope`, then moved by `shift`: Exposure, Contrast and the
    /// filters' neutral hold, the couplers then pulling on it.
    pub pivot: V3,
    pub slope: V3,
    pub shift: V3,
    /// Each layer's own moves, on what it develops on once the couplers have pulled, so a channel's move stays its own:
    /// turned about `own_pivot` by `own_slope`, then moved by `own_shift`, each channel's contrast and filter.
    pub own_pivot: V3,
    pub own_slope: V3,
    pub own_shift: V3,
    /// Exposure on each channel's tone share once developed again, held at the window's black and white: the share's
    /// odds `u / (1 − u)` taken by `e^odds` on the three, so a neutral stays one; 0 at rest.
    pub odds: f32,
    /// The colour tools on each layer's density read as the log exposure a grey develops it at, about `pivot`, Lighting's
    /// moves and filters laid: its neutral the luma's weight of the three, its chroma the rest. Saturation spreads the
    /// chroma by `spread`, the mixer turns, spreads and lights it by hue, the curve spreads it by the print's
    /// `lightness` of its neutral.
    pub spread: f32,
    pub mixer: Option<Spectrogram>,
    pub curve: Option<SaturationCurve>,
    /// The print's lightness, its luma on a display gamma, of a grey from `LIGHTNESS_LOW` to `LIGHTNESS_HIGH` log of
    /// exposure about the reference's.
    pub lightness: [f32; LIGHTNESS_NODES],
    /// The density's tone share: `origin + density / span`, 0 where the print lays black and 1 where it lays white;
    /// the points stretch it, the zones lay one share by the mean tone, the mids each channel's own.
    pub span: V3,
    pub origin: f32,
    pub points_black: V3,
    pub points_white: V3,
    pub zones: V2,
    pub mids: V3,
}

impl Negative {
    pub fn head(&self) -> [u32; 4] {
        [0; 4]
    }

    /// The uniform: the reading, the moves, the tones, the couplers, the colour tools, then the development.
    pub fn rows(&self) -> Vec<V4> {
        let d = self.development;
        let c = &d.couplers;
        let flag = |on: bool| if on { 1.0 } else { 0.0 };
        let mixer = self.mixer.map_or(vec![[0.0; 4]; 6], |m| m.rows());
        let curve = self.curve.map_or(vec![[0.0; 4]; 3], |k| k.rows());
        let mut rows = vec![
            v3(self.gain, self.pedestal),
            v3(self.black, self.log_guard),
            v3(self.pivot, self.sense),
            v3(self.slope, d.log_low),
            v3(self.shift, d.log_step),
            v3(self.span, self.origin),
            v3(self.points_black, self.zones[0]),
            v3(self.points_white, self.zones[1]),
            v3(self.mids, 0.0),
            v3(c[0], 0.0),
            v3(c[1], 0.0),
            v3(c[2], 0.0),
            v3(d.foot, 0.0),
            v3(d.top, 0.0),
            v3(self.own_pivot, 0.0),
            v3(self.own_slope, 0.0),
            v3(self.own_shift, 0.0),
            v3(self.scale, 0.0),
            [self.odds.exp(), 0.0, 0.0, 0.0],
            [self.spread, flag(self.curve.is_some()), flag(self.mixer.is_some()), 0.0],
        ];
        rows.extend(mixer);
        rows.extend(curve);
        rows.extend(self.lightness.chunks(4).map(|l| [l[0], l[1], l[2], l[3]]));
        rows.extend(d.normal.iter().map(|v| v3(*v, 0.0)));
        rows.extend(d.after.iter().map(|v| v3(*v, 0.0)));
        rows
    }

    /// The density above the bare film a pixel holds, in the development's sense.
    pub fn read(&self, s: V4) -> V3 {
        map(rgb(s), |c, v| self.sense * (-((v + self.pedestal) * self.gain[c]).max(self.log_guard).ln() * LOG10_E - self.black[c]))
    }

    /// Whether Exposure, Contrast or a filter moves any layer.
    pub fn moves(&self) -> bool {
        self.develops() || self.odds != 0.0
    }

    /// Whether a move in log exposure develops any layer again: Contrast, a filter or its neutral hold.
    fn develops(&self) -> bool {
        self.slope != [1.0; 3] || self.shift != [0.0; 3] || self.own_slope != [1.0; 3] || self.own_shift != [0.0; 3]
    }

    /// Each layer's log exposure as Lighting lays it, before the couplers pull.
    pub fn moved(&self, log: V3) -> V3 {
        map(log, |c, e| self.pivot[c] + (e - self.pivot[c]) * self.slope[c] + self.shift[c])
    }

    /// Each layer's log exposure, the couplers' pull taken, as its own moves lay it.
    pub fn owned(&self, inhibited: V3) -> V3 {
        map(inhibited, |c, x| self.own_pivot[c] + (x - self.own_pivot[c]) * self.own_slope[c] + self.own_shift[c])
    }

    /// Whether a colour tool moves the scene's colour.
    pub fn colours(&self) -> bool {
        self.spread != 1.0 || self.mixer.is_some() || self.curve.is_some()
    }

    /// The print's lightness of a grey `n` log of exposure about the reference's.
    pub fn lightness_at(&self, n: f32) -> f32 {
        let step = (LIGHTNESS_HIGH - LIGHTNESS_LOW) / (LIGHTNESS_NODES - 1) as f32;
        let f = clamp((n - LIGHTNESS_LOW) / step, 0.0, (LIGHTNESS_NODES - 1) as f32);
        let i = f.floor().min((LIGHTNESS_NODES - 2) as f32);
        let at = i as usize;
        mix(self.lightness[at], self.lightness[at + 1], f - i)
    }

    /// Each layer's grey log exposure, its colour moved by the colour tools: the chroma spread by Saturation, turned and
    /// spread by the mixer's band of its hue, then by the curve at its lightness, the luminance held; then lit by the
    /// band's luma.
    pub fn coloured(&self, log: V3) -> V3 {
        if !self.colours() {
            return log;
        }
        let about = map(log, |c, e| e - self.pivot[c]);
        let mut neutral = dot(about, LUMA);
        let before = map(about, |_, v| v - neutral);
        let mut chroma = before.map(|v| v * self.spread);
        let mut lift = 0.0;
        if let Some(m) = &self.mixer {
            let a = dot(chroma, [1.0, -0.5, -0.5]);
            let b = dot(chroma, [0.0, 0.866_025_4, -0.866_025_4]);
            let presence = smoothstep(0.0, PRESENCE, (a * a + b * b).sqrt());
            if presence > 0.0 {
                let mut hue = b.atan2(a) * 57.295_78;
                if hue < 0.0 {
                    hue += 360.0;
                }
                let [shift, boost, light] = m.doses(hue).map(|v| v * presence);
                // Rodrigues about the grey axis, then the luma's weight taken back out, so the turn keeps the neutral.
                let axis = [0.577_350_26_f32; 3];
                let (sn, cs) = (shift * 0.017_453_292).sin_cos();
                let cross = [
                    axis[1] * chroma[2] - axis[2] * chroma[1],
                    axis[2] * chroma[0] - axis[0] * chroma[2],
                    axis[0] * chroma[1] - axis[1] * chroma[0],
                ];
                let along = dot(axis, chroma);
                let turned = map(chroma, |i, v| v * cs + cross[i] * sn + axis[i] * along * (1.0 - cs));
                let held = dot(turned, LUMA);
                let gain = (1.0 + boost).max(0.0);
                chroma = map(turned, |_, v| (v - held) * gain);
                lift = light * LUMA_STOPS * LOG10_2;
            }
        }
        if let Some(curve) = &self.curve {
            let factor = curve.factor(self.lightness_at(neutral + lift));
            chroma = chroma.map(|v| v * factor);
        }
        // The luminance as it was: a colour spread or greyed keeps its brightness, as on the former print.
        let luminance = |c: V3| dot(c.map(|v| (v * LOG2_10).exp2()), LUMA);
        neutral += (luminance(before) / luminance(chroma)).log10() + lift;
        map(chroma, |c, v| self.pivot[c] + neutral + v)
    }

    /// What each layer develops on, its print's colour moved by the colour tools.
    pub fn coloured_on(&self, owned: V3) -> V3 {
        if !self.colours() {
            return owned;
        }
        let d = self.development;
        d.from_grey(self.coloured(d.as_grey(owned)))
    }

    /// The light the negative passes for a print's neutral, the grey `n` log of exposure from the reference's.
    pub fn grey(&self, n: f32) -> V4 {
        let d = self.development;
        let developed = d.developed_on(d.from_grey(self.pivot.map(|p| p + n)));
        let density = map(developed, |c, v| self.sense * v * self.scale[c]);
        with_alpha(map(self.toned(density), |c, v| (-LOG2_10 * (v + self.black[c])).exp2()), 1.0)
    }

    /// The film holding `density` in the development's sense under Lighting: developed again under its moves and the
    /// colour tools, then its Exposure on the tone share.
    pub fn redeveloped(&self, density: V3) -> V3 {
        self.exposed(self.developed_again(density))
    }

    /// `density` developed again under the moves and the colour tools. Over the shoulder a density takes the shoulder's
    /// move; under the toe's foot the foot's, less the nearer the fog, which no exposure moves.
    fn developed_again(&self, density: V3) -> V3 {
        if !self.develops() && !self.colours() {
            return density;
        }
        let d = self.development;
        let own = map(density, |c, v| v / self.scale[c]);
        let read = map(own, |c, v| v.max(d.foot[c]).min(d.top[c]));
        let developed = d.developed_on(self.coloured_on(self.owned(d.inhibited(self.moved(d.exposures(read))))));
        map(own, |c, v| {
            let fog = d.after[0][c];
            let share = if v < d.foot[c] { clamp((v - fog) / (d.foot[c] - fog).max(1e-6), 0.0, 1.0) } else { 1.0 };
            (v + share * (developed[c] - read[c])) * self.scale[c]
        })
    }

    /// A pixel's density above the bare film under Lighting.
    pub fn lit(&self, s: V4) -> V3 {
        let redeveloped = self.redeveloped(self.read(s));
        self.toned(map(redeveloped, |_, v| self.sense * v))
    }
}

/// One pixel's negative under Lighting, its density on the bare film's own as the light the window reads; alpha kept.
pub fn negative(s: V4, k: &Negative) -> V4 {
    with_alpha(map(k.lit(s), |c, v| (-LOG2_10 * (v + k.black[c])).exp2()), s[3])
}

mod tones;

#[cfg(test)]
mod tests;
