//! `dust_census <frame>…`: each frame's dust and shapes past its roll's window, its sidecar's corrections left out.

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
        settings.corrections.clear();
        let (d, _) = oni_decode::decode_for(path, 1.0, 0).expect("a decode");
        let source = graph.upload_half(d.width as u32, d.height as u32, &d.pixels).unwrap();
        drop(d);
        let framed = pipeline.framed(&mut graph, Image::new(&source), &settings, 1.0, &Inputs::default()).unwrap();
        let e = framed.extent();
        let pixels = graph.render(&framed, e, None).unwrap();
        let started = std::time::Instant::now();
        let found = read(&pixels, e.w as usize, &Limits::of(&settings));
        let dust: Vec<_> = found.dust.iter().map(|s| (s.bounds[0], s.bounds[1], s.area)).collect();
        let past: u64 = found.past.iter().map(|r| u64::from(r.end - r.start)).sum();
        println!("{}: {:.0} ms, {past} px past the window, dust {dust:?}", path.file_stem().unwrap().to_str().unwrap(), started.elapsed().as_secs_f64() * 1e3);
        graph.purge();
    }
}
