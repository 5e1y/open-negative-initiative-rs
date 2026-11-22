//! `frontier_compare <raw> <out> [side]`: a frame at rest on the Frontier's legacy print, the Frontier paper (a stop
//! under, at rest, a stop over) and Endura Premier, each as `w h` then f32 linear Rec. 2020 RGB.

use oni_app::render::{self, Device, Engine, Linear, Reading};
use oni_settings::tone::Print;
use oni_settings::{LegacyMachine, LegacyPrint, PaperStock, PipelineSettings, sidecar};
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
    let marked = raw.parent().and_then(oni_settings::roll_folder::roll).and_then(|r| r.stock).and_then(|k| oni_settings::dyes::dye_set(&k));
    let mut rest = PipelineSettings { dye_set: owner.film().map(|f| f.stock.dye_set()).or(marked).unwrap_or(owner.dye_set), ..owner.clone() };
    (rest.legacy_print, rest.density) = (None, 0.0);
    rest.print = Print { exposure: Print::resting_exposure(rest.mode), ..Print::default() };
    println!("{:?}, dyes {:?}, separation {}", rest.mode, rest.dye_set, rest.separation);

    let device = Device::open().expect("a GPU");
    let engine = Engine::shared(&device);
    let mut e = engine.lock().unwrap();
    let negative = e.prepare(render::decode(raw, Reading::of(&owner)).expect("a decode")).unwrap();
    let on = |paper: Option<PaperStock>, stops: f32| {
        let mut s = PipelineSettings { print_paper: paper, ..rest.clone() };
        s.print.exposure += stops;
        s
    };
    let mut today = on(None, 0.0);
    today.legacy_print = Some(LegacyPrint { machine: LegacyMachine::Frontier, ..LegacyPrint::default() });
    let frontier = Some(PaperStock::FujifilmFrontierSp3000);
    let renders = [
        ("scanner-frontier", today),
        ("frontier", on(frontier, 0.0)),
        ("frontier-under", on(frontier, -1.0)),
        ("frontier-over", on(frontier, 1.0)),
        ("endura", on(Some(PaperStock::KodakEnduraPremier), 0.0)),
    ];
    for (name, s) in &renders {
        if let Some(p) = oni_pipeline::conversion::printing(s) {
            println!("{name}: filtration {:?} CC", p.filtration);
        }
        write(out, name, &e.render(&negative, s, Some(side)).unwrap());
    }
}
