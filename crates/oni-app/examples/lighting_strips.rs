//! `lighting_strips <out> <raw>...` (`SIDE`, `TAG`, `PAPER`): each photo at Lighting's rest, Exposure ±1, Contrast ±0.5,
//! Shadows ±0.5 and Highlights ±0.5 as one PNG row, on its own print or, with `PAPER` set, on Portra Endura; originals
//! and sidecars only read.

use oni_app::render::{Device, Engine, Linear, Reading, decode};
use oni_settings::tone::Print;
use oni_settings::{PaperStock, PipelineSettings, sidecar};
use std::path::Path;

/// Lighting's moves the row lays, after its rest.
const MOVES: [(&str, fn(&mut Print)); 9] = [
    ("rest", |_| {}),
    ("exposure-1", |p| p.exposure -= 1.0),
    ("exposure+1", |p| p.exposure += 1.0),
    ("contrast-0.5", |p| p.contrast -= 0.5),
    ("contrast+0.5", |p| p.contrast += 0.5),
    ("shadows-0.5", |p| p.shadows -= 0.5),
    ("shadows+0.5", |p| p.shadows += 0.5),
    ("highlights-0.5", |p| p.highlights -= 0.5),
    ("highlights+0.5", |p| p.highlights += 0.5),
];

fn srgb(p: [f32; 4]) -> [u8; 3] {
    let m = [[1.6605, -0.5876, -0.0728], [-0.1246, 1.1329, -0.0083], [-0.0182, -0.1006, 1.1187]];
    let encode = |v: f32| {
        let v = v.clamp(0.0, 1.0);
        let e = if v <= 0.0031308 { 12.92 * v } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
        (e * 255.0 + 0.5) as u8
    };
    std::array::from_fn(|r| encode(m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2]))
}

fn save(tiles: &[Linear], path: &Path) {
    let (tw, th, gap) = (tiles[0].width, tiles[0].height, 6);
    let w = tiles.len() * tw + (tiles.len() - 1) * gap;
    let mut px = vec![[255u8; 3]; w * th];
    for (i, l) in tiles.iter().enumerate() {
        for y in 0..th.min(l.height) {
            for x in 0..tw.min(l.width) {
                px[y * w + i * (tw + gap) + x] = srgb(l.pixels[y * l.width + x]);
            }
        }
    }
    let file = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    let mut enc = png::Encoder::new(file, w as u32, th as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(&px.concat()).unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = Path::new(&args[0]);
    let side: usize = std::env::var("SIDE").ok().and_then(|v| v.parse().ok()).unwrap_or(420);
    let tag = std::env::var("TAG").unwrap_or_else(|_| "now".into());
    let on_paper = std::env::var_os("PAPER").is_some();
    std::fs::create_dir_all(out).unwrap();
    let device = Device::open().expect("a GPU");
    let engine = Engine::shared(&device);
    let mut e = engine.lock().unwrap();
    for raw in args[1..].iter().map(Path::new) {
        let name = raw.file_stem().unwrap().to_string_lossy().to_string();
        let mut read: PipelineSettings = sidecar::read(raw, true).expect("a sidecar").settings;
        if on_paper {
            read.print_paper = Some(PaperStock::default());
            read.paperless = false;
        }
        let negative = e.prepare(decode(raw, Reading::of(&read)).expect("a decode")).unwrap();
        let tiles: Vec<Linear> = MOVES
            .iter()
            .map(|(_, f)| {
                let mut s = read.clone();
                f(&mut s.print);
                e.render(&negative, &s, Some(side)).unwrap()
            })
            .collect();
        let pipe = if on_paper { "paper" } else { "own" };
        save(&tiles, &out.join(format!("{name}-{pipe}-{tag}.png")));
        println!("{name} on {pipe}: {} tiles", tiles.len());
    }
}
