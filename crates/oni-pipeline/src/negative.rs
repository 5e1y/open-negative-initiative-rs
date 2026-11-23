//! Lighting on the negative, the one operation every pipe takes: a frame's density read back to each layer's log
//! exposure through its stock's development, Exposure, Contrast, the colour tools and the filters laid there as a shot
//! exposed otherwise, the film developed again, then its points, zones and mids on its density; the print, ONI's or a
//! paper, at its rest, so it bounds every colour the tools lay.

use crate::conversion;
use crate::finishing;
use crate::film::bake;
use crate::film::dyes::{self, C41};
use crate::film::profile::{Profile, Silver};
use crate::paper::{self, bisected, film_ends, spanned};
use oni_render::{DYE_LIGHT_FLOOR, DYE_TONE_NODES, Development, LIGHTNESS_HIGH, LIGHTNESS_LOW, LIGHTNESS_NODES, LUMA, NEGATIVE_CURVE_NODES};
use oni_render::{Negative, Stage47, cpu};
use oni_settings::tone::color_density;
use oni_settings::{DyeSet, PipelineSettings, VirtualStock};
use std::sync::OnceLock;

type V3 = [f32; 3];

/// Contrast's slope on the layers' log exposure: this to its power, so the track's ends are a grade softer or harder.
pub const GRADE_BASE: f32 = 4.0;

/// The least slope a development reads back on, in density a log: a toe or a shoulder flat to its noise still rises.
pub const LEAST_SLOPE: f64 = 0.01;

/// The least slope a layer's exposure is read on, in density a log: a hundredth of density, a scan's grain, then
/// reads within a tenth of a log.
pub const READABLE: f64 = 0.1;

/// The log exposure a positive's line spans either side of its bare film, past any slide's range.
const LIGHT_SPAN: f64 = 8.0;

/// The film a frame's Lighting develops again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Film {
    /// A digital photo's virtual stock, its own curves, pushed ones included; a black and white one's single layer
    /// uncoupled.
    Stock(VirtualStock),
    /// A scanned colour negative's stock, by its dyes, read on its mean layer; the generic set on the C-41 profiles'.
    Dyes(DyeSet),
    /// A scanned black and white negative: the C-41 profiles' mean curve on its one layer, no couplers.
    Silver,
    /// A positive: its own light, each layer's density its log exposure turned over.
    Light,
}

impl Film {
    /// What `s`'s Lighting develops again: a slide on a paper is its internegative's digital photo, read before.
    pub fn of(s: &PipelineSettings) -> Film {
        if s.mode.reads_positive() {
            Film::Light
        } else if let Some(f) = s.film() {
            Film::Stock(f.stock)
        } else if s.mode.is_monochrome() {
            Film::Silver
        } else {
            Film::Dyes(s.dye_set)
        }
    }

    fn index(self) -> usize {
        let stocks = VirtualStock::ALL.len();
        match self {
            Film::Stock(v) => VirtualStock::ALL.iter().position(|s| *s == v).expect("every stock is listed"),
            Film::Dyes(d) => stocks + DyeSet::ALL.iter().position(|s| *s == d).expect("every set is listed"),
            Film::Silver => stocks + DyeSet::ALL.len(),
            Film::Light => stocks + DyeSet::ALL.len() + 1,
        }
    }
}

fn read(name: &str) -> Profile {
    Profile::read(dyes::source(name).expect("a shipped profile")).expect("a shipped profile reads")
}

/// The C-41 profiles' mean of `curves` over the log exposures they all hold, on the development's nodes.
fn generic(curves: fn(&Profile) -> &Vec<[f64; 3]>) -> (Vec<f64>, Vec<[f64; 3]>) {
    let profiles = C41.map(read);
    let low = profiles.iter().map(|p| p.log_exposure[0]).fold(f64::NEG_INFINITY, f64::max);
    let high = profiles.iter().map(|p| p.log_exposure[p.log_exposure.len() - 1]).fold(f64::INFINITY, f64::min);
    let le: Vec<f64> = (0..NEGATIVE_CURVE_NODES).map(|i| low + (high - low) * i as f64 / (NEGATIVE_CURVE_NODES - 1) as f64).collect();
    let n = profiles.len() as f64;
    let mean = le
        .iter()
        .map(|e| std::array::from_fn(|c| profiles.iter().map(|p| bake::interpolate(*e, &p.log_exposure, curves(p), c)).sum::<f64>() / n))
        .collect();
    (le, mean)
}

/// The mean of a stock's three layers, on each.
fn one_layer(density: &[[f64; 3]]) -> Vec<[f64; 3]> {
    density.iter().map(|d| [(d[0] + d[1] + d[2]) / 3.0; 3]).collect()
}

/// `table` along `le` on the development's uniform nodes, each node at least `LEAST_SLOPE` a log over the last.
fn rising(le: &[f64], table: &[[f64; 3]]) -> [V3; NEGATIVE_CURVE_NODES] {
    let (low, high) = (le[0], le[le.len() - 1]);
    let step = (high - low) / (NEGATIVE_CURVE_NODES - 1) as f64;
    let mut out = [[0.0f32; 3]; NEGATIVE_CURVE_NODES];
    for c in 0..3 {
        let mut last = f64::NEG_INFINITY;
        for (i, node) in out.iter_mut().enumerate() {
            let v = bake::interpolate(low + step * i as f64, le, table, c).max(last + LEAST_SLOPE * step);
            node[c] = v as f32;
            last = v;
        }
    }
    out
}

fn developing(le: &[f64], density: &[[f64; 3]], coupled: bool) -> Development {
    let (normal, after) = bake::curves_of(le, density);
    let (low, high) = (le[0], le[le.len() - 1]);
    let couplers = if coupled { bake::COUPLERS.map(|r| r.map(|v| v as f32)) } else { [[0.0; 3]; 3] };
    let after = if coupled { after } else { normal.clone() };
    let after = rising(le, &after);
    let step = (high - low) / (NEGATIVE_CURVE_NODES - 1) as f64;
    // The toe's foot where the curve first rises READABLE a log, the shoulder's end where it last does.
    let reads = |c: usize, i: usize| f64::from(after[i + 1][c] - after[i][c]) >= READABLE * step;
    let foot = std::array::from_fn(|c| (0..NEGATIVE_CURVE_NODES - 1).find(|i| reads(c, *i)).map_or(after[0][c], |i| after[i][c]));
    let top = std::array::from_fn(|c| (0..NEGATIVE_CURVE_NODES - 1).rev().find(|i| reads(c, *i)).map_or(after[NEGATIVE_CURVE_NODES - 1][c], |i| after[i + 1][c]));
    Development { normal: rising(le, &normal), after, log_low: low as f32, log_step: step as f32, foot, top, couplers }
}

fn developed(film: Film) -> Development {
    match film {
        Film::Stock(stock) if stock.is_silver() => {
            let p = Silver::read(dyes::source(stock.profile()).expect("a shipped profile")).expect("a shipped profile reads");
            let curve: Vec<[f64; 3]> = p.density_curve.iter().map(|d| [*d; 3]).collect();
            developing(&p.log_exposure, &curve, false)
        }
        Film::Stock(stock) => {
            let p = read(stock.profile());
            developing(&p.log_exposure, &p.density_curves, true)
        }
        // A scan's three layers are read on one curve, its stock's mean layer, each scaled to the frame's own: its windows
        // print the frame's neutral, which every common move then keeps; the model's own neutral is a virtual film's alone.
        Film::Dyes(set) => {
            let (le, density) = match set.profile() {
                Some(name) => {
                    let p = read(name);
                    (p.log_exposure, p.density_curves)
                }
                None => generic(|p| &p.density_curves),
            };
            developing(&le, &one_layer(&density), false)
        }
        // Silver is neutral: its scan reads a density, the sheets' own curves, no dye's amount.
        Film::Silver => {
            let (le, mean) = generic(|p| &p.sheet_density_curves);
            developing(&le, &one_layer(&mean), false)
        }
        Film::Light => {
            let (le, line) = ([-LIGHT_SPAN, LIGHT_SPAN], [[-LIGHT_SPAN; 3], [LIGHT_SPAN; 3]]);
            let table = rising(&le, &line);
            let (low, step) = (-LIGHT_SPAN as f32, (2.0 * LIGHT_SPAN / (NEGATIVE_CURVE_NODES - 1) as f64) as f32);
            let (foot, top) = ([-LIGHT_SPAN as f32; 3], [LIGHT_SPAN as f32; 3]);
            Development { normal: table, after: table, log_low: low, log_step: step, foot, top, couplers: [[0.0; 3]; 3] }
        }
    }
}

/// The film's development, baked at first use.
pub fn development(film: Film) -> &'static Development {
    const COUNT: usize = VirtualStock::ALL.len() + DyeSet::ALL.len() + 2;
    static TABLES: [OnceLock<Development>; COUNT] = [const { OnceLock::new() }; COUNT];
    TABLES[film.index()].get_or_init(|| developed(film))
}

/// Whether Lighting reads `s`'s dyes' amounts, as a paper does and a digital photo's negative is developed, before any
/// tone: then ONI's print takes the dyes' pass untoned.
pub fn reads_amounts(s: &PipelineSettings) -> bool {
    s.paper().is_some() || s.film().is_some()
}

/// Whether nothing lays on `s`'s negative: its print bypassed, or Lighting there and every colour tool at rest; a black
/// and white print has no colour for the tools to move.
pub fn rests(s: &PipelineSettings) -> bool {
    s.print.bypassed || (s.print.rests_on_negative() && (s.is_monochrome() || colour_rests(s)))
}

fn colour_rests(s: &PipelineSettings) -> bool {
    s.density == color_density::BASE && finishing::spectrogram(&s.spectrogram).is_none() && finishing::saturation(&s.saturation).is_none()
}

/// The print's lightness, its luma on a display gamma, of a grey at each of the negative's lightness nodes, as `k`
/// prints it: the saturation curve's axis, the one the picture's histogram is drawn on.
pub fn lightness(n: &Negative, k: &Stage47) -> [f32; LIGHTNESS_NODES] {
    let print = k.after_dyes();
    let step = (LIGHTNESS_HIGH - LIGHTNESS_LOW) / (LIGHTNESS_NODES - 1) as f32;
    std::array::from_fn(|i| {
        let v = cpu::stage47(n.grey(LIGHTNESS_LOW + step * i as f32), &print);
        (0..3).map(|c| LUMA[c] * v[c]).sum::<f32>().max(0.0).powf(1.0 / oni_settings::tone::Print::DISPLAY_GAMMA).clamp(0.0, 1.0)
    })
}

/// The tone reading each density above the bare film as `offset` more, straight on: what a print reading the density
/// itself takes.
fn straight(offset: V3) -> ([V3; DYE_TONE_NODES], V3, V3) {
    let (low, high) = (-1.0f32, 5.0f32);
    let step = (high - low) / (DYE_TONE_NODES - 1) as f32;
    (std::array::from_fn(|i| offset.map(|o| low + i as f32 * step + o)), [low; 3], [step; 3])
}

/// The frame's negative under its Lighting on `k`, stage 4-7's arguments with Lighting at rest; none while it rests.
/// The moves turn about the reference grey's exposure, so Contrast holds the grey and the print's rest is the bare film's.
pub fn negative(s: &PipelineSettings, k: &Stage47) -> Option<Negative> {
    let print = &s.print;
    if rests(s) {
        return None;
    }
    let mono = s.is_monochrome();
    let positive = s.mode.reads_positive();
    let sense = if positive { -1.0 } else { 1.0 };
    // A paper reads the dyes' amounts, and so does a digital photo, its negative the model's own; ONI's print reads a
    // scan's density, its dyes toned onto the grey its windows print, which keeps the frame's neutral.
    let film = Film::of(s);
    let on_paper = s.paper().is_some();
    let amounts = k.dyes.as_ref().filter(|_| reads_amounts(s));
    let (gain, pedestal, log_guard, black) = match (amounts, k.dyes.is_some()) {
        (Some(_), _) => ([1.0; 3], 0.0, DYE_LIGHT_FLOOR, [0.0; 3]),
        (None, true) => ([1.0; 3], 0.0, k.log_guard, film_ends(s).0),
        (None, false) => (k.gain, k.pedestal, k.log_guard, film_ends(s).0),
    };
    let (tone, tone_low, tone_step) = match (on_paper, amounts) {
        (false, Some(_)) => {
            let toned = conversion::stock_separation(s).expect("the frame's dyes");
            (toned.tone, toned.tone_low, toned.tone_step)
        }
        (true, _) => straight([0.0; 3]),
        (false, None) => straight(black),
    };
    let anchors = paper::anchors(s, amounts);
    let development = development(film);
    // A colour negative's layers developed so its reference grey is the stock's neutral at the speed's exposure, so
    // every common move on that neutral stays neutral; a positive's and silver's layers are one already, a black and
    // white film's density its own curve's.
    let grey = anchors[2].map(|v| sense * v);
    let speed = development.normal[development.at_speed()];
    let scale = match film {
        Film::Stock(stock) if stock.is_silver() => [1.0; 3],
        Film::Stock(_) | Film::Dyes(_) => std::array::from_fn(|c| grey[c] / speed[c]),
        Film::Silver | Film::Light => [1.0; 3],
    };
    let pivot = development.exposures(std::array::from_fn(|c| grey[c] / scale[c]));
    let own = |v: V3| if mono { [0.0; 3] } else { v };
    let stops = print.exposure;
    let shift = |hold: f32| [stops * std::f32::consts::LOG10_2 + hold; 3];
    let slope = [GRADE_BASE.powf(print.contrast); 3];
    // Each channel's own moves turn about the grey as its layer develops it under the neutral ones.
    let own_pivot = development.inhibited(pivot.map(|e| e + stops * std::f32::consts::LOG10_2));
    let filters = own(print.filters());
    let own_shift = filters.map(|f| f * std::f32::consts::LOG10_2);
    let own_slope = own(print.channel_contrast).map(|c| GRADE_BASE.powf(c));
    let points = print.channel_points(mono);
    let span = anchors[1].map(spanned);
    let at = Negative {
        gain,
        pedestal,
        log_guard,
        black,
        sense,
        development,
        scale,
        pivot,
        slope,
        shift: shift(0.0),
        own_pivot,
        own_slope,
        own_shift,
        spread: if mono { 1.0 } else { color_density::factor(s.density) },
        mixer: finishing::spectrogram(&s.spectrogram).filter(|_| !mono),
        curve: finishing::saturation(&s.saturation).filter(|_| !mono),
        lightness: [0.0; LIGHTNESS_NODES],
        span: if positive { span.map(|v| -v) } else { span },
        origin: if positive { 1.0 } else { 0.0 },
        points_black: points.map(|p| p.0),
        points_white: points.map(|p| p.1),
        zones: [print.shadows, print.highlights],
        mids: own(print.channel_mid),
        tone,
        tone_low,
        tone_step,
    };
    // Moves a black and white print reads none of lay nothing.
    if !at.moves() && !at.tones() && !at.colours() {
        return None;
    }
    if filters == [0.0; 3] {
        return Some(at);
    }
    // A filter tints and the grey keeps its weight: one neutral stop holds its densities' luma as they print unfiltered.
    let grey = |n: &Negative| {
        let d = development.developed_on(n.owned(development.inhibited(n.moved(pivot))));
        (0..3).map(|c| LUMA[c] * d[c] * scale[c]).sum::<f32>()
    };
    let target = grey(&Negative { own_shift: [0.0; 3], ..at });
    let hold = bisected(-2.0, 2.0, target, |h| grey(&Negative { shift: shift(h), ..at }));
    Some(Negative { shift: shift(hold), ..at })
}
