//! A patch lies on the decoded scan before the capture's exposure and the flat-field, so it takes the
//! gains of the place it lies on, as the scan around it does.

mod common;

use common::{graph, whole};
use oni_graph::{Graph, Image, Source};
use oni_pipeline::{Inputs, Pipeline, corrections, stages};
use oni_settings::cg::Point;
use oni_settings::frame::{Correction, FlatField, FlatFieldMask};
use oni_settings::{ConversionMode, PipelineSettings};

const SIZE: (u32, u32) = (400, 300);

/// A stroke off the centre, where the flat-field's mask falls away from its peak.
fn stroke() -> Correction {
    Correction { radius: 10.0, ..Correction::new(vec![Point::new(0.2, 0.25), Point::new(0.2001, 0.25)]) }
}

/// The green at the stroke's centre of `image`.
fn at_stroke(g: &mut Graph, image: &Image) -> f32 {
    let pixels = whole(g, image);
    let (x, y) = ((0.2 * f64::from(SIZE.0)) as usize, (0.25 * f64::from(SIZE.1)) as usize);
    pixels[y * SIZE.0 as usize + x][1]
}

/// The scan's uniform 0.3 and the patch's uniform 0.9.
fn sources(g: &mut Graph, c: &Correction) -> (Source, Source) {
    let (w, h) = SIZE;
    let scan = g.upload(w, h, &vec![[0.3, 0.3, 0.3, 1.0]; (w * h) as usize]).expect("the scan");
    let r = corrections::patch_rect(c, [0.0, 0.0, f64::from(w), f64::from(h)]);
    let patch = g.upload(r[2] as u32, r[3] as u32, &vec![[0.9, 0.9, 0.9, 1.0]; (r[2] * r[3]) as usize]).expect("the patch");
    (scan, patch)
}

#[test]
fn a_patch_takes_the_gains_of_the_place_it_lies_on() {
    let Some(mut g) = graph("a_patch_takes_the_gains_of_the_place_it_lies_on") else { return };
    let mut pipeline = Pipeline::new();
    let c = stroke();
    let (scan, patch) = sources(&mut g, &c);
    let inputs = Inputs { patches: [(c.id, &patch)].into_iter().collect(), ..Inputs::default() };
    let stopped = PipelineSettings { capture_stops: 1.0, corrections: vec![c.clone()], ..PipelineSettings::rest(ConversionMode::Negative, true) };
    let evened = PipelineSettings { flat_field: FlatField { mask: FlatFieldMask::ValoiEasy35V1, ..Default::default() }, ..PipelineSettings { capture_stops: 0.0, ..stopped.clone() } };
    for (name, s) in [("a capture stop", &stopped), ("a flat-field", &evened)] {
        let bare = PipelineSettings { corrections: Vec::new(), ..s.clone() };
        let healed = pipeline.healed(&mut g, Image::new(&scan), s, 1.0, &inputs).expect("healed");
        let around = pipeline.healed(&mut g, Image::new(&scan), &bare, 1.0, &inputs).expect("bare");
        let ratio = at_stroke(&mut g, &healed) / at_stroke(&mut g, &around);
        assert!((ratio - 3.0).abs() < 1e-5, "{name}: the patch over the scan around it is {ratio}, not 0.9 over 0.3");
    }
    // Twin: the patch laid after the stop, as before, keeps its own tone over a scan the stop halved.
    let stopped_first = pipeline.patched(&mut g, Image::new(&scan).then(stages::exposure(-1.0)), &stopped, 1.0, &inputs).expect("the old order");
    let bare = Image::new(&scan).then(stages::exposure(-1.0));
    let ratio = at_stroke(&mut g, &stopped_first) / at_stroke(&mut g, &bare);
    assert!((ratio - 6.0).abs() < 1e-4, "the old order's twin is {ratio}, not 6");
}
