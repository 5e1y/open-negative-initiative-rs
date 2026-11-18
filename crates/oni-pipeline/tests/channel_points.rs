//! Each channel's own points, mids and contrast through the whole render: equal on the three channels the points
//! reach the windows as the global points would, one channel's moves that channel, black and white reads none.

mod common;
use common::{V4, graph, whole};
use oni_graph::{Graph, Source};
use oni_pipeline::{Pipeline, Request, conversion, finishing};
use oni_settings::{ConversionMode, PipelineSettings};

const SIDE: (u32, u32) = (320, 200);

/// A density ramp across the frame, each channel on its own tilt, so every window holds some of it.
fn ramp(g: &mut Graph) -> Source {
    let (w, h) = SIDE;
    let pixels: Vec<V4> = (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) as f32 / w as f32, (i / w) as f32 / h as f32);
            let t = |tilt: f32| 10f32.powf(-3.0 * (x + tilt * y) / (1.0 + tilt));
            [t(0.1), t(0.3), t(0.5), 1.0]
        })
        .collect();
    g.upload(w, h, &pixels).unwrap()
}

fn render(g: &mut Graph, pipeline: &mut Pipeline, source: &Source, s: &PipelineSettings) -> Vec<V4> {
    let request = Request { full_width: Some(f64::from(SIDE.0)), ..Request::default() };
    let image = pipeline.image(g, source, s, &request, &Default::default()).unwrap();
    whole(g, &image)
}

/// The mean move of each channel between two renders.
fn shifts(a: &[V4], b: &[V4]) -> [f32; 3] {
    [0, 1, 2].map(|c| a.iter().zip(b).map(|(p, q)| (p[c] - q[c]).abs()).sum::<f32>() / a.len() as f32)
}

/// Channel `c` moved, at least twice as far as either other: those follow only through the kernel's
/// dye separation, chroma decompression and light-dependent colour, which mix the channels.
fn led_by(moved: [f32; 3], c: usize) -> bool {
    moved[c] > 5e-3 && (0..3).filter(|k| *k != c).all(|k| moved[k] * 2.0 < moved[c])
}

#[test]
fn the_channels_points_render_as_their_folds_say() {
    let Some(mut g) = graph("the_channels_points_render_as_their_folds_say") else { return };
    let source = ramp(&mut g);
    let mut pipeline = Pipeline::new();
    for mode in [ConversionMode::Negative, ConversionMode::Positive] {
        let mut lit = PipelineSettings::rest(mode, true);
        lit.print.black_point = 0.0625;
        lit.print.white_point = 0.125;
        let at_rest = render(&mut g, &mut pipeline, &source, &lit);
        // The same ends reached through each channel's own points: the fused kernel's arguments to the bit.
        let mut spread = lit.clone();
        (spread.print.black_point, spread.print.channel_black) = (0.03125, [0.03125; 3]);
        (spread.print.white_point, spread.print.channel_white) = (0.0625, [0.0625; 3]);
        assert_eq!(conversion::stage47(&spread), conversion::stage47(&lit), "{mode:?}: equal channel points reach as the global ones");
        for (c, name) in ["red", "green", "blue"].into_iter().enumerate() {
            let mut own = lit.clone();
            own.print.channel_white[c] = 0.2;
            own.print.channel_black[c] = 0.05;
            let moved = render(&mut g, &mut pipeline, &source, &own);
            // A colour move, not a contrast: the saturation the global points call for stays.
            assert_eq!(finishing::points_curve(&own.print, mode), finishing::points_curve(&lit.print, mode));
            let moved = shifts(&moved, &at_rest);
            println!("{mode:?} {name}'s own points move the channels by {moved:?}");
            assert!(led_by(moved, c), "{mode:?}: {name}'s points move {moved:?}");
        }
        // Twin: the same offset laid on the three channels leads with none of them.
        let mut all = lit.clone();
        all.print.channel_white = [0.2; 3];
        all.print.channel_black = [0.05; 3];
        let moved = shifts(&render(&mut g, &mut pipeline, &source, &all), &at_rest);
        println!("{mode:?} the three channels' points move them by {moved:?}");
        assert!((0..3).all(|c| !led_by(moved, c)), "{mode:?}: the offset on all three led with one channel {moved:?}");
    }
    // Black and white shows no channel: its points are read whole from Lighting's.
    let mut mono = PipelineSettings::rest(ConversionMode::Monochrome, true);
    mono.print.white_point = 0.125;
    let plain = render(&mut g, &mut pipeline, &source, &mono);
    let mut tinted = mono.clone();
    (tinted.print.channel_white, tinted.print.channel_black) = ([0.2, -0.1, 0.0], [0.0, 0.05, -0.1]);
    assert!(render(&mut g, &mut pipeline, &source, &tinted) == plain, "black and white read a channel's own point");
    // Twin: the same points on a negative move its render.
    let negative = |s: &PipelineSettings| PipelineSettings { mode: ConversionMode::Negative, ..s.clone() };
    let (a, b) = (render(&mut g, &mut pipeline, &source, &negative(&mono)), render(&mut g, &mut pipeline, &source, &negative(&tinted)));
    assert!(shifts(&a, &b)[0] > 5e-3);
}

#[test]
fn a_channels_own_mid_and_contrast_move_that_channel() {
    let Some(mut g) = graph("a_channels_own_mid_and_contrast_move_that_channel") else { return };
    let source = ramp(&mut g);
    let mut pipeline = Pipeline::new();
    for mode in [ConversionMode::Negative, ConversionMode::Positive, ConversionMode::Digital] {
        let rest = PipelineSettings::rest(mode, true);
        let k = conversion::stage47(&rest);
        assert_eq!((k.own_contrast, k.own_mid), ([0.0; 3], [0.0; 3]), "{mode:?}: a frame at rest carries no channel tone");
        let at_rest = render(&mut g, &mut pipeline, &source, &rest);
        for (c, name) in ["red", "green", "blue"].into_iter().enumerate() {
            for (what, contrast, mid) in [("contrast", 0.4, 0.0), ("mid", 0.0, 0.25)] {
                let mut own = rest.clone();
                own.print.channel_contrast[c] = contrast;
                own.print.channel_mid[c] = mid;
                let moved = shifts(&render(&mut g, &mut pipeline, &source, &own), &at_rest);
                println!("{mode:?} {name}'s own {what} moves the channels by {moved:?}");
                assert!(led_by(moved, c), "{mode:?}: {name}'s {what} moves {moved:?}");
            }
        }
        // Twin: the same mids laid on the three channels leads with none of them.
        let mut all = rest.clone();
        all.print.channel_mid = [0.25; 3];
        let moved = shifts(&render(&mut g, &mut pipeline, &source, &all), &at_rest);
        assert!((0..3).all(|c| !led_by(moved, c)), "{mode:?}: the mids on all three led with one channel {moved:?}");
    }
    // Black and white shows no channel: its own tone renders nothing.
    let mono = PipelineSettings::rest(ConversionMode::Monochrome, true);
    let mut toned = mono.clone();
    (toned.print.channel_contrast, toned.print.channel_mid) = ([0.4, -0.2, 0.1], [0.2, 0.0, -0.25]);
    assert!(render(&mut g, &mut pipeline, &source, &toned) == render(&mut g, &mut pipeline, &source, &mono), "black and white read a channel's own tone");
    // Twin: the same tone on a positive moves its render.
    let positive = |s: &PipelineSettings| PipelineSettings { mode: ConversionMode::Positive, ..s.clone() };
    let (a, b) = (render(&mut g, &mut pipeline, &source, &positive(&mono)), render(&mut g, &mut pipeline, &source, &positive(&toned)));
    assert!(shifts(&a, &b)[0] > 5e-3);
}
