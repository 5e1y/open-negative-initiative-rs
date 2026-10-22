//! What the pipeline tests share: a noise source, the judges of a stage against its reference, and
//! the Swift app's render dumps with the gap between two renders.

#![allow(dead_code)]

use oni_graph::{Graph, Image};
use std::path::Path;

pub type V4 = [f32; 4];

/// Largest error accepted, as |gpu - cpu| / (1 + |cpu|): lavapipe's `exp2` and `log2` sit near 1e-6.
pub const TOLERANCE: f32 = 1e-4;

pub fn graph(check: &str) -> Option<Graph> {
    Graph::new().map_err(|e| println!("SKIPPED {check}: {e}")).ok()
}

/// Deterministic noise in `low..high`, alpha one.
pub fn noise(width: u32, height: u32, seed: u64, low: f32, high: f32) -> Vec<V4> {
    let mut state = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
    let mut next = move || {
        state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        low + ((state >> 40) as f32 / (1u64 << 24) as f32) * (high - low)
    };
    (0..width * height).map(|_| [next(), next(), next(), 1.0]).collect()
}

/// Largest relative error; a NaN on either side is infinite, so a NaN pixel never scores 0.
pub fn error(a: &[V4], b: &[V4]) -> f32 {
    assert_eq!(a.len(), b.len(), "buffers of different sizes");
    assert!(!a.is_empty(), "nothing to compare");
    a.iter()
        .zip(b)
        .flat_map(|(p, q)| {
            (0..4).map(move |c| {
                if p[c] == q[c] {
                    return 0.0;
                }
                let e = (p[c] - q[c]).abs() / (1.0 + q[c].abs());
                if e.is_nan() { f32::INFINITY } else { e }
            })
        })
        .fold(0.0, f32::max)
}

/// The GPU agrees with the reference and refuses the twin.
pub fn judge(name: &str, gpu: &[V4], reference: &[V4], twin: &[V4]) {
    let (e, t) = (error(gpu, reference), error(gpu, twin));
    println!("{name}: error {e:.2e}, twin {t:.2e}");
    assert!(e < TOLERANCE, "{name}: the GPU is {e:.2e} off its reference");
    assert!(t > TOLERANCE, "{name}: the twin passes at {t:.2e}, so the check sees nothing");
}

pub fn whole(g: &mut Graph, image: &Image) -> Vec<V4> {
    g.render(image, image.extent(), None).expect("the stage renders")
}

pub fn mapped(pixels: &[V4], f: impl Fn(V4) -> V4) -> Vec<V4> {
    pixels.iter().map(|&p| f(p)).collect()
}

pub struct Dump {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<V4>,
}

/// `RenderDump`'s format: width and height as `Int32`, then RGBA `Float`, top row first.
pub fn dump(path: &Path) -> Dump {
    let b = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let int = |i: usize| i32::from_le_bytes(b[i..i + 4].try_into().unwrap()) as u32;
    let (width, height) = (int(0), int(4));
    let floats: Vec<f32> = b[8..].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
    assert_eq!(floats.len(), (width * height * 4) as usize, "{}: truncated", path.display());
    Dump { width, height, pixels: floats.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect() }
}

/// Mean, 99th centile and worst of the worst channel's gap, after the output clip both sides share.
pub fn gap(a: &[V4], b: &[V4]) -> (f64, f64, f64) {
    assert_eq!(a.len(), b.len(), "renders of different sizes");
    let clip = |x: f32| x.clamp(0.0, 1.0);
    let mut e: Vec<f64> =
        a.iter().zip(b).map(|(x, y)| (0..3).map(|c| f64::from((clip(x[c]) - clip(y[c])).abs())).fold(0.0, f64::max)).collect();
    let mean = e.iter().sum::<f64>() / e.len() as f64;
    e.sort_by(|x, y| x.total_cmp(y));
    (mean, e[e.len() * 99 / 100], *e.last().unwrap())
}
