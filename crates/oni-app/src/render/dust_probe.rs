//! `ONI_DUST_PROBE=<folder>`: the canvas's renders timed on a frame holding dust patches, each drained
//! on the GPU; without the variable it returns at once. `ONI_DUST_PROBE_FILE` names the frame in the
//! folder, `ONI_DUST_PROBE_COUNTS` how many of its corrections each round lays (`0,20,164`).
use oni_pipeline::corrections::PatchKey;
use super::*;
use crate::viewport::Viewport;
use oni_settings::frame::Correction;

fn drained(engine: &Engine) {
    let (device, queue) = engine.device();
    let (tx, rx) = std::sync::mpsc::channel();
    queue.on_submitted_work_done(move || {
        let _ = tx.send(());
    });
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    let _ = rx.recv();
}

struct Sample {
    cpu: f64,
    gpu: f64,
    dispatches: u64,
    pixels: u64,
    created: u64,
}

fn timed<T>(engine: &mut Engine, run: impl FnOnce(&mut Engine) -> T) -> (T, Sample) {
    drained(engine);
    let before = engine.stats();
    let started = Instant::now();
    let out = run(engine);
    let cpu = started.elapsed().as_secs_f64() * 1000.0;
    drained(engine);
    let gpu = started.elapsed().as_secs_f64() * 1000.0;
    let after = engine.stats();
    (out, Sample { cpu, gpu, dispatches: after.dispatches - before.dispatches, pixels: after.pixels - before.pixels, created: after.textures_created - before.textures_created })
}

fn said(title: &str, samples: &[Sample]) {
    let median = |f: &dyn Fn(&Sample) -> f64| {
        let mut v: Vec<f64> = samples.iter().map(f).collect();
        v.sort_by(f64::total_cmp);
        (v[v.len() / 2], *v.last().unwrap())
    };
    let (cpu, cpu_worst) = median(&|s| s.cpu);
    let (gpu, gpu_worst) = median(&|s| s.gpu);
    let (d, _) = median(&|s| s.dispatches as f64);
    let (p, _) = median(&|s| s.pixels as f64 / 1e6);
    let (c, c_worst) = median(&|s| s.created as f64);
    println!("  {title:<34} cpu {cpu:6.1} ms (worst {cpu_worst:6.1})  drained {gpu:6.1} ms (worst {gpu_worst:6.1})  {d:5.0} dispatches  {p:6.1} Mpx  {c:3.0} textures made (worst {c_worst:.0})");
}

#[test]
fn the_canvas_s_renders_timed_on_a_frame_holding_dust_patches() {
    let Some(folder) = std::env::var_os("ONI_DUST_PROBE") else { return };
    let raw = Path::new(&folder).join(std::env::var("ONI_DUST_PROBE_FILE").unwrap_or_else(|_| "P1072734.RW2".into()));
    let loaded = oni_settings::sidecar::read(&raw, true).expect("the sidecar");
    let all = loaded.settings;
    let device = Device::open().expect("a GPU");
    let mut engine = Engine::on(&device);
    let caches = std::env::temp_dir().join(format!("oni-dust-probe-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&caches);
    engine.caches = Some(caches.clone());
    let reading = Reading::of(&all);
    let started = Instant::now();
    let negative = engine.prepare(decode(&raw, reading.clone()).expect("the preview decode")).expect("the preview");
    let full = engine.full(decode_full(&raw, reading).expect("the full decode")).expect("the full frame");
    println!("decoded and laid in {:.0} ms: preview {:?}, full {:?}, {} corrections in the sidecar", started.elapsed().as_secs_f64() * 1000.0, negative.size(), full.extent(), all.corrections.len());
    let size: (f64, f64) = std::env::var("ONI_DUST_PROBE_CANVAS").ok().and_then(|v| v.split_once('x').map(|(w, h)| (w.parse().unwrap(), h.parse().unwrap()))).unwrap_or((2440.0, 1800.0));
    let counts: Vec<usize> = std::env::var("ONI_DUST_PROBE_COUNTS").ok().map(|v| v.split(',').map(|n| n.parse().unwrap()).collect()).unwrap_or_else(|| vec![0, 20, 164, all.corrections.len()]);
    let mut healed = 0;
    for n in counts {
        let n = n.min(all.corrections.len());
        let settings = PipelineSettings { corrections: all.corrections[..n].to_vec(), ..all.clone() };
        let healing = Instant::now();
        for k in healed..n {
            let c: &Correction = &settings.corrections[k];
            let upstream = PipelineSettings { corrections: settings.corrections[..k].to_vec(), ..settings.clone() };
            let (region, frame) = engine.search_region(&full, &upstream, c).expect("the search's region");
            let patch = crate::heal::generate(&region, c, frame.0, frame.1);
            let key = PatchKey::of(&upstream, c);
            crate::patches::write(&caches, key, patch.width as u32, patch.height as u32, &patch.pixels).expect("the patch written");
            engine.forget_patch(key);
            engine.purge();
        }
        if n > healed {
            println!("{} patches healed in {:.0} ms", n - healed, healing.elapsed().as_secs_f64() * 1000.0);
        }
        healed = healed.max(n);
        println!("{n} patches, canvas {size:?}:");
        let view = |scale: Option<f64>, coarse: bool| View { viewport: Viewport { scale, offset: (0.0, 0.0) }, size, full: Some(full.clone()), coarse };
        let notch = |k: usize| {
            let mut s = settings.clone();
            s.print.exposure = 0.01 * k as f32;
            s
        };
        // Warm: every kernel compiled, the levels built.
        for k in 0..3 {
            engine.resident(&negative, &notch(100 + k), Some(&view(None, false))).expect("a render");
            engine.counted(&negative, &notch(100 + k)).expect("the counted picture");
            engine.inputs(&negative, &notch(100 + k), true);
        }
        let fit = view(None, false);
        let mut at_fit = Vec::new();
        let (mut counted, mut inputs, mut inputs_rest) = (Vec::new(), Vec::new(), Vec::new());
        for k in 0..24 {
            let s = notch(k);
            at_fit.push(timed(&mut engine, |e| e.resident(&negative, &s, Some(&fit)).map(|_| ())).1);
            counted.push(timed(&mut engine, |e| e.counted(&negative, &s).map(|_| ())).1);
            inputs.push(timed(&mut engine, |e| e.inputs(&negative, &s, true).map(|_| ())).1);
        }
        for k in 0..6 {
            inputs_rest.push(timed(&mut engine, |e| e.inputs(&negative, &notch(50 + k), false).map(|_| ())).1);
        }
        said("notch at fit", &at_fit);
        said("its histogram picture", &counted);
        said("its input readings, held", &inputs);
        said("its input readings, at rest", &inputs_rest);
        // A zoom changes no setting: the notch stays the same through it.
        let s = notch(7);
        let fitted = Viewport::default().effective_scale(negative.full(), size);
        let mut pinch = Vec::new();
        for event in 1..=60usize {
            let k = if event <= 30 { event } else { 60 - event } as f64 / 30.0;
            let v = view(Some(fitted * 3f64.powf(k)), true);
            pinch.push(timed(&mut engine, |e| e.resident(&negative, &s, Some(&v)).map(|_| ())).1);
        }
        said("pinch event, fit to 3x and back", &pinch);
        for scale in [0.25, 0.5, 1.0, 2.0, 1.0, 0.5, 0.25] {
            let v = view(Some(scale), false);
            let one = timed(&mut engine, |e| e.resident(&negative, &s, Some(&v)).map(|_| ())).1;
            said(&format!("ladder rung {scale}"), &[one]);
        }
        let back = timed(&mut engine, |e| e.resident(&negative, &s, Some(&fit)).map(|_| ())).1;
        said("back to fit", &[back]);
        let at_full = view(Some(1.0), false);
        let mut zoomed = Vec::new();
        for k in 0..24 {
            let s = notch(k);
            zoomed.push(timed(&mut engine, |e| e.resident(&negative, &s, Some(&at_full)).map(|_| ())).1);
        }
        said("notch at 100 %", &zoomed);
        // A patch landing: read again from its file, the frame's corrections laid again with it.
        let mut landings = Vec::new();
        for c in settings.corrections.iter().take(12) {
            engine.forget_patch(PatchKey::of(&settings, c));
            landings.push(timed(&mut engine, |e| e.resident(&negative, &s, Some(&fit)).map(|_| ())).1);
        }
        if !landings.is_empty() {
            said("render after a patch landed", &landings);
            // The De-dust tab's reminders, which hold the engine while they render.
            let placing = settings.geometry.placing(negative.full());
            let strokes: Vec<(Vec<(f64, f64)>, f64)> = settings.corrections.iter().map(|c| (c.stroke.iter().map(|p| placing.on_frame(*p)).map(|p| (p.x, p.y)).collect(), c.radius)).collect();
            let reminders = timed(&mut engine, |e| e.originals(&negative, &s, &strokes).map(|_| ())).1;
            said("the tab's reminders, all at once", &[reminders]);
        }
    }
    together_against_in_turn(&mut engine, &full, &all, &caches);
    let _ = std::fs::remove_dir_all(&caches);
}

/// The frame's corrections laid together by the pipeline against each laid in turn through stages of
/// its own, on a part of the real frame at full resolution and on the whole of it reduced.
fn together_against_in_turn(engine: &mut Engine, full: &Source, settings: &PipelineSettings, caches: &Path) {
    use oni_pipeline::corrections;
    use oni_pipeline::stages::{Patched, Stroke, Translate, blurred};
    let (graph, pipeline) = engine.parts();
    let held: Vec<(Correction, Source)> = (settings.corrections.iter())
        .filter_map(|c| crate::patches::read(caches, PatchKey::of(settings, c)).and_then(|(w, h, pixels)| Some((c.clone(), graph.upload(w, h, &pixels).ok()?))))
        .collect();
    let inputs = Inputs { patches: held.iter().map(|(c, p)| (c.id, p)).collect(), custom_flat_field: None };
    let bare = PipelineSettings { corrections: Vec::new(), ..settings.clone() };
    let apart = |a: &[[f32; 4]], b: &[[f32; 4]]| a.iter().zip(b).flat_map(|(p, q)| (0..4).map(move |c| (p[c] - q[c]).abs() / (1.0 + q[c].abs()))).fold(0.0f32, f32::max);
    for reduce in [1.0, 0.29] {
        let source = if reduce == 1.0 { Image::new(full) } else { Image::new(full).lanczos(reduce) };
        let scale = f64::from(source.extent().w) / f64::from(full.extent().w);
        let together = pipeline.healed(graph, source.clone(), settings, scale, &inputs).expect("the recipe builds");
        let clean = pipeline.healed(graph, source, &bare, scale, &Inputs::default()).expect("the bare frame");
        let (mut in_turn, framed) = (clean.clone(), clean.extent());
        for (c, patch) in &held {
            let (tile, shift) = corrections::placed(c, (patch.extent().w, patch.extent().h), framed, scale);
            let mut placed = Image::new(patch);
            if scale != 1.0 {
                placed = placed.lanczos_shifted(scale, shift);
            }
            let at = placed.extent();
            let placed = placed.then(Translate(tile.x - at.x, tile.y - at.y));
            let tile = placed.extent();
            let points = corrections::stroke_points(c, framed);
            let row = graph.upload(points.len() as u32, 1, &points).expect("a stroke");
            let hard = Image::new(&row).then(Stroke { k: oni_render::StrokeMask { radius: (c.radius * scale) as f32, count: points.len() }, extent: tile });
            let sigma = corrections::feather(c, scale);
            let mask = if sigma <= oni_settings::axis::pipeline::BLUR_FLOOR { hard } else { blurred(hard, sigma, true) };
            in_turn = in_turn.then_reading(Patched { patch: tile, mask: tile }, &[placed, mask]);
        }
        // At full resolution, the frame's middle half; reduced, all of it.
        let region = if reduce == 1.0 { Rect::new(framed.x + framed.w as i32 / 4, framed.y + framed.h as i32 / 4, framed.w / 2, framed.h / 2) } else { framed };
        let (ours, theirs) = (graph.render(&together, region, None).expect("together"), graph.render(&in_turn, region, None).expect("in turn"));
        let clean = graph.render(&clean, region, None).expect("the bare frame");
        println!("{} corrections at scale {scale:.3}, {region:?}: together {:.2e} from each in turn; the bare frame {:.2e} away", held.len(), apart(&ours, &theirs), apart(&ours, &clean));
    }
}
