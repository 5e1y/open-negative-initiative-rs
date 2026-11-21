//! `heal_probe <frame> <out> [<x0> <y0> <x1> <y1>]…`: a frame's dust ringed at `ONI_DUST_SENSITIVITY`, or the strokes of `ONI_DUST_STROKES`
//! (JSON, frame pixels; whole under `ONI_DUST_WHOLE`), healed as the app heals, read again, each window written before and after.

use oni_app::render::{self, Device, Engine, Reading};
use oni_measure::dust::{self, Limits};
use oni_settings::cg::Point;
use oni_settings::frame::Correction;
use oni_settings::id::Id;
use oni_settings::sidecar;
use std::collections::HashSet;
use std::io::Write;
use std::path::Path;

fn view(pixels: &[[f32; 4]], width: usize, limits: &Limits, windows: &[oni_settings::levels::Levels; 3], rect: [usize; 4], out: &Path) {
    let (w, h) = (rect[2] - rect[0], rect[3] - rect[1]);
    let mut f = std::io::BufWriter::new(std::fs::File::create(out).unwrap());
    write!(f, "P6\n{w} {h}\n255\n").unwrap();
    for y in rect[1]..rect[3] {
        for x in rect[0]..rect[2] {
            let p = &pixels[y * width + x];
            let rgb: Vec<u8> = (0..3).map(|c| ((((limits.density(p, c) - windows[c].black) / (windows[c].white - windows[c].black)).clamp(0.0, 1.0)).powf(1.0 / 2.2) * 255.0).round() as u8).collect();
            f.write_all(&rgb).unwrap();
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (path, out) = (Path::new(&args[0]), Path::new(&args[1]));
    let rects: Vec<[usize; 4]> = args[2..].chunks(4).map(|c| [0, 1, 2, 3].map(|i| c[i].parse().unwrap())).collect();
    std::fs::create_dir_all(out).unwrap();
    let caches = out.join("caches");
    let _ = std::fs::remove_dir_all(&caches);
    let mut settings = sidecar::read(path, true).expect("a sidecar").settings;
    settings.corrections.clear();
    let device = Device::open().expect("a GPU");
    let engine = Engine::shared(&device);
    engine.lock().unwrap().caches = Some(caches.clone());
    let full = engine.full(render::decode_full(path, Reading::of(&settings)).expect("a decode")).unwrap();
    let scan = full.extent();
    let placing = settings.placing((f64::from(scan.w), f64::from(scan.h)));
    let limits = Limits::of(&settings);
    let on = settings.roll_reference.as_ref().map(|r| oni_core::balance::written(r, &settings));
    let on = on.as_ref().unwrap_or(&settings).clone();
    let windows = [on.levels.red, on.levels.green, on.levels.blue];
    let stem = path.file_stem().unwrap().to_str().unwrap().to_string();

    let (width, before) = engine.lock().unwrap().framed_whole(&full, &settings).unwrap();
    let height = before.len() / width;
    let sensitivity: f32 = std::env::var("ONI_DUST_SENSITIVITY").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let reading = dust::read(&before, width, &limits, 3072);
    let found = reading.at(sensitivity);
    println!("{stem}: {width} x {height}, {} rings at {sensitivity}", found.len());
    let corrections: Vec<Correction> = match std::env::var_os("ONI_DUST_STROKES") {
        Some(file) => {
            let list: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(file).unwrap()).unwrap();
            list.iter()
                .map(|v| {
                    let stroke = v["stroke"].as_array().unwrap().iter().map(|p| placing.on_scan(Point::new(p[0].as_f64().unwrap() / width as f64, p[1].as_f64().unwrap() / height as f64))).collect();
                    Correction { id: Id::new(), stroke, radius: v["radius"].as_f64().unwrap(), gesture: None }
                })
                .flat_map(|c| if std::env::var_os("ONI_DUST_WHOLE").is_some() { vec![c] } else { c.split((f64::from(scan.w), f64::from(scan.h))) })
                .collect()
        }
        None => {
            let strokes: Vec<_> = found.iter().flat_map(|s| dust::strokes(s, width, height)).collect();
            let gesture = (strokes.len() > 1).then(Id::new);
            strokes.into_iter().map(|(points, radius)| Correction { id: Id::new(), stroke: points.into_iter().map(|(x, y)| placing.on_scan(Point { x, y })).collect(), radius, gesture }).collect()
        }
    };
    let within = |x: f64, y: f64| rects.iter().any(|r| (r[0] as f64..r[2] as f64).contains(&x) && (r[1] as f64..r[3] as f64).contains(&y));
    let in_rects = |c: &Correction| rects.is_empty() || c.stroke.iter().map(|p| placing.on_frame(*p)).any(|p| within(p.x * width as f64, p.y * height as f64));
    settings.corrections = corrections.into_iter().filter(in_rects).collect();
    println!("  {} corrections", settings.corrections.len());
    for c in &settings.corrections {
        let on = |p: &Point| placing.on_frame(*p);
        let length: f64 = c.stroke.windows(2).map(|w| ((on(&w[1]).x - on(&w[0]).x) * width as f64).hypot((on(&w[1]).y - on(&w[0]).y) * height as f64)).sum();
        let first = on(&c.stroke[0]);
        println!("    at {:.0}, {:.0}: {} points, {length:.0} px long, radius {:.1}", first.x * width as f64, first.y * height as f64, c.stroke.len(), c.radius);
    }
    let started = std::time::Instant::now();
    let mut skip = HashSet::new();
    let mut latest = Some(settings.clone());
    let mut made = 0;
    oni_app::heal::ensure(&engine, &full, &caches, &mut skip, || latest.clone(), |tried| match tried {
        Ok((_, fallback, offset)) => {
            made += 1;
            println!("    patch from {:.0}, {:.0} px away{}", offset.0 * f64::from(scan.w), offset.1 * f64::from(scan.h), if fallback { " (fallback)" } else { "" });
        }
        Err((_, e)) => println!("    no patch: {e}"),
    });
    latest.take();
    println!("  {made} patches in {:.0} ms", started.elapsed().as_secs_f64() * 1e3);
    let (_, after) = engine.lock().unwrap().framed_whole(&full, &settings).unwrap();
    // What the first reading ringed in the windows, read again on the healed frame.
    let again = dust::read(&after, width, &limits, 3072);
    let holds = |r: &[usize; 4], b: [usize; 4]| b[0] >= r[0] && b[2] <= r[2] && b[1] >= r[1] && b[3] <= r[3];
    let inside = |s: &dust::Speck| rects.is_empty() || rects.iter().any(|r| holds(r, s.bounds.map(|v| v as usize)));
    for s in [0.0, 0.5, 1.0] {
        println!("  rings in the windows at {s}: {} before, {} after", reading.at(s).iter().filter(|s| inside(s)).count(), again.at(s).iter().filter(|s| inside(s)).count());
    }
    for (k, r) in rects.iter().enumerate() {
        view(&before, width, &limits, &windows, *r, &out.join(format!("{stem}-{k}-before.ppm")));
        view(&after, width, &limits, &windows, *r, &out.join(format!("{stem}-{k}-after.ppm")));
        // The healed pixels' rise over what they were: how much of the dust's density is left.
        let of = |s: &dust::Speck| s.runs.iter().flat_map(|run| (run.start..run.end).map(move |x| run.row as usize * width + x as usize)).collect::<Vec<_>>();
        let cells = found.iter().filter(|s| inside(s)).flat_map(|s| of(s)).filter(|i| (r[0]..r[2]).contains(&(i % width)) && (r[1]..r[3]).contains(&(i / width)));
        let (mut n, mut was, mut is) = (0u32, 0.0f64, 0.0f64);
        for i in cells {
            n += 1;
            was += f64::from(limits.density(&before[i], 1));
            is += f64::from(limits.density(&after[i], 1));
        }
        if n > 0 {
            println!("  window {k}: {n} dust pixels, green density {:.3} before, {:.3} after", was / f64::from(n), is / f64::from(n));
        }
    }
}
