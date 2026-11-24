//! `stock_chart <in.f32> <out> [stock profile,...]`: scene pixels (linear Rec. 2020, as a digital photo's decode holds
//! them) through each virtual stock at rest on the CPU references: the scene lit at the stock's white, its dye amounts,
//! its scan, its print on the default paper and on ONI's own print. Each as `w h` then f32 RGB.

use oni_pipeline::{conversion, film};
use oni_render::cpu;
use oni_settings::{ConversionMode, PipelineSettings, VirtualStock};
use std::io::{Read, Write};
use std::path::Path;

fn read(p: &Path) -> (usize, usize, Vec<[f32; 4]>) {
    let mut bytes = Vec::new();
    std::fs::File::open(p).unwrap().read_to_end(&mut bytes).unwrap();
    let nl = bytes.iter().position(|b| *b == b'\n').unwrap();
    let head: Vec<usize> = std::str::from_utf8(&bytes[..nl]).unwrap().split_whitespace().map(|s| s.parse().unwrap()).collect();
    let px = bytes[nl + 1..].chunks_exact(12).map(|c| {
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
    let (w, h, input) = read(Path::new(&args[0]));
    let out = Path::new(&args[1]);
    std::fs::create_dir_all(out).unwrap();
    let wanted: Option<Vec<String>> = args.get(2).map(|s| s.split(',').map(str::to_string).collect());
    let exposure: f32 = std::env::var("ONI_FILM_EXPOSURE").map_or(0.0, |v| v.parse().unwrap());
    println!("plain grey {}", film::plain_grey());
    for stock in VirtualStock::sheets() {
        if wanted.as_ref().is_some_and(|w| !w.iter().any(|n| n == stock.key())) {
            continue;
        }
        let mut paper = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(stock);
        if let Some(f) = paper.virtual_film.as_mut() {
            f.exposure = exposure;
        }
        let oni = PipelineSettings { print_paper: None, ..paper.clone() };
        let m = conversion::scene_white(&paper).expect("a digital photo's film");
        let s = film::stock(stock);
        let k = s.kernel_at(exposure);
        let scene: Vec<[f32; 4]> = input.iter().map(|p| cpu::color_matrix(*p, &m)).collect();
        let dyes: Vec<[f32; 4]> = scene.iter().map(|p| { let d = cpu::virtual_dyes(*p, &k, &s.table); [d[0], d[1], d[2], 1.0] }).collect();
        let scan: Vec<[f32; 4]> = scene.iter().map(|p| s.scanned(cpu::virtual_negative(*p, &k, &s.table))).collect();
        let (kp, ko) = (conversion::stage47(&paper), conversion::stage47(&oni));
        let name = stock.key();
        write(out, &format!("{name}-scene"), w, h, &scene);
        write(out, &format!("{name}-dyes"), w, h, &dyes);
        write(out, &format!("{name}-scan"), w, h, &scan);
        write(out, &format!("{name}-paper"), w, h, &scan.iter().map(|p| cpu::stage47(*p, &kp)).collect::<Vec<_>>());
        write(out, &format!("{name}-oni"), w, h, &scan.iter().map(|p| cpu::stage47(*p, &ko)).collect::<Vec<_>>());
        println!("{name}: {:?}", s.reference);
    }
}
