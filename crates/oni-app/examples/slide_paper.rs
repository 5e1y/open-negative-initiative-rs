//! `slide_paper <raw> <out> [side]`: a slide as edited on ONI's print, then through its internegative on every paper at
//! rest and on Endura Premier with Print lift, each as `w h` then f32 linear Rec. 2020 RGB.

use oni_app::render::{self, Device, Engine, Linear, Reading};
use oni_settings::{PaperStock, PipelineSettings, sidecar};
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

fn key(paper: PaperStock) -> String {
    serde_json::to_string(&paper).unwrap().trim_matches('"').to_owned()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (raw, out) = (Path::new(&args[0]), Path::new(&args[1]));
    let side: usize = args.get(2).map_or(1600, |s| s.parse().unwrap());
    std::fs::create_dir_all(out).unwrap();
    // Read through the migration, as the editor opens it; the sidecar is never written.
    let owner = sidecar::read(raw, true).expect("a sidecar").settings;
    println!("{:?}, gains {:?}, paper {:?}", owner.mode, owner.gains.stops, owner.print_paper);
    let device = Device::open().expect("a GPU");
    let engine = Engine::shared(&device);
    let mut e = engine.lock().unwrap();
    let slide = e.prepare(render::decode(raw, Reading::of(&owner)).expect("a decode")).unwrap();
    write(out, "oni", &e.render(&slide, &PipelineSettings { print_paper: None, ..owner.clone() }, Some(side)).unwrap());
    for paper in PaperStock::ALL {
        let mut s = PipelineSettings { print_paper: Some(paper), ..owner.clone() };
        s.print.print_lift = false;
        if let Some(p) = oni_pipeline::conversion::printing(&s) {
            println!("{}: filtration {:?} CC", key(paper), p.filtration);
        }
        write(out, &key(paper), &e.render(&slide, &s, Some(side)).unwrap());
    }
    let mut blacks = PipelineSettings { print_paper: Some(PaperStock::KodakEnduraPremier), ..owner.clone() };
    blacks.print.print_lift = true;
    write(out, "kodak-endura-premier-print-lift", &e.render(&slide, &blacks, Some(side)).unwrap());
}
