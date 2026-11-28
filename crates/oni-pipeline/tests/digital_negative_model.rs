//! A colour negative's virtual film read as its data sheet: the GPU's film against its scalar reference, the bare film
//! reading the sheet's Status M D-min, a neutral landing on the sheet's own curves through the layers' inhibition, a grey
//! ramp lit at the film's white showing its ends neutral on no paper. Each verdict has a twin.

mod common;
#[path = "../examples/chart/mod.rs"]
mod chart;

use common::{V4, graph, judge, mapped, noise, whole};
use oni_pipeline::film::{self, bake, profile::Profile};
use oni_pipeline::Pipeline;
use oni_render::cpu;
use oni_settings::{ConversionMode, PipelineSettings, VirtualStock};

/// A cine negative and a still one, each read as its sheet.
const CINE: VirtualStock = VirtualStock::of("kodak-vision3-250d");
const PORTRA: VirtualStock = VirtualStock::of("kodak-portra-800");

#[test]
fn the_gpu_develops_a_digital_negative_as_its_reference_does() {
    let Some(mut g) = graph("the_gpu_develops_a_digital_negative_as_its_reference_does") else { return };
    let (w, h) = (29, 11);
    let pixels: Vec<V4> = noise(w, h, 3, 0.0, 1.5).iter().map(|q| q.map(|v| v * v)).collect();
    let source = g.upload(w, h, &pixels).unwrap();
    let s = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(PORTRA);
    let exposure = s.film().unwrap().exposure;
    let filmed = Pipeline::new().filmed(&mut g, oni_graph::Image::new(&source), &s, None).unwrap();
    let out = whole(&mut g, &filmed);
    let stock = film::stock(PORTRA);
    let reference = mapped(&pixels, |p| stock.shot(p, exposure));
    // Twin: the same film developed with no inhibition between its layers.
    let entry = PORTRA.entry();
    let free = film::bake_through(&entry, [[0.0; 3]; 3]).unwrap();
    judge("a digital negative developed", &out, &reference, &mapped(&pixels, |p| free.shot(p, exposure)));
}

/// The sheet's Status M D-min, red, green and blue, as the colorimetry's reading of each sheet prints it.
const D_MIN: [(VirtualStock, [f32; 3]); 3] = [
    (PORTRA, [0.22, 0.59, 0.93]),
    (VirtualStock::of("kodak-gold-200"), [0.25, 0.63, 0.92]),
    (CINE, [0.21, 0.55, 0.85]),
];

#[test]
fn the_bare_film_reads_its_sheets_status_m_d_min() {
    for (stock, want) in D_MIN {
        let st = film::stock(stock);
        let bare = st.shot([0.0, 0.0, 0.0, 1.0], 0.0);
        // The light written less the veil its reading adds, read back as the negative's own density.
        let d: [f32; 3] = std::array::from_fn(|c| -((bare[c] + st.kernel.veil) / (1.0 + st.kernel.veil)).log10());
        println!("{stock:?}: bare film D {d:?}");
        assert!((0..3).all(|c| (d[c] - want[c]).abs() < 0.01), "{stock:?}: {d:?}");
        // The gains lay the bare film on grey, its black on every channel alike.
        let grey: [f32; 3] = std::array::from_fn(|c| bare[c] * film::stock(stock).reference.stops[c].exp2());
        assert!((0..3).all(|c| (grey[c] / grey[1] - 1.0).abs() < 1e-4), "{grey:?}");
    }
    // Twin: one sheet's bare film is not another's.
    let veil = film::stock(PORTRA).kernel.veil;
    let portra = film::stock(PORTRA).shot([0.0, 0.0, 0.0, 1.0], 0.0).map(|t| -((t + veil) / (1.0 + veil)).log10());
    assert!((0..3).any(|c| (portra[c] - D_MIN[1].1[c]).abs() > 0.02), "{portra:?}");
}

#[test]
fn a_neutral_lands_on_the_films_curves_through_the_layers_inhibition() {
    let entry = PORTRA.entry();
    let profile = Profile::read(entry.profile.as_bytes()).unwrap();
    let base = film::status::read(&profile.base_density);
    let le = &profile.log_exposure;
    // The curves its file fits with its couplers, its sheet's where it fits none.
    let curves: Vec<[f64; 3]> = entry.curves.as_deref().map_or_else(|| profile.sheet_density_curves.clone(), <[_]>::to_vec);
    let low: [f64; 3] = std::array::from_fn(|c| curves.iter().map(|d| d[c]).fold(f64::INFINITY, f64::min));
    let sheet = |log: f64, c: usize| base[c] + bake::interpolate(log, le, &curves, c) - low[c];
    let st = film::stock(PORTRA);
    let couplers = film::couplers(&entry);
    println!("Portra 800's couplers {couplers:?}");
    let mut worst = 0.0f64;
    for i in -40..=20 {
        let log = f64::from(i) / 10.0;
        let e = 10f64.powf(log) as f32;
        let developed = cpu::virtual_development([e, e, e, 1.0], &st.kernel, &st.table);
        for c in 0..3 {
            let read = (f64::from(developed[c]) + f64::from(st.kernel.veil)) / (1.0 + f64::from(st.kernel.veil));
            worst = worst.max((-read.log10() - sheet(log, c)).abs());
        }
    }
    println!("a neutral's widest gap to the film's curves {worst:.2e}");
    assert!(worst < 2e-3, "{worst}");
    // Twin: the same couplers laid on curves never corrected for them pull a neutral off the sheet.
    let k = st.kernel.couplers;
    let off = (-20..=10).map(|i| {
        let log = f64::from(i) / 10.0;
        let first: [f64; 3] = std::array::from_fn(|c| sheet(log, c) - base[c]);
        (0..3).map(|c| {
            let inhibited = log - (0..3).map(|m| first[m] * f64::from(k[m][c])).sum::<f64>();
            (base[c] + bake::interpolate(inhibited, le, &curves, c) - low[c] - sheet(log, c)).abs()
        }).fold(0.0, f64::max)
    }).fold(0.0, f64::max);
    assert!(couplers.iter().flatten().any(|v| *v > 0.0) && off > 0.02, "twin: {off}");
}

/// A ramp of greys lit at the film's own white (Vision3 250D: D55), as a daylight scene reaches it, on no paper: its
/// ends neutral, each channel's window from the bare film to a scene white, its mids the film's own crossover, nothing
/// solved on greys (owner's call; the mids' C* is printed for the preset's owner to judge). No paper lays the ends on pure
/// black and white: Vision3 250D's read 1.7, a RAW's 5000 K grey 2.7.
const NEUTRAL_ENDS: f64 = 2.0;

#[test]
fn a_grey_ramp_lit_at_the_films_white_shows_its_ends_neutral() {
    let white = oni_pipeline::conversion::scene_white(&PipelineSettings::rest(ConversionMode::Digital, true).on_stock(CINE)).unwrap();
    let rows = [white.r_vector, white.g_vector, white.b_vector].map(|r| [f64::from(r[0]), f64::from(r[1]), f64::from(r[2])]);
    let back = oni_settings::white::inverse(&rows);
    let lit = |v: f64| oni_settings::white::apply(&back, [chart::dec(v); 3]);
    let ramp = |r: &chart::Render, film: bool| chart::RAMP.map(|v| chart::cielab(chart::oklab(if film { r.scene(lit(v)) } else { r.code([v; 3]) })));
    let chroma = |l: &chart::V3| l[1].hypot(l[2]);
    let ends = |ramp: &[chart::V3]| chroma(&ramp[0]).max(chroma(&ramp[ramp.len() - 1]));
    let render = chart::Render::new(CINE.key(), film::couplers(&CINE.entry()));
    let (shot, raw) = (ramp(&render, true), ramp(&render, false));
    println!("lit at D55: L* a* b* {:?}", shot.map(|l| l.map(|v| (v * 10.0).round() / 10.0)));
    println!("a RAW's neutral, lit at 5000 K: {:?}", raw.map(|l| l.map(|v| (v * 10.0).round() / 10.0)));
    println!("the mids' widest C* {:.1}", shot.iter().map(chroma).fold(0.0, f64::max));
    assert!(ends(&shot) < NEUTRAL_ENDS, "the ends' C* {}", ends(&shot));
    // Twin: a RAW's own neutral is a 5000 K grey, warmer than the film's light, and prints its white warm.
    assert!(ends(&raw) > NEUTRAL_ENDS, "twin: {}", ends(&raw));
}
