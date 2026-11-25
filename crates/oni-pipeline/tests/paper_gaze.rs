//! A paper's gaze on a colour negative and the film's filtration through it, against the two sheets and the recipe as it
//! was prototyped outside the app; each film's grey scale printed for the eye. Each verdict with a twin.

use oni_pipeline::film::{self, bake, gaze, profile::Profile};
use oni_pipeline::conversion;
use oni_render::cpu;
use oni_settings::stocks::VirtualEntry;
use oni_settings::{ConversionMode, PipelineSettings, VirtualStock};
use serde_json::Value;

const PORTRA: VirtualStock = VirtualStock::of("kodak-portra-800");

fn prototype() -> Value {
    serde_json::from_str(include_str!("paper_gaze/prototype.json")).unwrap()
}

/// Every film a paper reads.
fn read_films() -> Vec<VirtualEntry> {
    VirtualStock::all().into_iter().map(|s| s.entry()).filter(|e| e.reader.is_some()).collect()
}

fn sheets(entry: &VirtualEntry) -> (Profile, gaze::Sensitivity) {
    let reader = entry.reader.as_ref().expect("a film a paper reads");
    (Profile::read(entry.profile.as_bytes()).unwrap(), gaze::Sensitivity::read(reader.sensitivity.as_bytes()).unwrap())
}

/// Each channel's density above the bare film that `scan` reads of the dyes' `amounts`.
fn read(scan: &film::projection::Scan, amounts: [f64; 3]) -> [f64; 3] {
    let coded = [10f32.powf(-amounts[0] as f32), 10f32.powf(-amounts[1] as f32), 10f32.powf(-amounts[2] as f32), 1.0];
    let light = cpu::projection(coded, &scan.projection, &scan.table);
    std::array::from_fn(|c| -f64::from(light[c]).log10())
}

/// The dyes' amounts a grey develops at log exposure `log`, the film uncoupled.
fn developed(profile: &Profile, log: f64) -> [f64; 3] {
    let (amounts, _) = bake::curves_through(&profile.log_exposure, &profile.density_curves, &[[0.0; 3]; 3]);
    std::array::from_fn(|c| bake::interpolate(log, &profile.log_exposure, &amounts, c))
}

/// What each channel reads of each dye at mid scale: a row a channel, a column a dye.
fn gaze_at_mid(scan: &film::projection::Scan, profile: &Profile) -> [[f64; 3]; 3] {
    let mid = developed(profile, 0.0);
    let rest = read(scan, mid);
    let columns: [[f64; 3]; 3] = std::array::from_fn(|dye| {
        let mut more = mid;
        more[dye] += 0.05;
        let seen = read(scan, more);
        std::array::from_fn(|c| (seen[c] - rest[c]) / 0.05)
    });
    std::array::from_fn(|c| std::array::from_fn(|dye| columns[dye][c]))
}

#[test]
fn endura_premier_s_gaze_on_portra_800_is_the_two_sheets() {
    let (profile, paper) = sheets(&PORTRA.entry());
    let seen = gaze_at_mid(&gaze::scan(&profile, &paper), &profile);
    let want = &prototype()["portra_800_gaze"];
    let camera = gaze_at_mid(&film::projection::scanner(&profile), &profile);
    let (mut off, mut twin) = (0.0f64, 0.0f64);
    for c in 0..3 {
        println!("{:?}", seen[c].map(|v| (v * 1000.0).round() / 1000.0));
        for dye in 0..3 {
            let prototyped = want[c][dye].as_f64().unwrap();
            off = off.max((seen[c][dye] - prototyped).abs());
            twin = twin.max((camera[c][dye] - prototyped).abs());
        }
    }
    println!("the gaze lies {off:.4} off the prototype's at worst, a camera's scan {twin:.3}");
    assert!(off < 5e-3, "the gaze is {off:.4} off");
    // Twin: the camera's scan, a daylight lamp seen by the 1931 observer, reads the dyes otherwise.
    assert!(twin > 0.05, "the camera's scan passes at {twin:.3}");
    // Seen by the paper the three layers hold nearly one contrast: each channel's own dye near one.
    assert!((0..3).all(|c| (0.8..1.2).contains(&seen[c][c])), "{seen:?}");
}

#[test]
fn the_bare_film_reads_one_and_no_light_lies_on_the_windows_black() {
    for entry in read_films() {
        let stock = film::stock(entry.stock);
        let bare = stock.shot([0.0, 0.0, 0.0, 1.0], 0.0);
        assert!((0..3).all(|c| (bare[c] - 1.0).abs() < 1e-4), "{:?}: its bare film reads {bare:?}", entry.stock);
        assert!(entry.reference.stops.iter().all(|s| s.abs() < 1e-4), "{:?}: no gain evens a bare film read on one", entry.stock);
        assert_eq!(entry.couplers, Some([[0.0; 3]; 3]), "{:?}: uncoupled", entry.stock);
    }
    // Twin: a film no paper reads keeps its orange base under the camera, which its gains even.
    let cine = VirtualStock::of("kodak-vision3-50d");
    assert!(cine.entry().reader.is_none() && cine.reference().stops[0] < -0.5);
    let bare = film::stock(cine).shot([0.0, 0.0, 0.0, 1.0], 0.0);
    assert!(bare[2] < 0.5 * bare[0], "{bare:?}");
}

/// A grey scene's three window shares on `stock` as a new photo on it reads them, `stops` about the mid grey.
fn grey_shares(stock: VirtualStock, baked: &film::Stock, stops: f32) -> [f32; 3] {
    let s = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(stock);
    let grey = film::GREY * stops.exp2();
    cpu::windowed(baked.shot([grey, grey, grey, 1.0], 0.0), &conversion::stage47(&s))
}

#[test]
fn each_film_s_filtration_is_its_two_sheets_and_lays_a_mid_grey_on_one_share() {
    let prototyped = prototype();
    let (mut kodak, mut fuji) = (0.0f64, 0.0f64);
    for entry in read_films() {
        let reader = entry.reader.as_ref().unwrap();
        let (profile, paper) = sheets(&entry);
        let derived = gaze::filtration(&profile, &paper).unwrap();
        assert!((0..3).all(|c| (derived[c] - reader.filtration[c]).abs() < 1e-5), "{:?}: its file holds {:?}, its sheets derive {derived:?}", entry.stock, reader.filtration);
        assert_eq!(reader.filtration[1], 0.0, "{:?}: green takes none", entry.stock);
        let shares = grey_shares(entry.stock, film::stock(entry.stock), 0.0);
        let apart = (shares[0] - shares[1]).abs().max((shares[2] - shares[1]).abs());
        assert!(apart < 2e-3, "{:?}: a mid grey's shares {shares:?}", entry.stock);
        // The recipe as prototyped: the same paper, and its filtration within what the papers' unprinted bands leave.
        let Some(theirs) = prototyped["films"].get(entry.stock.key()) else { continue };
        assert_eq!(theirs["paper"].as_str(), Some(reader.paper), "{:?}", entry.stock);
        let gap = (0..3).map(|c| (reader.filtration[c] - theirs["filtration"][c].as_f64().unwrap()).abs()).fold(0.0, f64::max);
        if reader.paper.starts_with("kodak") { kodak = kodak.max(gap) } else { fuji = fuji.max(gap) }
    }
    println!("filtrations against the prototype's, in log exposure: {kodak:.5} at worst on Kodak's papers, {fuji:.5} on Fujifilm's");
    // Kodak's sheets print their papers from 380 nm, and the prototype's solve settled on every Kodak film: the same
    // filtrations. Fujifilm's sheets start at 405 nm, where the prototype fell 0.05 log a nanometre and the files follow
    // the sheet's own end slope, and its solve cycled on a few kinked curves: those films' differ.
    assert!(kodak < 2e-3, "{kodak}");
    assert!(fuji > 2e-3, "twin: the prototype's Fujifilm filtrations pass at {fuji}, so the check sees nothing");
    // Twin: Portra 800 under no filtration parts a mid grey's shares.
    let mut plain = PORTRA.entry();
    plain.reader.as_mut().unwrap().filtration = [0.0; 3];
    let unfiltered = film::bake_through(&plain, [[0.0; 3]; 3]).unwrap();
    let mut s = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(PORTRA);
    (s.gains.stops, s.levels) = (unfiltered.reference.stops, unfiltered.reference.levels());
    let shares = cpu::windowed(unfiltered.shot([film::GREY, film::GREY, film::GREY, 1.0], 0.0), &conversion::stage47(&s));
    println!("Portra 800 under no filtration, a mid grey's shares: {shares:?}");
    assert!((shares[2] - shares[1]).abs() > 0.01, "the twin's shares {shares:?}");
}

/// CIELAB a* and b* of linear Rec. 2020 light, on D65.
fn hue(p: [f32; 4]) -> (f64, f64) {
    let xyz = bake::apply(&bake::rec2020_to_xyz(), [p[0], p[1], p[2]].map(|v| f64::from(v.clamp(0.0, 1.0))));
    let white = bake::apply(&bake::rec2020_to_xyz(), [1.0; 3]);
    let f = |t: f64| if t > (6.0f64 / 29.0).powi(3) { t.cbrt() } else { t / (3.0 * (6.0f64 / 29.0).powi(2)) + 4.0 / 29.0 };
    let [fx, fy, fz] = std::array::from_fn(|c| f(xyz[c] / white[c]));
    (500.0 * (fx - fy), 200.0 * (fy - fz))
}

/// The widest chroma a grey scene shows on `baked`, read on its own reference through the pure inversion, from 8 stops
/// under the sensor's saturation to it, and its a* and b* there.
fn grey_scale(stock: VirtualStock, baked: &film::Stock) -> (f64, f64, f64) {
    let mut s = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(stock);
    (s.gains.stops, s.levels) = (baked.reference.stops, baked.reference.levels());
    let k = conversion::stage47(&s);
    (0..=32)
        .map(|i| {
            let scene = film::saturation() * (-(i as f32) / 4.0).exp2();
            let (a, b) = hue(cpu::stage47(baked.shot([scene, scene, scene, 1.0], 0.0), &k));
            (a.hypot(b), a, b)
        })
        .fold((0.0, 0.0, 0.0), |worst, at| if at.0 > worst.0 { at } else { worst })
}

#[test]
fn each_film_s_grey_scale_is_printed_and_portra_800_s_stays_near_neutral() {
    let prototyped = prototype();
    println!("film · paper · filtration in stops, red and blue · the grey scale's widest chroma, its a* and b* (the prototype's chroma)");
    let mut rows: Vec<(f64, String)> = read_films()
        .iter()
        .map(|e| {
            let r = e.reader.as_ref().unwrap();
            let (chroma, a, b) = grey_scale(e.stock, film::stock(e.stock));
            let theirs = prototyped["films"].get(e.stock.key()).and_then(|t| t["grey_max_chroma"].as_f64()).map_or("-".to_string(), |v| format!("{v:.1}"));
            let stops = |c: usize| r.filtration[c] / 2f64.log10();
            (chroma, format!("{:<34} {:<38} {:+.2} {:+.2}   {chroma:5.1}  a* {a:+5.1} b* {b:+5.1}  ({theirs})", e.name, r.paper, stops(0), stops(2)))
        })
        .collect();
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    rows.iter().for_each(|(_, line)| println!("{line}"));
    let (portra, ..) = grey_scale(PORTRA, film::stock(PORTRA));
    assert!(portra < 6.0, "Portra 800's grey scale strays {portra:.1}");
    // Twin: under no filtration its grey scale strays further.
    let mut plain = PORTRA.entry();
    plain.reader.as_mut().unwrap().filtration = [0.0; 3];
    let (unfiltered, ..) = grey_scale(PORTRA, &film::bake_through(&plain, [[0.0; 3]; 3]).unwrap());
    println!("Kodak Portra 800 under no filtration: {unfiltered:.1}");
    assert!(unfiltered > portra + 1.0, "the twin's grey scale strays {unfiltered:.1} only");
}
