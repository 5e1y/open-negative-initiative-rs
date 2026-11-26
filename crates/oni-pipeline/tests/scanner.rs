//! ONI's scanner reading a colour negative: the GPU's film and scan against their scalar reference, the bare film's colour
//! against two real Portra 800 rolls scanned on real cameras, a film's filtration derived again from its sheet, and the
//! grain mixed by the scanner's own gaze. Each verdict has a twin.

mod common;

use common::{V4, graph, judge, mapped, noise, whole};
use oni_pipeline::{Pipeline, film, grain};
use oni_render::cpu;
use oni_settings::{ConversionMode, PipelineSettings, VirtualStock};

const PORTRA: VirtualStock = VirtualStock::of("kodak-portra-800");

/// Bare Portra 800 measured in the working space, red and blue over green, on the owner's rolls lit by a 6000 K LED: 37
/// frames on a Lumix S1R (1.80, 0.486) and 38 on a Nikon Z f (1.916, 0.526), each within 1 % from frame to frame.
const ROLLS: [[f32; 2]; 2] = [[1.80, 0.486], [1.916, 0.526]];
/// How far a bare film's ratios may lie from a roll's, in density: lamp, sensor and matrix each move it by about 0.05.
const BARE_TOLERANCE: f32 = 0.1;

fn ratios(bare: [f32; 4]) -> [f32; 2] {
    [bare[0] / bare[1], bare[2] / bare[1]]
}

/// The farthest a bare film's ratios lie from the nearer roll, in density.
fn off_the_rolls(r: [f32; 2]) -> f32 {
    ROLLS.iter().map(|m| (0..2).map(|i| (r[i] / m[i]).log10().abs()).fold(0.0, f32::max)).fold(f32::INFINITY, f32::min)
}

#[test]
fn the_gpu_scans_a_still_negative_as_its_reference_does() {
    let Some(mut g) = graph("the_gpu_scans_a_still_negative_as_its_reference_does") else { return };
    let (w, h) = (31, 9);
    // Lights from the bare film past the sensor's saturation, each channel its own: every band of the scan's table.
    let pixels: Vec<V4> = noise(w, h, 5, 0.0, 1.6).iter().map(|q| q.map(|v| v * v)).collect();
    let source = g.upload(w, h, &pixels).unwrap();
    let s = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(PORTRA);
    let exposure = s.film().unwrap().exposure;
    let filmed = Pipeline::new().filmed(&mut g, oni_graph::Image::new(&source), &s, None).unwrap();
    let out = whole(&mut g, &filmed);
    let stock = film::stock(PORTRA);
    let reference = mapped(&pixels, |p| stock.shot(p, exposure));
    // Twin: the read-out the scan replaces, each layer's amount on its own channel.
    judge("a still negative scanned", &out, &reference, &mapped(&pixels, |p| cpu::virtual_negative(p, &stock.kernel_at(exposure), &stock.table)));
}

#[test]
fn the_bare_film_reads_as_on_two_real_rolls() {
    let bare = film::stock(PORTRA).shot([0.0, 0.0, 0.0, 1.0], 0.0);
    let off = off_the_rolls(ratios(bare));
    println!("bare Portra 800 scanned: red {:.3}, blue {:.3} over green, {off:.3} D off the nearer roll", ratios(bare)[0], ratios(bare)[1]);
    assert!(off < BARE_TOLERANCE, "{off:.3} D");
    // Twin: the read-out alone, no lamp, sensor or base, misses both rolls by far.
    let alone = cpu::virtual_negative([0.0, 0.0, 0.0, 1.0], &film::stock(PORTRA).kernel, &film::stock(PORTRA).table);
    assert!(off_the_rolls(ratios(alone)) > 2.0 * BARE_TOLERANCE, "{:.3}", off_the_rolls(ratios(alone)));
}

#[test]
fn a_films_filtration_is_its_sheets_through_the_scanner() {
    let entry = PORTRA.entry();
    let held = entry.filtration.expect("a colour negative's file holds its filtration");
    let derived = film::derive(&entry.profile, entry.balance, false, false, film::couplers(&entry), None).unwrap();
    let derived = derived.filtration.expect("a colour negative derives its filtration");
    println!("Portra 800's filtration held {held:?}, derived {derived:?}");
    assert!((0..3).all(|c| (held[c] - derived[c]).abs() < 1e-5), "{held:?} {derived:?}");
    // The derivation lays a mid grey on one share of the three windows: the reference its filtration bakes reads it so.
    let held_reference = film::derive(&entry.profile, entry.balance, false, false, film::couplers(&entry), Some(held)).unwrap().reference;
    let reference = PORTRA.reference();
    assert!((0..3).all(|c| (held_reference.white[c] - reference.white[c]).abs() < 1e-5), "{held_reference:?}");
    // Twin: a filtration a tenth of a log exposure off bakes another reference.
    let off = film::derive(&entry.profile, entry.balance, false, false, film::couplers(&entry), Some([held[0] + 0.1, 0.0, held[2]])).unwrap().reference;
    assert!((off.white[0] - reference.white[0]).abs() > 1e-3, "{off:?}");
}

#[test]
fn the_grain_is_mixed_by_the_scanners_own_gaze() {
    let gaze = grain::gaze(PORTRA);
    println!("Portra 800's grain through the scanner {gaze:?}");
    assert!(gaze.iter().all(|row| (row.iter().sum::<f32>() - 1.0).abs() < 1e-5), "a grey's grain stays grey");
    // The red channel sees the magenta dye beside its cyan, as a camera's broad channels do on a real scan.
    assert!(gaze[0][1] > 0.15 && gaze[0][0] > gaze[0][1], "{gaze:?}");
    // Twins: a black and white film's layers are one, and a slide is mixed by its projector, not by the scanner.
    assert_eq!(grain::gaze(VirtualStock::of("ilford-hp5-plus")), film::OWN_LAYERS);
    assert_ne!(grain::gaze(VirtualStock::of("kodak-ektachrome-e100")), gaze);
}
