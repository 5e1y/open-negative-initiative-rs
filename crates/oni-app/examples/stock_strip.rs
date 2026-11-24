//! `stock_strip <raw> <out> [side] [stock profile,...]`: a digital photo rendered headless at its rest on each virtual
//! stock, as the editor renders it: on ONI Standard (`paper-<stock>`) and on no paper (`oni-<stock>`), and the
//! scene each film reads (`scene-<stock>`). `ONI_FILM_EXPOSURE` sets the virtual film's exposure, `ONI_PRINT_EXPOSURE`
//! moves Lighting's Exposure, `ONI_PAPER` names the paper by its key. Each as `w h` then f32 RGB, linear Rec. 2020.

use oni_app::render::{self, Device, Engine, Linear, Reading};
use oni_graph::Image;
use oni_pipeline::{Inputs, conversion, stages};
use oni_settings::{ConversionMode, PipelineSettings, VirtualStock};
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
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (raw, out) = (Path::new(&args[0]), Path::new(&args[1]));
    let side: usize = args.get(2).map_or(1200, |s| s.parse().unwrap());
    let wanted: Option<Vec<String>> = args.get(3).map(|s| s.split(',').map(str::to_string).collect());
    let film_stops: f32 = std::env::var("ONI_FILM_EXPOSURE").map_or(0.0, |v| v.parse().unwrap());
    let print_stops: f32 = std::env::var("ONI_PRINT_EXPOSURE").map_or(0.0, |v| v.parse().unwrap());
    let on_paper: Option<oni_settings::PaperStock> = std::env::var("ONI_PAPER").ok().map(|k| serde_json::from_str(&format!("\"{k}\"")).expect("a paper's key"));
    std::fs::create_dir_all(out).unwrap();
    // The photo's own framing and corrections, its colour at the digital rest.
    let owner = oni_settings::sidecar::read(raw, true).map(|l| l.settings).unwrap_or_else(|_| PipelineSettings::rest(ConversionMode::Digital, true));
    let rest = PipelineSettings { geometry: owner.geometry, ..PipelineSettings::rest(ConversionMode::Digital, owner.decoded_from_mosaic) };
    let device = Device::open().expect("a GPU");
    let engine = Engine::shared(&device);
    let mut e = engine.lock().unwrap();
    let negative = e.prepare(render::decode(raw, Reading::of(&rest)).expect("a decode")).unwrap();
    let (w, h) = negative.size();
    let scale = (side as f64 / f64::from(w.max(h))).min(1.0);
    // The photo as its sidecar holds it, then the same edit placed again on its own film's reference.
    if let Some(film) = owner.film() {
        let placed = owner.on_stock(film.stock);
        println!("own film {:?}: gains {:?} levels {:?} dyes {:?}", film.stock, owner.gains.stops, owner.levels.windows().map(|l| (l.black, l.white)), owner.dye_set);
        println!("its reference: gains {:?} levels {:?} dyes {:?}", placed.gains.stops, placed.levels.windows().map(|l| (l.black, l.white)), placed.dye_set);
        write(out, "owner", &e.render(&negative, &owner, Some(side)).unwrap());
        write(out, "placed", &e.render(&negative, &placed, Some(side)).unwrap());
    }
    for stock in VirtualStock::sheets() {
        if wanted.as_ref().is_some_and(|w| !w.iter().any(|n| n == stock.key())) {
            continue;
        }
        let mut paper = rest.on_stock(stock);
        if let Some(f) = paper.virtual_film.as_mut() {
            f.exposure = film_stops;
        }
        paper.print.exposure += print_stops;
        paper.print_paper = Some(on_paper.unwrap_or_default());
        let oni = PipelineSettings { print_paper: None, ..paper.clone() };
        let name = stock.key().replace('/', "-");
        write(out, &format!("paper-{name}"), &e.render(&negative, &paper, Some(side)).unwrap());
        write(out, &format!("oni-{name}"), &e.render(&negative, &oni, Some(side)).unwrap());
        let (graph, pipeline) = e.parts();
        let framed = pipeline.framed(graph, Image::new(&negative.preview).lanczos(scale), &paper, scale, &Inputs::default()).unwrap();
        let scene = match conversion::scene_white(&paper) {
            Some(m) => framed.then(stages::color_matrix(&m)),
            None => framed,
        };
        let region = scene.extent();
        let pixels = graph.render(&scene, region, None).unwrap();
        write(out, &format!("scene-{name}"), &Linear { width: region.w as usize, height: region.h as usize, pixels });
        println!("{name}");
    }
}
