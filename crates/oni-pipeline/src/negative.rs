//! Lighting on the negative, the one operation every pipe takes: a frame's density read back to each layer's log
//! exposure through its stock's development, Exposure, Contrast, the colour tools and the filters laid there as a shot
//! exposed otherwise, the film developed again, then its points, zones and mids on its density; the print, ONI's or a
//! paper, at its rest, so it bounds every colour the tools lay.

use crate::finishing;
use crate::film::bake;
use crate::baked::Baked;
use crate::film::dyes;
use crate::film::profile::{Profile, Silver};
use crate::paper::{self, bisected, film_ends, spanned};
use oni_render::{Development, LIGHTNESS_HIGH, LIGHTNESS_LOW, LIGHTNESS_NODES, LUMA, NEGATIVE_CURVE_NODES};
use oni_render::{Negative, Stage47, cpu};
use oni_settings::stocks::DyeSource;
use oni_settings::tone::color_density;
use oni_settings::{DyeSet, PipelineSettings, VirtualStock};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Film {
    /// A digital photo's virtual stock, its own curves, pushed ones included; a black and white one's single layer
    /// uncoupled.
    Stock(VirtualStock),
    /// A scanned colour negative's stock, by its name, read on its mean layer; the generic set on the C-41 profiles'.
    Dyes(DyeSet),
    /// A scanned black and white negative: the C-41 profiles' mean curve on its one layer, no couplers.
    Silver,
    /// A positive: its own light, each layer's density its log exposure turned over.
    Light,
    /// A slide projected: its stock's dyes' amounts, each layer read as the neutral density holding it and along the
    /// green layer's curve, so a grey stays grey; falling with exposure, read turned over.
    Slide(VirtualStock),
}

impl Film {
    /// What `s`'s Lighting develops again on stage 4-7; a projected slide's is its stock's (`slide`).
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

    /// Its file's content, which its development is kept under: an edited file develops anew.
    fn revision(self) -> u64 {
        match self {
            Film::Stock(v) => v.entry().revision,
            Film::Dyes(d) => d.entry().revision,
            Film::Silver => DyeSet::default().entry().revision,
            Film::Light => 0,
            Film::Slide(v) => v.entry().revision,
        }
    }
}

/// The mean of `curves` over the log exposures they all hold, on the development's nodes, of the profiles `set`
/// averages: the generic set's, the C-41 profiles.
fn generic(set: DyeSet, curves: fn(&Profile) -> &Vec<[f64; 3]>) -> (Vec<f64>, Vec<[f64; 3]>) {
    let profiles = dyes::sources(set);
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

fn developing(le: &[f64], density: &[[f64; 3]], coupled: Option<&bake::M3>) -> Development {
    let (normal, after) = bake::curves_through(le, density, coupled.unwrap_or(&bake::COUPLERS));
    let couplers = coupled.map_or([[0.0; 3]; 3], |m| m.map(|r| r.map(|v| v as f32)));
    let after = if coupled.is_some() { after } else { normal.clone() };
    tabled(le, &normal, &after, couplers)
}

/// The development of the curves `normal` and `after` along `le`, through `couplers`.
fn tabled(le: &[f64], normal: &[[f64; 3]], after: &[[f64; 3]], couplers: [V3; 3]) -> Development {
    let (low, high) = (le[0], le[le.len() - 1]);
    let after = rising(le, after);
    let step = (high - low) / (NEGATIVE_CURVE_NODES - 1) as f64;
    // The toe's foot where the curve first rises READABLE a log, the shoulder's end where it last does.
    let reads = |c: usize, i: usize| f64::from(after[i + 1][c] - after[i][c]) >= READABLE * step;
    let foot = std::array::from_fn(|c| (0..NEGATIVE_CURVE_NODES - 1).find(|i| reads(c, *i)).map_or(after[0][c], |i| after[i][c]));
    let top = std::array::from_fn(|c| (0..NEGATIVE_CURVE_NODES - 1).rev().find(|i| reads(c, *i)).map_or(after[NEGATIVE_CURVE_NODES - 1][c], |i| after[i + 1][c]));
    Development { normal: rising(le, normal), after, log_low: low as f32, log_step: step as f32, foot, top, couplers }
}

fn developed(film: Film) -> Development {
    match film {
        Film::Stock(stock) if stock.is_silver() => {
            let p = Silver::read(stock.entry().profile.as_bytes()).expect("a profile reads once its file is taken");
            let curve: Vec<[f64; 3]> = p.density_curve.iter().map(|d| [*d; 3]).collect();
            developing(&p.log_exposure, &curve, None)
        }
        Film::Stock(stock) => {
            let entry = stock.entry();
            let p = dyes::profile(&entry.profile);
            developing(&p.log_exposure, &p.density_curves, Some(&crate::film::couplers(&entry)))
        }
        // A scan's three layers are read on one curve, its stock's mean layer, each scaled to the frame's own: its windows
        // print the frame's neutral, which every common move then keeps; the model's own neutral is a virtual film's alone.
        Film::Dyes(set) => {
            let (le, density) = match set.entry().source {
                DyeSource::Profile(json) => {
                    let p = dyes::profile(&json);
                    (p.log_exposure, p.density_curves)
                }
                DyeSource::Mean(_) => generic(set, |p| &p.density_curves),
            };
            developing(&le, &one_layer(&density), None)
        }
        // Silver is neutral: its scan reads a density, the sheets' own curves, no dye's amount.
        Film::Silver => {
            let (le, mean) = generic(DyeSet::default(), |p| &p.sheet_density_curves);
            developing(&le, &one_layer(&mean), None)
        }
        Film::Slide(stock) => {
            let (le, amounts) = crate::film::projection::tone(stock);
            let turned: Vec<[f64; 3]> = amounts.iter().map(|a| a.map(|v| -v)).collect();
            tabled(&le, &turned, &turned, [[0.0; 3]; 3])
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

/// The film's development, baked at first use and again once its file changes.
pub fn development(film: Film) -> &'static Development {
    static TABLES: Baked<(Film, u64), Development> = Baked::new();
    TABLES.get((film, film.revision()), || developed(film))
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
    let print = k.after_negative();
    let step = (LIGHTNESS_HIGH - LIGHTNESS_LOW) / (LIGHTNESS_NODES - 1) as f32;
    std::array::from_fn(|i| {
        let v = cpu::stage47(n.grey(LIGHTNESS_LOW + step * i as f32), &print);
        (0..3).map(|c| LUMA[c] * v[c]).sum::<f32>().max(0.0).powf(1.0 / oni_settings::tone::Print::DISPLAY_GAMMA).clamp(0.0, 1.0)
    })
}

/// The frame's negative under its Lighting on `k`, stage 4-7's arguments with Lighting at rest; none while it rests.
/// The moves turn about the reference grey's exposure, so Contrast holds the grey and the print's rest is the bare film's.
pub fn negative(s: &PipelineSettings, k: &Stage47) -> Option<Negative> {
    if rests(s) {
        return None;
    }
    let positive = s.mode.reads_positive();
    let sense = if positive { -1.0 } else { 1.0 };
    // Each channel's density above its bare film, as the window reads it: a scan's, or a digital photo's layers.
    let film = Film::of(s);
    let anchors = paper::anchors(s);
    let development = development(film);
    // A colour negative's layers developed so its reference grey is the stock's neutral at the speed's exposure, so
    // every common move on that neutral stays neutral; a positive's and silver's layers are one already, a black and
    // white film's density its own curve's.
    let grey = anchors[2].map(|v| sense * v);
    let speed = development.normal[development.at_speed()];
    let scale = match film {
        Film::Stock(stock) if stock.is_silver() => [1.0; 3],
        Film::Stock(_) | Film::Dyes(_) => std::array::from_fn(|c| grey[c] / speed[c]),
        Film::Silver | Film::Light | Film::Slide(_) => [1.0; 3],
    };
    let span = anchors[1].map(spanned);
    let read = Read {
        gain: k.gain,
        pedestal: k.pedestal,
        log_guard: k.log_guard,
        black: film_ends(s).0,
        sense,
        development,
        scale,
        pivot: development.exposures(std::array::from_fn(|c| grey[c] / scale[c])),
        span: if positive { span.map(|v| -v) } else { span },
        origin: if positive { 1.0 } else { 0.0 },
    };
    moved(s, read)
}

/// A slide's dyes under the frame's Lighting, each layer's amount coded on its own channel as `10^-amount` in and out;
/// none while it rests. The moves turn about the stock's speed, where its sheet exposes a grey, and the tones read each
/// amount as a share of its layer's densest.
pub fn slide(s: &PipelineSettings, stock: VirtualStock) -> Option<Negative> {
    if rests(s) {
        return None;
    }
    let development = development(Film::Slide(stock));
    let densest = development.after[0].map(|v| -v);
    let read = Read {
        gain: [1.0; 3],
        pedestal: 0.0,
        log_guard: 10f32.powf(-oni_render::DYE_HIGH),
        black: [0.0; 3],
        sense: -1.0,
        development,
        scale: [1.0; 3],
        pivot: [0.0; 3],
        span: densest.map(|v| -spanned(v)),
        origin: 1.0,
    };
    moved(s, read)
}

/// How Lighting reads a film: the light coding each layer's density, its development, each layer's density over the
/// development's own, the log exposure the moves turn about, and the density's tone share, `origin + density / span`.
struct Read {
    gain: V3,
    pedestal: f32,
    log_guard: f32,
    black: V3,
    sense: f32,
    development: &'static Development,
    scale: V3,
    pivot: V3,
    span: V3,
    origin: f32,
}

/// The frame's Lighting and colour tools laid on the film `r` reads; none where they lay nothing.
fn moved(s: &PipelineSettings, r: Read) -> Option<Negative> {
    let print = &s.print;
    let mono = s.is_monochrome();
    let Read { development, scale, pivot, .. } = r;
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
    let at = Negative {
        gain: r.gain,
        pedestal: r.pedestal,
        log_guard: r.log_guard,
        black: r.black,
        sense: r.sense,
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
        span: r.span,
        origin: r.origin,
        points_black: points.map(|p| p.0),
        points_white: points.map(|p| p.1),
        zones: [print.shadows, print.highlights],
        mids: own(print.channel_mid),
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
