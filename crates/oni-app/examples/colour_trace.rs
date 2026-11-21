//! `colour_trace <settings.json> <in.f32> <out> <scene|scan> [rederive]` prints a virtual Portra 800 negative on the CPU
//! references, each stage as `w h` then f32 RGB; `rederive` reads the film's windows off the first two pixels (bare, grey).

use oni_pipeline::{conversion, film};
use oni_render::cpu;
use oni_settings::axis::graduation;
use oni_settings::{ConversionMode, PipelineSettings, VirtualFilm, VirtualStock};
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
        println!("mode {:?}\nstage47: {:?}\nlighting: {:?}", s.mode, conversion::stage47(&s), conversion::lighting(&s));
        return;
    }
    let mut s: PipelineSettings = serde_json::from_str(&std::fs::read_to_string(&args[0]).unwrap()).unwrap();
    let (w, h, input) = read(Path::new(&args[1]));
    let out = Path::new(&args[2]);
    std::fs::create_dir_all(out).unwrap();
    let stock = film::stock(VirtualStock::KodakPortra800);
    let mut reference = stock.reference.clone();
    let scan: Vec<[f32; 4]> = if args[3] == "scene" {
        let k = stock.kernel_at(reference.exposure);
        input.iter().map(|p| cpu::virtual_negative(*p, &k, &stock.table)).collect()
    } else {
        input.clone()
    };
    if args.get(4).map(String::as_str) == Some("rederive") {
        let (bare, grey) = (scan[0], scan[1]);
        let stops = [bare[1] / bare[0], 1.0, bare[1] / bare[2]].map(f32::log2);
        let share = film::digital_share(film::GREY);
        let fraction = |d: f32| graduation::fraction(d, ConversionMode::Negative);
        let black: [f32; 3] = std::array::from_fn(|c| fraction(film::density(bare[c], stops[c])));
        let white = std::array::from_fn(|c| black[c] + (fraction(film::density(grey[c], stops[c])) - black[c]) / share);
        reference = VirtualFilm { stock: VirtualStock::KodakPortra800, exposure: 0.0, stops, black, white };
    }
    s.virtual_film = Some(reference.clone());
    let k = conversion::stage47(&s);
    let lit = conversion::lighting(&s);
    let mut info = std::fs::File::create(out.join("params.txt")).unwrap();
    writeln!(info, "reference: {reference:?}\nstage47: {k:?}\nlighting: {lit:?}\nceiling {}", film::CEILING).unwrap();
    let dens: Vec<[f32; 4]> = scan.iter().map(|p| std::array::from_fn(|c| if c < 3 { film::density(p[c], reference.stops[c]) } else { 1.0 })).collect();
    let picture: Vec<[f32; 4]> = scan.iter().map(|p| cpu::stage47(*p, &k)).collect();
    let printed: Vec<[f32; 4]> = match &lit {
        Some(l) => picture.iter().map(|p| cpu::print(*p, l)).collect(),
        None => picture.clone(),
    };
    write(out, "scan", w, h, &scan);
    write(out, "density", w, h, &dens);
    write(out, "picture", w, h, &picture);
    write(out, "printed", w, h, &printed);
    println!("{w} x {h}; lighting {}", lit.is_some());
}
