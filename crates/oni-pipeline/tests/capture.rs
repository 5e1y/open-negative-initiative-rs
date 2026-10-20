//! `CaptureExposure.selfCheck`, its render: a scan's capture offset is evened before anything else
//! reads it, so a frame shot over and brought back renders as the frame shot alike.

use oni_graph::{Graph, Rect};
use oni_pipeline::{Inputs, Pipeline, Request};
use oni_settings::PipelineSettings;

type V3 = [f32; 3];

/// A flat 2-pixel square of `t·(1, 0.8, 0.6)` measured through the per-pixel chain.
fn read(g: &mut Graph, t: f32, stops: f32) -> V3 {
    let mut s = PipelineSettings::default().per_pixel_only();
    s.capture_stops = stops;
    let pixels = vec![[t, t * 0.8, t * 0.6, 1.0]; 4];
    let source = g.upload(2, 2, &pixels).expect("the patch uploads");
    let request = Request { region: Some(Rect { x: 0, y: 0, w: 1, h: 1 }), measuring: true, ..Request::default() };
    let (_, px) = Pipeline::new().render(g, &source, &s, &request, &Inputs::default(), None).expect("the patch renders");
    [px[0][0], px[0][1], px[0][2]]
}

fn gap(a: V3, b: V3) -> f32 {
    (0..3).map(|c| (a[c] - b[c]).abs()).fold(0.0, f32::max)
}

#[test]
fn a_scan_captured_over_and_brought_back_renders_as_the_frame_captured_alike() {
    let Ok(mut g) = Graph::new() else {
        println!("SKIPPED: no GPU");
        return;
    };
    let (shot, over) = (0.12f32, 0.67f32);
    let alike = read(&mut g, shot, 0.0);
    let evened = gap(read(&mut g, shot * 2f32.powf(over), over), alike);
    assert!(evened < 1e-5, "brought back, the scan stands {evened:.1e} from the frame captured alike");
    // Twin: left as it was, the same scan lands apart.
    let apart = gap(read(&mut g, shot * 2f32.powf(over), 0.0), alike);
    assert!(apart > 1e-2, "left as it was, it lands only {apart:.3} away");
}
