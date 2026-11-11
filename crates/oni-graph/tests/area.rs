//! A stage confined to an area: run over that area alone, onto its input, as it renders run whole.

mod common;

use common::{Shifted, Stamp, noise};
use oni_graph::stages::Gain;
use oni_graph::{Cache, Graph, Image, Rect, Source};

const W: u32 = 300;
const H: u32 = 200;

fn graph(check: &str) -> Option<Graph> {
    Graph::new().map_err(|e| println!("SKIPPED {check}: {e}")).ok()
}

fn source(g: &mut Graph, seed: u64) -> (Source, Vec<[f32; 4]>) {
    let pixels = noise(W, H, seed);
    (g.upload(W, H, &pixels).expect("upload"), pixels)
}

/// Corners, edges, the interior and a single pixel: every way a region meets the extent.
fn regions(extent: Rect) -> Vec<Rect> {
    let (x, y, w, h) = (extent.x, extent.y, extent.w, extent.h);
    vec![
        Rect::new(x, y, 13, 11),
        Rect::new(x + w as i32 - 9, y + h as i32 - 7, 9, 7),
        Rect::new(x + 21, y, 30, 5),
        Rect::new(x, y + 20, 4, 25),
        Rect::new(x + 31, y + 17, 40, 29),
        Rect::new(x + 50, y + 33, 1, 1),
    ]
}

#[test]
fn a_stage_confined_to_an_area_costs_that_area_and_renders_as_it_does_whole() {
    let Some(mut g) = graph("a_stage_confined_to_an_area_costs_that_area_and_renders_as_it_does_whole") else { return };
    let (s, _) = source(&mut g, 21);
    // Twelve patches across the frame, two overlapping and one on the edge.
    let ats: Vec<Rect> = (0..10)
        .map(|k| Rect::new(8 + 29 * k, 12 + (k % 3) * 61, 17 + (k as u32 % 4) * 5, 13 + (k as u32 % 3) * 4))
        .chain([Rect::new(14, 18, 20, 20), Rect::new(280, 185, 20, 15)])
        .collect();
    let patches: Vec<Source> = ats.iter().enumerate().map(|(k, &at)| g.upload_at(at, &noise(at.w, at.h, 100 + k as u64)).unwrap()).collect();
    let chain = |area: &dyn Fn(Rect) -> Option<Rect>, count: usize| {
        let mut image = Image::new(&s).then(Gain([1.1, 0.9, 1.2, 1.0]));
        for (&at, patch) in ats.iter().zip(&patches).take(count) {
            image = image.then_reading(Stamp { at, area: area(at) }, &[Image::new(patch)]);
        }
        image.then(Gain([0.8; 4]))
    };
    let (confined, whole, none) = (chain(&Some, ats.len()), chain(&|_| None, ats.len()), chain(&Some, 0));
    let extent = confined.extent();
    let far = Rect::new(150, 95, 55, 30);
    let near = Rect::new(0, 0, 60, 50);
    assert!(ats.iter().all(|a| a.intersect(&far).is_empty()) && near.contains(&ats[0]) && near.contains(&ats[10]));
    for region in regions(extent).into_iter().chain([extent, far, near]) {
        let part = g.render(&confined, region, None).expect("confined");
        assert_eq!(part, g.render(&whole, region, None).expect("whole"), "{region:?} differs from the stages run whole");
    }

    // Away from every patch a render costs what it costs without them; over two, their areas alone.
    let work = |g: &mut Graph, image: &Image, region: Rect| {
        let before = g.stats().pixels;
        g.render(image, region, None).expect("render");
        g.stats().pixels - before
    };
    assert_eq!(work(&mut g, &confined, far), work(&mut g, &none, far), "patches away from the region cost nothing");
    let held: u64 = ats.iter().map(|a| a.intersect(&near).area()).sum();
    assert_eq!(work(&mut g, &confined, near), work(&mut g, &none, near) + 2 * held, "a patch costs its area, put back");
    // Twin: run whole, each patch costs the whole region, so the counter sees the cost it removes.
    assert_eq!(work(&mut g, &whole, far), work(&mut g, &none, far) + ats.len() as u64 * far.area());

    // A cache across renders, and an input another stage still reads, change nothing either.
    let mut cache = Cache::frontier();
    for (image, region) in [(&confined, near), (&chain(&Some, 11), near), (&confined, extent), (&confined, far)] {
        assert_eq!(g.render(image, region, Some(&mut cache)).expect("cached"), g.render(image, region, None).expect("fresh"), "{region:?}");
    }
    let shared = |area: &dyn Fn(Rect) -> Option<Rect>| {
        let base = Image::new(&s).then(Gain([1.1; 4]));
        base.clone().then_reading(Stamp { at: ats[0], area: area(ats[0]) }, &[Image::new(&patches[0])]).then_reading(Shifted { by: 2, honest: true }, &[base])
    };
    assert_eq!(g.render(&shared(&Some), near, None).expect("shared"), g.render(&shared(&|_| None), near, None).expect("shared whole"));

    // Twin: a stage declaring a column less than it writes leaves that column as its input was.
    let lying = chain(&|at| Some(Rect::new(at.x, at.y, at.w - 1, at.h)), ats.len());
    assert_ne!(g.render(&lying, near, None).expect("lying"), g.render(&whole, near, None).expect("whole"), "a wrong area went unseen");
}
