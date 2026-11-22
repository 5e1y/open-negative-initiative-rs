//! Every stock and paper, each written from its maker's data sheet, bakes and prints finite on the CPU and the GPU.
//! Twins: a profile holding a number not finite is refused, and a NaN in a print is seen.

mod common;

use common::{V4, graph, noise};
use oni_graph::Graph;
use oni_pipeline::film::{self, GREY};
use oni_pipeline::{Pipeline, Request, conversion, paper};
use oni_render::cpu;
use oni_settings::{ConversionMode, PaperStock, PipelineSettings, VirtualStock};

fn sheet_stocks() -> Vec<VirtualStock> {
    VirtualStock::ALL.to_vec()
}

/// Every paper but the Frontier, a machine measured.
fn sheet_papers() -> Vec<PaperStock> {
    PaperStock::ALL.into_iter().filter(|p| p.profile().is_some()).collect()
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
    scenes().into_iter().map(|p| cpu::stage47(cpu::virtual_negative(p, &baked.kernel, &baked.table), &k)).collect()
}

fn rendered(g: &mut Graph, s: &PipelineSettings) -> Vec<V4> {
    let source = g.upload(24, 16, &noise(24, 16, 11, 0.0, 2.0)).unwrap();
    Pipeline::new().render(g, &source, s, &Request::default(), &Default::default(), None).unwrap().1
}

#[test]
fn every_sheet_profile_reads_and_one_holding_a_number_not_finite_is_refused() {
    let (stocks, papers) = (sheet_stocks(), sheet_papers());
    println!("{} films and {} papers from their data sheets", stocks.len(), papers.len());
    assert_eq!((stocks.len(), papers.len()), (75, 8));
    for stock in &stocks {
        let json = film::dyes::source(stock.profile()).expect("shipped");
        let read = if stock.is_silver() { film::profile::Silver::read(json).err() } else { film::profile::Profile::read(json).err() };
        assert!(read.is_none(), "{stock:?}: {read:?}");
    }
    for p in &papers {
        assert!(paper::validate(paper::source(*p).expect("shipped")).is_ok(), "{p:?}");
    }
    // Twin: a curve's first node turned into a number past any double, which no reading takes as a finite one.
    let spoiled = |json: &[u8], key: &str| {
        let text = std::str::from_utf8(json).unwrap();
        let at = text.find(key).expect("the key") + key.len();
        let first = at + text[at..].find(|c: char| c.is_ascii_digit() || c == '-').unwrap();
        let last = first + text[first..].find(|c: char| !(c.is_ascii_digit() || "-+.e".contains(c))).unwrap();
        format!("{}1e999{}", &text[..first], &text[last..]).into_bytes()
    };
    let film_json = film::dyes::source(stocks[0].profile()).unwrap();
    assert!(film::profile::Profile::read(&spoiled(film_json, "\"density_curves\"")).is_err());
    let silver_json = film::dyes::source(VirtualStock::IlfordHP5Plus.profile()).unwrap();
    assert!(film::profile::Silver::read(&spoiled(silver_json, "\"density_curve\"")).is_err());
    let paper_json = paper::source(papers[0]).unwrap();
    assert!(paper::validate(&spoiled(paper_json, "\"density_curves\"")).is_err());
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
    let mut out = printed(VirtualStock::KodakPortra800, &shot(VirtualStock::KodakPortra800, None));
    out[3][1] = f32::NAN;
    assert!(!finite(&out));
}

#[test]
fn every_sheet_paper_prints_finite_on_the_cpu() {
    for p in sheet_papers() {
        for stock in [VirtualStock::KodakPortra800, VirtualStock::KodakVision3_500T] {
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
        assert!(finite(&rendered(&mut g, &shot(VirtualStock::KodakPortra800, Some(p)))), "{p:?}");
    }
    // Twin: a NaN a render carries is seen.
    let mut out = rendered(&mut g, &shot(VirtualStock::KodakPortra800, None));
    out[0][0] = f32::NAN;
    assert!(!finite(&out));
}

