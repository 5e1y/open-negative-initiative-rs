//! What every kernel test shares: the judge with its twin, and roll #8, the reference roll, decoded
//! once per test binary and cropped to its centre at full resolution.

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::OnceLock;

use oni_render::{cpu, gpu::Gpu, ColorCube, ExposureAdjust, Image, Stage47, SCANNER_LIFT};
use serde_json::Value;

/// Largest error accepted, as |gpu - cpu| / (1 + |cpu|); the spread measured on lavapipe sits under it.
pub const TOLERANCE: f32 = 1e-4;

/// The Swift app, read for its measured scanner table, or `None` after a skip line.
pub fn app_dir(check: &str) -> Option<PathBuf> {
    oni_fixtures::present(("ONI_APP", "/home/claude/open-negative-initiative"), check)
}

/// The frame every test reads: a 0.3 Contrast on the scanner face exercises its last branch.
pub const FRAME: &str = "DSC_6448.NEF";
pub const CROP: (usize, usize) = (1024, 768);

pub struct Roll {
    /// False on the stand-in negative CI runs on, where no RAW is staged.
    pub real: bool,
    /// The whole frame, scene-linear, as the decoder hands it.
    pub frame: Image,
    /// Its centre crop.
    pub negative: Image,
    /// The negative brought back by `captureStops` and converted at the sidecar's gains, windows and
    /// print exposure, other Lighting at rest: near the app's render, not equal to it.
    pub positive: Image,
    pub settings: Value,
    pub stage47: Stage47,
}

/// The adapter, or `None` after a skip line on a machine that has none.
pub fn gpu(check: &str) -> Option<&'static Gpu> {
    static GPU: OnceLock<Option<Gpu>> = OnceLock::new();
    let found = GPU.get_or_init(|| match Gpu::new() {
        Ok(g) => {
            println!("adapter: {}", g.adapter);
            Some(g)
        }
        Err(e) => {
            println!("no GPU adapter: {e:?}");
            None
        }
    });
    if found.is_none() {
        println!("SKIPPED {check}: no GPU adapter");
    }
    found.as_ref()
}

pub fn num(v: &Value, path: &str) -> f32 {
    path.split('.')
        .fold(v, |v, k| match k.parse::<usize>() {
            Ok(i) => &v[i],
            Err(_) => &v[k],
        })
        .as_f64()
        .unwrap_or_else(|| panic!("{path} missing from the sidecar")) as f32
}

/// Roll #8's frame, or without the roll a stand-in negative of the same crop under the same
/// frame's settings, so every kernel is still judged where no RAW is staged.
pub fn roll() -> &'static Roll {
    static ROLL: OnceLock<Roll> = OnceLock::new();
    ROLL.get_or_init(|| {
        let Some(dir) = oni_fixtures::raw_dir("roll #8 inputs, judged on a stand-in negative instead") else {
            return stand_in();
        };
        let path = dir.join(FRAME);
        let decoded = oni_decode::decode(&path, 1.0).unwrap_or_else(|e| panic!("{} does not decode: {e:?}", path.display()));
        let sidecar: Value = serde_json::from_slice(&std::fs::read(path.with_extension("NEF.json")).expect("sidecar"))
            .expect("sidecar JSON");
        let (w, h) = CROP;
        let (x0, y0) = ((decoded.width - w) / 2, (decoded.height - h) / 2);
        let pixels = (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).map(|(x, y)| decoded.pixels[(y0 + y) * decoded.width + x0 + x]).collect();
        let frame = Image::new(decoded.width, decoded.height, decoded.pixels);
        held(true, frame, Image::new(w, h, pixels), sidecar["settings"].clone())
    })
}

fn held(real: bool, frame: Image, negative: Image, settings: Value) -> Roll {
    let stage47 = sidecar_stage47(&settings);
    let lifted = ExposureAdjust { ev: -num(&settings, "captureStops") };
    let positive = each(&negative, |p| cpu::stage47(cpu::exposure_adjust(p, &lifted), &stage47));
    Roll { real, frame, negative, positive, settings, stage47 }
}

/// A hash of a lattice point and a channel, in 0..1.
fn lattice(x: usize, y: usize, c: usize) -> f32 {
    let mut v = (x as u32).wrapping_mul(0x9e37_79b1) ^ (y as u32).wrapping_mul(0x85eb_ca77) ^ (c as u32).wrapping_mul(0xc2b2_ae3d);
    v ^= v >> 15;
    v = v.wrapping_mul(0x2c1b_3c6d);
    v ^= v >> 12;
    v as f32 / u32::MAX as f32
}

/// Value noise on a lattice of `cell` pixels, bilinear between its points: grain a few pixels wide.
fn grain(x: usize, y: usize, c: usize, cell: usize) -> f32 {
    let (fx, fy) = ((x % cell) as f32 / cell as f32, (y % cell) as f32 / cell as f32);
    let (ix, iy) = (x / cell, y / cell);
    let top = lattice(ix, iy, c) * (1.0 - fx) + lattice(ix + 1, iy, c) * fx;
    let bottom = lattice(ix, iy + 1, c) * (1.0 - fx) + lattice(ix + 1, iy + 1, c) * fx;
    top * (1.0 - fy) + bottom * fy
}

/// Transmittances a negative scan holds, densest highlight to clear base, with a grain of its own
/// in each channel: a smooth or periodic field hides the denoise twin and repeats a planted patch.
fn stand_in() -> Roll {
    let (w, h) = CROP;
    let pixels = (0..w * h)
        .map(|i| {
            let (px, py) = (i % w, i / w);
            let (x, y) = (px as f32 / w as f32, py as f32 / h as f32);
            let scene = 0.5 + 0.25 * (7.3 * x + 2.9 * y * y).sin() + 0.2 * (4.1 * y - 3.7 * x * x).cos();
            let base = 10f32.powf(-2.6 * (scene + 0.3 * grain(px, py, 3, 9) - 0.15).clamp(0.0, 1.0));
            let dye = |c: usize, tint: f32| base * tint * (0.85 + 0.3 * grain(px, py, c, 3));
            [dye(0, 0.6 + 0.3 * x), dye(1, 0.5 + 0.2 * y), dye(2, 0.35 + 0.1 * (x + y)), 1.0]
        })
        .collect();
    let negative = Image::new(w, h, pixels);
    let settings: Value = serde_json::from_str(include_str!("../data/DSC_6448.settings.json")).expect("settings JSON");
    held(false, negative.clone(), negative, settings)
}

/// Stages 4 to 7 at a sidecar's gains, windows and print exposure, other Lighting at rest: near the
/// app's render, not equal to it.
pub fn sidecar_stage47(settings: &Value) -> Stage47 {
    let pedestal = 10f32.powf(-num(settings, "densityCeiling"));
    // `Levels.gamma`: the power that carries the window's mid onto one half.
    let level = |c: &str| {
        let lv = |k: &str| num(settings, &format!("levels.{c}.{k}"));
        [lv("black"), lv("white"), 0.5f32.ln() / lv("mid").ln()]
    };
    let stops = num(settings, "print.exposure");
    Stage47 {
        gain: std::array::from_fn(|c| num(settings, &format!("gains.stops.{c}")).exp2()),
        red: level("red"),
        green: level("green"),
        blue: level("blue"),
        pedestal,
        log_guard: pedestal / 8.0,
        print_on: 1.0,
        print_stops: stops,
        base_stops: stops,
        ..Stage47::rest()
    }
}

/// The adversarial ring: negatives, zeros, values past 1 and a black, on every channel.
pub fn adversarial(n: usize) -> Vec<[f32; 4]> {
    let mut state = 0x9e37_79b9_u32;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state as f32 / u32::MAX as f32
    };
    let edges = [-0.25, 0.0, 1e-7, 1.0, 1.5, 3.0];
    (0..n)
        .map(|i| {
            let mut p = [0.0, 0.0, 0.0, 1.0];
            for c in &mut p[..3] {
                *c = if i % 5 == 0 { edges[(next() * edges.len() as f32) as usize % edges.len()] } else { 1.4 * next() - 0.1 };
            }
            p
        })
        .collect()
}

/// Worst error and where, infinities matching only themselves.
pub fn error(gpu: &[[f32; 4]], cpu: &[[f32; 4]]) -> (f32, usize) {
    assert_eq!(gpu.len(), cpu.len(), "lengths");
    let mut worst = (0.0f32, 0);
    for (i, (g, c)) in gpu.iter().zip(cpu).enumerate() {
        for ch in 0..4 {
            let (a, b) = (g[ch], c[ch]);
            let e = if a.is_nan() || b.is_nan() || a.is_infinite() || b.is_infinite() {
                if a == b || (a.is_nan() && b.is_nan()) { 0.0 } else { f32::INFINITY }
            } else {
                (a - b).abs() / (1.0 + b.abs())
            };
            if e > worst.0 {
                worst = (e, i);
            }
        }
    }
    worst
}

/// Collects verdicts: the kernel within tolerance of its reference, and the twin, a reference
/// deliberately wrong, outside it.
#[derive(Default)]
pub struct Judge {
    failures: Vec<String>,
}

impl Judge {
    pub fn check(&mut self, name: &str, gpu: &[[f32; 4]], reference: &[[f32; 4]], twin: &[[f32; 4]], tolerance: f32) {
        let (err, at) = error(gpu, reference);
        let (twin_err, _) = error(gpu, twin);
        println!("{name:<44} max {err:.3e} at {at:>7}   twin {twin_err:.3e}");
        if err > tolerance {
            self.failures.push(format!("{name}: {err:e} > {tolerance:e} at {at}: gpu {:?}, cpu {:?}", gpu[at], reference[at]));
        }
        if twin_err <= tolerance {
            self.failures.push(format!("{name}: twin {twin_err:e} within tolerance, the check cannot see it"));
        }
    }

    pub fn image(&mut self, name: &str, gpu: &Image, reference: &Image, twin: &Image) {
        assert_eq!((gpu.width, gpu.height), (reference.width, reference.height), "{name}: shape");
        self.check(name, &gpu.pixels, &reference.pixels, &twin.pixels, TOLERANCE);
    }

    pub fn fail(&mut self, message: String) {
        self.failures.push(message);
    }

    pub fn done(self) {
        assert!(self.failures.is_empty(), "{}", self.failures.join("\n"));
    }
}

/// `f` over every pixel, for per-pixel references.
pub fn each(image: &Image, f: impl Fn([f32; 4]) -> [f32; 4]) -> Image {
    Image { pixels: image.pixels.iter().map(|p| f(*p)).collect(), ..image.clone() }
}

/// `f` over pixel pairs of two images of one shape.
pub fn pair(a: &Image, b: &Image, f: impl Fn([f32; 4], [f32; 4]) -> [f32; 4]) -> Image {
    Image { pixels: a.pixels.iter().zip(&b.pixels).map(|(x, y)| f(*x, *y)).collect(), ..a.clone() }
}

/// The adversarial ring as an image of `width` by `height`.
pub fn adversarial_image(width: usize, height: usize) -> Image {
    Image::new(width, height, adversarial(width * height))
}

/// `f` over every pixel on every core: the whole frame's reference would take a minute on one.
pub fn each_parallel(image: &Image, f: impl Fn([f32; 4]) -> [f32; 4] + Sync) -> Image {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = image.pixels.len().div_ceil(threads).max(1);
    let mut out = vec![[0f32; 4]; image.pixels.len()];
    std::thread::scope(|scope| {
        for (src, dst) in image.pixels.chunks(chunk).zip(out.chunks_mut(chunk)) {
            let f = &f;
            scope.spawn(move || dst.iter_mut().zip(src).for_each(|(d, s)| *d = f(*s)));
        }
    });
    image.with_pixels(out)
}

/// The measured Frontier as the Swift app ships it: its table, its looks and its click offsets.
pub struct Frontier {
    pub json: Value,
    pub render: ColorCube,
    pub warmth_plus: ColorCube,
    pub warmth_minus: ColorCube,
}

/// The Frontier, or `None` after a skip line where the Swift app is not checked out.
pub fn frontier(check: &str) -> Option<Frontier> {
    let dir = app_dir(check)?.join("Resources/Scanner");
    let json: Value = serde_json::from_slice(&std::fs::read(dir.join("frontier.json")).expect("frontier.json")).unwrap();
    let cube = |name: &str, nodes: usize| ColorCube::from_bytes(nodes, &std::fs::read(dir.join(name)).expect(name)).expect("cube size");
    let render = cube("frontier.cube", json["render"]["nodes"].as_u64().unwrap() as usize);
    // `ScannerRender.lifted`: the looks' moves raised by the lift, which the output kernel takes back.
    let lifted = |name: &str| {
        let look = &json["looks"][name];
        let mut cube = cube(look["cube"].as_str().unwrap(), look["nodes"].as_u64().unwrap() as usize);
        cube.cube_data.iter_mut().for_each(|v| v[..3].iter_mut().for_each(|c| *c += SCANNER_LIFT));
        cube
    };
    let (warmth_plus, warmth_minus) = (lifted("warmthPlus"), lifted("warmthMinus"));
    Some(Frontier { json, render, warmth_plus, warmth_minus })
}

impl Frontier {
    pub fn v3(v: &Value) -> [f32; 3] {
        std::array::from_fn(|c| v[c].as_f64().unwrap() as f32)
    }

    /// `Machine.offset`: a count is half the machine's measured click, an odd one halfway between two.
    pub fn offset(&self, clicks: [(&str, i64); 4]) -> [f32; 3] {
        let mut sum = [0f32; 3];
        for (key, count) in clicks {
            let measured = |n: i64| Self::v3(&self.json["clicks"][key][n.to_string()]);
            let (a, b) = (measured((count as f64 / 2.0).floor() as i64), measured((count as f64 / 2.0).ceil() as i64));
            for c in 0..3 {
                sum[c] += (a[c] + b[c]) / 2.0;
            }
        }
        sum
    }
}

