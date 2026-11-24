//! Every stock and paper, each written from its maker's data sheet, bakes and prints finite on the CPU and the GPU, and
//! a film's dyes' amounts build a neutral its sheet's densitometer reads as printed. Each has a twin.

mod common;

use common::{V4, graph, noise};
use oni_graph::Graph;
use oni_pipeline::film::{self, GREY};
use oni_pipeline::{Pipeline, Request, conversion, paper};
use oni_render::cpu;
use oni_settings::{ConversionMode, PaperStock, PipelineSettings, VirtualStock};

fn sheet_stocks() -> Vec<VirtualStock> {
    VirtualStock::all().to_vec()
}

/// Every paper but the Frontier, a machine measured.
fn sheet_papers() -> Vec<PaperStock> {
    PaperStock::all().into_iter().filter(|p| !p.is_machine()).collect()
}

fn finite(pixels: &[V4]) -> bool {
    pixels.iter().all(|p| p.iter().all(|v| v.is_finite()))
}

/// A digital photo at rest on `stock`, printed without paper or on `paper`.
fn shot(stock: VirtualStock, paper: Option<PaperStock>) -> PipelineSettings {
    let s = PipelineSettings::rest(ConversionMode::Digital, true).per_pixel_only().on_stock(stock);
    PipelineSettings { print_paper: paper, paperless: paper.is_none(), ..s }
}

/// Scenes from 8 stops under the grey to 8 over, and every hue at three saturations.
fn scenes() -> Vec<V4> {
    let mut out: Vec<V4> = (-16..=16).map(|h| [GREY * 2f32.powf(h as f32 / 2.0); 3]).map(|v| [v[0], v[1], v[2], 1.0]).collect();
    for hue in 0..12 {
        let a = hue as f32 / 12.0 * std::f32::consts::TAU;
        for s in [0.3, 0.7, 1.0] {
            let c = [a.cos(), (a - 2.1).cos(), (a + 2.1).cos()].map(|v| GREY * (1.0 + s * v).max(0.02));
            out.push([c[0], c[1], c[2], 1.0]);
        }
    }
    out
}

/// The scenes through the stock's negative and the frame's print, on the CPU.
fn printed(stock: VirtualStock, s: &PipelineSettings) -> Vec<V4> {
    let baked = film::stock(stock);
    let k = conversion::stage47(s);
    scenes().into_iter().map(|p| cpu::stage47(baked.scanned(cpu::virtual_negative(p, &baked.kernel, &baked.table)), &k)).collect()
}

fn rendered(g: &mut Graph, s: &PipelineSettings) -> Vec<V4> {
    let source = g.upload(24, 16, &noise(24, 16, 11, 0.0, 2.0)).unwrap();
    Pipeline::new().render(g, &source, s, &Request::default(), &Default::default(), None).unwrap().1
}

#[test]
fn every_sheet_profile_reads_and_one_holding_a_number_not_finite_is_refused() {
    let (stocks, papers) = (sheet_stocks(), sheet_papers());
    println!("{} films and {} papers from their data sheets", stocks.len(), papers.len());
    assert_eq!((stocks.len(), papers.len()), (79, 7));
    for stock in &stocks {
        let json = stock.entry().profile;
        let read = if stock.is_silver() { film::profile::Silver::read(json.as_bytes()).err() } else { film::profile::Profile::read(json.as_bytes()).err() };
        assert!(read.is_none(), "{stock:?}: {read:?}");
    }
    for p in &papers {
        assert!(paper::validate(paper::source(*p).expect("shipped").as_bytes()).is_ok(), "{p:?}");
    }
    // Twin: a curve's first node turned into a number past any double, which no reading takes as a finite one.
    let spoiled = |json: &[u8], key: &str| {
        let text = std::str::from_utf8(json).unwrap();
        let at = text.find(key).expect("the key") + key.len();
        let first = at + text[at..].find(|c: char| c.is_ascii_digit() || c == '-').unwrap();
        let last = first + text[first..].find(|c: char| !(c.is_ascii_digit() || "-+.e".contains(c))).unwrap();
        format!("{}1e999{}", &text[..first], &text[last..]).into_bytes()
    };
    let film_json = VirtualStock::default().entry().profile;
    assert!(film::profile::Profile::read(&spoiled(film_json.as_bytes(), "\"density_curves\"")).is_err());
    let silver_json = VirtualStock::of("ilford-hp5-plus").entry().profile;
    assert!(film::profile::Silver::read(&spoiled(silver_json.as_bytes(), "\"density_curve\"")).is_err());
    let paper_json = paper::source(papers[0]).unwrap();
    assert!(paper::validate(&spoiled(paper_json.as_bytes(), "\"density_curves\"")).is_err());
}

#[test]
fn every_sheet_film_prints_finite_on_the_cpu_with_and_without_paper() {
    for stock in sheet_stocks() {
        for paper in [None, Some(PaperStock::default())] {
            let out = printed(stock, &shot(stock, paper));
            assert!(finite(&out), "{stock:?} on {paper:?}");
        }
    }
    // Twin: a NaN a scene carries is seen.
    let mut out = printed(VirtualStock::of("kodak-portra-800"), &shot(VirtualStock::of("kodak-portra-800"), None));
    out[3][1] = f32::NAN;
    assert!(!finite(&out));
}

#[test]
fn every_sheet_paper_prints_finite_on_the_cpu() {
    for p in sheet_papers() {
        for stock in [VirtualStock::of("kodak-portra-800"), VirtualStock::of("kodak-vision3-500t")] {
            assert!(finite(&printed(stock, &shot(stock, Some(p)))), "{p:?} under {stock:?}");
        }
    }
}

#[test]
fn every_sheet_film_and_paper_renders_finite_on_the_gpu() {
    let Some(mut g) = graph("every_sheet_film_and_paper_renders_finite_on_the_gpu") else { return };
    for stock in sheet_stocks() {
        assert!(finite(&rendered(&mut g, &shot(stock, None))), "{stock:?}");
    }
    for p in sheet_papers() {
        assert!(finite(&rendered(&mut g, &shot(VirtualStock::of("kodak-portra-800"), Some(p)))), "{p:?}");
    }
    // Twin: a NaN a render carries is seen.
    let mut out = rendered(&mut g, &shot(VirtualStock::of("kodak-portra-800"), None));
    out[0][0] = f32::NAN;
    assert!(!finite(&out));
}


/// ISO 5-3's spectral products (ANSI/ISO 5-3-1995, tables 3 and 4), red, green and blue: the first wavelength, the log10
/// products every 10 nm to a 5.000 peak, and the log slopes past either end.
type Status = [(f64, &'static [f64], f64, f64); 3];
const STATUS_M: Status = [
    (620.0, &[2.109, 4.479, 5.000, 4.899, 4.578, 4.252, 3.875, 3.491, 3.099, 2.687, 2.269, 1.859, 1.449, 1.054, 0.654, 0.254], 0.260, -0.040),
    (470.0, &[1.152, 2.207, 3.156, 3.804, 4.272, 4.626, 4.872, 5.000, 4.995, 4.818, 4.458, 3.915, 3.172, 2.239, 1.070], 0.106, -0.120),
    (410.0, &[2.103, 4.111, 4.632, 4.871, 5.000, 4.955, 4.743, 4.343, 3.743, 2.990, 1.852], 0.250, -0.220),
];
const STATUS_A: Status = [
    (600.0, &[2.568, 4.638, 5.000, 4.871, 4.604, 4.286, 3.900, 3.551, 3.165, 2.776, 2.383, 1.970, 1.551, 1.141, 0.741, 0.341], 0.270, -0.040),
    (500.0, &[1.650, 3.822, 4.782, 5.000, 4.906, 4.644, 4.221, 3.609, 2.766, 1.579], 0.220, -0.170),
    (420.0, &[3.602, 4.819, 5.000, 4.912, 4.620, 4.040, 2.989, 1.566, 0.165], 0.380, -0.140),
];

/// Channel `c`'s product at `nm`, linear in log between the table's points.
fn product(status: &Status, c: usize, nm: f64) -> f64 {
    let (first, logs, before, after) = status[c];
    let last = first + 10.0 * (logs.len() - 1) as f64;
    let log = if nm < first {
        logs[0] + before * (nm - first)
    } else if nm >= last {
        logs[logs.len() - 1] + after * (nm - last)
    } else {
        let k = ((nm - first) / 10.0) as usize;
        let t = (nm - first) / 10.0 - k as f64;
        logs[k] + t * (logs[k + 1] - logs[k])
    };
    10f64.powf(log - 5.0)
}

/// What `status` reads of the spectral density `at(l)` on the wavelengths it is known.
fn reading(status: &Status, known: &[usize], at: impl Fn(usize) -> f64) -> [f64; 3] {
    std::array::from_fn(|c| {
        let w = |l: usize| product(status, c, 380.0 + 5.0 * l as f64);
        let seen: f64 = known.iter().map(|&l| w(l) * 10f64.powf(-at(l))).sum();
        -(seen / known.iter().map(|&l| w(l)).sum::<f64>()).log10()
    })
}

/// The worst gap, in density, between the sheet's curves and the neutral `amounts` builds over the base, as `status` reads it.
fn neutral_gap(json: &[u8], status: &Status, amounts: &str) -> f64 {
    let v: serde_json::Value = serde_json::from_slice(json).unwrap();
    let d = &v["data"];
    let number = |x: &serde_json::Value| x.as_f64();
    let dyes: Vec<[Option<f64>; 3]> = d["channel_density"].as_array().unwrap().iter().map(|r| std::array::from_fn(|j| number(&r[j]))).collect();
    let base: Vec<Option<f64>> = d["base_density"].as_array().unwrap().iter().map(number).collect();
    let known: Vec<usize> = (0..base.len()).filter(|&l| base[l].is_some() && dyes[l].iter().all(Option::is_some)).collect();
    let rows = |key: &str| -> Vec<[f64; 3]> { d[key].as_array().unwrap().iter().map(|r| std::array::from_fn(|j| r[j].as_f64().unwrap())).collect() };
    let (a, sheet) = (rows(amounts), rows("sheet_density_curves"));
    let bare = reading(status, &known, |l| base[l].unwrap());
    let mut worst = 0.0f64;
    for (a, want) in a.iter().zip(&sheet) {
        let read = reading(status, &known, |l| base[l].unwrap() + (0..3).map(|j| a[j] * dyes[l][j].unwrap()).sum::<f64>());
        worst = (0..3).map(|c| (read[c] - bare[c] - want[c]).abs()).fold(worst, f64::max);
    }
    worst
}

#[test]
fn every_colour_films_neutral_of_its_dye_amounts_reads_its_printed_curves() {
    // A negative's sheet reads Status M, a reversal film's Status A.
    let status = |s: VirtualStock| if s.entry().reversal { &STATUS_A } else { &STATUS_M };
    let films = sheet_stocks().into_iter().filter(|s| !s.is_silver()).map(|s| (s.name(), s.entry().profile, status(s)));
    let (mut worst, mut least_miss) = (0.0f64, f64::INFINITY);
    for (name, json, status) in films {
        let json = json.as_bytes();
        let (gap, miss) = (neutral_gap(json, status, "density_curves"), neutral_gap(json, status, "sheet_density_curves"));
        println!("{name}: the neutral within {gap:.1e} of its sheet; the printed curves taken as amounts miss it by {miss:.3}");
        // Within 0.02 D: where the traces disagree, on a shoulder, the unmix draws a dip, held as an amount never falls.
        assert!(gap < 0.02, "{name}: {gap}");
        // Twin: the printed curves as the dyes' amounts read a neutral denser than its sheet's.
        assert!(miss > 0.05, "{name}: {miss}");
        (worst, least_miss) = (worst.max(gap), least_miss.min(miss));
    }
    println!("worst {worst:.1e}, least miss {least_miss:.3}");
}
