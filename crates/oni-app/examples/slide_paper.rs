//! `slide_paper <raw> <out> [side]`: a slide as edited on ONI's print, then on Kodak Ektachrome Radiance III at rest
//! and on pure black, each as `w h` then f32 linear Rec. 2020 RGB.

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
    let negative = e.prepare(render::decode(raw, Reading::of(&owner)).expect("a decode")).unwrap();
    let mut paper = PipelineSettings { print_paper: Some(PaperStock::KodakEktachromeRadianceIII), ..owner.clone() };
    paper.print.paper_blacks = true;
    let mut pure = paper.clone();
    pure.print.paper_blacks = false;
    for (name, s) in [("oni", &owner), ("radiance", &paper), ("radiance-pure-black", &pure)] {
        if let Some(p) = oni_pipeline::conversion::printing(s) {
            println!("{name}: filtration {:?} CC", p.filtration);
        }
        write(out, name, &e.render(&negative, s, Some(side)).unwrap());
    }
}
