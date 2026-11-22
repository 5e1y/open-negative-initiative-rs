//! `enlarger_states <raw> <out> [side]`: a photo as edited, rendered headless in the enlarger's three states: on ONI's
//! print (`oni`), through the virtual enlarger on its paper (`paper`) and on the neutral paper (`neutral`); each as `w h`
//! then f32 RGB, linear Rec. 2020, with the enlarger's filtration printed.

use oni_app::render::{self, Device, Engine, Linear, Reading};
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
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (raw, out) = (Path::new(&args[0]), Path::new(&args[1]));
    let side: usize = args.get(2).map_or(1200, |s| s.parse().unwrap());
    std::fs::create_dir_all(out).unwrap();
    // Read through the migration, as the editor opens it; the sidecar is never written.
    let owner = sidecar::read(raw, true).expect("a sidecar").settings;
    let device = Device::open().expect("a GPU");
    let engine = Engine::shared(&device);
    let mut e = engine.lock().unwrap();
    let negative = e.prepare(render::decode(raw, Reading::of(&owner)).expect("a decode")).unwrap();
    let paper = PipelineSettings { print_paper: Some(owner.fitting_paper()), paperless: false, ..owner.clone() };
    let states = [
        ("oni", PipelineSettings { print_paper: None, ..owner.clone() }),
        ("neutral", PipelineSettings { paperless: true, ..paper.clone() }),
        ("paper", paper),
    ];
    println!("{:?} on {:?}, as edited {:?}", owner.mode, owner.fitting_paper(), owner.print_paper);
    for (name, s) in &states {
        if let Some(p) = oni_pipeline::conversion::printing(s) {
            println!("{name}: filtration {:?} CC, time {:.4}", p.filtration.map(|v| (v * 10.0).round() / 10.0), p.paper.scale.log10());
        }
        write(out, name, &e.render(&negative, s, Some(side)).unwrap());
    }
}
