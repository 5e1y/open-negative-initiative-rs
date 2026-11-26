//! `welcome_stills <raw> <out dir> [x y]`: the welcome's huge gift as the app renders one digital photo, each an sRGB
//! JPEG: the photo on the neutral film, its negative on Portra 800 as the scanner reads it, the inverted print, and the
//! print at 100 % about (x, y), shares of the frame, where its grain and halation show.

use oni_app::export;
use oni_app::render::{self, Device, Engine, Reading};
use oni_export::{Image, Preset, Settings};
use oni_graph::Image as Node;
use oni_pipeline::{Inputs, conversion, grain, stages};
use oni_settings::{VirtualStock, sidecar};
use std::path::Path;

/// The close-up's side, in the frame's own pixels.
const CLOSE: usize = 900;

fn written(image: &Image, settings: &Settings, path: &Path) {
    let rendered = oni_export::converted(image, settings).expect("a conversion");
    let data = oni_export::encoded(&rendered, settings, &Default::default()).expect("a JPEG");
    std::fs::write(path, data).unwrap();
    println!("{}: {} x {}", path.display(), image.width, image.height);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (raw, out) = (Path::new(&args[0]), Path::new(&args[1]));
    let at: (f64, f64) = (args.get(2).map_or(0.5, |v| v.parse().unwrap()), args.get(3).map_or(0.5, |v| v.parse().unwrap()));
    std::fs::create_dir_all(out).unwrap();
    let read = sidecar::read(raw, true).expect("a sidecar").settings;
    let web = Preset::Web.applied(&Settings::default());
    let device = Device::open().expect("a GPU");
    let engine = Engine::shared(&device);
    let portra = read.on_stock(VirtualStock::of("kodak-portra-800"));
    for (name, s) in [("film-photo", read.on_stock(VirtualStock::NEUTRAL)), ("film-print", portra.clone())] {
        let full = export::render_full_beside(&engine, export::full_decode(raw, Reading::of(&s)).unwrap(), &s).unwrap();
        if name == "film-print" {
            let (x0, y0) = (((full.width - CLOSE) as f64 * at.0) as usize, ((full.height - CLOSE) as f64 * at.1) as usize);
            let pixels = (y0..y0 + CLOSE).flat_map(|y| full.pixels[(y * full.width + x0) * 4..][..CLOSE * 4].to_vec()).collect();
            written(&Image { width: CLOSE, height: CLOSE, pixels }, &web, &out.join("film-grain.jpg"));
        }
        let scale = web.resize.scale(full.width as f64, full.height as f64);
        written(&oni_export::resized(full, scale), &web, &out.join(format!("{name}.jpg")));
    }
    // The negative: the chain to the developed and grained film, short of its inversion, at the preview's side.
    let mut e = engine.lock().unwrap();
    let negative = e.prepare(render::decode(raw, Reading::of(&portra)).expect("a decode")).unwrap();
    let (graph, pipeline) = e.parts();
    let input = Node::new(&negative.preview);
    let scan = input.extent();
    let framed = pipeline.framed(graph, input, &portra, 1.0, &Inputs::default()).unwrap();
    let region = framed.extent();
    let lit = match conversion::scene_white(&portra) {
        Some(m) => framed.then(stages::color_matrix(&m)),
        None => framed,
    };
    let film = pipeline.filmed(graph, lit, &portra, Some(scan)).unwrap();
    let film = match grain::grain(&portra, scan, 1.0) {
        Some(k) => grain::grained(film, &k),
        None => film,
    };
    let pixels: Vec<f32> = graph.render(&film, region, None).unwrap().into_iter().flatten().collect();
    written(&Image { width: region.w as usize, height: region.h as usize, pixels }, &web, &out.join("film-negative.jpg"));
}
