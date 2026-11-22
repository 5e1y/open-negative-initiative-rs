//! The fused conversion kernel (`pipeline.metal`): the dye separation then inversion, levels, the print curve;
//! or the dye separation then the paper's print, its own, which only the colour density follows.

use super::*;

/// The fused kernel's arguments, named as the Metal kernel names them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stage47 {
    pub gain: [f32; 3],
    pub red: [f32; 3],
    pub green: [f32; 3],
    pub blue: [f32; 3],
    pub invert: f32,
    pub density: f32,
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
    /// The stock's dyes unmixed on the scan's densities before the windows, a pass of its own on the GPU.
    pub dyes: Option<super::dye_separation::DyeSeparation>,
    /// The negative printed on a paper in place of the windows and the print curve, laid in the same GPU pass.
    pub paper: Option<super::paper::Paper>,
}

impl Stage47 {
    /// A negative at rest: no print, identity separation, no decompression.
    pub fn rest() -> Self {
        Self {
            gain: [1.0; 3],
            red: [0.0, 2.0, 1.0],
            green: [0.0, 2.0, 1.0],
            blue: [0.0, 2.0, 1.0],
            invert: 1.0,
            density: 1.0,
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
            dyes: None,
            paper: None,
        }
    }

    /// What stage 4 reads past the dye separation's pass: its dye amounts as light, at unit gain and no pedestal.
    pub fn after_dyes(&self) -> Self {
        match self.dyes {
            Some(_) => Self { gain: [1.0; 3], pedestal: 0.0, dyes: None, ..*self },
            None => *self,
        }
    }

    /// The uniform block, every vector padded to four lanes as WGSL lays it out.
    pub fn packed(&self, count: u32) -> Packed {
        Packed {
            rows: [
                v3(self.gain, self.invert),
                v3(self.red, self.density),
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

fn apply_levels(e: f32, lv: V3) -> f32 {
    ((e - lv[0]) / (lv[1] - lv[0]).max(1e-4)).max(0.0).powf(lv[2])
}

fn apply_window(e: f32, lv: V3, c1: f32, c3: f32) -> f32 {
    let t = apply_levels(e, lv);
    if c1 == 0.25 && c3 == 0.75 {
        return t;
    }
    let u = t.min(1.0);
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

/// Stages 4 to 7 on one pixel, the stock's dyes unmixed first where the frame names them, then its paper.
pub fn stage47(s: [f32; 4], k: &Stage47) -> [f32; 4] {
    let (s, k) = match &k.dyes {
        Some(d) => (super::dye_separation::dye_separation(s, k, d), k.after_dyes()),
        None => (s, *k),
    };
    let s = match &k.paper {
        Some(p) => super::paper::paper(s, p),
        None => s,
    };
    let e = printed3(windowed(s, &k), &k);
    [e[0], e[1], e[2], s[3]]
}

/// Stages 4 and 5: the film's density, its windows and the chroma decompression; a paper's print passes as it is.
fn windowed(s: [f32; 4], k: &Stage47) -> V3 {
    if k.paper.is_some() {
        return rgb(s);
    }
    let t = map([s[0], s[1], s[2]], |c, x| (x + k.pedestal) * k.gain[c]);
    let d = map(t, |_, x| -x.max(k.log_guard).log10());
    let mut e = if k.invert > 0.5 {
        d
    } else {
        map(d, |_, x| k.window - x)
    };
    let lv = [k.red, k.green, k.blue];

    let film = e;
    e = map(e, |c, x| apply_window(x, lv[c], k.low_points[c], k.high_points[c]));
    if k.decompression != 1.0 || k.film_share != 0.0 {
        let place = map(film, |c, x| (x - k.balance_low[c]) / k.balance_span[c]);
        let mean = (place[0] + place[1] + place[2]) / 3.0;
        let neutral = map(k.balance_low, |c, x| x + mean * k.balance_span[c]);
        let toned = map(neutral, |c, x| apply_window(x, lv[c], k.low_points[c], k.high_points[c]));
        let held = map(toned, |c, x| x + k.decompression * (e[c] - x) + k.film_share * (film[c] - neutral[c]));
        let shift = dot(e, LUMA) - dot(held, LUMA);
        e = map(held, |_, x| x + shift);
    }
    e
}

/// Stages 6 and 7 on a windowed pixel: the print, its filters, Hunt and the colour density; Contrast's colour is
/// held downstream by its saturation curve.
fn printed3(windowed_in: V3, k: &Stage47) -> V3 {
    let mut density = k.density;
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
    if density != 1.0 {
        let pw = map(e, |_, x| x.max(0.0).powf(density));
        let p = dot(pw, LUMA);
        if p > 1e-12 {
            let f = p.powf(1.0 / density - 1.0);
            e = map(pw, |_, x| x * f);
        }
    }
    e
}
