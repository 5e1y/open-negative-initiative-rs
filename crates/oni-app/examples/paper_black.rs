//! `paper_black <out> <raw>...`: each photo printed on its paper (the default one where it has none) as the editor renders
//! it, its pick (Prepare the Roll's black) printed as it stands, at Lighting's rest and with Paper blacks on and off, then
//! strips of Exposure, Contrast, Temperature and Brightness either way, Paper blacks on and off, each as `w h` then f32
//! RGB, linear Rec. 2020. The originals and their sidecars are only read.

use oni_app::render::{self, Device, Engine, Linear, Reading};
use oni_pipeline::conversion;
use oni_render::cpu;
use oni_settings::tone::Print;
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

/// The pick, the windows' black, printed through `s`.
fn pick(s: &PipelineSettings) -> [f32; 3] {
    let k = conversion::stage47(s);
    let black = s.levels.windows().map(|w| w.black);
    let light: [f32; 3] = std::array::from_fn(|c| 10f32.powf(-black[c]) / k.gain[c] - k.pedestal);
    let v = cpu::stage47([light[0], light[1], light[2], 1.0], &k);
    [v[0], v[1], v[2]]
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = Path::new(&args[0]);
    std::fs::create_dir_all(out).unwrap();
    let device = Device::open().expect("a GPU");
    let engine = Engine::shared(&device);
    let mut e = engine.lock().unwrap();
    for raw in args[1..].iter().map(Path::new) {
        let name = raw.file_stem().unwrap().to_string_lossy().to_string();
        let read = sidecar::read(raw, true).expect("a sidecar").settings;
        let owner = PipelineSettings { print_paper: read.print_paper.or(Some(Default::default())), ..read };
        let rest = PipelineSettings { print: Print { exposure: Print::resting_exposure(owner.mode), ..Print::default() }, ..owner.clone() };
        let with = |s: &PipelineSettings, on: bool| PipelineSettings { print: Print { paper_blacks: on, ..s.print }, ..s.clone() };
        let (stops, print) = (owner.print.exposure - rest.print.exposure, &owner.print);
        println!("{name}: paper {:?}, Exposure {stops:+.3} from the rest, Brightness {:+.3}, Paper blacks {}", owner.print_paper, print.brightness, print.paper_blacks);
        for (label, s) in [("rest", &rest), ("its Lighting", &owner)] {
            let (on, off) = (pick(&with(s, true)), pick(&with(s, false)));
            println!("  {label}: the pick prints at {on:.6?}, on pure black {off:.6?}");
        }
        let negative = e.prepare(render::decode(raw, Reading::of(&owner)).expect("a decode")).unwrap();
        let side = Some(900);
        let strips: [(&str, [f32; 3], fn(&mut PipelineSettings, f32)); 4] = [
            ("exposure", [-1.0, 0.0, 1.0], |s, v| s.print.exposure += v),
            ("contrast", [-0.3, 0.0, 0.3], |s, v| s.print.contrast += v),
            ("temperature", [-0.5, 0.0, 0.5], |s, v| {
                let f = s.print.filters();
                s.print.set_filters(std::array::from_fn(|c| f[c] + v * Print::TEMPERATURE_AXIS[c]));
            }),
            ("brightness", [-1.0, 0.0, 1.0], |s, v| s.print.brightness += v),
        ];
        for (strip, travels, f) in strips {
            for (i, v) in travels.into_iter().enumerate() {
                let mut moved = with(&owner, true);
                f(&mut moved, v);
                write(out, &format!("{name}-{strip}-{i}"), &e.render(&negative, &moved, side).unwrap());
                println!("  {strip} {v:+}: the pick prints at {:.6?}", pick(&moved));
            }
        }
        for (i, on) in [true, false].into_iter().enumerate() {
            write(out, &format!("{name}-paperblacks-{i}"), &e.render(&negative, &with(&owner, on), side).unwrap());
        }
    }
}
