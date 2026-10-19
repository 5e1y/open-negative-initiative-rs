//! Kernels folded into the blur that reads them, against the steps run one after the other: on the
//! CPU reference for each fold, and on the GPU for Texture's whole chain against its ten passes.

use oni_graph::{Graph, Image, Rect};
use oni_pipeline::stages::{self, Before};
use oni_pipeline::finishing;
use oni_render::cpu::{self, Grid};
use oni_render::{Gaussian, TextureCoeff};
use oni_settings::{ConversionMode, PipelineSettings};

type V4 = [f32; 4];

/// As the stage checks: |gpu - reference| / (1 + |reference|).
const TOLERANCE: f32 = 1e-4;

fn noise(width: u32, height: u32, seed: u64, low: f32, high: f32) -> Vec<V4> {
    let mut state = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
    let mut next = move || {
        state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        low + ((state >> 40) as f32 / (1u64 << 24) as f32) * (high - low)
    };
    (0..width * height).map(|_| [next(), next(), next(), 1.0]).collect()
}

fn error(a: &[V4], b: &[V4]) -> f32 {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).flat_map(|(p, q)| (0..4).map(move |c| (p[c] - q[c]).abs() / (1.0 + q[c].abs()))).fold(0.0, f32::max)
}

fn judge(name: &str, gpu: &[V4], reference: &[V4], twin: &[V4]) {
    let (e, t) = (error(gpu, reference), error(gpu, twin));
    println!("{name}: error {e:.2e}, twin {t:.2e}");
    assert!(e <= TOLERANCE, "{name}: {e:.2e} from its reference");
    assert!(t > TOLERANCE, "{name}: the twin passes too ({t:.2e}), the check is blind");
}

fn whole(g: &mut Graph, image: &Image) -> Vec<V4> {
    g.render(image, image.extent(), None).expect("renders")
}

/// Each step a blur folds into its taps against the step applied first, then both axes blurred on
/// the CPU; the twin folds the wrong step, or the right one at another edge contrast.
#[test]
fn a_blur_reading_through_a_step_is_the_step_then_the_blur() {
    let Ok(mut g) = Graph::new() else { return println!("SKIPPED: no adapter") };
    let extent = Rect::new(4, -2, 41, 27);
    let pixels = noise(extent.w, extent.h, 17, 0.0, 1.2);
    let input = g.upload_at(extent, &pixels).unwrap();
    let w = oni_settings::axis::pipeline::LUMA_WEIGHTS;
    let luma = |p: V4| {
        let y = p[0] * w[0] + p[1] * w[1] + p[2] * w[2];
        [y, y, y, p[3]]
    };
    let blurred = |f: &dyn Fn(V4) -> V4, sigma: f32| {
        let grid = Grid::new(extent.x, extent.y, extent.w as usize, extent.h as usize, pixels.iter().map(|&p| f(p)).collect());
        let k = |vertical| Gaussian { sigma, vertical, transparent: false };
        cpu::gaussian(&cpu::gaussian(&grid, &k(false)), &k(true)).pixels
    };
    let coeff = |relative| move |p: V4| cpu::texture_coeff(p, &TextureCoeff { relative });
    let (right, off) = (coeff(0.125), coeff(0.13));
    let cases: [(&str, Before, &dyn Fn(V4) -> V4, &dyn Fn(V4) -> V4); 3] = [
        ("luma", Before::Luma, &luma, &cpu::texture_moments),
        ("moments", Before::Moments, &cpu::texture_moments, &luma),
        ("coefficients", Before::Coeff(0.125), &right, &off),
    ];
    for (name, before, step, wrong) in cases {
        let out = whole(&mut g, &stages::blurred_of(Image::new(&input), 1.6, before));
        judge(&format!("blur through {name}"), &out, &blurred(step, 1.6), &blurred(wrong, 1.6));
    }
}

/// `Pipeline.applyTexture`'s ten passes, as the stages ran them one by one.
fn ten_passes(image: Image, narrow: f64, wide: f64, gain: f32) -> Image {
    let floor = oni_settings::axis::pipeline::BLUR_FLOOR;
    let blur = |i: Image, sigma: f64| if sigma <= floor { i } else { stages::blurred(i, sigma, false) };
    let narrowed = blur(image.clone().then(stages::luma_only()), narrow);
    let moments = narrowed.clone().then(stages::texture_moments());
    let coeff = blur(moments, wide).then(stages::texture_coeff(&TextureCoeff { relative: finishing::texture::EDGE_CONTRAST }));
    image.then_reading(stages::texture(gain), &[narrowed, blur(coeff, wide)])
}

/// The pipeline's Texture, its kernels folded into the blurs, against the same chain in ten passes;
/// the twin is that chain at another gain. At 780 px the narrow blur is under its floor, at 4000 over.
#[test]
fn texture_in_seven_passes_is_texture_in_ten() {
    let Ok(mut g) = Graph::new() else { return println!("SKIPPED: no adapter") };
    for (w, h) in [(780u32, 520u32), (4000, 300)] {
        let pixels = noise(w, h, 5, 0.02, 0.9);
        let source = g.upload(w, h, &pixels).unwrap();
        let mut s = PipelineSettings::rest(ConversionMode::Positive, true);
        s.texture.amount = 0.8;
        let reference = f64::from(w.max(h));
        let (narrow, wide, gain) = finishing::texture(&s.texture, reference).expect("Texture reaches");
        let fused = whole(&mut g, &oni_pipeline::texture(Image::new(&source), &s, reference));
        let ten = whole(&mut g, &ten_passes(Image::new(&source), narrow, wide, gain));
        let off = whole(&mut g, &ten_passes(Image::new(&source), narrow, wide, gain * 1.1));
        judge(&format!("Texture at {w} px, narrow {narrow:.2}"), &fused, &ten, &off);
    }
}
