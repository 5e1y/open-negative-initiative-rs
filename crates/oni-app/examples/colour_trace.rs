//! `colour_trace <settings.json> <in.f32> <out> <scene|scan> [rederive]` prints a virtual Portra 800 negative on the CPU
//! references, each stage as `w h` then f32 RGB; `rederive` reads the film's windows off the first two pixels (bare, grey).
//! In place of a file, `rest:<dose>` is a digital photo at rest, its stock's measured dyes unmixed at that dose, printed on
//! its paper; `rest:<dose>:oni` the same on ONI's own print, `rest:<dose>:<paper key>` on another paper.

use oni_pipeline::{conversion, film};
use oni_render::cpu;
use oni_settings::{PipelineSettings, VirtualFilm, VirtualStock};
use std::io::{Read, Write};
use std::path::Path;

fn read(p: &Path) -> (usize, usize, Vec<[f32; 4]>) {
    let mut bytes = Vec::new();
    std::fs::File::open(p).unwrap().read_to_end(&mut bytes).unwrap();
    let nl = bytes.iter().position(|b| *b == b'\n').unwrap();
    let head: Vec<usize> = std::str::from_utf8(&bytes[..nl]).unwrap().split_whitespace().map(|s| s.parse().unwrap()).collect();
    let body = &bytes[nl + 1..];
    let px = body.chunks_exact(12).map(|c| {
        let f = |i: usize| f32::from_le_bytes(c[i * 4..i * 4 + 4].try_into().unwrap());
        [f(0), f(1), f(2), 1.0]
    });
    (head[0], head[1], px.collect())
}

fn write(out: &Path, name: &str, w: usize, h: usize, px: &[[f32; 4]]) {
    let mut f = std::io::BufWriter::new(std::fs::File::create(out.join(format!("{name}.f32"))).unwrap());
    writeln!(f, "{w} {h}").unwrap();
    for p in px {
        for v in &p[..3] {
            f.write_all(&v.to_le_bytes()).unwrap();
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args[1] == "params" {
        // A real frame's own conversion, read from its sidecar: the kernel's arguments alone.
        let s = oni_settings::sidecar::read(Path::new(&args[0]), true).expect("a sidecar").settings;
        println!("mode {:?}\nstage47: {:?}", s.mode, conversion::stage47(&s));
        return;
    }
    let rest = PipelineSettings::rest(oni_settings::ConversionMode::Digital, true);
    let mut s: PipelineSettings = match args[0].strip_prefix("rest:") {
        Some(spec) => {
            let mut parts = spec.split(':');
            let dose = parts.next().unwrap().parse().unwrap();
            let print_paper = match parts.next() {
                None => rest.print_paper,
                Some("oni") => None,
                Some(key) => Some(serde_json::from_str(&format!("\"{key}\"")).expect("a paper's key")),
            };
            PipelineSettings { separation: dose, print_paper, ..rest }
        }
        None => serde_json::from_str(&std::fs::read_to_string(&args[0]).unwrap()).unwrap(),
    };
    let (w, h, input) = read(Path::new(&args[1]));
    let out = Path::new(&args[2]);
    std::fs::create_dir_all(out).unwrap();
    let stock = film::stock(VirtualStock::of("kodak-portra-800"));
    let reference = stock.reference;
    let scan: Vec<[f32; 4]> = if args[3] == "scene" {
        let k = stock.kernel_at(0.0);
        input.iter().map(|p| cpu::virtual_negative(*p, &k, &stock.table)).collect()
    } else {
        input.clone()
    };
    s.virtual_film = Some(VirtualFilm { stock: VirtualStock::of("kodak-portra-800"), exposure: 0.0 });
    let k = conversion::stage47(&s);
    let mut info = std::fs::File::create(out.join("params.txt")).unwrap();
    writeln!(info, "reference: {reference:?}\nstage47: {k:?}\nceiling {}", film::CEILING).unwrap();
    let dens: Vec<[f32; 4]> = scan.iter().map(|p| std::array::from_fn(|c| if c < 3 { film::density(p[c], reference.stops[c]) } else { 1.0 })).collect();
    let picture: Vec<[f32; 4]> = scan.iter().map(|p| cpu::stage47(*p, &k)).collect();
    write(out, "scan", w, h, &scan);
    write(out, "density", w, h, &dens);
    write(out, "picture", w, h, &picture);
    println!("{w} x {h}");
}
