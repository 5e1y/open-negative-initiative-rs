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

#[test]
fn the_pool_recycles_and_frees() {
    let Some(mut g) = graph("the_pool_recycles_and_frees") else { return };
    let (s, _) = source(&mut g, 5);
    let source_bytes = g.stats().live_bytes;
    let image = Image::new(&s).then(Gain([2.0; 4])).then(BoxBlur3).then(Gain([0.5; 4]));
    let region = Rect::new(10, 10, 100, 80);
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

    // A new shape for `IDLE_RENDERS` renders: the old shape's textures, unused by any, are freed.
    let other = Rect::new(0, 0, 50, 40);
    for _ in 0..oni_graph::IDLE_RENDERS {
        g.render(&image, other, None).expect("render");
    }
    let held = g.stats().live_bytes - source_bytes;
    let expected = 2 * 3 * 16 * other.area() as i64;
    assert!(held <= expected, "{held} bytes held past the source, at most {expected} for the live shape");
    // Twin: the old shape's three textures alone pass that bound, so a pool keeping them is seen.
    assert!(3 * 16 * region.area() as i64 > expected, "the bound cannot see the old shape kept");

    // Twin: regions of a fresh shape each time make the counter climb.
    let before = g.stats().textures_created;
    for i in 0..3 {
        g.render(&image, Rect::new(0, 0, 60 + i, 40), None).expect("render");
    }
    assert!(g.stats().textures_created >= before + 3, "the counter misses allocations");
    // A zoom's shapes, new every render, hold no more idle than the budget; the newest stay.
    let budget = 3 * 16 * region.area() as u64;
    g.idle_budget(budget);
    for i in 0..6 {
        g.render(&image, Rect::new(0, 0, 100 + i, 80), None).expect("render");
    }
    let idle = g.stats().live_bytes - source_bytes;
    assert!(idle as u64 <= budget + 3 * 16 * 105 * 80, "{idle} bytes past a {budget} budget");
    g.render(&image, Rect::new(0, 0, 105, 80), None).expect("render");
    // Twin: without the budget the eight renders' shapes stay, three textures each.
    assert!(6 * 3 * 16 * 100 * 80 > budget + 3 * 16 * 105 * 80, "the bound cannot see a pool keeping every shape");
    g.purge();
    assert_eq!(g.stats().idle_textures, 0);
    assert_eq!(g.stats().live_bytes, source_bytes, "purged, only the source remains");
}
