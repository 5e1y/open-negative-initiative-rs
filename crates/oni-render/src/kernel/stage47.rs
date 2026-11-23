//! The fused conversion kernel: Lighting on the negative, then each channel's density read through its window, then
//! ONI's print curve, or the paper the window's shares expose in its place, its own ends included.

use super::*;

/// The fused kernel's arguments, named as the Metal kernel names them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stage47 {
    pub gain: [f32; 3],
    pub red: [f32; 3],
    pub green: [f32; 3],
    pub blue: [f32; 3],
    pub invert: f32,
    pub pedestal: f32,
    pub log_guard: f32,
    pub window: f32,
    pub low_points: [f32; 3],
    pub high_points: [f32; 3],
    pub print_on: f32,
    pub print_stops: f32,
    pub print_contrast: f32,
    pub print_zones: [f32; 2],
    pub own_stops: [f32; 3],
    /// Each channel's own contrast and mids, after the RGB curve and before its filter's stops.
    pub own_contrast: [f32; 3],
    pub own_mid: [f32; 3],
    pub hunt: f32,
    pub decompression: f32,
    pub balance_low: [f32; 3],
    pub balance_span: [f32; 3],
    pub film_share: f32,
    /// Lighting on the negative, in the pass the print reads; none while Lighting rests.
    pub negative: Option<super::negative::Negative>,
    /// The window's shares printed on a paper in place of the print curve, laid in the same GPU pass.
    pub paper: Option<super::paper::Paper>,
}

impl Stage47 {
    /// A negative at rest: no print, no decompression.
    pub fn rest() -> Self {
        Self {
            gain: [1.0; 3],
            red: [0.0, 2.0, 1.0],
            green: [0.0, 2.0, 1.0],
            blue: [0.0, 2.0, 1.0],
            invert: 1.0,
            pedestal: 0.0,
            log_guard: 1e-6,
            window: 3.0,
            low_points: [0.25; 3],
            high_points: [0.75; 3],
            print_on: 0.0,
            print_stops: 0.0,
            print_contrast: 0.0,
            print_zones: [0.0; 2],
            own_stops: [0.0; 3],
            own_contrast: [0.0; 3],
            own_mid: [0.0; 3],
            hunt: 0.0,
            decompression: 1.0,
            balance_low: [0.0; 3],
            balance_span: [1.0; 3],
            film_share: 0.0,
            negative: None,
            paper: None,
        }
    }

    /// What stage 4 reads past Lighting on the negative: density as light, at unit gain and no pedestal.
    pub fn after_negative(&self) -> Self {
        if self.negative.is_none() {
            return *self;
        }
        Self { gain: [1.0; 3], pedestal: 0.0, negative: None, ..*self }
    }

    /// The uniform block, every vector padded to four lanes as WGSL lays it out.
    pub fn packed(&self, count: u32) -> Packed {
        Packed {
            rows: [
                v3(self.gain, self.invert),
                v3(self.red, 0.0),
                v3(self.green, self.pedestal),
                v3(self.blue, self.log_guard),
                v3(self.low_points, self.window),
                v3(self.high_points, self.print_on),
                [self.print_stops, self.print_contrast, self.hunt, if self.paper.is_some() { 1.0 } else { 0.0 }],
                v3(self.own_stops, self.film_share),
                v3(self.balance_low, 0.0),
                v3(self.balance_span, self.decompression),
                v22(self.print_zones, [0.0; 2]),
                v3(self.own_contrast, 0.0),
                v3(self.own_mid, 0.0),
            ],
            count: [count, 0, 0, 0],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Packed {
    rows: [[f32; 4]; 13],
    count: [u32; 4],
}

/// The window's share of density `e`, through its mid's power over the black, straight on under it: a paper reads a
/// density under the bare film as one.
fn apply_levels(e: f32, lv: V3) -> f32 {
    let n = (e - lv[0]) / (lv[1] - lv[0]).max(1e-4);
    if n > 0.0 { n.powf(lv[2]) } else { n }
}

fn apply_window(e: f32, lv: V3, c1: f32, c3: f32) -> f32 {
    let t = apply_levels(e, lv);
    if c1 == 0.25 && c3 == 0.75 {
        return t;
    }
    let u = t.clamp(0.0, 1.0);
    let v = 1.0 - u;
    let curve = c1 * (4.0 * u * v * v * v) + 0.5 * (6.0 * u * u * v * v) + c3 * (4.0 * u * u * u * v) + u * u * u * u;
    curve + (t - u)
}

fn quarter_pass(p: f32, t: f32, high: bool) -> f32 {
    if high {
        p + t * 4.0 * p * p * p * (1.0 - p)
    } else {
        p + t * 4.0 * p * (1.0 - p) * (1.0 - p) * (1.0 - p)
    }
}

pub(super) fn quartered(p: f32, travel: f32, high: bool) -> f32 {
    if travel.abs() <= 0.25 {
        return quarter_pass(p, travel, high);
    }
    let first = if travel > 0.0 { 0.25 } else { -0.25 };
    quarter_pass(quarter_pass(p, first, high), travel - first, high)
}

fn shaped(p_in: f32, stops: f32, contrast: f32, zones: [f32; 2]) -> f32 {
    let held = clamp(p_in, 0.0, 1.0);
    let mut p = contrasted(held, contrast) + (p_in - held);
    if stops != 0.0 && p > 0.0 {
        let top = p.min(1.0);
        let g = stops.exp2();
        let v = top.powf(DISPLAY_GAMMA);
        p = (v * g / (1.0 + v * (g - 1.0))).powf(1.0 / DISPLAY_GAMMA) + (p - top);
    }
    if zones != [0.0, 0.0] && (0.0..=1.0).contains(&p) {
        p = quartered(p, zones[0], false);
        p = quartered(p, zones[1], true);
    }
    p
}

/// Lighting's mids bend on the display value, held inside 0 to 1.
pub(super) fn midded(p: f32, mids: f32) -> f32 {
    if mids == 0.0 || !(0.0..=1.0).contains(&p) {
        return p;
    }
    p + mids * 16.0 * p * p * (1.0 - p) * (1.0 - p)
}

/// One channel printed: the RGB curve, then its own contrast, filter and mids.
fn printed(n: f32, stops: f32, contrast: f32, zones: [f32; 2], own: f32, own_contrast: f32, own_mid: f32) -> f32 {
    let p = shaped(n, stops, contrast, zones);
    midded(shaped(p, own, own_contrast, [0.0, 0.0]), own_mid).max(0.0).powf(DISPLAY_GAMMA)
}

/// Stages 4 to 7 on one pixel: Lighting on the negative, each channel's window, then ONI's print, its chroma
/// decompression first, or the paper.
pub fn stage47(s: [f32; 4], k: &Stage47) -> [f32; 4] {
    let s = match &k.negative {
        Some(n) => super::negative::negative(s, n),
        None => s,
    };
    let k = k.after_negative();
    let film = density(s, &k);
    let shares = window(film, &k);
    match &k.paper {
        Some(p) => super::paper::paper(with_alpha(shares, s[3]), p),
        None => with_alpha(printed3(decompressed(film, shares, &k), &k), s[3]),
    }
}

/// Stage 4: each channel's density, turned over on a positive.
fn density(s: [f32; 4], k: &Stage47) -> V3 {
    let t = map([s[0], s[1], s[2]], |c, x| (x + k.pedestal) * k.gain[c]);
    let d = map(t, |_, x| -x.max(k.log_guard).log10());
    if k.invert > 0.5 { d } else { map(d, |_, x| k.window - x) }
}

fn window(film: V3, k: &Stage47) -> V3 {
    let lv = [k.red, k.green, k.blue];
    map(film, |c, x| apply_window(x, lv[c], k.low_points[c], k.high_points[c]))
}

/// Stages 4 and 5: each channel's density read through its window, the share both prints read.
pub fn windowed(s: [f32; 4], k: &Stage47) -> V3 {
    window(density(s, k), k)
}

/// ONI's print's chroma decompression on the window's shares `e` of the density `film`: the windows' colour and the
/// film's own, the luma held.
fn decompressed(film: V3, e: V3, k: &Stage47) -> V3 {
    if k.decompression == 1.0 && k.film_share == 0.0 {
        return e;
    }
    let place = map(film, |c, x| (x - k.balance_low[c]) / k.balance_span[c]);
    let mean = (place[0] + place[1] + place[2]) / 3.0;
    let neutral = map(k.balance_low, |c, x| x + mean * k.balance_span[c]);
    let toned = window(neutral, k);
    let held = map(toned, |c, x| x + k.decompression * (e[c] - x) + k.film_share * (film[c] - neutral[c]));
    let shift = dot(e, LUMA) - dot(held, LUMA);
    map(held, |_, x| x + shift)
}

/// Stages 6 and 7 on a windowed pixel: the print, its filters and Hunt's colour, each channel's light to a power and
/// the luma's own power undone.
fn printed3(windowed_in: V3, k: &Stage47) -> V3 {
    let mut density = 1.0f32;
    let mut e = windowed_in;
    if k.print_on > 0.5 {
        let (ps, pc, pz) = (k.print_stops, k.print_contrast, k.print_zones);
        let windowed = e;
        e = map(e, |c, x| printed(x, ps, pc, pz, k.own_stops[c], k.own_contrast[c], k.own_mid[c]));
        if k.own_stops != [0.0; 3] {
            // The luma held is the channels' own tone's, so a filter alone is levelled out.
            let plain = map(windowed, |c, x| printed(x, ps, pc, pz, 0.0, k.own_contrast[c], k.own_mid[c]));
            let filtered = dot(e, LUMA);
            if filtered > 1e-6 {
                let f = dot(plain, LUMA) / filtered;
                e = map(e, |_, x| x * f);
            }
        }
        if k.hunt > 0.0 {
            let g = (windowed[0] + windowed[1] + windowed[2]) / 3.0;
            density *= (-k.hunt * (shaped(g, ps, pc, pz) - g)).exp();
        }
    }
    coloured(e, density)
}

/// Each channel's light to the power `density`, the luma's own power undone.
fn coloured(e: V3, density: f32) -> V3 {
    if density == 1.0 {
        return e;
    }
    let pw = map(e, |_, x| x.max(0.0).powf(density));
    let p = dot(pw, LUMA);
    if p > 1e-12 {
        let f = p.powf(1.0 / density - 1.0);
        return map(pw, |_, x| x * f);
    }
    e
}
