//! `stage_cost <photo> [canvas w h]`: a photo's render at fit on a canvas (3456 × 2034 drawable pixels by default),
//! each stage of its chain timed as the render up to it less the one before, then Exposure and grain notches through
//! the canvas's own path, each to the GPU's end, one way so no notch meets a render the canvas keeps. Its sidecar is read, never written.

use oni_app::render::{self, Device, Engine, Reading, View};
use oni_graph::{Graph, Image, Rect};
use oni_pipeline::{Inputs, Request, conversion, grain, stages};
use oni_settings::{PipelineSettings, sidecar};
use std::path::Path;
use std::time::Instant;

const RUNS: usize = 15;

/// `image` made whole, then one pixel read back, which waits for every pass queued before it.
fn ms(g: &mut Graph, image: &Image, region: Option<Rect>) -> f64 {
    let started = Instant::now();
    let made = g.materialise_cached(image, region.unwrap_or_else(|| image.extent()), None).expect("renders");
    let e = made.extent();
    g.render(&Image::new(&made), Rect::new(e.x, e.y, 1, 1), None).expect("reads");
    started.elapsed().as_secs_f64() * 1000.0
}

fn inputs_seeded(seed: [u32; 2]) -> Inputs<'static> {
    Inputs { seed, ..Inputs::default() }
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let photo = Path::new(&args[0]);
    let canvas = (args.get(1).map_or(3456.0, |v| v.parse().unwrap()), args.get(2).map_or(2034.0, |v| v.parse().unwrap()));
    let s = sidecar::read(photo, true).expect("a sidecar").settings;
    println!("mode {:?}, film {:?}, grain {} size {}, halation {}, bloom {:?}", s.mode, s.film(), s.grain.amount, s.grain.size, s.halation.amount, s.bloom);
    let device = Device::open().expect("a GPU");
    let engine = Engine::shared(&device);
    let mut e = engine.lock().unwrap();
    let started = Instant::now();
    let decoded = render::decode(photo, Reading::of(&s)).expect("a decode");
    let decode = started.elapsed().as_secs_f64() * 1000.0;
    let started = Instant::now();
    let negative = e.prepare(decoded).unwrap();
    let prepare = started.elapsed().as_secs_f64() * 1000.0;
    let (w, h) = negative.size();
    println!("decode {decode:.0} ms, prepare {prepare:.0} ms, preview {w} x {h}, full width {}", negative.full_width);

    // The canvas's whole render at fit, then a notch of each kind through its cache.
    let view = View { viewport: Default::default(), size: canvas, full: None, coarse: false };
    let resident = |e: &mut Engine, s: &PipelineSettings| {
        let started = Instant::now();
        let (source, _) = e.resident(&negative, s, Some(&view)).unwrap();
        let x = source.extent();
        e.parts().0.render(&Image::new(&source), Rect::new(x.x, x.y, 1, 1), None).unwrap();
        (started.elapsed().as_secs_f64() * 1000.0, x)
    };
    let s = match std::env::var("ONI_COST_FILM").ok().filter(|k| !k.is_empty()) {
        Some(key) => {
            let mut t = s.clone();
            t.virtual_film = Some(oni_settings::VirtualFilm { stock: oni_settings::VirtualStock::of(Box::leak(key.into_boxed_str())), exposure: 0.0 });
            t.halation.amount = std::env::var("ONI_COST_HALATION").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
            t
        }
        None => s,
    };
    let (_, region) = resident(&mut e, &s);
    println!("canvas {} x {}: region {} x {}", canvas.0, canvas.1, region.w, region.h);
    let notches: [(&str, fn(&mut PipelineSettings, usize)); 13] = [
        ("Halation size", |t, _| t.halation.size += 1.0),
        ("Bloom", |t, _| t.bloom.size += 0.1),
        ("Grain size", |t, _| t.grain.size += 0.2),
        ("Exposure", |t, _| t.print.exposure += 0.01),
        ("Contrast", |t, _| t.print.contrast += 0.01),
        ("Shadows", |t, _| t.print.shadows += 0.01),
        ("Red gain", |t, _| t.gains.stops[0] += 0.01),
        ("Density", |t, _| t.density += 0.01),
        ("Chroma", |t, _| t.chroma.force = (t.chroma.force + 0.01).max(0.0)),
        ("Sharpen", |t, _| t.sharpen.amount = (t.sharpen.amount + 0.01).max(0.0)),
        ("Grain", |t, _| t.grain.amount = (t.grain.amount + 0.01).max(0.0)),
        ("Halation", |t, _| t.halation.amount = (t.halation.amount + 0.01).max(0.0)),
        ("Vignette", |t, _| t.vignette.stops += 0.01),
    ];
    for (name, step) in notches {
        let mut t = s.clone();
        let times: Vec<f64> = (0..RUNS + 2).map(|i| {
            step(&mut t, i);
            resident(&mut e, &t).0
        }).skip(2).collect();
        println!("  {name} notch: {:.1} ms p50, {:.1} ms worst", median(times.clone()), times.iter().cloned().fold(0.0, f64::max));
    }

    // At 100 % on the full frame: a notch through the cache, then pans of a tenth of the canvas, each a new region.
    let started = Instant::now();
    let full = e.full(render::decode_full(photo, Reading::of(&s)).expect("a decode")).unwrap();
    println!("full decode and upload {:.0} ms", started.elapsed().as_secs_f64() * 1000.0);
    let at = |dx: f64| View { viewport: oni_app::viewport::Viewport { scale: Some(1.0), offset: (dx, 0.0) }, size: canvas, full: Some(full.clone()), coarse: false };
    let near = |e: &mut Engine, s: &PipelineSettings, v: &View| {
        let started = Instant::now();
        let (source, _) = e.resident(&negative, s, Some(v)).unwrap();
        let x = source.extent();
        e.parts().0.render(&Image::new(&source), Rect::new(x.x, x.y, 1, 1), None).unwrap();
        started.elapsed().as_secs_f64() * 1000.0
    };
    let first = near(&mut e, &s, &at(0.0));
    let mut t = s.clone();
    let notches: Vec<f64> = (0..RUNS + 2).map(|_| {
        t.print.exposure += 0.01;
        near(&mut e, &t, &at(0.0))
    }).skip(2).collect();
    let pans: Vec<f64> = (1..=RUNS).map(|i| near(&mut e, &s, &at(-(i as f64) * canvas.0 / 10.0))).collect();
    println!("  100 %: first {first:.1} ms, Exposure notch {:.1} ms p50, pan {:.1} ms p50, {:.1} worst", median(notches), median(pans.clone()), pans.iter().cloned().fold(0.0, f64::max));
    // A pinch: lighter renders at a new scale each, then the same with each film effect at rest.
    let pinch = |e: &mut Engine, s: &PipelineSettings| {
        let times: Vec<f64> = (0..RUNS).map(|i| {
            let v = View { viewport: oni_app::viewport::Viewport { scale: Some(0.4 + 0.03 * i as f64), offset: (0.0, 0.0) }, size: canvas, full: Some(full.clone()), coarse: true };
            near(e, s, &v)
        }).collect();
        median(times)
    };
    let quiet = |f: fn(&mut PipelineSettings)| {
        let mut t = s.clone();
        f(&mut t);
        t
    };
    let pinched = pinch(&mut e, &s);
    let no_grain = pinch(&mut e, &quiet(|t| t.grain.amount = 0.0));
    let no_halation = pinch(&mut e, &quiet(|t| t.halation.amount = 0.0));
    let no_bloom = pinch(&mut e, &quiet(|t| t.bloom = Default::default()));
    let none = pinch(&mut e, &quiet(|t| {
        t.grain.amount = 0.0;
        t.halation.amount = 0.0;
        t.bloom = Default::default();
    }));
    println!("  pinch, a lighter render: {pinched:.1} ms p50; without grain {no_grain:.1}, halation {no_halation:.1}, bloom {no_bloom:.1}, any {none:.1}");
    // The owner's open question: a gesture's lighter render taken from a neighbour scale's, stretched, against the exact
    // one; its cost is the stretch alone, its difference read in display code on the overlap.
    if std::env::var_os("ONI_COST_NEIGHBOUR").is_some() {
        let request = |scale: f64| Request { full_width: Some(negative.full_width), scale, ..Request::default() };
        let inputs = inputs_seeded(negative.seed);
        let code = |v: f32| if v <= 0.003_130_8 { 12.92 * v } else { 1.055 * v.max(0.0).powf(1.0 / 2.4) - 0.055 };
        for ratio in [2f64.powf(0.125), 2f64.powf(0.25), 2f64.powf(0.5)] {
            let (mut mean, mut p99, mut grain) = (Vec::new(), Vec::new(), Vec::new());
            for s0 in [0.45, 0.6, 0.8] {
                let (graph, pipeline) = e.parts();
                let exact = pipeline.image(graph, &full, &s, &request(s0), &inputs).unwrap();
                let rung = pipeline.image(graph, &full, &s, &request(s0 / ratio), &inputs).unwrap();
                let made = graph.materialise(&rung).unwrap();
                let stretched = Image::new(&made).lanczos(ratio);
                let r = exact.extent().intersect(&stretched.extent());
                let r = Rect::new(r.x + r.w as i32 / 4, r.y + r.h as i32 / 4, r.w / 2, r.h / 2);
                let a = graph.render(&exact, r, None).unwrap();
                let b = graph.render(&stretched, r, None).unwrap();
                let mut d: Vec<f32> = a.iter().zip(&b).flat_map(|(x, y)| (0..3).map(move |c| (code(x[c]) - code(y[c])).abs())).collect();
                mean.push(d.iter().sum::<f32>() / d.len() as f32);
                d.sort_by(f32::total_cmp);
                p99.push(d[d.len() * 99 / 100]);
                // The grain's own spread in display code, 1-pixel differences of green, exact against stretched.
                let spread = |v: &[[f32; 4]]| {
                    let g: Vec<f32> = v.windows(2).map(|w| code(w[1][1]) - code(w[0][1])).collect();
                    (g.iter().map(|x| x * x).sum::<f32>() / g.len() as f32).sqrt()
                };
                grain.push(spread(&b) / spread(&a));
            }
            let avg = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
            println!("  neighbour scale x{ratio:.3}: display code off by {:.4} mean, {:.4} p99 (of 1); pixel-to-pixel spread {:.2} of the exact's", avg(&mean), avg(&p99), avg(&grain));
        }
    }
    // A gesture's pan: lighter renders while the hand moves, then the sharp one it ends on.
    let gesture = |dx: f64, coarse: bool| View { coarse, ..at(dx) };
    let ended: Vec<f64> = (0..5).map(|i| {
        let from = -(i as f64) * canvas.0 / 4.0;
        near(&mut e, &s, &gesture(from, false));
        for step in 1..=6 {
            near(&mut e, &s, &gesture(from - f64::from(step) * canvas.0 / 24.0, true));
        }
        near(&mut e, &s, &gesture(from - canvas.0 / 4.0, false))
    }).collect();
    println!("  100 %: a gesture's sharp render after a quarter canvas pan {:.1} ms p50", median(ended));
    // The last pan's render against the same region rendered afresh, then against it a pixel over, which must differ.
    let last = at(-(RUNS as f64) * canvas.0 / 10.0);
    let (panned, _) = e.resident(&negative, &s, Some(&last)).unwrap();
    let r = panned.extent();
    let request = Request { full_width: Some(negative.full_width), ..Request::default() };
    let (graph, pipeline) = e.parts();
    let plate_inputs = Inputs { plate: Some(&negative.preview), ..inputs_seeded(negative.seed) };
    let image = pipeline.image(graph, &full, &s, &request, &plate_inputs).unwrap();
    let read = |g: &mut Graph, source: &oni_graph::Source| g.render(&Image::new(source), source.extent(), None).unwrap();
    let kept = read(graph, &panned);
    let fresh = graph.materialise_cached(&image, r, None).unwrap();
    let worst = |a: &[[f32; 4]], b: &[[f32; 4]]| a.iter().zip(b).map(|(x, y)| (0..3).map(|c| (x[c] - y[c]).abs()).fold(0.0f32, f32::max)).fold(0.0f32, f32::max);
    println!("  last pan against a fresh render: worst {:.3e}", worst(&kept, &read(graph, &fresh)));
    let shifted = graph.materialise_cached(&image, Rect::new(r.x - 1, r.y, r.w, r.h), None).unwrap();
    println!("  twin, a pixel over: worst {:.3e}", worst(&kept, &read(graph, &shifted)));

    // The chain's stages on the same pixels: at fit the preview reduced to the region's width, each prefix made whole;
    // at 100 % the full frame's centre, the canvas's size.
    // At fit from the full frame too, as the canvas renders once the whole decode is in, at the canvas's own size.
    let fitted = (canvas.0 / f64::from(full.extent().w)).min(canvas.1 / f64::from(full.extent().h));
    let cases = [
        ("fit, from the preview", negative.preview.clone(), false, f64::from(region.w) / f64::from(w)),
        ("fit, from the full frame", full.clone(), false, fitted),
        ("100 %", full.clone(), true, 1.0),
    ];
    for (case, base, zoomed, scale) in cases {
    let (w, h) = (base.extent().w, base.extent().h);
    let shown = zoomed.then(|| Rect::new(((w as f64 - canvas.0) / 2.0) as i32, ((h as f64 - canvas.1) / 2.0) as i32, canvas.0 as u32, canvas.1 as u32));
    println!("{case}:");
    let request = Request { full_width: Some(negative.full_width), scale, ..Request::default() };
    let inputs = Inputs { seed: negative.seed, ..Inputs::default() };
    let (graph, pipeline) = e.parts();
    let input = if zoomed { Image::new(&base) } else { Image::new(&base).lanczos(scale) };
    let scan = input.extent();
    let full_scale = f64::from(scan.w) / negative.full_width;
    let framed = pipeline.framed(graph, input.clone(), &s, full_scale, &inputs).unwrap();
    let lit = match conversion::scene_white(&s) {
        Some(m) => framed.clone().then(stages::color_matrix(&m)),
        None => framed.clone(),
    };
    let digital = s.mode.is_digital();
    let effects = digital.then_some(scan);
    let mut steps: Vec<(&str, Image)> = vec![("reduced", input), ("framed and lit", lit.clone())];
    let filmed = pipeline.filmed(graph, lit.clone(), &s, effects).unwrap();
    steps.push(("film and scan", filmed.clone()));
    let grained = match effects.and_then(|scan| grain::grain(&s, scan, full_scale)).map(|k| k.seeded(negative.seed)) {
        Some(k) => grain::grained(filmed, &k),
        None => filmed,
    };
    steps.push(("grain", grained.clone()));
    steps.push(("inversion", stages::converted(grained, &conversion::stage47(&s))));
    steps.push(("finishing", pipeline.image(graph, &base, &s, &request, &inputs).unwrap()));
    if let Some(dir) = std::env::var_os("ONI_COST_DUMP") {
        let last = &steps.last().unwrap().1;
        let r = shown.unwrap_or_else(|| last.extent());
        let px = graph.render(last, r, None).unwrap();
        let bytes: Vec<u8> = px.iter().flat_map(|p| p.iter().flat_map(|v| v.to_le_bytes())).collect();
        let name = format!("{}-{}-{}x{}.f32", photo.file_name().unwrap().to_string_lossy(), case.replace([' ', ','], ""), r.w, r.h);
        std::fs::write(Path::new(&dir).join(name), bytes).unwrap();
    }
    let mut before = 0.0;
    for (name, image) in &steps {
        let t = median((0..RUNS).map(|_| ms(graph, image, shown)).collect());
        println!("  {name:>16}: {t:7.1} ms total, {:+7.1} ms", t - before);
        before = t;
    }
    }
    let st = e.stats();
    println!("graph: peak {:.0} MB, live {:.0} MB", st.peak_bytes as f64 / 1e6, st.live_bytes as f64 / 1e6);
}
