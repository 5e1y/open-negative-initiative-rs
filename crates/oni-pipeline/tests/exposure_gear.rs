//! Exposure on the print's curve: its distance from the rest taken twice, the grey moving as the gear's table says, a
//! rest unmoved to the bit, and an older document's render kept by its migration. Each verdict has a twin.

use oni_pipeline::conversion;
use oni_render::{Stage47, cpu};
use oni_settings::document::{self, Document};
use oni_settings::migrate::{self, chain};
use oni_settings::tone::Print;
use oni_settings::{ConversionMode, PaperStock, PipelineSettings};

/// The print's grey, linear: what a plain RAW's grey prints at.
const GREY: f32 = 0.398;

/// The kernel's print alone on window outputs read straight, as `conversion` hands its stops over.
fn print_alone(k: &Stage47) -> Stage47 {
    Stage47 {
        red: [0.0, 1.0, 1.0],
        green: [0.0, 1.0, 1.0],
        blue: [0.0, 1.0, 1.0],
        log_guard: 1e-30,
        print_on: 1.0,
        print_stops: k.print_stops,
        hunt: 1.0,
        ..Stage47::rest()
    }
}

fn printed(n: f32, k: &Stage47) -> f32 {
    let t = 10f32.powf(-n);
    cpu::stage47([t, t, t, 1.0], k)[1]
}

/// The window output the print at rest lays on the grey.
fn grey_input(k: &Stage47) -> f32 {
    let (mut low, mut high) = (0.0f32, 1.0f32);
    for _ in 0..60 {
        let mid = 0.5 * (low + high);
        if printed(mid, k) < GREY { low = mid } else { high = mid }
    }
    0.5 * (low + high)
}

/// The grey's move in stops on the odds gain of `stops`.
fn odds_move(stops: f32) -> f32 {
    let g = stops.exp2();
    (g * GREY / (1.0 + (g - 1.0) * GREY) / GREY).log2()
}

#[test]
fn the_grey_moves_as_the_gears_table_says() {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.print_paper = None;
    let rest = Print::resting_exposure(s.mode);
    let n = grey_input(&print_alone(&conversion::stage47(&s)));
    let table = [(-4.0, -7.27), (-2.0, -3.33), (-1.0, -1.49), (1.0, 0.87), (2.0, 1.20), (4.0, 1.32)];
    let today = [(-4.0, -3.33), (-2.0, -1.49), (-1.0, -0.68), (1.0, 0.52), (2.0, 0.87), (4.0, 1.20)];
    for ((stops, want), (_, ungeared)) in table.into_iter().zip(today) {
        let mut lit = s.clone();
        lit.print.exposure = rest + stops;
        let k = conversion::stage47(&lit);
        let moved = (printed(n, &print_alone(&k)) / GREY).log2();
        println!("Exposure {stops:+}: the grey {moved:+.3} stops");
        assert!((moved - want).abs() < 0.01 && (moved - odds_move(2.0 * stops)).abs() < 2e-3, "{stops:+}: {moved}");
        // Twin: the curve fed the slider's stops once moves the grey as before the gear, off the table.
        let once = Stage47 { print_stops: lit.print.exposure, ..k };
        let before = (printed(n, &print_alone(&once)) / GREY).log2();
        assert!((before - ungeared).abs() < 0.01 && (before - want).abs() > 0.1, "{stops:+}: {before}");
    }
    // On a paper the curve rests at none and takes the same twice.
    let mut papered = s.clone();
    papered.print_paper = Some(PaperStock::default());
    papered.print.exposure = rest + 1.0;
    assert_eq!(conversion::stage47(&papered).print_stops, 2.0);
}

/// Every mode at rest, on ONI's print and on a paper where it prints one.
fn rests() -> Vec<PipelineSettings> {
    let mut out = Vec::new();
    for mode in ConversionMode::ALL {
        let s = PipelineSettings::rest(mode, true);
        out.push(PipelineSettings { print_paper: None, ..s.clone() });
        if s.takes_paper() {
            out.push(PipelineSettings { print_paper: Some(PaperStock::default()), ..s });
        }
    }
    out
}

#[test]
fn a_rest_reaches_the_kernel_to_the_bit() {
    for s in rests() {
        let k = conversion::stage47(&s);
        let rest = Print::resting_exposure(s.mode);
        let stops = if s.paper().is_some() { 0.0 } else { rest };
        assert_eq!(k.print_stops.to_bits(), stops.to_bits(), "{:?} {:?}", s.mode, s.print_paper);
        // Twin: a notch of a hundredth off the rest reaches the curve twice over.
        let notch = PipelineSettings { print: Print { exposure: s.print.exposure + 0.01, ..s.print }, ..s.clone() };
        assert!((conversion::stage47(&notch).print_stops - stops - 0.02).abs() < 1e-6, "{:?}", s.mode);
    }
}

/// Densities a scan holds, each channel apart, from past the base to the densest highlight.
fn densities() -> Vec<[f32; 4]> {
    let mut state = 0x9e37_79b9_u32;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state as f32 / u32::MAX as f32
    };
    let mut t = move || 10f32.powf(-3.0 * next());
    (0..4096).map(|_| [t(), t(), t(), 1.0]).collect()
}

/// The kernel's arguments before the gear, for `s` as an older build read it: the slider's stops on the curve once.
fn before_the_gear(s: &PipelineSettings) -> Stage47 {
    let k = conversion::stage47(s);
    let rest = Print::resting_exposure(s.mode);
    let print_stops = if s.paper().is_some() { s.print.exposure - rest } else { s.print.exposure };
    Stage47 { print_stops, ..k }
}

fn stored(s: &PipelineSettings, version: i64) -> Vec<u8> {
    let mut value = serde_json::to_value(Document::of(s.clone())).unwrap();
    value["version"] = version.into();
    serde_json::to_vec(&value).unwrap()
}

#[test]
fn an_older_documents_render_stays_through_its_migration() {
    let older = chain::EXPOSURE_GEAR_VERSION - 1;
    let pixels = densities();
    let mut worst = 0.0f32;
    for mut s in rests() {
        let rest = Print::resting_exposure(s.mode);
        s.print.exposure = rest + 1.3;
        s.print.contrast = 0.2;
        let read = migrate::document(&stored(&s, older)).unwrap().document.settings;
        assert_eq!(read.print.exposure, Print::exposure_at(s.print.exposure, rest));
        let (was, now, unmigrated) = (before_the_gear(&s), conversion::stage47(&read), conversion::stage47(&s));
        let (mut gap, mut doubled) = (0.0f32, 0.0f32);
        for p in &pixels {
            let (a, b, c) = (cpu::stage47(*p, &was), cpu::stage47(*p, &now), cpu::stage47(*p, &unmigrated));
            for i in 0..3 {
                gap = gap.max((a[i] - b[i]).abs());
                doubled = doubled.max((a[i] - c[i]).abs());
            }
        }
        println!("{:?}, paper {:?}: render moved {gap:e}, unmigrated {doubled:.3}", s.mode, s.print_paper.is_some());
        worst = worst.max(gap);
        assert!(gap < 1e-5, "{:?}: {gap}", s.mode);
        // Twin: read with no migration, the edit doubles on the curve and the render moves.
        assert!(doubled > 0.05, "{:?}: {doubled}", s.mode);
    }
    println!("worst render difference after the migration: {worst:e}");
    // A document of this format is read as it stands.
    let mut current = PipelineSettings::rest(ConversionMode::Negative, true);
    current.print.exposure = 2.3;
    let read = migrate::document(&stored(&current, document::VERSION)).unwrap().document.settings;
    assert_eq!(read.print.exposure, 2.3);
}
