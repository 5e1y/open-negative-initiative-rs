//! `ONI_BENCH_COST=1`: Texture's GPU time on a 2000 px preview, its kernels folded into the blurs
//! against the same chain in ten passes, runs interleaved so both see the same load, and a digital
//! photo's film effects at rest and on a notch, a virtual negative, the dye separation and the paper on a full-screen region; without the
//! variable it returns at once.

use oni_graph::{Cache, Graph, Image, Rect, Source};
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
    // The finishing's runs: four kernels after the curves, three at the end, laid or one by one.
    let mut s2 = s.clone();
    s2.spectrogram.saturation[2] = 0.4;
    let run = || {
        vec![
            stages::spectrogram(&finishing::spectrogram(&s2.spectrogram).unwrap()),
            stages::saturation(&finishing::saturation(&finishing::sauce(0.8)).unwrap()),
            stages::saturation(&finishing::saturation(&finishing::sauce(1.2)).unwrap()),
            stages::vignette(Rect::new(0, 0, w, h), -0.5, 0.5),
            stages::saturation(&finishing::saturation(&finishing::sauce(1.4)).unwrap()),
            stages::output_clip(),
        ]
    };
    let apart = run().into_iter().fold(Image::new(&source), |i, k| i.then(k));
    let laid = stages::laid(Image::new(&source), run());
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for _ in 0..RUNS {
        a.push(ms(&mut g, &apart));
        b.push(ms(&mut g, &laid));
    }
    println!("six per-pixel kernels: one pass each {:.2} ms, laid as one {:.2} ms", median(a), median(b));
    for (name, image) in parts {
        let before = g.stats().textures_created;
        let t = median((0..RUNS).map(|_| ms(&mut g, &image)).collect());
        println!("  {name}: {t:.2} ms, {} textures made over {RUNS} runs", g.stats().textures_created - before);
    }
}

/// The whole pipeline on 2000 px with every finishing feature on, then each feature at rest in
/// turn: what each costs on the notch path, the materialise and a one-pixel read included.
#[test]
fn each_feature_s_share_of_a_busy_render() {
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
    let mut busy = PipelineSettings::rest(ConversionMode::Negative, true);
    busy.chroma.force = 0.5;
    busy.sharpen.amount = 1.0;
    busy.texture.amount = 0.5;
    busy.vignette.stops = 0.5;
    busy.secret_sauce = 0.5;
    busy.spectrogram.hue[0] = 0.1;
    busy.manual_curves.contrast = 0.2;
    let request = oni_pipeline::Request { full_width: Some(f64::from(w)), ..Default::default() };
    let mut pipeline = oni_pipeline::Pipeline::new();
    let mut time = |s: &PipelineSettings| {
        let image = pipeline.image(&mut g, &source, s, &request, &Default::default()).unwrap();
        median((0..RUNS).map(|_| ms(&mut g, &image)).collect())
    };
    let all = time(&busy);
    let rest = time(&PipelineSettings::rest(ConversionMode::Negative, true));
    println!("busy {all:.2} ms, at rest {rest:.2} ms");
    let off: [(&str, fn(&mut PipelineSettings)); 7] = [
        ("chroma denoise", |s| s.chroma.force = 0.0),
        ("sharpen", |s| s.sharpen.amount = 0.0),
        ("texture", |s| s.texture.amount = 0.0),
        ("vignette", |s| s.vignette.stops = 0.0),
        ("secret sauce", |s| s.secret_sauce = 0.0),
        ("spectrogram", |s| s.spectrogram = Default::default()),
        ("curves", |s| s.manual_curves = Default::default()),
    ];
    for (name, rest_it) in off {
        let mut s = busy.clone();
        rest_it(&mut s);
        println!("  without {name}: {:.2} ms ({:+.2})", time(&s), time(&s) - all);
    }
    // A digital photo's Lighting after its curves: a pass of its own only when finishing's first is empty.
    let digital = PipelineSettings::rest(ConversionMode::Digital, true).per_pixel_only();
    let mut lit = digital.clone();
    lit.print.exposure = 0.6;
    let curved = |mut s: PipelineSettings| {
        s.manual_curves.contrast = 0.2;
        s
    };
    let (plain, bright) = (time(&digital), time(&lit));
    let (bent, both) = (time(&curved(digital.clone())), time(&curved(lit.clone())));
    println!("digital {plain:.2} ms, lit {bright:.2} ({:+.2}); curved {bent:.2} ms, lit {both:.2} ({:+.2})", bright - plain, both - bent);
}

fn ramp(g: &mut Graph, w: u32, h: u32) -> Source {
    let pixels: Vec<[f32; 4]> = (0..w * h).map(|i| {
        let speck = ((i.wrapping_mul(2_654_435_761) >> 8) % 1000) as f32 / 10_000.0;
        [0.02 + (i % w) as f32 / w as f32 * 0.9 + speck, 0.3 + speck, 0.05 + (i / w) as f32 / h as f32 * 0.6, 1.0]
    }).collect();
    g.upload(w, h, &pixels).unwrap()
}

/// A digital photo's grain and halation on a whole render at three sizes, then an exposure notch through
/// the canvas's cache with and without grain: a print notch never pays the grain.
#[test]
fn a_digital_photo_s_film_effects_at_rest_and_on_a_notch() {
    if std::env::var_os("ONI_BENCH_COST").is_none() {
        return;
    }
    let Ok(mut g) = Graph::new() else { return };
    let rest = PipelineSettings::rest(ConversionMode::Digital, true);
    let with = |grain: f32, size: f32, halation: f32| {
        let mut s = rest.clone();
        (s.grain.amount, s.grain.size, s.halation.amount) = (grain, size, halation);
        s
    };
    let cases = [
        ("rest", with(0.0, 0.5, 0.0)),
        ("grain size 0", with(0.5, 0.0, 0.0)),
        ("grain size 0.5", with(0.5, 0.5, 0.0)),
        ("grain size 1", with(0.5, 1.0, 0.0)),
        ("halation", with(0.0, 0.5, 0.5)),
        ("both", with(0.5, 0.5, 0.5)),
    ];
    let mut pipeline = oni_pipeline::Pipeline::new();
    for (w, h) in [(2000u32, 1333u32), (3024, 2016)] {
        let source = ramp(&mut g, w, h);
        let request = oni_pipeline::Request { full_width: Some(f64::from(w)), ..Default::default() };
        let images: Vec<Image> = cases.iter().map(|(_, s)| pipeline.image(&mut g, &source, s, &request, &Default::default()).unwrap()).collect();
        let mut times = vec![Vec::new(); images.len()];
        for _ in 0..RUNS {
            for (t, image) in times.iter_mut().zip(&images) {
                t.push(ms(&mut g, image));
            }
        }
        let line: Vec<String> = cases.iter().zip(times).map(|((name, _), t)| format!("{name} {:.2}", median(t))).collect();
        println!("digital {w}x{h}, whole render in ms: {}", line.join(", "));
        // Exposure notches back and forth, each case through its own cache, interleaved.
        let notched = [("no grain", with(0.0, 0.5, 0.0)), ("grain", with(0.5, 0.5, 0.0)), ("grain and halation", with(0.5, 0.5, 0.5))];
        let mut caches: Vec<Cache> = notched.iter().map(|_| Cache::frontier()).collect();
        let mut times = vec![Vec::new(); notched.len()];
        for run in 0..RUNS + 2 {
            for (i, (_, s)) in notched.iter().enumerate() {
                let mut t = s.clone();
                t.print.exposure += if run % 2 == 0 { 0.01 } else { 0.02 };
                let image = pipeline.image(&mut g, &source, &t, &request, &Default::default()).unwrap();
                let started = Instant::now();
                let made = g.materialise_cached(&image, image.extent(), Some(&mut caches[i])).expect("renders");
                let e = made.extent();
                g.render(&Image::new(&made), Rect::new(e.x, e.y, 1, 1), None).expect("reads");
                // The first two notches fill the cache.
                if run >= 2 {
                    times[i].push(started.elapsed().as_secs_f64() * 1000.0);
                }
            }
        }
        let line: Vec<String> = notched.iter().zip(times).map(|((name, _), t)| format!("{name} {:.2}", median(t))).collect();
        println!("digital {w}x{h}, exposure notch in ms: {}", line.join(", "));
    }
}

/// The virtual negative's pass on the region a full-screen window shows at full resolution on a 16" Retina
/// display, 3456 × 2234 pixels, against one plain per-pixel pass on it.
#[test]
fn the_virtual_negative_s_gpu_time_on_a_full_screen_region() {
    if std::env::var_os("ONI_BENCH_COST").is_none() {
        return;
    }
    let Ok(mut g) = Graph::new() else { return };
    let (w, h) = (3456u32, 2234u32);
    let pixels: Vec<[f32; 4]> = (0..w * h)
        .map(|i| {
            let x = (i % w) as f32 / w as f32;
            let y = (i / w) as f32 / h as f32;
            [0.01 + 2.0 * x * x, 0.02 + y, 0.05 + x * y, 1.0]
        })
        .collect();
    let source = g.upload(w, h, &pixels).unwrap();
    let stock = oni_pipeline::film::stock(oni_settings::VirtualStock::KodakPortra800);
    let (tw, th) = stock.kernel.table_size();
    let table = g.upload(tw as u32, th as u32, &stock.table).unwrap();
    let film = Image::new(&source).then_reading(stages::virtual_negative(&stock.kernel_at(0.3)), &[Image::new(&table)]);
    let plain = Image::new(&source).then(stages::luma_only());
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for _ in 0..RUNS {
        a.push(ms(&mut g, &film));
        b.push(ms(&mut g, &plain));
    }
    println!("virtual negative on {w}x{h}: {:.2} ms; one plain per-pixel pass {:.2} ms", median(a), median(b));
}

/// The dye separation's pass on the same full-screen region, a negative's scan, against one plain per-pixel pass.
#[test]
fn the_dye_separation_s_gpu_time_on_a_full_screen_region() {
    if std::env::var_os("ONI_BENCH_COST").is_none() {
        return;
    }
    let Ok(mut g) = Graph::new() else { return };
    let (w, h) = (3456u32, 2234u32);
    let stock = oni_pipeline::film::stock(oni_settings::VirtualStock::KodakPortra800);
    let pixels: Vec<[f32; 4]> = (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) as f32 / w as f32, (i / w) as f32 / h as f32);
            oni_render::cpu::virtual_negative([0.01 + 2.0 * x * x, 0.02 + y, 0.05 + x * y, 1.0], &stock.kernel, &stock.table)
        })
        .collect();
    let source = g.upload(w, h, &pixels).unwrap();
    let k = oni_pipeline::conversion::stage47(&PipelineSettings::rest(ConversionMode::Digital, true));
    let unmixed = Image::new(&source).then(stages::dye_separation(&k, &k.dyes.expect("a digital photo's dyes")));
    let plain = Image::new(&source).then(stages::luma_only());
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for _ in 0..RUNS {
        a.push(ms(&mut g, &unmixed));
        b.push(ms(&mut g, &plain));
    }
    println!("dye separation on {w}x{h}: {:.2} ms; one plain per-pixel pass {:.2} ms", median(a), median(b));
}

/// A print notch on the same full-screen region, past the dye separation the cache keeps: the paper laid with the rest
/// of stage 4-7 in one pass, against ONI's own print in its pass; then the notch's arguments solved on the CPU.
#[test]
fn the_paper_s_gpu_time_on_a_full_screen_region() {
    if std::env::var_os("ONI_BENCH_COST").is_none() {
        return;
    }
    let Ok(mut g) = Graph::new() else { return };
    let (w, h) = (3456u32, 2234u32);
    let stock = oni_pipeline::film::stock(oni_settings::VirtualStock::KodakPortra800);
    let paper = PipelineSettings::rest(ConversionMode::Digital, true);
    let separated = oni_pipeline::conversion::stage47(&paper);
    let d = separated.dyes.expect("a digital photo's dyes");
    let pixels: Vec<[f32; 4]> = (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) as f32 / w as f32, (i / w) as f32 / h as f32);
            let scan = oni_render::cpu::virtual_negative([0.01 + 2.0 * x * x, 0.02 + y, 0.05 + x * y, 1.0], &stock.kernel, &stock.table);
            oni_render::cpu::dye_separation(scan, &separated, &d)
        })
        .collect();
    let source = g.upload(w, h, &pixels).unwrap();
    let after = separated.after_dyes();
    let printed = stages::laid(Image::new(&source), vec![stages::paper(&separated.paper.expect("a digital photo's paper")), stages::stage47(&after)]);
    let onis = oni_pipeline::conversion::stage47(&PipelineSettings { print_paper: None, ..paper.clone() });
    let plain = Image::new(&source).then(stages::stage47(&onis.after_dyes()));
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for _ in 0..RUNS {
        a.push(ms(&mut g, &printed));
        b.push(ms(&mut g, &plain));
    }
    println!("paper and stage 4-7 on {w}x{h}: {:.2} ms; ONI's print alone {:.2} ms", median(a), median(b));
    // A Lighting notch's arguments on the CPU: the enlarger solved and its print laid out again.
    let notches: Vec<f64> = (0..RUNS)
        .map(|i| {
            let mut t = paper.clone();
            t.print.exposure += i as f32 * 0.01;
            let started = Instant::now();
            std::hint::black_box(oni_pipeline::conversion::stage47(&t));
            started.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    println!("a Lighting notch's paper arguments on the CPU: {:.3} ms", median(notches));
}
