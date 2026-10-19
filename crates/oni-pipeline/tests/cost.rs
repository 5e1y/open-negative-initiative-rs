//! `ONI_BENCH_COST=1`: Texture's GPU time on a 2000 px preview, its kernels folded into the blurs
//! against the same chain in ten passes, runs interleaved so both see the same load; without the
//! variable it returns at once.

use oni_graph::{Graph, Image, Rect};
use oni_pipeline::{finishing, stages};
use oni_render::TextureCoeff;
use oni_settings::{ConversionMode, PipelineSettings};
use std::time::Instant;

const RUNS: usize = 41;

/// Materialised, then one pixel read back, which waits for every pass queued before it.
fn ms(g: &mut Graph, image: &Image) -> f64 {
    let started = Instant::now();
    let made = g.materialise(image).expect("renders");
    let e = made.extent();
    g.render(&Image::new(&made), Rect::new(e.x, e.y, 1, 1), None).expect("reads");
    started.elapsed().as_secs_f64() * 1000.0
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

#[test]
fn texture_s_gpu_time_on_the_preview() {
    if std::env::var_os("ONI_BENCH_COST").is_none() {
        return;
    }
    let Ok(mut g) = Graph::new() else { return };
    let (w, h) = (2000u32, 1333u32);
    let pixels: Vec<[f32; 4]> = (0..w * h).map(|i| {
        let grain = ((i.wrapping_mul(2_654_435_761) >> 8) % 1000) as f32 / 10_000.0;
        [0.2 + (i % w) as f32 / w as f32 * 0.6 + grain, 0.4 + grain, 0.3 + (i / w) as f32 / h as f32 * 0.5, 1.0]
    }).collect();
    let source = g.upload(w, h, &pixels).unwrap();
    let mut s = PipelineSettings::rest(ConversionMode::Positive, true);
    s.texture.amount = 0.8;
    let reference = f64::from(w);
    let (narrow, wide, gain) = finishing::texture(&s.texture, reference).unwrap();
    let ten = {
        let image = Image::new(&source);
        let narrowed = stages::blurred(image.clone().then(stages::luma_only()), narrow, false);
        let moments = narrowed.clone().then(stages::texture_moments());
        let coeff = stages::blurred(moments, wide, false).then(stages::texture_coeff(&TextureCoeff { relative: finishing::texture::EDGE_CONTRAST }));
        image.then_reading(stages::texture(gain), &[narrowed, stages::blurred(coeff, wide, false)])
    };
    let seven = oni_pipeline::texture(Image::new(&source), &s, reference);
    let plain = Image::new(&source).then(stages::luma_only());
    let (mut a, mut b, mut c) = (Vec::new(), Vec::new(), Vec::new());
    for _ in 0..RUNS {
        a.push(ms(&mut g, &ten));
        b.push(ms(&mut g, &seven));
        c.push(ms(&mut g, &plain));
    }
    let (ten, seven, plain) = (median(a), median(b), median(c));
    println!("Texture on 2000 px: ten passes {ten:.2} ms, seven {seven:.2} ms; one per-pixel pass {plain:.2} ms");
    let parts = [
        ("narrow blur", stages::blurred(Image::new(&source), narrow, false)),
        ("wide blur", stages::blurred(Image::new(&source), wide, false)),
        ("wide rows", Image::new(&source).then(stages::Blur(oni_render::Gaussian { sigma: wide as f32, vertical: false, transparent: false }))),
        ("wide columns", Image::new(&source).then(stages::Blur(oni_render::Gaussian { sigma: wide as f32, vertical: true, transparent: false }))),
        ("two per-pixel", Image::new(&source).then(stages::luma_only()).then(stages::texture_moments())),
        ("texture kernel", Image::new(&source).then_reading(stages::texture(gain), &[Image::new(&source), Image::new(&source)])),
    ];
    for (name, image) in parts {
        let before = g.stats().textures_created;
        let t = median((0..RUNS).map(|_| ms(&mut g, &image)).collect());
        println!("  {name}: {t:.2} ms, {} textures made over {RUNS} runs", g.stats().textures_created - before);
    }
}
