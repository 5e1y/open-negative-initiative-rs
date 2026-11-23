//! `coupler_fit <out> <stock>=<reference>... -- <scene.f32>...`: each stock's six cross-layer coupler terms fitted so its
//! digital render's colour crosstalk (a colour against the same channels each sent alone through the grey response)
//! matches its reference's, on a set of chart colours; then each scene rendered on the common and the fitted matrix.

use oni_pipeline::film::{self, bake::COUPLERS, profile::Profile};
use oni_pipeline::conversion;
use oni_render::{ColorMatrix, Stage47, cpu};
use oni_settings::{ConversionMode, PipelineSettings, VirtualStock};
use std::io::Write;
use std::path::Path;

type V3 = [f64; 3];
type M3 = [[f64; 3]; 3];

const SRGB_TO_XYZ: M3 = [[0.4124564, 0.3575761, 0.1804375], [0.2126729, 0.7151522, 0.0721750], [0.0193339, 0.1191920, 0.9503041]];
const M2020: M3 = [[0.6369580, 0.1446169, 0.1688810], [0.2627002, 0.6779981, 0.0593017], [0.0, 0.0280727, 1.0609851]];

fn mul(m: &M3, v: V3) -> V3 {
    std::array::from_fn(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}
fn matmul(a: &M3, b: &M3) -> M3 {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}
fn inv(m: &M3) -> M3 {
    let d = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    let c = |a: usize, b: usize, e: usize, f: usize| m[a][b] * m[e][f] - m[a][f] * m[e][b];
    [
        [c(1, 1, 2, 2) / d, -c(0, 1, 2, 2) / d, c(0, 1, 1, 2) / d],
        [-c(1, 0, 2, 2) / d, c(0, 0, 2, 2) / d, -c(0, 0, 1, 2) / d],
        [c(1, 0, 2, 1) / d, -c(0, 0, 2, 1) / d, c(0, 0, 1, 1) / d],
    ]
}
fn dec(v: f64) -> f64 {
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}
fn enc(v: f64) -> f64 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.0031308 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
}
/// sRGB code to Oklab.
fn oklab(code: V3) -> V3 {
    let [r, g, b] = code.map(dec);
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    [0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s, 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s, 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s]
}
/// Oklch to linear sRGB.
fn oklch(l: f64, c: f64, h: f64) -> V3 {
    let (a, b) = (c * h.cos(), c * h.sin());
    let l_ = (l + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m_ = (l - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s_ = (l - 0.0894841775 * a - 1.2914855480 * b).powi(3);
    [4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_, -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_, -0.0041960863 * l_ - 0.7034186147 * m_ + 1.7076147010 * s_]
}

struct Cube {
    n: usize,
    nodes: Vec<V3>,
}
impl Cube {
    fn read(path: &str) -> Cube {
        let (_, look) = oni_settings::stocks::cube::read(&std::fs::read(path).unwrap()).unwrap();
        Cube { n: look.size, nodes: look.nodes.iter().map(|p| [p[0] as f64, p[1] as f64, p[2] as f64]).collect() }
    }
    fn at(&self, c: V3) -> V3 {
        let n = self.n;
        let p = c.map(|v| v.clamp(0.0, 1.0) * (n - 1) as f64);
        let i = p.map(|v| (v.floor() as usize).min(n - 2));
        let f: V3 = std::array::from_fn(|k| p[k] - i[k] as f64);
        let mut out = [0.0; 3];
        for (db, dg, dr) in (0..8).map(|q| (q >> 2 & 1, q >> 1 & 1, q & 1)) {
            let w = (if db == 1 { f[2] } else { 1.0 - f[2] }) * (if dg == 1 { f[1] } else { 1.0 - f[1] }) * (if dr == 1 { f[0] } else { 1.0 - f[0] });
            let node = self.nodes[(i[0] + dr) + n * (i[1] + dg) + n * n * (i[2] + db)];
            (0..3).for_each(|k| out[k] += w * node[k]);
        }
        out
    }
}

/// A digital photo's render on `stock` baked through `couplers`, at rest on the default paper, as a CPU chain; a slide
/// projected.
struct Render {
    stock: film::Stock,
    projected: Option<&'static film::projection::Projected>,
    white: ColorMatrix,
    k47: Stage47,
    to_2020: M3,
    from_2020: M3,
}
impl Render {
    fn new(key: &'static str, couplers: M3) -> Render {
        let v = VirtualStock::of(key);
        let stock = film::bake_through(&v.entry(), couplers).unwrap();
        let mut s = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(v);
        s.gains.stops = stock.reference.stops;
        s.levels = stock.reference.levels();
        let to_2020 = matmul(&inv(&M2020), &SRGB_TO_XYZ);
        let projected = v.entry().reversal.then(|| film::projection::projected(v));
        Render { white: conversion::scene_white(&s).unwrap(), k47: conversion::stage47(&s), to_2020, from_2020: inv(&to_2020), stock, projected }
    }
    /// Scene light in linear Rec. 2020 to the print's display code.
    fn scene(&self, p: V3) -> V3 {
        let lit = cpu::color_matrix([p[0] as f32, p[1] as f32, p[2] as f32, 1.0], &self.white);
        let neg = cpu::virtual_negative(lit, &self.stock.kernel_at(0.0), &self.stock.table);
        let out = match self.projected {
            Some(p) => cpu::projection(neg, &p.projection, &p.table),
            None => cpu::stage47(neg, &self.k47),
        };
        mul(&self.from_2020, [out[0] as f64, out[1] as f64, out[2] as f64]).map(enc)
    }
    /// A display code read as the scene it encodes.
    fn code(&self, c: V3) -> V3 {
        self.scene(mul(&self.to_2020, c.map(dec)))
    }
}

/// The crosstalk of `f` on `c`: its colour against each channel sent alone through its grey response, in Oklab.
fn crosstalk(f: &dyn Fn(V3) -> V3, c: V3) -> V3 {
    let alone: V3 = std::array::from_fn(|k| f([c[k]; 3])[k]);
    let (a, b) = (oklab(f(c)), oklab(alone));
    std::array::from_fn(|k| a[k] - b[k])
}

/// The chart's colours: the ColorChecker's chromatic patches and Oklch rings held in sRGB, as display codes.
fn colours() -> Vec<V3> {
    let cc: [V3; 18] = [
        [37.54, 14.37, 14.92], [64.66, 19.27, 17.5], [49.32, -3.82, -22.54], [43.46, -12.74, 22.72], [54.94, 9.61, -24.79], [70.48, -32.26, -0.37],
        [62.73, 35.83, 56.5], [39.43, 10.75, -45.17], [50.57, 48.64, 16.67], [30.1, 22.54, -20.87], [71.77, -24.13, 58.19], [71.51, 18.24, 67.37],
        [28.37, 15.42, -49.8], [54.38, -39.72, 32.27], [42.43, 51.05, 28.62], [81.8, 2.67, 80.41], [50.63, 51.28, -14.12], [49.57, -29.71, -28.32],
    ];
    let brad: M3 = [[0.9555766, -0.0230393, 0.0631636], [-0.0282895, 1.0099416, 0.0210077], [0.0122982, -0.0204830, 1.3299098]];
    let to_srgb = inv(&SRGB_TO_XYZ);
    let mut out: Vec<V3> = cc
        .iter()
        .map(|&[l, a, b]| {
            let fy = (l + 16.0) / 116.0;
            let f = |t: f64| if t.powi(3) > 0.008856 { t.powi(3) } else { (t - 16.0 / 116.0) / 7.787 };
            let xyz = [0.96422 * f(fy + a / 500.0), f(fy), 0.82521 * f(fy - b / 200.0)];
            mul(&to_srgb, mul(&brad, xyz)).map(enc)
        })
        .collect();
    for l in [0.5, 0.65, 0.8] {
        for c in [0.05, 0.1] {
            for h in 0..12 {
                let lin = oklch(l, c, h as f64 * std::f64::consts::TAU / 12.0);
                if lin.iter().all(|v| (0.0..=1.0).contains(v)) {
                    out.push(lin.map(enc));
                }
            }
        }
    }
    out
}

/// Whether `couplers` fold one of `key`'s layers back, which the stock files refuse.
fn folds(key: &'static str, couplers: &M3) -> bool {
    let entry = VirtualStock::of(key).entry();
    let p = Profile::read(entry.profile.as_bytes()).unwrap();
    let develop = if entry.reversal { film::bake::curves_falling } else { film::bake::curves_through };
    let (first, _) = develop(&p.log_exposure, &p.density_curves, couplers);
    film::bake::folds(&p.log_exposure, &first, couplers)
}

fn loss(render: &Render, targets: &[(V3, V3)]) -> f64 {
    let f = |c: V3| render.code(c);
    targets.iter().map(|(c, t)| crosstalk(&f, *c).iter().zip(t).map(|(a, b)| (a - b).powi(2)).sum::<f64>()).sum::<f64>() / targets.len() as f64
}

/// The common matrix's diagonal and `off` across it; a reversal film's diagonal left out, its curves steep enough that a
/// layer's own inhibition folds their axis back.
fn with(off: &[f64; 6], reversal: bool) -> M3 {
    let mut m = COUPLERS;
    if reversal {
        (0..3).for_each(|c| m[c][c] = 0.0);
    }
    let places = [(0, 1), (0, 2), (1, 0), (1, 2), (2, 0), (2, 1)];
    for (k, &(i, j)) in places.iter().enumerate() {
        m[i][j] = off[k];
    }
    m
}

/// Nelder-Mead on `f` from `x0`, the terms held to 0..0.9.
fn minimise(f: &dyn Fn(&[f64; 6]) -> f64, x0: [f64; 6], evals: usize) -> ([f64; 6], f64) {
    let held = |x: [f64; 6]| x.map(|v| v.clamp(0.0, 0.9));
    let mut simplex: Vec<([f64; 6], f64)> = (0..7)
        .map(|i| {
            let mut x = x0;
            if i > 0 {
                x[i - 1] += 0.1;
            }
            let x = held(x);
            (x, f(&x))
        })
        .collect();
    let mut n = 7;
    while n < evals {
        simplex.sort_by(|a, b| a.1.total_cmp(&b.1));
        let centre: [f64; 6] = std::array::from_fn(|k| simplex[..6].iter().map(|s| s.0[k]).sum::<f64>() / 6.0);
        let at = |t: f64| held(std::array::from_fn(|k| centre[k] + t * (simplex[6].0[k] - centre[k])));
        let r = at(-1.0);
        let fr = f(&r);
        n += 1;
        if fr < simplex[0].1 {
            let e = at(-2.0);
            let fe = f(&e);
            n += 1;
            simplex[6] = if fe < fr { (e, fe) } else { (r, fr) };
        } else if fr < simplex[5].1 {
            simplex[6] = (r, fr);
        } else {
            let c = at(0.5);
            let fc = f(&c);
            n += 1;
            if fc < simplex[6].1 {
                simplex[6] = (c, fc);
            } else {
                let best = simplex[0].0;
                for s in simplex.iter_mut().skip(1) {
                    s.0 = held(std::array::from_fn(|k| best[k] + 0.5 * (s.0[k] - best[k])));
                    s.1 = f(&s.0);
                    n += 1;
                }
            }
        }
    }
    simplex.sort_by(|a, b| a.1.total_cmp(&b.1));
    simplex[0]
}

/// Each layer's gamma at mid density on the data sheet's curves, then Kodak's interimage ratio the matrix implies.
fn ratios(key: &'static str, m: &M3) -> (V3, V3) {
    let p = Profile::read(VirtualStock::of(key).entry().profile.as_bytes()).unwrap();
    let gamma: V3 = std::array::from_fn(|c| {
        let d: Vec<f64> = p.density_curves.iter().map(|v| v[c]).collect();
        let (lo, hi) = (d.iter().cloned().fold(f64::INFINITY, f64::min), d.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
        let at = |share: f64| p.log_exposure[d.iter().position(|&v| v >= lo + share * (hi - lo)).unwrap()];
        let (a, b) = (at(0.3), at(0.6));
        0.3 * (hi - lo) / (b - a)
    });
    let ratio = std::array::from_fn(|r| 1.0 / (1.0 - (0..3).filter(|&k| k != r).map(|k| m[k][r] * gamma[k]).sum::<f64>()));
    (gamma, ratio)
}

fn read_scene(path: &str) -> (usize, usize, Vec<V3>) {
    let bytes = std::fs::read(path).unwrap();
    let nl = bytes.iter().position(|&b| b == b'\n').unwrap();
    let head = std::str::from_utf8(&bytes[..nl]).unwrap();
    let mut it = head.split_whitespace().map(|v| v.parse::<usize>().unwrap());
    let (w, h) = (it.next().unwrap(), it.next().unwrap());
    let px = bytes[nl + 1..].chunks_exact(12).map(|c| std::array::from_fn(|k| f32::from_le_bytes(c[4 * k..4 * k + 4].try_into().unwrap()) as f64)).collect();
    (w, h, px)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = Path::new(&args[0]);
    std::fs::create_dir_all(out).unwrap();
    let split = args.iter().position(|a| a == "--").unwrap_or(args.len());
    let scenes: Vec<&String> = args.get(split + 1..).unwrap_or(&[]).iter().collect();
    let colours = colours();
    println!("{} chart colours", colours.len());
    let mut report = String::new();
    for pair in &args[1..split] {
        let (key, path) = pair.split_once('=').unwrap();
        let key: &'static str = Box::leak(key.to_string().into_boxed_str());
        let cube = Cube::read(path);
        let reversal = VirtualStock::of(key).entry().reversal;
        let targets: Vec<(V3, V3)> = colours.iter().map(|&c| (c, crosstalk(&|x| cube.at(x), c))).collect();
        let started = std::time::Instant::now();
        let common = loss(&Render::new(key, COUPLERS), &targets);
        let bare = loss(&Render::new(key, with(&[0.0; 6], reversal)), &targets);
        println!("{key}: one render {:.2} s", started.elapsed().as_secs_f64() / 2.0);
        let x0 = [COUPLERS[0][1], COUPLERS[0][2], COUPLERS[1][0], COUPLERS[1][2], COUPLERS[2][0], COUPLERS[2][1]];
        // From the common matrix and from no cross term, the better kept: one start can stall where the other does not.
        let fit = |start| minimise(&|x| if folds(key, &with(x, reversal)) { f64::INFINITY } else { loss(&Render::new(key, with(x, reversal)), &targets) }, start, 220);
        let ((a, fa), (b, fb)) = (fit(x0), fit([0.0; 6]));
        let (best, fitted) = if fa <= fb { (a, fa) } else { (b, fb) };
        let m = with(&best, reversal);
        // Kodak's ratio reads a negative's rising curves; a reversal film's fall, and none is told.
        let none = ([f64::NAN; 3], [f64::NAN; 3]);
        let (gamma, ratio) = if reversal { none } else { ratios(key, &m) };
        let (_, common_ratio) = if reversal { none } else { ratios(key, &COUPLERS) };
        let rounded = |v: f64| (v * 1000.0).round() / 1000.0;
        let line = format!(
            "{key}\n  crosstalk error (Oklab², mean): common {common:.6}, no cross term {bare:.6}, fitted {fitted:.6}\n  fitted {:?}\n  \
             layer gammas {:?}; interimage ratios R/G/B common {:?}, fitted {:?}\n",
            m.map(|r| r.map(rounded)),
            gamma.map(|v| (v * 100.0).round() / 100.0),
            common_ratio.map(|v| (v * 100.0).round() / 100.0),
            ratio.map(|v| (v * 100.0).round() / 100.0)
        );
        print!("{line}");
        report += &line;
        for scene in &scenes {
            let (w, h, px) = read_scene(scene);
            let name = Path::new(scene).file_stem().unwrap().to_string_lossy().replace("-scene", "");
            for (tag, couplers) in [("common", COUPLERS), ("fitted", m)] {
                let r = Render::new(key, couplers);
                let mut f = std::io::BufWriter::new(std::fs::File::create(out.join(format!("{name}-{key}-{tag}.rgb"))).unwrap());
                writeln!(f, "{w} {h}").unwrap();
                for p in &px {
                    f.write_all(&r.scene(*p).map(|v| (v * 255.0).round() as u8)).unwrap();
                }
            }
        }
    }
    std::fs::write(out.join("report.txt"), report).unwrap();
}
