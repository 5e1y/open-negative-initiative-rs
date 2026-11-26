//! `export_cost <photo> [runs]`: a photo's full-definition export timed stage by stage, as the export writes it: the
//! whole decode, its render read back, the resize and paper, the conversion, the encoding. Nothing is written.

use oni_app::export;
use oni_app::render::{Device, Engine, Reading};
use oni_export::Settings;
use oni_settings::sidecar;
use std::path::Path;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let photo = Path::new(&args[0]);
    let runs: usize = args.get(1).map_or(3, |v| v.parse().unwrap());
    let s = sidecar::read(photo, true).expect("a sidecar").settings;
    let device = Device::open().expect("a GPU");
    let engine = Engine::shared(&device);
    let settings = Settings::default();
    let line = export::opening_line(photo, None, "", "", &Default::default());
    let names = ["decode", "upload, render, readback", "resize and paper", "convert", "encode"];
    let mut sums = [0.0f64; 5];
    for run in 0..runs {
        let mut at = Instant::now();
        let mut laps = [0.0f64; 5];
        let mut lap = |i: usize, at: &mut Instant| {
            laps[i] = at.elapsed().as_secs_f64() * 1000.0;
            *at = Instant::now();
        };
        let decoded = export::full_decode(photo, Reading::of(&s)).unwrap();
        lap(0, &mut at);
        let full = export::render_full_beside(&engine, decoded, &s).unwrap();
        lap(1, &mut at);
        let scale = settings.resize.scale(full.width as f64, full.height as f64);
        let mut out = oni_export::resized(full, scale);
        oni_colour::paper::laid(&s.paper_correction, &mut out.pixels, settings.space());
        lap(2, &mut at);
        let rendered = oni_export::converted(&out, &settings).unwrap();
        drop(out);
        lap(3, &mut at);
        let data = oni_export::encoded(&rendered, &settings, &line.row.metadata).unwrap();
        lap(4, &mut at);
        println!("run {run}: {} bytes; {}", data.len(), names.iter().zip(laps).map(|(n, t)| format!("{n} {t:.0}")).collect::<Vec<_>>().join(", "));
        // The first run pays the pipelines' compilation and the tables' baking.
        if run > 0 {
            for (s, t) in sums.iter_mut().zip(laps) {
                *s += t;
            }
        }
    }
    let n = (runs - 1).max(1) as f64;
    for (name, sum) in names.iter().zip(sums) {
        println!("  {name:<26} {:>7.0} ms", sum / n);
    }
    println!("  {:<26} {:>7.0} ms", "sum", sums.iter().sum::<f64>() / n);
}
