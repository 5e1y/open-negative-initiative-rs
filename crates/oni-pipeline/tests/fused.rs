//! Kernels folded into the blur that reads them, against the steps run one after the other: on the
//! CPU reference for each fold, and on the GPU for Texture's whole chain against its ten passes.

mod common;

use common::{V4, graph, judge, noise, whole};
use oni_graph::{Image, Rect};
use oni_pipeline::stages::{self, Before};
use oni_pipeline::finishing;
use oni_render::cpu::{self, Grid};
use oni_render::{Gaussian, TextureCoeff};
use oni_settings::{ConversionMode, PipelineSettings};

/// Each step a blur folds into its taps against the step applied first, then both axes blurred on
/// the CPU; the twin folds the wrong step, or the right one at another edge contrast.
#[test]
fn a_blur_reading_through_a_step_is_the_step_then_the_blur() {
    let Some(mut g) = graph("a_blur_reading_through_a_step_is_the_step_then_the_blur") else { return };
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
    let Some(mut g) = graph("texture_in_seven_passes_is_texture_in_ten") else { return };
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

/// Kernels laid as one pass against the same kernels one pass each, a kernel twice among them; the
/// twin runs them in another order. A kernel reading an auxiliary image breaks the run.
#[test]
fn kernels_in_one_pass_are_the_kernels_one_after_the_other() {
    let Some(mut g) = graph("kernels_in_one_pass_are_the_kernels_one_after_the_other") else { return };
    let extent = Rect::new(-3, 5, 64, 40);
    let pixels = noise(extent.w, extent.h, 23, 0.0, 1.3);
    let input = g.upload_at(extent, &pixels).unwrap();
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.spectrogram.saturation[2] = 0.4;
    s.spectrogram.hue[5] = -0.3;
    let kernels = || {
        vec![
            stages::spectrogram(&finishing::spectrogram(&s.spectrogram).unwrap()),
            stages::saturation(&finishing::saturation(&finishing::sauce(0.8)).unwrap()),
            stages::vignette(extent, -0.6, 0.5),
            stages::exposure(0.3),
            stages::saturation(&finishing::saturation(&finishing::sauce(1.4)).unwrap()),
            stages::luma_only(),
            stages::output_clip(),
        ]
    };
    let one_by_one = |order: Vec<stages::PerPixel>| order.into_iter().fold(Image::new(&input), |i, k| i.then(k));
    let fused = whole(&mut g, &stages::laid(Image::new(&input), kernels()));
    let mut swapped = kernels();
    swapped.swap(0, 3);
    judge("seven kernels in one pass", &fused, &whole(&mut g, &one_by_one(kernels())), &whole(&mut g, &one_by_one(swapped)));
}
