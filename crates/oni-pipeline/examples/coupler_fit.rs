//! `coupler_fit <out> <stock>=<reference>... -- <scene.f32>...`: each stock's six cross-layer coupler terms fitted so its
//! digital render matches its reference on a chart, the ColorChecker, Oklch rings and a grey ramp: a negative printed by the
//! virtual enlarger on no paper (the paper is laid over a render, never part of it), a slide projected, each patch's
//! Oklab distance to the reference's, the render's grey matched to the reference's, the ramp weighted. Then each scene rendered on the common and the
//! fitted matrix.

mod chart;
#[path = "chart/cube.rs"]
mod cube;

use chart::{M3, RAMP, Render, V3, oklab};
use cube::{Cube, colours, judged, printed};
use oni_pipeline::film::{self, bake::COUPLERS, profile::Profile};
use oni_settings::VirtualStock;
use std::io::Write;
use std::path::Path;

/// The ramp's weight against one colour patch: a tint on the neutral axis shows on every photo.
const NEUTRAL_WEIGHT: f64 = 2.0;

fn weight(i: usize, chart: usize) -> f64 {
    if i + RAMP.len() >= chart { NEUTRAL_WEIGHT } else { 1.0 }
}

/// The mean weighted squared Oklab distance of the printed chart to the reference's.
fn loss(printed: &[V3], targets: &[V3]) -> f64 {
    let n = targets.len();
    let total: f64 = (0..n).map(|i| weight(i, n) * (0..3).map(|k| (printed[i][k] - targets[i][k]).powi(2)).sum::<f64>()).sum();
    total / (0..n).map(|i| weight(i, n)).sum::<f64>()
}

/// Whether `couplers` fold one of `key`'s layers back, which the stock files refuse.
fn folds(key: &'static str, couplers: &M3) -> bool {
    let entry = VirtualStock::of(key).entry();
    let p = Profile::read(entry.profile.as_bytes()).unwrap();
    let develop = if entry.reversal { film::bake::curves_falling } else { film::bake::curves_through };
    let (first, _) = develop(&p.log_exposure, &p.density_curves, couplers);
    film::bake::folds(&p.log_exposure, &first, couplers)
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
        let targets: Vec<V3> = colours.iter().map(|&c| oklab(cube.at(c))).collect();
        let of = |m: M3| printed(&Render::new(key, m), &colours, &cube);
        let started = std::time::Instant::now();
        let (common, bare) = (of(COUPLERS), of(with(&[0.0; 6], reversal)));
        println!("{key}: one render {:.2} s", started.elapsed().as_secs_f64() / 2.0);
        let x0 = [COUPLERS[0][1], COUPLERS[0][2], COUPLERS[1][0], COUPLERS[1][2], COUPLERS[2][0], COUPLERS[2][1]];
        // From the common matrix and from no cross term, the better kept: one start can stall where the other does not.
        let fit = |start| minimise(&|x| if folds(key, &with(x, reversal)) { f64::INFINITY } else { loss(&of(with(x, reversal)), &targets) }, start, 260);
        let ((a, fa), (b, fb)) = (fit(x0), fit([0.0; 6]));
        let best = if fa <= fb { a } else { b };
        let m = with(&best, reversal);
        let fitted = of(m);
        // Kodak's ratio reads a negative's rising curves; a reversal film's fall, and none is told.
        let none = ([f64::NAN; 3], [f64::NAN; 3]);
        let (gamma, ratio) = if reversal { none } else { ratios(key, &m) };
        let (_, common_ratio) = if reversal { none } else { ratios(key, &COUPLERS) };
        let rounded = |v: f64| (v * 1000.0).round() / 1000.0;
        let told = |p: &[V3]| {
            let [de, dl, chroma, ramp, cube_ramp] = judged(p, &targets);
            format!("loss {:.6}, mean dE00 {de:.2} (|dL*| {dl:.2}), ColorChecker chroma {chroma:.3} of the reference's, ramp C* {ramp:.2} (reference {cube_ramp:.2})", loss(p, &targets))
        };
        let line = format!(
            "{key}\n  common: {}\n  no cross term: {}\n  fitted: {}\n  fitted {:?}\n  layer gammas {:?}; interimage ratios R/G/B common {:?}, fitted {:?}\n",
            told(&common),
            told(&bare),
            told(&fitted),
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
