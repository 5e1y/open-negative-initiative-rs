//! Lighting on the negative: a pixel's density above the bare film read back to each layer's log exposure through its
//! stock's development, couplers included, Exposure, Contrast and the filters laid there and the film developed again,
//! as a shot exposed otherwise; then the points, zones and mids on its density, and that density coded as the light the
//! print reads.

use super::*;
use super::stage47::{midded, quartered};

/// The nodes of each layer's development, on a uniform log exposure axis.
pub const NEGATIVE_CURVE_NODES: usize = 256;

/// Newton's steps through the couplers: their pull is under 0.8 of a layer's own slope, so four bring every exposure
/// within 1e-5 log of its root.
pub const NEGATIVE_STEPS: usize = 4;

const LOG2_10: f32 = std::f32::consts::LOG2_10;
const LOG10_E: f32 = std::f32::consts::LOG10_E;

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
/// in log exposure, the tones on its density and the density the print reads.
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
    /// The density's tone share: `origin + density / span`, 0 where the print lays black and 1 where it lays white;
    /// the points stretch it, the zones lay one share by the mean tone, the mids each channel's own.
    pub span: V3,
    pub origin: f32,
    pub points_black: V3,
    pub points_white: V3,
    pub zones: V2,
    pub mids: V3,
    /// Each channel's density above the bare film to the density the print reads, from `tone_low` by `tone_step`,
    /// straight on past either end.
    pub tone: [V3; DYE_TONE_NODES],
    pub tone_low: V3,
    pub tone_step: V3,
}

use super::dye_separation::DYE_TONE_NODES;

impl Negative {
    pub fn head(&self) -> [u32; 4] {
        [0; 4]
    }

    /// The uniform: the reading, the moves, the tones, the couplers, then the development and the print's tone.
    pub fn rows(&self) -> Vec<V4> {
        let d = self.development;
        let c = &d.couplers;
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
            v3(self.tone_low, 0.0),
            v3(self.tone_step, 0.0),
            v3(c[0], 0.0),
            v3(c[1], 0.0),
            v3(c[2], 0.0),
            v3(d.foot, 0.0),
            v3(d.top, 0.0),
            v3(self.own_pivot, 0.0),
            v3(self.own_slope, 0.0),
            v3(self.own_shift, 0.0),
            v3(self.scale, 0.0),
        ];
        rows.extend(d.normal.iter().map(|v| v3(*v, 0.0)));
        rows.extend(d.after.iter().map(|v| v3(*v, 0.0)));
        rows.extend(self.tone.iter().map(|v| v3(*v, 0.0)));
        rows
    }

    /// The density above the bare film a pixel holds, in the development's sense.
    pub fn read(&self, s: V4) -> V3 {
        map(rgb(s), |c, v| self.sense * (-((v + self.pedestal) * self.gain[c]).max(self.log_guard).ln() * LOG10_E - self.black[c]))
    }

    /// Whether Exposure, Contrast or a filter moves any layer.
    pub fn moves(&self) -> bool {
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

    /// The film holding `density` in the development's sense developed again under Lighting's moves. Over the shoulder
    /// a density takes the shoulder's move; under the toe's foot the foot's, less the nearer the fog, which no exposure
    /// moves.
    pub fn redeveloped(&self, density: V3) -> V3 {
        if !self.moves() {
            return density;
        }
        let d = self.development;
        let own = map(density, |c, v| v / self.scale[c]);
        let read = map(own, |c, v| v.max(d.foot[c]).min(d.top[c]));
        let developed = d.developed_on(self.owned(d.inhibited(self.moved(d.exposures(read)))));
        map(own, |c, v| {
            let fog = d.after[0][c];
            let share = if v < d.foot[c] { clamp((v - fog) / (d.foot[c] - fog).max(1e-6), 0.0, 1.0) } else { 1.0 };
            (v + share * (developed[c] - read[c])) * self.scale[c]
        })
    }

    /// Whether the points, zones or mids lay any density.
    pub fn tones(&self) -> bool {
        self.points_black != [0.0; 3] || self.points_white != [0.0; 3] || self.zones != [0.0; 2] || self.mids != [0.0; 3]
    }

    /// The density above the bare film Lighting's tones leave, each read on its tone share.
    pub fn toned(&self, density: V3) -> V3 {
        if !self.tones() {
            return density;
        }
        let u = map(density, |c, d| (self.origin + d / self.span[c] - self.points_black[c]) / (1.0 - self.points_black[c] - self.points_white[c]));
        let tone = (u[0] + u[1] + u[2]) / 3.0;
        let lift = if self.zones != [0.0; 2] && (0.0..=1.0).contains(&tone) {
            quartered(quartered(tone, self.zones[0], false), self.zones[1], true) - tone
        } else {
            0.0
        };
        map(u, |c, v| (midded(v + lift, self.mids[c]) - self.origin) * self.span[c])
    }

    /// The density the print reads for a density above the bare film.
    pub fn printed(&self, density: V3) -> V3 {
        map(density, |c, v| {
            let f = (v - self.tone_low[c]) / self.tone_step[c];
            let i = clamp(f.floor(), 0.0, (DYE_TONE_NODES - 2) as f32);
            let at = i as usize;
            mix(self.tone[at][c], self.tone[at + 1][c], f - i)
        })
    }

    /// A pixel's density above the bare film under Lighting.
    pub fn lit(&self, s: V4) -> V3 {
        let redeveloped = self.redeveloped(self.read(s));
        self.toned(map(redeveloped, |_, v| self.sense * v))
    }
}

/// One pixel's negative under Lighting, as the light whose density the print reads; alpha kept.
pub fn negative(s: V4, k: &Negative) -> V4 {
    with_alpha(map(k.printed(k.lit(s)), |_, v| (-LOG2_10 * v).exp2()), s[3])
}

#[cfg(test)]
mod tests;
