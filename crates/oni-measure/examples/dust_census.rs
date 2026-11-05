//! `dust_census <frame>…`: each frame's dust at five sensitivities, and how many of its sidecar's strokes it falls on.

use oni_graph::{Graph, Image};
use oni_measure::dust::{Limits, read};
use oni_pipeline::{Inputs, Pipeline};
use oni_settings::sidecar;
use std::path::Path;

fn main() {
    let mut graph = Graph::new().expect("a GPU");
    let mut pipeline = Pipeline::new();
    for name in std::env::args().skip(1) {
        let path = Path::new(&name);
        let mut settings = sidecar::read(path, true, false).expect("a sidecar").settings;
        let painted = std::mem::take(&mut settings.corrections);
        let (d, _) = oni_decode::decode_for(path, 1.0, 0).expect("a decode");
        let source = graph.upload_half(d.width as u32, d.height as u32, &d.pixels).unwrap();
        drop(d);
        let framed = pipeline.framed(&mut graph, Image::new(&source), &settings, 1.0, &Inputs::default()).unwrap();
        let e = framed.extent();
        let pixels = graph.render(&framed, e, None).unwrap();
        let started = std::time::Instant::now();
        let reading = read(&pixels, e.w as usize, &Limits::of(&settings), 3072);
        let took = started.elapsed();
        let cells: Vec<String> = [0.0, 0.25, 0.5, 0.75, 1.0]
            .iter()
            .map(|&s| {
                let found = reading.at(s);
                let on_stroke = |c: &oni_settings::frame::Correction| {
                    let (cx, cy) = (c.stroke.iter().map(|p| p.x).sum::<f64>() / c.stroke.len() as f64 * f64::from(e.w), c.stroke.iter().map(|p| p.y).sum::<f64>() / c.stroke.len() as f64 * f64::from(e.h));
                    found.iter().any(|d| f64::from(d.bounds[0]) < cx + c.radius + 30.0 && f64::from(d.bounds[2]) > cx - c.radius - 30.0 && f64::from(d.bounds[1]) < cy + c.radius + 30.0 && f64::from(d.bounds[3]) > cy - c.radius - 30.0)
                };
                format!("{:.0}%: {} ({}/{} strokes)", s * 100.0, found.len(), painted.iter().filter(|c| on_stroke(c)).count(), painted.len())
            })
            .collect();
        println!("{}: read {:.0} ms, {} px past the white; {}", path.file_stem().unwrap().to_str().unwrap(), took.as_secs_f64() * 1e3, reading.past, cells.join("  "));
        graph.purge();
    }
}
