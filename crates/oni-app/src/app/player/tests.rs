use super::*;
use crate::tutorial::synthetic;

/// The frames the decoder hands at `pace`, at most `limit`: (pixels, delay) each.
fn handed(clip: &'static [u8], pace: Pace, limit: usize) -> Vec<(Option<Vec<u8>>, Duration)> {
    let (sender, frames) = sync_channel(1);
    let (_spare, spares) = channel();
    std::thread::spawn(move || decode(clip, pace, &sender, &spares));
    frames.iter().take(limit).map(|d| (d.pixels, d.delay)).collect()
}

fn clip(places: &[u16]) -> &'static [u8] {
    Box::leak(synthetic(40, 30, places).into_boxed_slice())
}

#[test]
fn once_hands_every_frame_then_ends_and_an_unmoved_frame_carries_no_pixels() {
    let frames = handed(clip(&[0, 10, 10, 20]), Pace::Once, 100);
    assert_eq!(frames.len(), 4, "once: the clip's four frames, then the end");
    assert!(frames.iter().all(|(_, d)| *d == Duration::from_millis(50)));
    assert!(frames[2].0.is_none(), "the repeated place moves nothing, so nothing is written or redrawn");
    // Twin: the frames around it do move.
    assert!(frames[1].0.is_some() && frames[3].0.is_some());
}

#[test]
fn a_loop_starts_again_from_its_first_frame() {
    let frames = handed(clip(&[0, 10, 20]), Pace::Loop, 7);
    assert_eq!(frames.len(), 7, "a loop never ends while it is watched");
    assert_eq!(frames[3].0, frames[0].0, "the fourth frame is the first again");
    // Twin: the frame before it is not.
    assert_ne!(frames[2].0, frames[0].0);
}

#[test]
fn still_hands_the_last_frame_alone() {
    let places = [0, 10, 20];
    let all = handed(clip(&places), Pace::Once, 100);
    let still = handed(clip(&places), Pace::Still, 100);
    assert_eq!(still.len(), 1, "Reduce Motion: one picture, nothing after");
    assert_eq!(still[0].0, all[2].0, "the last frame");
    // Twin: not the first.
    assert_ne!(still[0].0, all[0].0);
}
