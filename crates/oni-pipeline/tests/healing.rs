//! A frame's corrections laid together against each one laid in turn through stages of its own: the
//! same picture at every scale, in passes their number leaves alone.

mod common;

use common::{V4, error, graph, noise};
use oni_graph::{Graph, Image, Rect, Source};
use oni_pipeline::stages::{self, Patched, Stroke, Translate};
use oni_pipeline::{Inputs, Pipeline, corrections};
use oni_render::StrokeMask;
use oni_settings::axis::pipeline::BLUR_FLOOR;
use oni_settings::cg::Point;
use oni_settings::frame::Correction;
use oni_settings::{ConversionMode, PipelineSettings};

/// The full-resolution frame the corrections were painted on.
const FULL: (u32, u32) = (2400, 1600);

/// The most the corrections laid together may lie from each laid in turn: Metal rounds a kernel
/// compiled for many instances a few ulps apart from the one compiled for one.
const ULPS: f32 = 1e-5;

/// Dots and strokes across the frame: two pairs that overlap, one on the edge, brushes from the
/// smallest, whose mask stays hard on a small copy, to one whose patch is a hundred times its area.
fn painted() -> Vec<Correction> {
    let stroke = |points: &[(f64, f64)], radius: f64| Correction { radius, ..Correction::new(points.iter().map(|&(x, y)| Point::new(x, y)).collect()) };
    let mut list: Vec<Correction> = (0..14).map(|k| stroke(&[(0.06 + 0.065 * k as f64, 0.1 + 0.27 * (k % 3) as f64), (0.062 + 0.065 * k as f64, 0.104 + 0.27 * (k % 3) as f64)], 4.0 + 3.0 * (k % 5) as f64)).collect();
    list.push(stroke(&[(0.3, 0.6), (0.34, 0.63), (0.39, 0.61), (0.45, 0.66)], 30.0));
    list.push(stroke(&[(0.33, 0.62), (0.36, 0.6)], 12.0));
    list.push(stroke(&[(0.34, 0.63), (0.341, 0.631)], 20.0));
    list.push(stroke(&[(0.995, 0.99), (0.999, 0.996)], 16.0));
    list
}

/// Each correction's patch at full resolution, as the search would have written it.
fn patches(g: &mut Graph, list: &[Correction]) -> Vec<Source> {
    let extent = [0.0, 0.0, f64::from(FULL.0), f64::from(FULL.1)];
    (list.iter().enumerate())
        .map(|(k, c)| {
            let r = corrections::patch_rect(c, extent);
            g.upload(r[2] as u32, r[3] as u32, &noise(r[2] as u32, r[3] as u32, 40 + k as u64, 0.1, 0.9)).expect("a patch")
        })
        .collect()
}

/// The frame with each correction laid in turn, an image of its own through its own stages: what the
/// batch stands for.
fn in_turn(g: &mut Graph, frame: &Source, list: &[(&Correction, &Source)], scale: f64) -> Vec<V4> {
    let mut out = Image::new(frame);
    let framed = out.extent();
    for (c, patch) in list {
        let (tile, shift) = corrections::placed(c, (patch.extent().w, patch.extent().h), framed, scale);
        let mut placed = Image::new(patch);
        if scale != 1.0 {
            placed = placed.lanczos_shifted(scale, shift);
        }
        let at = placed.extent();
        let placed = placed.then(Translate(tile.x - at.x, tile.y - at.y));
        let tile = placed.extent();
        let points = corrections::stroke_points(c, framed);
        let row = g.upload(points.len() as u32, 1, &points).expect("a stroke");
        let hard = Image::new(&row).then(Stroke { k: StrokeMask { radius: (c.radius * scale) as f32, count: points.len() }, extent: tile });
        let sigma = corrections::feather(c, scale);
        let mask = if sigma <= BLUR_FLOOR { hard } else { stages::blurred(hard, sigma, true) };
        out = out.then_reading(Patched { patch: tile, mask: tile }, &[placed, mask]);
    }
    whole(g, &out)
}

fn settings(list: &[Correction]) -> PipelineSettings {
    PipelineSettings { corrections: list.to_vec(), ..PipelineSettings::rest(ConversionMode::Negative, true) }
}

/// The frame at `scale` as a source of its own, and the pipeline's framed image on it.
fn framed(g: &mut Graph, pipeline: &mut Pipeline, list: &[Correction], held: &[Source], scale: f64, seed: u64) -> (Source, Image) {
    let (w, h) = ((f64::from(FULL.0) * scale).round() as u32, (f64::from(FULL.1) * scale).round() as u32);
    let frame = g.upload(w, h, &noise(w, h, seed, 0.05, 0.95)).expect("the frame");
    let inputs = Inputs { patches: list.iter().zip(held).map(|(c, p)| (c.id, p)).collect(), ..Inputs::default() };
    let image = pipeline.framed(g, Image::new(&frame), &settings(list), f64::from(w) / f64::from(FULL.0), &inputs).expect("the recipe builds");
    (frame, image)
}

fn whole(g: &mut Graph, image: &Image) -> Vec<V4> {
    g.render(image, image.extent(), None).expect("the frame renders")
}

#[test]
fn corrections_laid_together_are_each_laid_in_turn_at_every_scale() {
    let Some(mut g) = graph("corrections_laid_together_are_each_laid_in_turn_at_every_scale") else { return };
    let mut pipeline = Pipeline::new();
    let list = painted();
    let held = patches(&mut g, &list);
    // The frame itself, a third of it, and a copy small enough that the small brushes' masks stay hard.
    for (scale, seed) in [(1.0, 3), (0.37, 4), (0.06, 5)] {
        let (frame, together) = framed(&mut g, &mut pipeline, &list, &held, scale, seed);
        let scale = f64::from(frame.extent().w) / f64::from(FULL.0);
        let pairs: Vec<(&Correction, &Source)> = list.iter().zip(&held).collect();
        let reference = in_turn(&mut g, &frame, &pairs, scale);
        let ours = whole(&mut g, &together);
        let bare = whole(&mut g, &Image::new(&frame));
        let e = error(&ours, &reference);
        // Twins: the frame with no correction, one left out, and the pair that meets laid the other way.
        let fewer = in_turn(&mut g, &frame, &pairs[..pairs.len() - 1], scale);
        let mut turned = pairs.clone();
        turned.swap(14, 15);
        let swapped = in_turn(&mut g, &frame, &turned, scale);
        let twins = [error(&ours, &bare), error(&ours, &fewer), error(&ours, &swapped)];
        println!("scale {scale:.3}: {e:.2e} from the corrections laid in turn; twins {twins:?}");
        assert!(e < ULPS, "at {scale}: laid together, the corrections lie {e:.2e} from each laid in turn");
        assert!(twins.iter().all(|t| *t > 1e-3), "at {scale}: a twin passes, {twins:?}");
        // A region is the whole render's own pixels there, with and without the pipeline's kept recipe.
        let e = together.extent();
        for region in [Rect::new(e.x, e.y, e.w / 3, e.h / 2), Rect::new(e.x + e.w as i32 / 3, e.y + e.h as i32 / 2, e.w / 3, e.h / 3), Rect::new(e.right() - 9, e.bottom() - 7, 9, 7)] {
            let part = g.render(&together, region, None).expect("a region");
            let cut: Vec<V4> = (0..region.h).flat_map(|y| (0..region.w).map(move |x| ((region.y - e.y) as u32 + y) * e.w + (region.x - e.x) as u32 + x)).map(|i| ours[i as usize]).collect();
            assert_eq!(part, cut, "at {scale}: {region:?} is not the whole render's part");
        }
    }
}

#[test]
fn a_frame_s_corrections_cost_passes_their_number_leaves_alone() {
    let Some(mut g) = graph("a_frame_s_corrections_cost_passes_their_number_leaves_alone") else { return };
    let mut pipeline = Pipeline::new();
    // Dots of one brush, apart from each other.
    let dots = |n: usize| -> Vec<Correction> {
        (0..n).map(|k| (0.03 + 0.047 * (k % 20) as f64, 0.05 + 0.09 * (k / 20) as f64)).map(|(x, y)| Correction { radius: 10.0, ..Correction::new(vec![Point::new(x, y), Point::new(x + 0.001, y)]) }).collect()
    };
    let passes = |g: &mut Graph, pipeline: &mut Pipeline, list: &[Correction], scale: f64| {
        let held = patches(g, list);
        let (frame, image) = framed(g, pipeline, list, &held, scale, 9);
        let before = g.stats().dispatches;
        let rendered = whole(g, &image);
        let together = g.stats().dispatches - before;
        let pairs: Vec<(&Correction, &Source)> = list.iter().zip(&held).collect();
        let before = g.stats().dispatches;
        let turned = in_turn(g, &frame, &pairs, f64::from(frame.extent().w) / f64::from(FULL.0));
        assert!(error(&rendered, &turned) < ULPS);
        (together, g.stats().dispatches - before)
    };
    for scale in [1.0, 0.37] {
        let (few, few_in_turn) = passes(&mut g, &mut pipeline, &dots(5), scale);
        let (many, many_in_turn) = passes(&mut g, &mut pipeline, &dots(200), scale);
        println!("scale {scale}: {few} passes for 5 corrections and {many} for 200; in turn {few_in_turn} and {many_in_turn}");
        assert_eq!(few, many, "at {scale}: 200 corrections cost more passes than 5");
        // Twin: laid in turn, each correction costs passes of its own, which the counter sees.
        assert!(many_in_turn >= few_in_turn + 195 * 4, "at {scale}: {few_in_turn} then {many_in_turn} passes in turn");
    }
}

#[test]
fn corrections_past_one_sheet_of_patches_are_laid_in_their_order() {
    // A texture side small enough that 150 patches take two sheets.
    let Ok(mut g) = Graph::with_max_side(256).map_err(|e| println!("SKIPPED corrections_past_one_sheet_of_patches_are_laid_in_their_order: {e}")) else { return };
    let mut pipeline = Pipeline::new();
    let (w, h) = (250u32, 180u32);
    // Dots in a row, each meeting the next, so their order shows.
    let list: Vec<Correction> = (0..150).map(|k| (0.04 + 0.006 * k as f64, 0.2 + 0.25 * (k % 3) as f64)).map(|(x, y)| Correction { radius: 4.0, ..Correction::new(vec![Point::new(x, y), Point::new(x + 0.004, y)]) }).collect();
    let extent = [0.0, 0.0, f64::from(w), f64::from(h)];
    let sizes: Vec<(u32, u32)> = list.iter().map(|c| corrections::patch_rect(c, extent)).map(|r| (r[2] as u32, r[3] as u32)).collect();
    let held: Vec<Source> = sizes.iter().enumerate().map(|(k, s)| g.upload(s.0, s.1, &noise(s.0, s.1, 70 + k as u64, 0.1, 0.9)).expect("a patch")).collect();
    assert!(sizes.iter().map(|s| u64::from(s.0) * u64::from(s.1)).sum::<u64>() > 256 * 256, "the patches fit one sheet: the check sees no second one");
    let frame = g.upload(w, h, &noise(w, h, 8, 0.05, 0.95)).expect("the frame");
    let inputs = Inputs { patches: list.iter().zip(&held).map(|(c, p)| (c.id, p)).collect(), ..Inputs::default() };
    let together = pipeline.framed(&mut g, Image::new(&frame), &settings(&list), 1.0, &inputs).expect("the recipe builds");
    let ours = whole(&mut g, &together);
    let pairs: Vec<(&Correction, &Source)> = list.iter().zip(&held).collect();
    let reference = in_turn(&mut g, &frame, &pairs, 1.0);
    assert!(error(&ours, &reference) < ULPS, "{} from the corrections laid in turn", error(&ours, &reference));
    // Twin: two that meet across the sheets' seam laid the other way round are another picture.
    let mut turned = pairs.clone();
    let seam = (1..pairs.len()).find(|&k| sizes[..k].iter().map(|s| u64::from(s.0) * u64::from(s.1)).sum::<u64>() > 200 * 256).expect("a seam");
    turned.swap(seam - 3, seam);
    let swapped = in_turn(&mut g, &frame, &turned, 1.0);
    assert!(error(&ours, &swapped) > 1e-3, "an order turned went unseen: {}", error(&ours, &swapped));
    // A subset of the frame's corrections, as a search names, reads the sheets already gathered.
    let before = g.stats().copies;
    let fewer: Vec<Correction> = list.iter().step_by(2).cloned().collect();
    let part = pipeline.framed(&mut g, Image::new(&frame), &settings(&fewer), 1.0, &inputs).expect("the recipe builds");
    let half: Vec<(&Correction, &Source)> = pairs.iter().step_by(2).copied().collect();
    assert!(error(&whole(&mut g, &part), &in_turn(&mut g, &frame, &half, 1.0)) < ULPS);
    assert_eq!(g.stats().copies, before, "a subset's patches were gathered again");
}
