//! Moves a document written for the former tone chain (the linked and luma windows, the exposure
//! factor, the curves' contrast, the finishing balance and the fade) onto Lighting, by a fit.

use crate::axis::pipeline;
use crate::curves::{Curve, CurveSet, Evaluated};
use crate::levels::{Levels, LevelsChannel};
use crate::modes::ConversionMode;
use crate::settings::PipelineSettings;
use crate::tolerant::{Object, number_f32};
use crate::tone::{Print, Tone, color_density, contrast};
use serde_json::Value;

type V3 = [f32; 3];

#[derive(Debug, Clone, PartialEq)]
pub struct Former {
    pub linked: Levels,
    pub luma: Levels,
    pub exposure: f32,
    pub contrast: f32,
    /// Five zones' colour offsets, black to white, as the finishing balance stored them.
    pub balance: [V3; 5],
    pub fade: f32,
    pub density: f32,
    pub mode: ConversionMode,
}

impl Default for Former {
    fn default() -> Self {
        Former {
            linked: Levels { mid: Levels::resting_mid(ConversionMode::Negative), ..Levels::default() },
            luma: Levels::default(),
            exposure: 0.0,
            contrast: 0.0,
            balance: [[0.0; 3]; 5],
            fade: 0.0,
            density: 0.0,
            mode: ConversionMode::Negative,
        }
    }
}

impl Former {
    pub fn balance_is_neutral(&self) -> bool {
        self.balance.iter().all(|z| *z == [0.0; 3])
    }
}

// MARK: - Reading the raw document

/// The former chain a raw settings object carries; `None` once it holds a Lighting block.
pub fn former(settings: &Value) -> Option<Former> {
    let settings = settings.as_object()?;
    if settings.contains_key("print")
        || !["levels", "exposure", "curves", "toneBalance", "fade"].iter().any(|k| settings.contains_key(*k))
    {
        return None;
    }
    fn levels(any: Option<&Value>, fallback: Levels) -> Levels {
        let Some(d) = any.and_then(Value::as_object) else { return fallback };
        let read = |key: &str, v: f32| number_f32(d.get(key)).unwrap_or(v);
        Levels {
            black: read("black", fallback.black),
            white: read("white", fallback.white),
            mid: read("mid", fallback.mid),
            shadows: read("shadows", fallback.shadows),
            highlights: read("highlights", fallback.highlights),
        }
    }
    let inner = |key: &str, sub: &str| settings.get(key).and_then(Value::as_object).and_then(|o| o.get(sub));
    let mut f = Former {
        mode: settings
            .get("mode")
            .and_then(Value::as_str)
            .and_then(ConversionMode::from_raw)
            .unwrap_or(ConversionMode::Negative),
        ..Former::default()
    };
    let rest = Levels { mid: Levels::resting_mid(f.mode), ..Levels::default() };
    let set: Option<&Object> = settings.get("levels").and_then(Value::as_object);
    f.linked = levels(set.and_then(|s| s.get("linked")), rest);
    f.luma = levels(set.and_then(|s| s.get("luma")), Levels::default());
    f.exposure = number_f32(settings.get("exposure")).unwrap_or(0.0);
    f.contrast = number_f32(inner("curves", "contrast")).unwrap_or(0.0);
    f.fade = number_f32(inner("fade", "amount")).unwrap_or(0.0);
    f.density = number_f32(settings.get("density")).unwrap_or(0.0);
    if let Some(balance) = settings.get("toneBalance").and_then(Value::as_object) {
        for (i, zone) in ["black", "shadows", "midtones", "highlights", "white"].iter().enumerate() {
            let colour: Option<Vec<f32>> = balance
                .get(*zone)
                .and_then(Value::as_object)
                .and_then(|z| z.get("colour"))
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|v| number_f32(Some(v))).collect());
            if let Some(c) = colour
                && c.len() == 3
            {
                f.balance[i] = [c[0], c[1], c[2]];
            }
        }
    }
    Some(f)
}

// MARK: - The former chain on the grey axis

fn window(e: f32, lv: &Levels) -> f32 {
    let t = ((e - lv.black) / (lv.white - lv.black).max(1e-4)).max(0.0).powf(lv.gamma());
    let (c1, c3) = (lv.shadow_ordinate(), lv.highlight_ordinate());
    if c1 == 0.25 && c3 == 0.75 {
        return t;
    }
    let u = t.min(1.0);
    let v = 1.0 - u;
    c1 * 4.0 * u * v * v * v + 3.0 * u * u * v * v + c3 * 4.0 * u * u * u * v + u * u * u * u + (t - u)
}

fn balanced(x: f32, channel: usize, f: &Former) -> f32 {
    let raw: Vec<f32> = (0..5)
        .map(|i| {
            let offset = f.balance[i][channel] * 0.25;
            let soft = 0.25 * (1.0 - (-offset.abs() / 0.25).exp());
            i as f32 / 4.0 + if offset < 0.0 { -soft } else { soft }
        })
        .collect();
    let low = raw[0];
    let high = raw[4].max(low);
    let mut c = vec![low];
    let mut floor = low;
    for r in &raw[1..4] {
        floor = r.max(floor).min(high);
        c.push(floor);
    }
    c.push(high);
    let t = x.clamp(0.0, 1.0);
    let u = 1.0 - t;
    let curve = c[0] * u * u * u * u
        + c[1] * 4.0 * t * u * u * u
        + c[2] * 6.0 * t * t * u * u
        + c[3] * 4.0 * t * t * t * u
        + c[4] * t * t * t * t;
    curve + (x - t)
}

/// The former fade's shoulder: the toe lifting black by 0.01, the knee at 0.55 pulling white 0.24.
fn faded(x: f32, amount: f32) -> f32 {
    let v = x.clamp(0.0, 1.0);
    let t = (1.0 - v / 0.30).max(0.0);
    let o = (v - 0.55).max(0.0);
    let span: f32 = 0.45;
    let target = span - amount * 0.24;
    if target >= span {
        return v;
    }
    let (mut low, mut high) = (target, 1e4f32);
    for _ in 0..60 {
        let room = (low + high) / 2.0;
        if room * (1.0 - (-span / room).exp()) < target {
            low = room;
        } else {
            high = room;
        }
    }
    let room = (low + high) / 2.0;
    let u = o / room;
    v + amount * 0.01 * t * t - room * (u - 1.0 + (-u).exp())
}

/// A curve set evaluated once, read past 1 on its last slope.
pub struct Curves {
    tables: [Evaluated; 4],
    luma: Evaluated,
    active: bool,
}

impl Curves {
    pub fn new(set: &CurveSet) -> Self {
        Curves {
            tables: [set.linked.evaluated(), set.red.evaluated(), set.green.evaluated(), set.blue.evaluated()],
            luma: set.luma.evaluated(),
            active: !set.touched().is_empty(),
        }
    }

    fn read(table: &Evaluated, v: f32) -> f32 {
        if v <= 1.0 { table.value(f64::from(v.max(0.0))) as f32 } else { table.value(1.0) as f32 + v - 1.0 }
    }

    pub fn applied(&self, rgb: V3) -> V3 {
        if !self.active {
            return rgb;
        }
        let e = [Self::read(&self.tables[1], rgb[0]), Self::read(&self.tables[2], rgb[1]), Self::read(&self.tables[3], rgb[2])];
        let mut e = e.map(|c| Self::read(&self.tables[0], c));
        let y = pipeline::luma(e);
        if y > 1e-4 {
            let k = (Self::read(&self.luma, y) / y).clamp(0.0, 8.0);
            e = e.map(|c| c * k);
        }
        e
    }
}

/// One factor on three channels: a stop moved shouldered into three quarters of the top channel's room.
fn former_exposure(peak: f32, stops: f32) -> f32 {
    let k = stops.exp2();
    if k == 1.0 || peak <= 0.0 {
        return k;
    }
    let room = (1.0 - peak).max(1e-6) * 0.75;
    let travel = room * (1.0 - (-peak * (k - 1.0).abs() / room).exp());
    (if k > 1.0 { peak + travel } else { peak - travel }) / peak
}

pub fn former_pixel(n: V3, f: &Former, manual: &Curves, colour: &Curves) -> V3 {
    let mut e = n.map(|c| window(c, &f.linked));
    let m = e[0].max(e[1]).max(e[2]);
    if f.exposure != 0.0 && m > 0.0 {
        let k = former_exposure(m, f.exposure);
        e = e.map(|c| c * k);
    }
    if !f.mode.is_monochrome() {
        e = color_density::rendered(e, f.density + f.exposure * -0.2);
    }
    let y = pipeline::luma(e);
    if y > 1e-4 {
        let k = (window(y, &f.luma) / y).clamp(0.0, 8.0);
        e = e.map(|c| c * k);
    }
    if f.contrast != 0.0 {
        let c = |v: f32| {
            if v <= 1.0 {
                contrast::apply(v.max(0.0), f.contrast, 0.5)
            } else {
                contrast::apply(1.0, f.contrast, 0.5) + v - 1.0
            }
        };
        e = e.map(c);
    }
    let mut rgb = colour.applied(manual.applied(e));
    if !f.balance_is_neutral() {
        rgb = [balanced(rgb[0], 0, f), balanced(rgb[1], 1, f), balanced(rgb[2], 2, f)];
    }
    if f.fade > 0.0 {
        rgb = rgb.map(|c| faded(c, f.fade));
    }
    rgb
}

pub fn new_pixel(n: V3, print: &Print, density: f32, gammas: V3, monochrome: bool, manual: &Curves, colour: &Curves) -> V3 {
    let moved = print.pointed(n, gammas);
    let rgb = if monochrome {
        [
            print.output(moved[0], LevelsChannel::Red, true),
            print.output(moved[1], LevelsChannel::Green, true),
            print.output(moved[2], LevelsChannel::Blue, true),
        ]
    } else {
        print.rendered(moved, density)
    };
    colour.applied(manual.applied(rgb))
}

// MARK: - The fit

/// The grey axis sampled where the eye reads it.
fn greys() -> Vec<V3> {
    (0..=48).map(|i| [(i as f32 / 48.0).powf(1.3); 3]).collect()
}

/// Each channel pushed either way about six lightnesses, where the two exposures part.
fn colours() -> Vec<V3> {
    let mut out = Vec::new();
    for t in [0.15f32, 0.3, 0.45, 0.6, 0.75, 0.9] {
        for c in 0..3 {
            let mut up = [t; 3];
            let mut down = up;
            up[c] += 0.08;
            down[c] -= 0.08;
            out.push(up);
            out.push(down);
        }
    }
    out
}

const COLOUR_WEIGHT: f32 = 1.2;

pub fn display(v: f32) -> f32 {
    v.clamp(0.0, 1.0).powf(1.0 / Print::DISPLAY_GAMMA)
}

type Bounds = (f32, f32);

/// A damped Gauss-Newton (Levenberg-Marquardt) on bounded parameters, with a ridge pulling each to zero.
fn minimised(start: Vec<f32>, bounds: &[Bounds], ridge: &[f32], residuals: &dyn Fn(&[f32]) -> Vec<f32>) -> Vec<f32> {
    let cost = |r: &[f32], p: &[f32]| -> f32 {
        let a = r.iter().fold(0.0f32, |s, v| s + v * v);
        p.iter().zip(ridge).fold(a, |s, (p, k)| s + p * p * k * k)
    };
    let all = |p: &[f32]| -> Vec<f32> {
        let mut r = residuals(p);
        r.extend(p.iter().zip(ridge).map(|(p, k)| p * k));
        r
    };
    let mut x = start;
    let mut r = all(&x);
    let mut current = cost(&residuals(&x), &x);
    let mut lambda: f32 = 1e-3;
    for _ in 0..80 {
        let n = x.len();
        let m = r.len();
        let mut jac = vec![0f32; m * n];
        for j in 0..n {
            let mut p = x.clone();
            let h = (1e-3 * (bounds[j].1 - bounds[j].0)).max(1e-5);
            p[j] = x[j] + h;
            let rj = all(&p);
            for i in 0..m {
                jac[i * n + j] = (rj[i] - r[i]) / h;
            }
        }
        let mut jtj = vec![0f32; n * n];
        let mut jtr = vec![0f32; n];
        for i in 0..m {
            for a in 0..n {
                let ja = jac[i * n + a];
                jtr[a] += ja * r[i];
                for b in 0..n {
                    jtj[a * n + b] += ja * jac[i * n + b];
                }
            }
        }
        let mut improved = false;
        for _ in 0..8 {
            let mut a = jtj.clone();
            for d in 0..n {
                a[d * n + d] *= 1.0 + lambda;
                a[d * n + d] += 1e-9;
            }
            let b: Vec<f32> = jtr.iter().map(|v| -v).collect();
            let Some(step) = solved(a, b, n) else {
                lambda *= 10.0;
                continue;
            };
            let trial: Vec<f32> =
                (0..n).map(|j| (x[j] + step[j]).max(bounds[j].0).min(bounds[j].1)).collect();
            let c = cost(&residuals(&trial), &trial);
            if c < current {
                let gain = current - c;
                x = trial;
                current = c;
                r = all(&x);
                lambda = (lambda / 3.0).max(1e-7);
                improved = true;
                if gain < 1e-10 {
                    return x;
                }
                break;
            }
            lambda *= 10.0;
        }
        if !improved {
            break;
        }
    }
    x
}

/// Gaussian elimination with partial pivoting; ties keep the first row, as the reference does.
fn solved(mut m: Vec<f32>, mut v: Vec<f32>, n: usize) -> Option<Vec<f32>> {
    for c in 0..n {
        let mut p = c;
        for row in (c + 1)..n {
            if m[p * n + c].abs() < m[row * n + c].abs() {
                p = row;
            }
        }
        if m[p * n + c].abs() <= 1e-12 {
            return None;
        }
        if p != c {
            for k in 0..n {
                m.swap(c * n + k, p * n + k);
            }
            v.swap(c, p);
        }
        for r in (c + 1)..n {
            let f = m[r * n + c] / m[c * n + c];
            if f == 0.0 {
                continue;
            }
            for k in c..n {
                m[r * n + k] -= f * m[c * n + k];
            }
            v[r] -= f * v[c];
        }
    }
    let mut x = vec![0f32; n];
    for r in (0..n).rev() {
        let mut s = v[r];
        for k in (r + 1)..n {
            s -= m[r * n + k] * x[k];
        }
        x[r] = s / m[r * n + r];
    }
    Some(x)
}

fn tone_bounds() -> Vec<Bounds> {
    vec![Print::EXPOSURE_RANGE, Print::CONTRAST_RANGE, (0.0, 0.0), Print::ZONE_RANGE, Print::ZONE_RANGE, (0.0, 0.0)]
}

fn tone(v: &[f32]) -> Tone {
    Tone { exposure: v[0], contrast: v[1], blacks: v[2], shadows: v[3], highlights: v[4], whites: v[5], ..Tone::NEUTRAL }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Fit {
    pub print: Print,
    pub density: f32,
    pub worst: f32,
}

/// Lighting, the colour density and the filters that best render the former chain; Lighting pins
/// black and white, so a former end that clipped is not reproduced.
pub fn fitted(f: &Former, manual_curves: &CurveSet, curves: &CurveSet, gammas: V3) -> Fit {
    let manual = Curves::new(manual_curves);
    let mut colour_set = curves.clone();
    colour_set.contrast = 0.0;
    let colour = Curves::new(&colour_set);
    // The channel tables held Temperature and Tint, retired on the way in: the new chain runs without.
    let mut unfiltered = colour_set.clone();
    unfiltered.red = Curve::default();
    unfiltered.green = Curve::default();
    unfiltered.blue = Curve::default();
    let filters_moved = unfiltered != colour_set;
    let bare = Curves::new(&unfiltered);
    let mono = f.mode.is_monochrome();
    let greys = greys();
    let mut points = greys.clone();
    let mut weights = vec![1f32; greys.len()];
    if !mono {
        let colours = colours();
        weights.extend(std::iter::repeat_n(COLOUR_WEIGHT, colours.len()));
        points.extend(colours);
    }
    let targets: Vec<V3> = points.iter().map(|p| former_pixel(*p, f, &manual, &colour)).collect();
    let residuals = |print: &Print, density: f32| -> Vec<f32> {
        let mut out = Vec::with_capacity(points.len() * 3);
        for i in 0..points.len() {
            let got = new_pixel(points[i], print, density, gammas, mono, &manual, &bare);
            for c in 0..3 {
                out.push(weights[i] * (display(got[c]) - display(targets[i][c])));
            }
        }
        out
    };
    // The RGB curve and the colour density first, the density starting where the former one stood.
    let former = f.density + f.exposure * -0.2;
    let mut bounds = tone_bounds();
    let density_at = bounds.len();
    if !mono {
        bounds.push((color_density::RANGE.0 - former, color_density::RANGE.1 - former));
    }
    let resting = |p: &[f32]| {
        let mut print = Print { black_point: 0.0, ..Print::default() };
        print.set_master(tone(&p[0..6]));
        print.hunt_base = Some(print.master());
        print
    };
    let first = minimised(vec![0.0; bounds.len()], &bounds, &vec![2e-3; bounds.len()], &|p| {
        residuals(&resting(p), if mono { f.density } else { former + p[density_at] })
    });
    // The former render had no lifted black, and the compensation measures from the fitted curve.
    let mut print = resting(&first);
    let density = if mono { f.density } else { former + first[density_at] };
    // Then the three filters, only where the balance or the former filters parted the channels.
    if !f.balance_is_neutral() || filters_moved {
        let filters = minimised(vec![0.0; 3], &[Print::FILTER_RANGE; 3], &[1e-2; 3], &|p| {
            let mut trial = print;
            trial.red = Tone::exposure(p[0]);
            trial.green = Tone::exposure(p[1]);
            trial.blue = Tone::exposure(p[2]);
            residuals(&trial, density)
        });
        print.red = Tone::exposure(filters[0]);
        print.green = Tone::exposure(filters[1]);
        print.blue = Tone::exposure(filters[2]);
    }
    let worst = residuals(&print, density).iter().fold(0.0f32, |w, r| w.max(r.abs()));
    Fit { print, density, worst }
}

/// Settings decoded from a former document, with Lighting set to render as it did.
pub fn migrated(settings: &PipelineSettings, f: &Former, colour: &CurveSet) -> PipelineSettings {
    let l = &settings.levels;
    let gammas = [l.red.gamma(), l.green.gamma(), l.blue.gamma()];
    let fit = fitted(f, &settings.manual_curves, colour, gammas);
    let mut out = settings.clone();
    out.print = fit.print;
    out.density = fit.density;
    out
}
