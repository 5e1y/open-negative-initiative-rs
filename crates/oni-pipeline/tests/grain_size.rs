//! Grain size is a grain's width on the film: its field's grains are as wide as it says, every resolution lays
//! the same grain on the picture, and a photo of an earlier format keeps the grain it laid; each with a twin.

use oni_graph::Rect;
use oni_pipeline::grain;
use oni_render::{GRAIN_NODES, Grain, cpu};
use oni_settings::blocks::Grain as GrainBlock;
use oni_settings::migrate::grain_size::{self, microns};
use oni_settings::migrate::{Refusal, chain::GRAIN_ON_FILM_VERSION};
use oni_settings::{ConversionMode, PipelineSettings};
use serde_json::json;

const IDENTITY: [[f32; 3]; 2] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];

fn grained(size: f32) -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Digital, true);
    (s.grain.amount, s.grain.size) = (0.5, size);
    s
}

/// The field's mean run of one sign along rows a grain apart and more, in cells of `cell` pixels.
fn mean_run(cell: f32) -> f64 {
    let k = Grain::on(IDENTITY, cell, 1.0, 0.5, [1.0; GRAIN_NODES]);
    let grey = [0.18, 0.18, 0.18, 1.0];
    let (mut runs, mut length) = (0usize, 0usize);
    for row in 0..300 {
        let y = row as f32 * 2.37 * cell + 0.5;
        let lighter: Vec<bool> = (0..4000).map(|x| cpu::grain(grey, grey, [x as f32 + 0.5, y], &k)[1] > grey[1]).collect();
        let changes: Vec<usize> = (1..lighter.len()).filter(|&i| lighter[i] != lighter[i - 1]).collect();
        if let (Some(first), Some(last)) = (changes.first(), changes.last()) {
            runs += changes.len() - 1;
            length += last - first;
        }
    }
    length as f64 / runs as f64 / f64::from(cell)
}

#[test]
fn a_grain_is_as_wide_as_its_size_says() {
    let measured = mean_run(16.0);
    println!("a grain's mean width: {measured:.3} cells");
    assert!((measured / GrainBlock::CLUMP_CELLS - 1.0).abs() < 0.02, "{measured} cells against {}", GrainBlock::CLUMP_CELLS);
    // Twin: the cell itself taken for a grain's width is refused by the same judge.
    assert!((measured / 1.0 - 1.0).abs() >= 0.02);
}

#[test]
fn every_resolution_lays_the_same_grain_on_the_picture() {
    let s = grained(GrainBlock::SIZE_REST);
    let cells = |w: u32, h: u32| {
        let k = grain::grain(&s, Rect::new(0, 0, w, h), 1.0).unwrap();
        f64::from(w.max(h)) * f64::from(k.per_cell)
    };
    // 24 and 47 Mpx, a portrait, and a reduced render of the 47: as many cells along the long side.
    let (z, s1r) = (cells(6048, 4032), cells(8368, 5584));
    assert!((z / s1r - 1.0).abs() < 1e-6 && (cells(4032, 6048) / z - 1.0).abs() < 1e-6, "{z} {s1r}");
    // A RAW decoded at half size holds the same frame on half the pixels: as many cells again.
    assert!((cells(3024, 2016) / z - 1.0).abs() < 1e-6, "{} {z}", cells(3024, 2016));
    let reduced = grain::grain(&s, Rect::new(0, 0, 2092, 1396), 0.25).unwrap();
    assert!((8368.0 * f64::from(reduced.per_cell) / s1r - 1.0).abs() < 1e-6);
    // Its width in pixels follows the resolution: 20 µm of a 36 mm frame.
    assert!((GrainBlock::pixels(20.0, 6048.0) - 3.36).abs() < 1e-9 && (GrainBlock::pixels(20.0, 8368.0) - 4.6489).abs() < 1e-4);
    // Twin: a cell fixed in pixels, as a share of no picture, lays a coarser grain on the 24 Mpx frame.
    let fixed = |w: u32| f64::from(w) / (GrainBlock::pixels(20.0, 8368.0) / GrainBlock::CLUMP_CELLS);
    assert!((fixed(6048) / fixed(8368) - 1.0).abs() > 0.2);
}

/// The former cell, in pixels, of a share on a scan `w` wide, and its blur's deviation.
fn former(share: f32, w: f64) -> (f64, f64) {
    let grow = 4f64.powf(f64::from(share));
    (w * grow / 6000.0, 0.5 * w / 6000.0 * (grow - 1.0))
}

#[test]
fn a_photo_of_an_earlier_format_keeps_the_grain_it_laid() {
    let older = GRAIN_ON_FILM_VERSION - 1;
    for (share, (w, h)) in [(0.0, (6048, 4032)), (0.4, (6048, 4032)), (0.5, (4032, 6048)), (1.0, (5584, 8368)), (0.75, (8368, 5584))] {
        let stored = json!({ "grain": { "amount": 0.5, "size": share } });
        let scan = Rect::new(0, 0, w, h);
        let moved = grain_size::caught_up(older, grained(0.5), Some(&stored), &|_| Ok((f64::from(w), f64::from(h)))).unwrap();
        let k = grain::grain(&moved, scan, 1.0).unwrap();
        let (cell, sigma) = former(share, f64::from(w));
        assert!((1.0 / f64::from(k.per_cell) / cell - 1.0).abs() < 1e-5, "{share} on {w}×{h}: cell {} against {cell}", 1.0 / k.per_cell);
        assert!((grain::dissolve(&moved, scan) - sigma).abs() < 1e-5, "{share} on {w}×{h}: blur");
        // Twin: a portrait read as wider than tall lays a grain half as coarse again.
        if h > w {
            let flat = grain_size::unfiled(older, grained(0.5), Some(&stored));
            let wrong = grain::grain(&flat, scan, 1.0).unwrap();
            assert!((1.0 / f64::from(wrong.per_cell) / cell - 1.0).abs() > 0.4);
        }
    }
    // A grain laid asks the file its shape; a file that cannot say holds the step.
    let laid = json!({ "grain": { "amount": 0.5, "size": 0.5 } });
    assert_eq!(grain_size::caught_up(older, grained(0.5), Some(&laid), &|_| Err(Refusal::Away)), Err(Refusal::Away));
    assert!((microns(0.5, 1.0) - 31.68).abs() < 1e-3);
}
