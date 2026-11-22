//! Every virtual stock a digital photo can be shot on, at rest on its own reference and the default paper: a
//! neutral scene, lit at the stock's own white, prints neutral from the shadows to the white, its grey where a
//! plain RAW prints one and its white where Portra 800 prints its own. Each verdict has a wrong twin.

mod common;

use common::{V4, graph};
use oni_pipeline::film::{self, GREY, cie};
use oni_pipeline::{Pipeline, Request, conversion};
use oni_render::cpu;
use oni_settings::virtual_film::Balance;
use oni_settings::{ConversionMode, PipelineSettings, VirtualStock};

/// A neutral ramp of the scene in stops about its grey, the white a 90 % reflectance at +2.3.
const STOPS: [f32; 7] = [-3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 2.3];
const GREY_AT: usize = 3;
const WHITE: usize = 6;

/// A neutral's widest CIELAB chroma on the print, a light one's (L* 80 up), the white's luma off Portra 800's in
/// stops, and the grey's distance to a plain RAW's. Each stock's shadows keep its own layers' toes: up to 3.7 (Pro 400H).
const CHROMA: f32 = 4.0;
const LIGHT_CHROMA: f32 = 2.0;
const WHITE_SPREAD: f32 = 0.06;
const GREY_GAP: f32 = 2e-3;

/// A stock whose neutral the checks hold: spektrafilm's, its layers balanced on a grey. A data-sheet stock prints its
/// sheet's own layers, their crossovers included: its numbers are shown, the owner judging them (owner's call).
fn balanced(stock: VirtualStock) -> bool {
    !stock.profile().starts_with("datasheet/")
}

/// A digital photo on `stock` at rest, its neighbourhood stages off, on the paper's own black where its enlarger rests.
fn on(stock: VirtualStock) -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(stock).per_pixel_only();
    s.print.paper_blacks = true;
    s
}

/// The print of a scene light lit at the film's white, through the CPU references of the chain.
fn printed(stock: VirtualStock, s: &PipelineSettings, light: f32) -> [f32; 3] {
    let st = film::stock(stock);
    let scan = cpu::virtual_negative([light, light, light, 1.0], &st.kernel_at(0.0), &st.table);
    let p = cpu::stage47(scan, &conversion::stage47(s));
    [p[0], p[1], p[2]]
}

fn luma(p: [f32; 3]) -> f32 {
    oni_render::LUMA.iter().zip(p).map(|(w, v)| w * v).sum()
}

/// A print's CIELAB lightness and chroma, read on D65 from linear Rec. 2020.
fn lab(p: [f32; 3]) -> (f32, f32) {
    let xyz = film::bake::apply(&film::bake::rec2020_to_xyz(), p.map(f64::from));
    let white = film::bake::apply(&film::bake::rec2020_to_xyz(), [1.0; 3]);
    let f = |t: f64| if t > (6.0f64 / 29.0).powi(3) { t.cbrt() } else { t / (3.0 * (6.0f64 / 29.0).powi(2)) + 4.0 / 29.0 };
    let [fx, fy, fz] = std::array::from_fn(|c| f(xyz[c] / white[c]));
    ((116.0 * fy - 16.0) as f32, (500.0 * (fx - fy)).hypot(200.0 * (fy - fz)) as f32)
}


/// The ramp's prints: each step's CIELAB lightness and chroma, the grey's green and the white's luma.
struct Ramp {
    steps: Vec<(f32, f32)>,
    grey: f32,
    white: f32,
}

fn ramp(stock: VirtualStock, s: &PipelineSettings) -> Ramp {
    let prints: Vec<[f32; 3]> = STOPS.iter().map(|h| printed(stock, s, GREY * h.exp2())).collect();
    Ramp { steps: prints.iter().map(|p| lab(*p)).collect(), grey: prints[GREY_AT][1], white: luma(prints[WHITE]) }
}

impl Ramp {
    fn chroma(&self) -> f32 {
        self.steps.iter().map(|(_, c)| *c).fold(0.0, f32::max)
    }

    fn light_chroma(&self) -> f32 {
        self.steps.iter().filter(|(l, _)| *l >= 80.0).map(|(_, c)| *c).fold(0.0, f32::max)
    }
}

/// The table: every stock's neutral ramp, grey and white, each checked; its twin, the red gain a fifth of a stop off
/// the stock's reference, takes the ramp past the chroma bound.
#[test]
fn every_stock_prints_a_neutral_scene_neutral_its_grey_as_a_plain_raw_and_its_white_as_portra_800s() {
    let plain = film::plain_grey();
    let portra = ramp(VirtualStock::KodakPortra800, &on(VirtualStock::KodakPortra800)).white;
    println!("stock: L* and C*ab from -3 to +2.3 stops · grey · white luma (Portra 800 {portra:.4}) · twin's widest C*ab");
    let rows: Vec<(VirtualStock, Ramp, Ramp)> = VirtualStock::ALL
        .into_iter()
        .map(|stock| {
            let s = on(stock);
            let mut off = s.clone();
            off.gains.stops[0] += 0.2;
            let (held, twin) = (ramp(stock, &s), ramp(stock, &off));
            let steps: Vec<String> = held.steps.iter().map(|(l, c)| format!("L{l:.0} C{c:.1}")).collect();
            println!("{:<28} {} · {:.4} · {:.4} · {:.1}", stock.name(), steps.join(" "), held.grey, held.white, twin.chroma());
            (stock, held, twin)
        })
        .collect();
    for (stock, held, twin) in rows.into_iter().filter(|(stock, _, _)| balanced(*stock)) {
        assert!(held.chroma() < CHROMA && held.light_chroma() < LIGHT_CHROMA, "{stock:?}: a neutral prints at C*ab {}", held.chroma());
        assert!((held.grey - plain).abs() < GREY_GAP, "{stock:?}: its grey prints {}, a plain RAW's {plain}", held.grey);
        assert!((held.white / portra).log2().abs() < WHITE_SPREAD, "{stock:?}: its white {}, Portra 800's {portra}", held.white);
        assert!(twin.chroma() > CHROMA, "{stock:?}: twin, the red gain off its reference, prints at C*ab {} only", twin.chroma());
    }
}

/// A tungsten stock's scene stands at its lamp: under daylight, the scene a daylight stock sees as neutral, its print
/// turns blue. Twin: a daylight stock prints that scene neutral.
#[test]
fn a_tungsten_stock_turns_a_daylight_scene_blue() {
    let daylight = |stock: VirtualStock| {
        let to_white = oni_settings::white::relative(oni_settings::white::uv(stock.white()), oni_settings::white::anchor(true));
        let day = oni_settings::white::relative(oni_settings::white::uv(VirtualStock::KodakPortra800.white()), oni_settings::white::anchor(true));
        // A grey under D55, as the decode reads it, then lit at the stock's white.
        let decoded = oni_settings::white::apply(&oni_settings::white::inverse(&day.map(|r| r.map(f64::from))), [f64::from(GREY); 3]);
        let lit: [f64; 3] = std::array::from_fn(|r| (0..3).map(|c| f64::from(to_white[r][c]) * decoded[c]).sum());
        let st = film::stock(stock);
        let scan = cpu::virtual_negative([lit[0] as f32, lit[1] as f32, lit[2] as f32, 1.0], &st.kernel_at(0.0), &st.table);
        cpu::stage47(scan, &conversion::stage47(&on(stock)))
    };
    let tungsten = daylight(VirtualStock::KodakVision3_500T);
    println!("a D55 grey on 500T {tungsten:?}, on 250D {:?}", daylight(VirtualStock::KodakVision3_250D));
    assert!(tungsten[2] > 1.5 * tungsten[0], "{tungsten:?}");
    let day = daylight(VirtualStock::KodakVision3_250D);
    assert!(lab([day[0], day[1], day[2]]).1 < 0.5, "{day:?}");
}

/// Each stock's light is its profile's: the white it stands at is the chromaticity of the spectrum its bake reads.
/// Twin: the two lights are far apart.
#[test]
fn a_stocks_white_is_the_spectrum_its_bake_reads() {
    let xy = |spd: &[f64; cie::WAVELENGTHS]| {
        let sum = |cmf: &[f64; cie::WAVELENGTHS]| (0..cie::WAVELENGTHS).map(|l| spd[l] * cmf[l]).sum::<f64>();
        let (x, y, z) = (sum(&cie::X), sum(&cie::Y), sum(&cie::Z));
        (x / (x + y + z), y / (x + y + z))
    };
    for stock in VirtualStock::ALL {
        let (want, got) = (xy(film::reference_light(stock)), stock.balance().xy());
        assert!((want.0 - got.0).abs() < 2e-5 && (want.1 - got.1).abs() < 2e-5, "{stock:?}: {want:?} {got:?}");
    }
    let (d, t) = (Balance::Daylight.xy(), Balance::Tungsten.xy());
    assert!((d.0 - t.0).abs() > 0.1);
}

/// Each stock bakes once, lazily, at its first use: its time and the table it holds. Twin: two stocks hold two tables.
#[test]
fn each_stock_bakes_at_its_first_use_and_holds_one_table() {
    for stock in VirtualStock::ALL {
        let start = std::time::Instant::now();
        let st = film::stock(stock);
        let first = start.elapsed();
        let again = std::time::Instant::now();
        assert!(std::ptr::eq(st, film::stock(stock)), "{stock:?} is baked once");
        let bytes = st.table.len() * std::mem::size_of::<[f32; 4]>();
        println!("{:<28} baked in {:>6.1} ms, then {:>5.1} µs; table {} KB", stock.name(), first.as_secs_f64() * 1e3, again.elapsed().as_secs_f64() * 1e6, bytes / 1024);
    }
    assert!(!std::ptr::eq(film::stock(VirtualStock::KodakPortra800), film::stock(VirtualStock::KodakPortra800Push1)));
}

/// The GPU chain on every stock: a neutral scene ramp lit at the stock's white, rendered as the editor renders a
/// digital photo, prints within the CPU references' chroma. Twin: the same scene lit at the other light is coloured.
#[test]
fn the_gpu_prints_every_stocks_neutral_ramp_neutral() {
    let Some(mut g) = graph("the_gpu_prints_every_stocks_neutral_ramp_neutral") else { return };
    let mut pipeline = Pipeline::new();
    for stock in VirtualStock::ALL {
        let s = on(stock);
        let scene = |white: oni_settings::white::White| -> Vec<V4> {
            let m = oni_settings::white::relative(oni_settings::white::uv(white), oni_settings::white::anchor(true));
            let back = oni_settings::white::inverse(&m.map(|r| r.map(f64::from)));
            STOPS.iter().map(|h| {
                let v = oni_settings::white::apply(&back, [f64::from(GREY * h.exp2()); 3]);
                [v[0] as f32, v[1] as f32, v[2] as f32, 1.0]
            }).collect()
        };
        let render = |g: &mut oni_graph::Graph, pipeline: &mut Pipeline, pixels: &[V4]| {
            let source = g.upload(pixels.len() as u32, 1, pixels).unwrap();
            pipeline.render(g, &source, &s, &Request::default(), &Default::default(), None).unwrap().1
        };
        let lit = render(&mut g, &mut pipeline, &scene(stock.white()));
        let worst = lit.iter().map(|p| lab([p[0], p[1], p[2]]).1).fold(0.0, f32::max);
        let other = match stock.balance() {
            Balance::Daylight => VirtualStock::KodakVision3_500T.white(),
            Balance::Tungsten => VirtualStock::KodakPortra800.white(),
        };
        let twin = render(&mut g, &mut pipeline, &scene(other)).iter().map(|p| lab([p[0], p[1], p[2]]).1).fold(0.0, f32::max);
        println!("{:<28} GPU widest C*ab {worst:.2}, lit at the other light {twin:.1}", stock.name());
        if !balanced(stock) {
            continue;
        }
        assert!(worst < CHROMA, "{stock:?}: {lit:?}");
        assert!(twin > 4.0 * CHROMA, "{stock:?}: twin at C*ab {twin} only");
    }
}
