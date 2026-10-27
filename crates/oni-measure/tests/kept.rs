//! The measurer keeps a frame's measuring copy, so a second press reads without decoding again.

use oni_measure::key::{Decoded, Measurer};
use oni_settings::PipelineSettings;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn measurer(decodes: Arc<AtomicUsize>) -> Option<Measurer> {
    let graph = oni_graph::Graph::new().ok()?;
    Some(Measurer::new(
        graph,
        Box::new(move |_, _| {
            decodes.fetch_add(1, Ordering::SeqCst);
            let (width, height) = (300u32, 200u32);
            let pixels = (0..width * height).map(|i| [0.2 + (i % 7) as f32 * 0.05, 0.3, 0.4, 1.0]).collect();
            Ok(Decoded { width, height, pixels, full_width: f64::from(width) })
        }),
        Box::new(|_: &std::path::Path, s: &PipelineSettings| format!("{}", s.primaries_converted)),
    ))
}

#[test]
fn a_frame_read_twice_the_same_way_is_decoded_once() {
    let decodes = Arc::new(AtomicUsize::new(0));
    let Some(m) = measurer(decodes.clone()) else { return };
    let s = PipelineSettings::default();
    m.prepare("a", &s);
    let read = m.with("a", &s, |_, copy| Some(copy.source.extent()));
    assert!(read.is_some());
    assert_eq!(decodes.load(Ordering::SeqCst), 1, "the prepared copy is read");
    // Twin: another reading, then a third frame pushing both out, each decode again.
    let other = PipelineSettings { primaries_converted: !s.primaries_converted, ..s.clone() };
    m.with("a", &other, |_, _| Some(()));
    assert_eq!(decodes.load(Ordering::SeqCst), 2, "another reading decodes again");
    m.with("b", &s, |_, _| Some(()));
    m.with("a", &s, |_, _| Some(()));
    assert_eq!(decodes.load(Ordering::SeqCst), 4, "two copies kept, the oldest let go");
}

#[test]
fn a_reading_that_panics_leaves_the_next_one_to_read() {
    let decodes = Arc::new(AtomicUsize::new(0));
    let Some(m) = measurer(decodes.clone()) else { return };
    let s = PipelineSettings::default();
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| m.with("a", &s, |_, _| -> Option<()> { panic!("a reading fails") })));
    assert!(panicked.is_err());
    assert!(m.with("a", &s, |_, copy| Some(copy.source.extent())).is_some(), "a panic left the measurer reading nothing");
    // Twin: the copy made before the panic is dropped with it, so the frame is decoded again.
    assert_eq!(decodes.load(Ordering::SeqCst), 2);
}
