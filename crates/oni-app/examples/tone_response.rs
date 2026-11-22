//! `tone_response <out> <tag> [file...]`: the whole system's tone response, print L* against the scene's stops from its
//! grey, on synthetic neutral ramps rendered through the engine, the paper's received log exposure traced on the CPU
//! references; then each file as its pipe prints it, with its L* spread. A digital photo's ramp is a scene; a scan's is
//! its stock's mean layer read through the frame's own measure; a slide's needs `SLIDE_CURVE` (JSON `x`, `m`: density
//! above its least along log exposure from its grey). `STOCK`, `DYES`, `PAPER` (a key, `oni` or `neutral`), `BLACKS`
//! (Paper blacks on) and `DERIVED` (a digital photo on its stock's derived reference) override.

use oni_app::render::{Decoded, Device, Engine, Linear, Reading};
use oni_pipeline::{conversion, film, paper};
use oni_render::{PAPER_CURVE_NODES, Paper, cpu};
use oni_settings::axis::density_axis;
use oni_settings::tone::Print;
use oni_settings::{ConversionMode, PipelineSettings, sidecar};
use std::io::Write;
use std::path::Path;

type V3 = [f32; 3];
type V4 = [f32; 4];

const PATCH: usize = 16;
const COLUMNS: usize = 16;
/// The ramp, in stops from the scene's grey.
const LOW: f32 = -8.0;
const HIGH: f32 = 6.0;
const STEP: f32 = 0.25;

fn stops() -> Vec<f32> {
    let n = ((HIGH - LOW) / STEP).round() as usize;
    (0..=n).map(|i| LOW + STEP * i as f32).collect()
}

fn luminance(v: &[f32]) -> f32 {
    0.2627 * v[0] + 0.6780 * v[1] + 0.0593 * v[2]
}

fn lstar(y: f32) -> f32 {
    let f = if y > 0.008856 { y.cbrt() } else { 7.787 * y + 16.0 / 116.0 };
    116.0 * f - 16.0
}

fn srgb(p: &[f32; 4]) -> [u8; 3] {
    let m = [[1.6605, -0.5876, -0.0728], [-0.1246, 1.1329, -0.0083], [-0.0182, -0.1006, 1.1187]];
    let encode = |v: f32| {
        let v = v.clamp(0.0, 1.0);
        let e = if v <= 0.0031308 { 12.92 * v } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
        (e * 255.0 + 0.5) as u8
    };
    std::array::from_fn(|r| encode(m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2]))
}

fn save_png(path: &Path, l: &Linear) {
    let file = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    let mut enc = png::Encoder::new(file, l.width as u32, l.height as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().unwrap();
    w.write_image_data(&l.pixels.iter().flat_map(srgb).collect::<Vec<u8>>()).unwrap();
}

/// The classic chart's 24 patches, sRGB 8-bit, as linear Rec. 2020.
fn chart() -> Vec<V4> {
    let srgb: [[u8; 3]; 24] = [
        [115, 82, 68], [194, 150, 130], [98, 122, 157], [87, 108, 67], [133, 128, 177], [103, 189, 170],
        [214, 126, 44], [80, 91, 166], [193, 90, 99], [94, 60, 108], [157, 188, 64], [224, 163, 46],
        [56, 61, 150], [70, 148, 73], [175, 54, 60], [231, 199, 31], [187, 86, 149], [8, 133, 161],
        [243, 243, 242], [200, 200, 200], [160, 160, 160], [122, 122, 121], [85, 85, 85], [52, 52, 52],
    ];
    let m = [[0.627404, 0.329283, 0.043313], [0.069097, 0.919540, 0.011362], [0.016391, 0.088013, 0.895595]];
    srgb.iter()
        .map(|c| {
            let lin = c.map(|v| {
                let v = f32::from(v) / 255.0;
                if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
            });
            let r: V3 = std::array::from_fn(|i| m[i][0] * lin[0] + m[i][1] * lin[1] + m[i][2] * lin[2]);
            [r[0], r[1], r[2], 1.0]
        })
        .collect()
}

/// Patches laid in rows as an image the engine decodes.
fn patches(values: &[V4]) -> Linear {
    let rows = values.len().div_ceil(COLUMNS);
    let (w, h) = (COLUMNS * PATCH, rows * PATCH);
    let mut pixels = vec![[0.0, 0.0, 0.0, 1.0]; w * h];
    for (i, v) in values.iter().enumerate() {
        let (px, py) = ((i % COLUMNS) * PATCH, (i / COLUMNS) * PATCH);
        for y in 0..PATCH {
            for x in 0..PATCH {
                pixels[(py + y) * w + px + x] = *v;
            }
        }
    }
    Linear { width: w, height: h, pixels }
}

/// Each patch's centre read back, its median per channel.
fn read_back(l: &Linear, count: usize) -> Vec<V3> {
    (0..count)
        .map(|i| {
            let (px, py) = ((i % COLUMNS) * PATCH, (i / COLUMNS) * PATCH);
            let mut ch: [Vec<f32>; 3] = Default::default();
            for y in PATCH / 4..3 * PATCH / 4 {
                for x in PATCH / 4..3 * PATCH / 4 {
                    let p = l.pixels[(py + y) * l.width + px + x];
                    (0..3).for_each(|c| ch[c].push(p[c]));
                }
            }
            std::array::from_fn(|c| {
                ch[c].sort_by(f32::total_cmp);
                ch[c][ch[c].len() / 2]
            })
        })
        .collect()
}

/// A relative curve `m(x)`, density above its least along log exposure from the grey, read from a JSON file.
struct Curve {
    x: Vec<f32>,
    m: Vec<f32>,
}

impl Curve {
    /// The mean of a stock's three printed curves above its least: the density a scan reads, its dyes' amounts mixed.
    fn of_stock(profile: &str) -> Curve {
        let json: serde_json::Value = serde_json::from_slice(oni_pipeline::film::dyes::source(profile).expect("a shipped profile")).expect("a profile");
        let numbers = |v: &serde_json::Value| v.as_array().expect("an array").iter().map(|x| x.as_f64().expect("a number")).collect::<Vec<f64>>();
        let rows = json["data"]["sheet_density_curves"].as_array().expect("the sheet's curves");
        let mean: Vec<f32> = rows.iter().map(|r| (numbers(r).iter().sum::<f64>() / 3.0) as f32).collect();
        let least = mean.iter().copied().fold(f32::MAX, f32::min);
        Curve { x: numbers(&json["data"]["log_exposure"]).iter().map(|v| *v as f32).collect(), m: mean.iter().map(|v| v - least).collect() }
    }

    fn read(path: &str) -> Curve {
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let get = |k: &str| v[k].as_array().unwrap().iter().map(|n| n.as_f64().unwrap() as f32).collect();
        Curve { x: get("x"), m: get("m") }
    }

    fn at(&self, x: f32) -> f32 {
        let n = self.x.len();
        if x <= self.x[0] {
            return self.m[0];
        }
        if x >= self.x[n - 1] {
            return self.m[n - 1];
        }
        let hi = self.x.partition_point(|v| *v <= x).min(n - 1);
        let t = (x - self.x[hi - 1]) / (self.x[hi] - self.x[hi - 1]);
        self.m[hi - 1] + t * (self.m[hi] - self.m[hi - 1])
    }
}

/// A frame's scan of the neutral ramp under its own measure: each channel's density above its bare film scaled so the
/// reference grey sits at the curve's grey, as the frame's gains and pedestal read it.
fn scan_ramp(s: &PipelineSettings, curve: &Curve) -> Vec<V4> {
    let (bare, _) = paper::film_ends(s);
    let grey = paper::reference_grey(s);
    let m0 = curve.at(0.0);
    let gain = s.gains.stops.map(f32::exp2);
    let pedestal = density_axis::pedestal(conversion::rendered_ceiling(s));
    stops()
        .iter()
        .map(|st| {
            let m = curve.at(st * std::f32::consts::LOG10_2) / m0;
            let d: V3 = std::array::from_fn(|c| bare[c] + (grey[c] - bare[c]) * m);
            let v: V3 = std::array::from_fn(|c| 10f32.powf(-d[c]) / gain[c] - pedestal);
            [v[0], v[1], v[2], 1.0]
        })
        .collect()
}

fn scene_ramp() -> Vec<V4> {
    stops().iter().map(|st| [film::GREY * st.exp2(); 3]).map(|v| [v[0], v[1], v[2], 1.0]).collect()
}

/// The paper's curve per layer: least and most density, the log exposures at 10 % and 90 % of the range, the steepest
/// slope, where the slope first and last exceeds 0.1.
fn curve_facts(p: &Paper) -> [[f32; 6]; 3] {
    std::array::from_fn(|c| {
        let d: Vec<f32> = p.curves.iter().map(|v| v[c]).collect();
        let x = |i: f32| p.curve_low + p.curve_step * i;
        let (lo, hi) = (d[0].min(d[PAPER_CURVE_NODES - 1]), d.iter().copied().fold(f32::MIN, f32::max));
        let at = |share: f32| {
            let t = lo + share * (hi - lo);
            let i = d.iter().position(|v| *v >= t).unwrap_or(PAPER_CURVE_NODES - 1).max(1);
            x((i - 1) as f32 + (t - d[i - 1]) / (d[i] - d[i - 1]).max(1e-9))
        };
        let slope: Vec<f32> = d.windows(2).map(|w| (w[1] - w[0]) / p.curve_step).collect();
        let steep = slope.iter().copied().fold(0.0f32, f32::max);
        let foot = slope.iter().position(|v| *v > 0.1).map_or(f32::NAN, |i| x(i as f32));
        let top = slope.iter().rposition(|v| *v > 0.1).map_or(f32::NAN, |i| x(i as f32 + 1.0));
        [hi - lo, at(0.1), at(0.9), steep, foot, top]
    })
}

/// The ramp's measures: L* at the grey, its slope a stop there, the shadows' and highlights' mean slopes, L* at −3 stops,
/// the deepest and lightest L*, where the highlights clip (within 1 L* of the lightest), and the print's density gamma.
fn measures(st: &[f32], l: &[f32], y: &[f32]) -> String {
    let at = |s: f32| {
        let i = st.iter().position(|v| (*v - s).abs() < 1e-4).unwrap();
        (l[i], -y[i].max(1e-9).log10())
    };
    let slope = |a: f32, b: f32| (at(b).0 - at(a).0) / (b - a);
    let gamma = |a: f32, b: f32| (at(b).1 - at(a).1) / ((a - b) * std::f32::consts::LOG10_2);
    let lmax = l.iter().copied().fold(f32::MIN, f32::max);
    let clip = st.iter().zip(l).find(|(_, v)| **v >= lmax - 1.0).map_or(f32::NAN, |(s, _)| *s);
    let toe = st.iter().zip(l).find(|(_, v)| **v >= l[0] + 1.0).map_or(f32::NAN, |(s, _)| *s);
    format!(
        "L*0 {:.1} | slope@0 {:.2} sh(-3..-1) {:.2} hl(+1..+3) {:.2} L*/stop | L*-3 {:.1} L*-2 {:.1} L*+2 {:.1} | black {:.1} @{toe:+.2} white {lmax:.1} clip@{clip:+.2} | gamma@0 {:.2} sh {:.2} hl {:.2}",
        at(0.0).0, slope(-0.5, 0.5), slope(-3.0, -1.0), slope(1.0, 3.0), at(-3.0).0, at(-2.0).0, at(2.0).0, l[0], gamma(-0.5, 0.5), gamma(-3.0, -1.0), gamma(1.0, 3.0)
    )
}

/// Stage 4-7 on the CPU, the paper's received log exposure and density per layer where one prints.
fn traced(s: &PipelineSettings, input: V4) -> (Option<(V3, V3, V3)>, V4) {
    let k = conversion::stage47(s);
    let mut x = input;
    if let Some(d) = &k.dyes {
        x = cpu::dye_separation(x, &k, d);
    }
    if let Some(n) = &k.negative {
        x = cpu::negative(x, n);
    }
    let after = k.after_dyes();
    let paper = after.paper.map(|p| {
        let a = p.amounts(x);
        let e = p.brightened(p.exposed(a));
        (a, e, p.developed(e))
    });
    (paper, cpu::stage47(input, &k))
}

fn rest_lighting(s: &PipelineSettings) -> PipelineSettings {
    let mut s = s.clone();
    s.print = Print { exposure: Print::resting_exposure(s.mode), paper_blacks: std::env::var("BLACKS").is_ok(), ..Print::default() };
    s
}

/// The settings a synthetic input is read with: no framing, capture exposure, flat-field, patch or neighbourhood stage.
fn bare_input(s: &PipelineSettings) -> PipelineSettings {
    let mut s = s.per_pixel_only();
    s.geometry = Default::default();
    s.capture_stops = 0.0;
    s.flat_field = Default::default();
    s.corrections.clear();
    s.grain.amount = 0.0;
    s.halation.amount = 0.0;
    s
}

fn overridden(mut s: PipelineSettings) -> PipelineSettings {
    let key = |k: &str| format!("\"{k}\"");
    if let Ok(stock) = std::env::var("STOCK")
        && s.mode.is_digital()
    {
        let exposure = s.virtual_film.map_or(0.0, |f| f.exposure);
        s = s.on_stock(serde_json::from_str(&key(&stock)).expect("a stock's key"));
        if let Some(f) = s.virtual_film.as_mut() {
            f.exposure = exposure;
        }
    }
    if let Ok(dyes) = std::env::var("DYES")
        && !s.mode.is_digital()
    {
        s.dye_set = serde_json::from_str(&key(&dyes)).expect("a dye set's key");
    }
    if std::env::var("BLACKS").is_ok() {
        s.print.paper_blacks = true;
    }
    match std::env::var("PAPER").as_deref() {
        Ok("oni") => s.print_paper = None,
        Ok("neutral") => {
            s.print_paper = Some(s.fitting_paper());
            s.paperless = true;
        }
        Ok(k) => {
            s.print_paper = Some(serde_json::from_str(&key(k)).expect("a paper's key"));
            s.paperless = false;
        }
        Err(_) => {}
    }
    if std::env::var("DERIVED").is_ok()
        && let Some(f) = s.film()
    {
        let r = film::stock(f.stock).reference;
        s.gains.stops = r.stops;
        s.levels = r.levels();
    }
    s
}

struct Run<'a> {
    out: &'a Path,
    tag: String,
    log: std::fs::File,
}

impl Run<'_> {
    fn line(&mut self, text: String) {
        println!("{text}");
        writeln!(self.log, "{text}").unwrap();
    }

    /// The ramp rendered through the engine with `s`, measured, traced on the CPU, written as a CSV.
    fn ramp(&mut self, e: &mut Engine, name: &str, s: &PipelineSettings, ramp: &[V4], extra: &[V4], scene: &dyn Fn(V4) -> V4) {
        let mut values = ramp.to_vec();
        values.extend_from_slice(extra);
        let image = patches(&values);
        let decoded = Decoded { full_width: image.width as f64, linear: image, undeclared: None, seed: [0, 0] };
        let negative = e.prepare(decoded).unwrap();
        let shown = e.render(&negative, s, None).unwrap();
        let back = read_back(&shown, values.len());
        let st = stops();
        let ys: Vec<f32> = back[..st.len()].iter().map(|v| luminance(v)).collect();
        let ls: Vec<f32> = ys.iter().map(|y| lstar(*y)).collect();
        let mut csv = std::fs::File::create(self.out.join(format!("{}-{name}.csv", self.tag))).unwrap();
        writeln!(csv, "stops,L,Y,R,G,B,cpuL,paper_amount_g,logH_r,logH_g,logH_b,paper_D_r,paper_D_g,paper_D_b").unwrap();
        let mut cpu_miss = 0.0f32;
        let mut received: Vec<V3> = Vec::new();
        for (i, s_) in st.iter().enumerate() {
            let (p, out) = traced(s, scene(ramp[i]));
            let cl = lstar(luminance(&out));
            cpu_miss = cpu_miss.max((cl - ls[i]).abs());
            let (a, lh, pd) = p.unwrap_or(([f32::NAN; 3], [f32::NAN; 3], [f32::NAN; 3]));
            if p.is_some() {
                received.push(lh);
            }
            writeln!(csv, "{s_},{},{},{},{},{},{cl},{},{},{},{},{},{},{}", ls[i], ys[i], back[i][0], back[i][1], back[i][2], a[1], lh[0], lh[1], lh[2], pd[0], pd[1], pd[2]).unwrap();
        }
        let chroma = back[..st.len()].iter().map(|v| {
            let y = luminance(v).max(1e-9);
            (v[0] / y - 1.0).abs().max((v[2] / y - 1.0).abs())
        });
        let tint = chroma.zip(&st).filter(|(_, s)| (-4.0..=3.0).contains(*s)).map(|(c, _)| c).fold(0.0f32, f32::max);
        self.line(format!("{} {name}: {} | stage 4-7 alone against the render, max dL* {cpu_miss:.2} | grey ramp channel spread (-4..+3) {tint:.3}", self.tag, measures(&st, &ls, &ys)));
        let k = conversion::stage47(s);
        if let Some(p) = k.after_dyes().paper {
            let f = curve_facts(&p);
            let at = |sv: f32| received[st.iter().position(|v| (*v - sv).abs() < 1e-4).unwrap()];
            let (m4, p4, z) = (at(-4.0), at(4.0), at(0.0));
            for (c, ch) in ["R", "G", "B"].iter().enumerate() {
                self.line(format!(
                    "    paper {ch}: curve range D {:.2}, 10-90% at logH {:+.3}..{:+.3} ({:.3} wide), steepest {:.2}, slope>0.1 {:+.3}..{:+.3} | ramp lands: -4st {:+.3}, grey {:+.3}, +4st {:+.3} -> span(-4..+4) {:.3} logH (scene 2.408), paper-in gamma {:.3}",
                    f[c][0], f[c][1], f[c][2], f[c][2] - f[c][1], f[c][3], f[c][4], f[c][5], m4[c], z[c], p4[c], m4[c] - p4[c], (m4[c] - p4[c]) / 2.408
                ));
            }
        }
        // Digital: the film's own densities along the ramp, its scene's ends.
        if let Some(f) = s.film() {
            let stock = film::stock(f.stock);
            let kn = stock.kernel_at(f.exposure);
            let white = conversion::scene_white(s);
            let d = |sv: f32| {
                let g = film::GREY * sv.exp2();
                let v = [g, g, g, 1.0];
                cpu::virtual_dyes(white.as_ref().map_or(v, |m| cpu::color_matrix(v, m)), &kn, &stock.table)
            };
            let (m4, z, p4) = (d(-4.0), d(0.0), d(4.0));
            let g = |a: f32, b: f32| -> V3 { let (da, db) = (d(a), d(b)); std::array::from_fn(|c| (db[c] - da[c]) / ((b - a) * std::f32::consts::LOG10_2)) };
            self.line(format!(
                "    film {:?}: dye density -4st {:.3?} grey {:.3?} +4st {:.3?}; gamma@grey {:.3?} sh {:.3?} hl {:.3?}",
                f.stock, m4, z, p4, g(-0.5, 0.5), g(-3.0, -1.0), g(1.0, 3.0)
            ));
        }
        save_png(&self.out.join(format!("{}-{name}.png", self.tag)), &shown);
    }

    /// A file rendered with `s`, its L* spread, saved.
    fn image(&mut self, e: &mut Engine, raw: &Path, name: &str, s: &PipelineSettings) {
        let neg = e.prepare(oni_app::render::decode(raw, Reading::of(s)).unwrap()).unwrap();
        let l = e.render(&neg, s, Some(900)).unwrap();
        let mut ls: Vec<f32> = l.pixels.iter().map(|p| lstar(luminance(p))).collect();
        ls.sort_by(f32::total_cmp);
        let q = |f: f32| ls[((ls.len() - 1) as f32 * f) as usize];
        let mean = ls.iter().sum::<f32>() / ls.len() as f32;
        let sd = (ls.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / ls.len() as f32).sqrt();
        let under = ls.iter().filter(|v| **v < 10.0).count() as f32 / ls.len() as f32;
        let over = ls.iter().filter(|v| **v > 95.0).count() as f32 / ls.len() as f32;
        self.line(format!(
            "{} {name}: L* p1 {:.1} p5 {:.1} p25 {:.1} p50 {:.1} p75 {:.1} p95 {:.1} p99 {:.1} | mean {mean:.1} sd {sd:.1} | p95-p5 {:.1} | <10 {:.1}% >95 {:.1}%",
            self.tag, q(0.01), q(0.05), q(0.25), q(0.5), q(0.75), q(0.95), q(0.99), q(0.95) - q(0.05), under * 100.0, over * 100.0
        ));
        save_png(&self.out.join(format!("{}-{name}.png", self.tag)), &l);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = Path::new(&args[0]);
    std::fs::create_dir_all(out).unwrap();
    let tag = args[1].clone();
    let log = std::fs::OpenOptions::new().create(true).append(true).open(out.join(format!("{tag}.log"))).unwrap();
    let mut run = Run { out, tag, log };
    let device = Device::open().expect("a GPU");
    let engine = Engine::shared(&device);
    let mut e = engine.lock().unwrap();
    let identity = |v: V4| v;
    // A digital photo's scene on its virtual film, at the digital rest.
    let digital = overridden(bare_input(&PipelineSettings::rest(ConversionMode::Digital, true)));
    let white = conversion::scene_white(&digital);
    let f = digital.film().expect("a virtual film");
    let (stock, kernel) = (film::stock(f.stock), film::stock(f.stock).kernel_at(f.exposure));
    let to_film = move |v: V4| cpu::virtual_negative(white.as_ref().map_or(v, |m| cpu::color_matrix(v, m)), &kernel, &stock.table);
    for (name, paper) in [("digital-paper", None), ("digital-oni", Some("oni")), ("digital-neutral", Some("neutral"))] {
        let mut s = digital.clone();
        match paper {
            Some("oni") => s.print_paper = None,
            Some(_) => s.paperless = true,
            None => {}
        }
        if std::env::var("PAPER").is_ok() && paper.is_some() {
            continue;
        }
        run.ramp(&mut e, name, &s, &scene_ramp(), &chart(), &to_film);
    }
    let slide_curve = std::env::var("SLIDE_CURVE").ok().map(|p| Curve::read(&p));
    for file in &args[2..] {
        let raw = Path::new(file);
        let stem = raw.file_stem().unwrap().to_string_lossy().to_string();
        let edited = match sidecar::read(raw, true) {
            Ok(l) => overridden(l.settings),
            Err(err) => {
                run.line(format!("{} {stem}: unread ({err:?})", run.tag));
                continue;
            }
        };
        let at_rest = rest_lighting(&edited);
        if edited.mode.is_digital() {
            run.image(&mut e, raw, &format!("{stem}-edited"), &edited);
            let rest = overridden(PipelineSettings { geometry: edited.geometry, ..PipelineSettings::rest(ConversionMode::Digital, edited.decoded_from_mosaic) });
            run.image(&mut e, raw, &format!("{stem}-rest-paper"), &rest);
            if std::env::var("PAPER").is_err() {
                run.image(&mut e, raw, &format!("{stem}-rest-oni"), &PipelineSettings { print_paper: None, ..rest.clone() });
            }
            continue;
        }
        let stock = edited.dye_set.profile().unwrap_or(oni_settings::VirtualStock::default().profile());
        let scan_curve = Curve::of_stock(stock);
        let curve = match (edited.mode.reads_positive(), &slide_curve) {
            (false, _) => &scan_curve,
            (true, Some(c)) => c,
            (true, None) => continue,
        };
        let variants: Vec<(&str, PipelineSettings)> = if std::env::var("PAPER").is_ok() {
            vec![("as-set", at_rest.clone())]
        } else {
            vec![
                ("oni", PipelineSettings { print_paper: None, ..at_rest.clone() }),
                ("paper", at_rest.clone().on_paper()),
                ("neutral", PipelineSettings { paperless: true, ..at_rest.clone().on_paper() }),
            ]
        };
        for (v, s) in &variants {
            let input = bare_input(s);
            run.ramp(&mut e, &format!("{stem}-ramp-{v}"), &input, &scan_ramp(&input, curve), &[], &identity);
            run.image(&mut e, raw, &format!("{stem}-{v}"), s);
        }
        run.image(&mut e, raw, &format!("{stem}-edited"), &edited);
        if std::env::var("PAPER").is_err() {
            run.image(&mut e, raw, &format!("{stem}-edited-paper"), &edited.clone().on_paper());
        }
    }
}
