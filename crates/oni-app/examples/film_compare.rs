//! `film_compare <raw> <out> [side]`: a digital photo rendered headless as the editor renders it, plain and on its
//! virtual Portra 800, with the scene the film reads and the film's scan; each as `w h` then f32 RGB, linear Rec. 2020.

use oni_app::render::{self, Device, Engine, Linear, Reading};
use oni_graph::Image;
use oni_pipeline::{Inputs, conversion, film, stages};
use oni_settings::{PipelineSettings, VirtualStock, sidecar};
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
    let owner = sidecar::read(raw, true).expect("a sidecar").settings;
    let shot = oni_decode::as_shot(raw);
    println!("as shot: {shot:?}; the sidecar's white: {:?}", owner.white);
    let mut rest = owner.clone();
    (rest.white, rest.shot) = (shot, shot);
    std::fs::write(out.join("settings-owner.json"), serde_json::to_string_pretty(&owner).unwrap()).unwrap();
    std::fs::write(out.join("settings-rest.json"), serde_json::to_string_pretty(&PipelineSettings::rest(owner.mode, owner.decoded_from_mosaic)).unwrap()).unwrap();
    let reference = film::stock(VirtualStock::KodakPortra800).reference.clone();
    println!("reference: {reference:?}");

    let device = Device::open().expect("a GPU");
    let engine = Engine::shared(&device);
    let mut e = engine.lock().unwrap();
    let negative = e.prepare(render::decode(raw, Reading::of(&owner)).expect("a decode")).unwrap();
    let (w, h) = negative.size();
    let scale = (side as f64 / f64::from(w.max(h))).min(1.0);
    println!("preview {w} x {h}, scale {scale:.4}, full width {}", negative.full_width);
    // The film's own reference illuminant, the profile's daylight: the scene white a film locks.
    let mut d55 = owner.clone();
    d55.white = Some(oni_settings::white::of_uv(oni_settings::white::uv_of_xy(0.33242, 0.34743)));
    println!("d55 white: {:?}", d55.white);
    for (tag, plain) in [("rest", &rest), ("owner", &owner), ("d55", &d55)] {
        let mut filmed = plain.clone();
        filmed.virtual_film = Some(reference.clone());
        write(out, &format!("A-{tag}"), &e.render(&negative, plain, Some(side)).unwrap());
        write(out, &format!("B-{tag}"), &e.render(&negative, &filmed, Some(side)).unwrap());
        // The chain's own steps up to the film: framed, Digital's white, then the virtual negative's scan.
        let (graph, pipeline) = e.parts();
        let image = Image::new(&negative.preview).lanczos(scale);
        let framed = pipeline.framed(graph, image, &filmed, scale, &Inputs::default()).unwrap();
        let scene = match conversion::white(&filmed) {
            Some(m) => framed.then(stages::color_matrix(&m)),
            None => framed,
        };
        let scan = pipeline.filmed(graph, scene.clone(), &filmed).unwrap();
        for (name, image) in [("scene", scene), ("scan", scan)] {
            let region = image.extent();
            let pixels = graph.render(&image, region, None).unwrap();
            write(out, &format!("{name}-{tag}"), &Linear { width: region.w as usize, height: region.h as usize, pixels });
        }
    }
}
