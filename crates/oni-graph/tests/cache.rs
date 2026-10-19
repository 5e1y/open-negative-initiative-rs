//! The canvas's cache: only the results the next notch starts from are kept.

use oni_graph::stages::{BoxBlur3, Gain};
use oni_graph::{Cache, Graph, Image, Rect, Source};

fn graph(check: &str) -> Option<Graph> {
    Graph::new().map_err(|e| println!("SKIPPED {check}: {e}")).ok()
}

fn source(g: &mut Graph, seed: u64) -> (Source, Vec<[f32; 4]>) {
    let pixels: Vec<[f32; 4]> = (0..300u64 * 200)
        .map(|i| {
            let v = ((i.wrapping_mul(2_654_435_761).wrapping_add(seed * 97)) % 1000) as f32 / 1000.0;
            [v, 1.0 - v, v * 0.5, 1.0]
        })
        .collect();
    (g.upload(300, 200, &pixels).expect("upload"), pixels)
}

#[test]
fn a_frontier_cache_keeps_only_what_the_next_notch_starts_from() {
    let Some(mut g) = graph("a_frontier_cache_keeps_only_what_the_next_notch_starts_from") else { return };
    let (s, _) = source(&mut g, 5);
    let image = |a: f32, b: f32| Image::new(&s).then(Gain([a; 4])).then(BoxBlur3).then(Gain([b; 4]));
    let region = Rect::new(40, 30, 120, 90);
    let mut cache = Cache::frontier();
    let rendered = |g: &mut Graph, img: &Image, cache: &mut Cache| {
        let before = g.stats().dispatches;
        let out = g.materialise_cached(img, region, Some(cache)).expect("cached render");
        let ran = g.stats().dispatches - before;
        let cached = g.render(img, region, None).expect("fresh");
        let shown = g.render(&Image::new(&out), region, None).expect("read back");
        assert_eq!(shown, cached, "a cached render differs from a fresh one");
        ran
    };
    assert_eq!(rendered(&mut g, &image(1.0, 1.0), &mut cache), 3, "the first render runs every stage");
    assert_eq!(rendered(&mut g, &image(1.0, 1.1), &mut cache), 3, "the hand's first notch: nothing kept yet");
    assert_eq!(cache.len(), 1, "only the blur the last gain reads");
    assert_eq!(rendered(&mut g, &image(1.0, 1.2), &mut cache), 1, "the next notch runs the last gain alone");
    assert_eq!(rendered(&mut g, &image(1.0, 1.3), &mut cache), 1);
    // Twin: a notch on the first gain moves everything, and the kept blur is let go.
    assert_eq!(rendered(&mut g, &image(1.1, 1.3), &mut cache), 3, "an early change reruns all");
    assert_eq!(rendered(&mut g, &image(1.2, 1.3), &mut cache), 3, "the source is never kept");
    assert_eq!(cache.len(), 0);
}
