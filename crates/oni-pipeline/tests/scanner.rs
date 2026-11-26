//! ONI's scanner reading a colour negative: the GPU's film and scan against their scalar reference, the bare film's colour
//! against two real Portra 800 rolls scanned on real cameras, a film's two points and where a grey leaves its black, and
//! the grain mixed by the scanner's own gaze. Each verdict has a twin.

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

/// The two points' ranges, red and blue over green, and the stops under the mid grey where each channel's share of a grey
/// ramp passes 0.005, as the colorimetry's measure off the app gives them (`spec-deux-points.md`).
const TWO_POINTS: [(&str, [f32; 2], [f32; 3]); 3] = [
    ("kodak-portra-800", [0.825, 1.212], [-5.22, -5.29, -5.23]),
    ("kodak-gold-200", [0.843, 1.152], [-5.68, -5.69, -5.15]),
    ("kodak-vision3-250d", [0.829, 1.065], [-6.54, -6.48, -6.13]),
];

/// Where each channel of a grey ramp leaves the black on `stock`'s two points, in stops under the mid grey; the scene
/// grey lit by `tint` a channel.
fn leaves_black(stock: VirtualStock, tint: [f32; 3]) -> [f32; 3] {
    let k = oni_pipeline::conversion::stage47(&film::on_reference(stock, &stock.reference()));
    let code = |l: f32| if l <= 0.003_130_8 { 12.92 * l } else { 1.055 * l.powf(1.0 / 2.4) - 0.055 };
    let mut out = [f32::NAN; 3];
    for i in (0..=900).rev() {
        let stops = -(i as f32) / 100.0;
        let v = film::GREY * stops.exp2();
        let shown = cpu::stage47(film::shot(stock, [v * tint[0], v * tint[1], v * tint[2], 1.0], 0.0), &k);
        for c in 0..3 {
            if out[c].is_nan() && code(shown[c]) > 0.005 {
                out[c] = stops;
            }
        }
    }
    out
}

#[test]
fn a_grey_leaves_the_black_on_the_three_channels_together() {
    for (key, ranges, exits) in TWO_POINTS {
        let stock = VirtualStock::of(key);
        let r = stock.reference();
        let span: [f32; 3] = std::array::from_fn(|c| r.white[c] - r.black[c]);
        let (red, blue) = (span[0] / span[1], span[2] / span[1]);
        let left = leaves_black(stock, [1.0; 3]);
        println!("{key}: ranges {red:.3} {blue:.3}, a grey leaves the black at {left:?} stops");
        assert!((red - ranges[0]).abs() < 0.01 && (blue - ranges[1]).abs() < 0.01, "{key}: {red} {blue}");
        assert!((0..3).all(|c| (left[c] - exits[c]).abs() < 0.1), "{key}: {left:?}");
        // Twin: the reference format 79 laid, its layers offset, holds other ranges.
        let was = oni_settings::migrate::two_points::offset(stock);
        let was_span: [f32; 3] = std::array::from_fn(|c| was.white[c] - was.black[c]);
        assert!((was_span[2] / was_span[1] - ranges[1]).abs() > 0.01, "twin {key}: {was:?}");
    }
    // Portra 800's three channels leave together; twin: a grey lit by the layers' former offsets leaves them apart.
    let portra = leaves_black(PORTRA, [1.0; 3]);
    let spread = |l: [f32; 3]| l.iter().cloned().fold(f32::MIN, f32::max) - l.iter().cloned().fold(f32::MAX, f32::min);
    assert!(spread(portra) < 0.1, "{portra:?}");
    let offset = leaves_black(PORTRA, [10f32.powf(0.22651), 1.0, 10f32.powf(0.34729)]);
    println!("a grey lit by the former offsets leaves at {offset:?}");
    assert!(spread(offset) > 0.3, "twin: {offset:?}");
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
