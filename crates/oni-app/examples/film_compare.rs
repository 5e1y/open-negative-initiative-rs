//! `film_compare <raw> <out> [side]`: a digital photo rendered headless as the editor renders it, on its virtual
//! Portra 800, with the scene the film reads and the film's scan; then the same photo with its negative's dye
//! separation, its chroma decompression or both at rest, to weigh each; then on its stock's measured dyes, at each dose
//! `ONI_COMPARE_DOSES` lists (comma-separated, the photo's own by default), a scan's on its roll's stock. Each as `w h` then f32 RGB, linear Rec. 2020.

use oni_app::render::{self, Device, Engine, Linear, Reading};
use oni_graph::Image;
use oni_pipeline::{Inputs, conversion, stages};
use oni_settings::{PipelineSettings, sidecar};
use std::io::Write;
use std::path::Path;

fn write(out: &Path, name: &str, l: &Linear) {
    let mut f = std::io::BufWriter::new(std::fs::File::create(out.join(format!("{name}.f32"))).unwrap());
    writeln!(f, "{} {}", l.width, l.height).unwrap();
    for p in &l.pixels {
        for v in &p[..3] {
            f.write_all(&v.to_le_bytes()).unwrap();
        }
    }
    println!("{name}: {} x {}", l.width, l.height);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (raw, out) = (Path::new(&args[0]), Path::new(&args[1]));
    let side: usize = args.get(2).map_or(1600, |s| s.parse().unwrap());
    std::fs::create_dir_all(out).unwrap();
    // Read through the migration, as the editor opens it; the original's sidecar is never written.
    let owner = sidecar::read(raw, true).expect("a sidecar").settings;
    std::fs::write(out.join("settings-owner.json"), serde_json::to_string_pretty(&owner).unwrap()).unwrap();
    println!("film {:?}, gains {:?}, separation {} {:?}, decompression {}", owner.film(), owner.gains.stops, owner.separation, owner.dye_set, owner.chroma_decompression);

    let device = Device::open().expect("a GPU");
    let engine = Engine::shared(&device);
    let mut e = engine.lock().unwrap();
    let negative = e.prepare(render::decode(raw, Reading::of(&owner)).expect("a decode")).unwrap();
    let (w, h) = negative.size();
    let scale = (side as f64 / f64::from(w.max(h))).min(1.0);
    println!("preview {w} x {h}, scale {scale:.4}, full width {}", negative.full_width);
    let unmixed = PipelineSettings { separation: 0.0, ..owner.clone() };
    let undecompressed = PipelineSettings { chroma_decompression: 1.0, ..owner.clone() };
    let neither = PipelineSettings { separation: 0.0, chroma_decompression: 1.0, ..owner.clone() };
    // The measured dyes a switch lays: a digital photo's film's, else its roll marker's stock's.
    let marked = raw.parent().and_then(oni_settings::roll_folder::roll).and_then(|r| r.stock).and_then(|k| oni_settings::dyes::dye_set(&k));
    let dyed = PipelineSettings { dye_set: owner.film().map(|f| f.stock.dye_set()).or(marked).unwrap_or(owner.dye_set), ..owner.clone() };
    println!("measured dyes {:?}", dyed.dye_set);
    let doses = std::env::var("ONI_COMPARE_DOSES").unwrap_or_else(|_| owner.separation.to_string());
    let on_dyes: Vec<(String, PipelineSettings)> =
        doses.split(',').map(|d| (format!("dyes-{d}"), PipelineSettings { separation: d.parse().unwrap(), ..dyed.clone() })).collect();
    let plain = [("rest", &owner), ("nosep", &unmixed), ("nodecomp", &undecompressed), ("neither", &neither)].map(|(t, s)| (t.to_string(), s.clone()));
    for (tag, s) in plain.iter().chain(&on_dyes) {
        let (tag, s) = (tag.as_str(), s);
        write(out, &format!("B-{tag}"), &e.render(&negative, s, Some(side)).unwrap());
        // The chain's own steps up to the film: framed, the scene lit at the film's white, then its scan.
        let (graph, pipeline) = e.parts();
        let image = Image::new(&negative.preview).lanczos(scale);
        let framed = pipeline.framed(graph, image, s, scale, &Inputs::default()).unwrap();
        let scene = match conversion::scene_white(s) {
            Some(m) => framed.then(stages::color_matrix(&m)),
            None => framed,
        };
        let scan = pipeline.filmed(graph, scene.clone(), s).unwrap();
        for (name, image) in [("scene", scene), ("scan", scan)] {
            let region = image.extent();
            let pixels = graph.render(&image, region, None).unwrap();
            write(out, &format!("{name}-{tag}"), &Linear { width: region.w as usize, height: region.h as usize, pixels });
        }
    }
}
