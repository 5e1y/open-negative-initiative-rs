//! Every shipped stock, dye set and paper rendered on the CPU and the GPU, each render's bits hashed: two builds
//! printing the same lines render every one alike. Run: cargo run -p oni-pipeline --example preset_fingerprints.

use oni_graph::Graph;
use oni_pipeline::film::{self, GREY};
use oni_pipeline::{Pipeline, Request, conversion};
use oni_render::cpu;
use oni_settings::{ConversionMode, DyeSet, PaperStock, PipelineSettings, VirtualStock};

type V4 = [f32; 4];

fn fnv(pixels: &[V4]) -> u64 {
    pixels.iter().flatten().flat_map(|v| v.to_bits().to_le_bytes()).fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

fn noise(width: u32, height: u32, seed: u64, low: f32, high: f32) -> Vec<V4> {
    let mut state = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
    let mut next = move || {
        state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        low + ((state >> 40) as f32 / (1u64 << 24) as f32) * (high - low)
    };
    (0..width * height).map(|_| [next(), next(), next(), 1.0]).collect()
}

fn scenes() -> Vec<V4> {
    (-16..=16).map(|h| GREY * 2f32.powf(h as f32 / 2.0)).map(|v| [v, v * 0.8, v * 1.1, 1.0]).collect()
}

fn shot(stock: VirtualStock, paper: Option<PaperStock>) -> PipelineSettings {
    let s = PipelineSettings::rest(ConversionMode::Digital, true).per_pixel_only().on_stock(stock);
    PipelineSettings { print_paper: paper, ..s }
}

fn roll(mode: ConversionMode, dyes: DyeSet, paper: Option<PaperStock>) -> PipelineSettings {
    let mut s = PipelineSettings::rest(mode, true).per_pixel_only();
    s.dye_set = dyes;
    PipelineSettings { print_paper: paper, ..s }
}

fn main() {
    let mut g = Graph::new().ok();
    let source = g.as_mut().map(|g| g.upload(24, 16, &noise(24, 16, 11, 0.0, 2.0)).unwrap());
    let mut gpu = |s: &PipelineSettings| match (g.as_mut(), source.as_ref()) {
        (Some(g), Some(src)) => format!("{:016x}", fnv(&Pipeline::new().render(g, src, s, &Request::default(), &Default::default(), None).unwrap().1)),
        _ => "no-gpu".into(),
    };
    let papers: Vec<Option<PaperStock>> = std::iter::once(None).chain(PaperStock::all().into_iter().map(Some)).collect();
    for stock in VirtualStock::all() {
        let key = serde_json::to_string(&stock).unwrap();
        let baked = film::stock(stock);
        for paper in [None, Some(PaperStock::default())] {
            let s = shot(stock, paper);
            let k = conversion::stage47(&s);
            let cpu: Vec<V4> = scenes().into_iter().map(|p| cpu::stage47(baked.scanned(cpu::virtual_negative(p, &baked.kernel, &baked.table)), &k)).collect();
            let on = paper.map_or("none".into(), |p| serde_json::to_string(&p).unwrap());
            println!("stock {key} {on} cpu {:016x} gpu {} name {} white {:?} ref {:?}", fnv(&cpu), gpu(&s), stock.name(), stock.white(), stock.reference());
        }
    }
    for set in DyeSet::all() {
        let key = serde_json::to_string(&set).unwrap();
        for paper in &papers {
            let s = roll(ConversionMode::Negative, set, *paper);
            println!("dyes {key} {} gpu {}", paper.map_or("none".into(), |p| serde_json::to_string(&p).unwrap()), gpu(&s));
        }
    }
    for paper in &papers {
        let s = roll(ConversionMode::Monochrome, DyeSet::default(), *paper);
        println!("silver {} gpu {}", paper.map_or("none".into(), |p| serde_json::to_string(&p).unwrap()), gpu(&s));
        let s = roll(ConversionMode::Positive, DyeSet::default(), *paper);
        println!("slide {} gpu {}", paper.map_or("none".into(), |p| serde_json::to_string(&p).unwrap()), gpu(&s));
    }
    for kind in oni_settings::film::stocks().iter() {
        println!(
            "catalogue {} {} {} {:?} {:?} canister {} offered {} line {} dyes {:?}",
            kind.id,
            kind.display_name,
            kind.brand,
            kind.iso,
            kind.kind,
            kind.canister.map_or(kind.kind.generic(), |c| c.trim_end_matches(".svg")),
            kind.offered,
            kind.line,
            oni_settings::dyes::dye_set(kind.id).map(|d| serde_json::to_string(&d).unwrap())
        );
    }
    for p in PaperStock::all() {
        println!("paper {} {}", serde_json::to_string(&p).unwrap(), p.name());
    }
}
