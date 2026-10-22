//! The texture pool: a shape rendered again takes its textures back, an idle shape is freed.

mod common;

use common::noise;
use oni_graph::stages::{BoxBlur3, Gain};
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

/// The bytes a pooled texture holding `w` × `h` takes, its sides rounded up to the pool's step.
fn held(w: u32, h: u32) -> i64 {
    let up = |v: u32| i64::from(v.div_ceil(oni_graph::STEP) * oni_graph::STEP);
    16 * up(w) * up(h)
}

#[test]
fn the_pool_recycles_and_frees() {
    let Some(mut g) = graph("the_pool_recycles_and_frees") else { return };
    let (s, _) = source(&mut g, 5);
    let source_bytes = g.stats().live_bytes;
    let image = Image::new(&s).then(Gain([2.0; 4])).then(BoxBlur3).then(Gain([0.5; 4]));
    let region = Rect::new(10, 10, 250, 180);
    g.render(&image, region, None).expect("render");
    let created = g.stats().textures_created;
    for _ in 0..3 {
        g.render(&image, region, None).expect("render");
    }
    assert_eq!(g.stats().textures_created, created, "a same-shaped render allocated again");
    // A small read between two renders of the chain leaves its textures to be taken again.
    let corner = Rect::new(0, 0, 4, 4);
    for _ in 0..3 {
        g.render(&Image::new(&s).then(Gain([2.0; 4])), corner, None).expect("render");
        g.render(&image, region, None).expect("render");
    }
    let between = g.stats().textures_created;
    assert!(between <= created + 1, "a read between two renders cost the chain its textures: {}", between - created);
    // Stages a few pixels apart in reach share one rounded size: the blur's margin costs no texture.
    let near = Rect::new(12, 12, 246, 176);
    g.render(&image, near, None).expect("render");
    assert_eq!(g.stats().textures_created, between, "a region a few pixels smaller allocated again");

    // A new shape for `IDLE_RENDERS` renders: the old shape's textures, unused by any, are freed.
    let other = Rect::new(0, 0, 50, 40);
    for _ in 0..oni_graph::IDLE_RENDERS {
        g.render(&image, other, None).expect("render");
    }
    let kept = g.stats().live_bytes - source_bytes;
    let expected = 2 * 3 * held(other.w, other.h);
    assert!(kept <= expected, "{kept} bytes held past the source, at most {expected} for the live shape");
    // Twin: the old shape's three textures alone pass that bound, so a pool keeping them is seen.
    assert!(3 * held(region.w, region.h) > expected, "the bound cannot see the old shape kept");

    // Twin: regions of a fresh rounded shape each time make the counter climb.
    let before = g.stats().textures_created;
    for w in [100, 200, 290] {
        g.render(&image, Rect::new(0, 0, w, 40), None).expect("render");
    }
    assert!(g.stats().textures_created >= before + 3, "the counter misses allocations");
    // A zoom's shapes, new every render, hold no more idle than the budget; the newest stay.
    let shapes = [(100, 80), (200, 80), (290, 80), (100, 180), (200, 180), (290, 180)];
    let budget = 3 * held(256, 256) as u64;
    g.idle_budget(budget);
    for (w, h) in shapes {
        g.render(&image, Rect::new(0, 0, w, h), None).expect("render");
    }
    let idle = g.stats().live_bytes - source_bytes;
    let last = 3 * held(290, 180);
    assert!(idle as u64 <= budget + last as u64, "{idle} bytes past a {budget} budget");
    // Twin: without the budget every shape's three textures stay.
    let every: i64 = shapes.iter().map(|&(w, h)| 3 * held(w, h)).sum();
    assert!(every as u64 > budget + last as u64, "the bound cannot see a pool keeping every shape");
    g.purge();
    assert_eq!(g.stats().idle_textures, 0);
    assert_eq!(g.stats().live_bytes, source_bytes, "purged, only the source remains");
}

#[test]
fn a_source_in_half_floats_reads_as_its_full_floats_at_half_the_memory() {
    let Some(mut g) = graph("a_source_in_half_floats_reads_as_its_full_floats_at_half_the_memory") else { return };
    let pixels = noise(W, H, 9);
    let full = g.upload(W, H, &pixels).expect("upload");
    let before = g.stats().live_bytes;
    let half = g.upload_half(W, H, &pixels).expect("upload");
    assert_eq!(g.stats().live_bytes - before, 8 * (W * H) as i64, "four half floats a pixel");
    assert_eq!((full.precision(), half.precision()), (oni_graph::Precision::Full, oni_graph::Precision::Half));
    // Relative to the value: half floats keep 11 significant bits.
    let gap = |a: &[[f32; 4]], b: &[[f32; 4]]| a.iter().zip(b).flat_map(|(p, q)| (0..3).map(move |c| (p[c] - q[c]).abs() / p[c].abs().max(1e-3))).fold(0.0f32, f32::max);
    let whole = Rect::new(0, 0, W, H);
    let bare = g.render(&Image::new(&half), whole, None).expect("a bare half source reads back");
    assert!(gap(&pixels, &bare) < 1e-3, "{}", gap(&pixels, &bare));
    let chain = |s: &Source| Image::new(s).then(Gain([2.0; 4])).then(BoxBlur3).lanczos(0.5);
    let (ours, theirs) = (chain(&half), chain(&full));
    let (a, b) = (g.render(&ours, ours.extent(), None).unwrap(), g.render(&theirs, theirs.extent(), None).unwrap());
    assert!(gap(&b, &a) < 2e-3, "{}", gap(&b, &a));
    let packed = g.materialise_half(&theirs).expect("packed");
    assert_eq!(packed.precision(), oni_graph::Precision::Half);
    let read = g.render(&Image::new(&packed), packed.extent(), None).unwrap();
    assert!(gap(&b, &read) < 2e-3, "{}", gap(&b, &read));
    // Twin: a chain a gain apart is past the bound, so the bound sees a wrong reading.
    let other = Image::new(&full).then(Gain([2.01; 4])).then(BoxBlur3).lanczos(0.5);
    let c = g.render(&other, other.extent(), None).unwrap();
    assert!(gap(&b, &c) > 2e-3);
}
