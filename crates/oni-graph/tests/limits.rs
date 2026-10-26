//! The graph at its limits: a request reading past its input, a sheet past the uniform ring or the
//! texture side.

mod common;

use common::{Reaching, cut, error, noise};
use oni_graph::cpu::Plane;
use oni_graph::stages::Gain;
use oni_graph::{Graph, Image, Rect, Source};

const W: u32 = 300;
const H: u32 = 200;

fn graph(check: &str) -> Option<Graph> {
    Graph::new().map_err(|e| println!("SKIPPED {check}: {e}")).ok()
}

fn source(g: &mut Graph, seed: u64) -> (Source, Vec<[f32; 4]>) {
    let pixels = noise(W, H, seed);
    (g.upload(W, H, &pixels).expect("upload"), pixels)
}

#[test]
fn a_region_whose_input_lies_past_the_extent_reads_the_nearest_edge() {
    let Some(mut g) = graph("a_region_whose_input_lies_past_the_extent_reads_the_nearest_edge") else { return };
    let (s, pixels) = source(&mut g, 11);
    // Behind a stage, so the input is a texture of the declared region and not the whole source.
    let image = Image::new(&s).then(Gain([1.0; 4])).then(Reaching { by: (250, 0), honest: true });
    let full = g.render(&image, image.extent(), None).expect("full render");
    // Columns 60 to 100 read 310 to 350, wholly past the 300 columns: the edge column, row by row.
    let region = Rect::new(60, 20, 40, 30);
    let part = g.render(&image, region, None).expect("a region reading past the extent renders");
    assert_eq!(part, cut(&full, image.extent(), region), "the region is the cut of the full render");
    let plane = Plane::new(s.extent(), pixels);
    let reference: Vec<[f32; 4]> = (region.y..region.bottom()).flat_map(|y| (region.x..region.right()).map(move |x| (x, y))).map(|(x, y)| plane.load(x + 250, y)).collect();
    assert_eq!(part, reference, "the kernels' clamped load");
    // Twin: one pixel of that reach, the nearest point, repeats it down the rows.
    let liar = Image::new(&s).then(Gain([1.0; 4])).then(Reaching { by: (250, 0), honest: false });
    let lied = g.render(&liar, region, None).expect("region render");
    assert!(error(&lied, &reference) > 1e-3, "a single edge pixel went unseen");
}

#[test]
fn a_sheet_staging_past_the_uniform_ring_draws_each_tile_with_its_own_uniforms() {
    let Some(mut g) = graph("a_sheet_staging_past_the_uniform_ring_draws_each_tile_with_its_own_uniforms") else { return };
    let Ok(gpu) = oni_graph::gpu() else { return println!("SKIPPED the ring: no device") };
    // Each Gain stages its frame (48 bytes) and its gains (16), each at the device's alignment.
    let align = u64::from(gpu.device.limits().min_uniform_buffer_offset_alignment).max(16);
    let per_dispatch = 48u64.div_ceil(align) * align + 16u64.div_ceil(align) * align;
    const RING: u64 = 4 << 20;
    const STAGES: usize = 40;
    let tiles = (RING * 5 / 4 / per_dispatch / STAGES as u64) as usize + 1;
    let (side, columns) = (4u32, 64usize);
    let pixels = noise(side, side, 13);
    let s = g.upload(side, side, &pixels).expect("upload");
    let gain = |tile: usize, stage: usize| 1.0 + ((tile * 7 + stage * 3) % 11) as f32 * 1e-3 - if stage == STAGES - 1 { 0.5 - tile as f32 * 1e-3 } else { 0.0 };
    let images: Vec<Image> = (0..tiles).map(|t| (0..STAGES).fold(Image::new(&s), |i, k| i.then(Gain([gain(t, k), gain(t, k), 1.0, 1.0])))).collect();
    let at = |t: usize| (((t % columns) as u32 * side) as i32, ((t / columns) as u32 * side) as i32);
    let size = (columns as u32 * side, tiles.div_ceil(columns) as u32 * side);
    let before = g.stats().dispatches;
    let laid: Vec<(&Image, (i32, i32))> = images.iter().enumerate().map(|(t, i)| (i, at(t))).collect();
    let sheet = g.laid_out(&laid, size).expect("the sheet");
    let staged = (g.stats().dispatches - before) * per_dispatch;
    assert!(staged > RING, "one submission staged {staged} bytes, under the ring");
    let read = g.read_back(&[&sheet]).expect("read back").remove(0);
    let sheet_rect = Rect::new(0, 0, size.0, size.1);
    let expected = |t: usize| -> Vec<[f32; 4]> {
        pixels.iter().map(|p| (0..STAGES).fold(*p, |q, k| [q[0] * gain(t, k), q[1] * gain(t, k), q[2], q[3]])).collect()
    };
    let worst = (0..tiles).map(|t| error(&cut(&read, sheet_rect, Rect::new(at(t).0, at(t).1, side, side)), &expected(t))).fold(0.0, f32::max);
    assert!(worst < 1e-5, "a tile drew another's uniforms: error {worst}");
    // Twin: each tile judged against its neighbour's gains.
    let twin = error(&cut(&read, sheet_rect, Rect::new(at(0).0, at(0).1, side, side)), &expected(1));
    assert!(twin > 1e-4, "a neighbour's uniforms went unseen: error {twin}");
}

#[test]
fn a_sheet_past_the_texture_side_is_refused() {
    let Some(mut g) = graph("a_sheet_past_the_texture_side_is_refused") else { return };
    let (s, _) = source(&mut g, 14);
    let image = Image::new(&s).then(Gain([0.5, 1.0, 1.0, 1.0]));
    let side = g.max_side();
    assert!(matches!(g.laid_out(&[(&image, (0, 0))], (W, side + 1)), Err(oni_graph::Error::TooLarge)), "a sheet one row past the side");
    // Twin: the sheet at the side itself is laid.
    assert!(g.laid_out(&[(&image, (0, 0))], (W, side)).is_ok());
}
