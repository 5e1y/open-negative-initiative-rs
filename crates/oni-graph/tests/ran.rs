//! Waiting on the GPU with the graph let go: `ran` returns once the work it names has run.

use oni_graph::stages::BoxBlur3;
use oni_graph::{Graph, Image};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[test]
fn ran_returns_once_the_gpu_has_run_the_graph_s_last_submission() {
    let Ok(mut g) = Graph::new() else { return println!("SKIPPED ran: no adapter") };
    assert!(g.last_submission().is_none(), "nothing was handed to the queue yet");
    let side = 2048u32;
    let source = g.upload(side, side, &vec![[0.5, 0.25, 0.125, 1.0]; (side * side) as usize]).expect("upload");
    let image = (0..12).fold(Image::new(&source), |image, _| image.then(BoxBlur3));
    g.materialise(&image).expect("a render");
    let (device, index) = g.last_submission().expect("the render's submission");
    let run = Arc::new(AtomicBool::new(false));
    let seen = run.clone();
    g.device().1.on_submitted_work_done(move || seen.store(true, Ordering::SeqCst));
    // Twin: polled at once, twelve passes over 4 MP handed over a moment ago have not run yet.
    let _ = device.poll(wgpu::PollType::Poll);
    assert!(!run.load(Ordering::SeqCst), "the twin polled after the GPU had already run the work");
    oni_graph::ran(&device, &index).expect("the device holds");
    assert!(run.load(Ordering::SeqCst), "ran returned before the GPU had run the submission");
}
