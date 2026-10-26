//! The help card's demo: the topic's GIF decoded frame by frame on a thread of its own, each frame
//! laid on the card at its own delay while the card stays open, and dropped when it closes.

use crate::demos;
use crate::tutorial::Frames;
use crate::{MainWindow, Tip};
use slint::{ComponentHandle, Image, Rgba8Pixel, SharedPixelBuffer, Timer, TimerMode};
use std::cell::RefCell;
use std::sync::mpsc::{Receiver, TryRecvError, sync_channel};
use std::time::Duration;

/// How soon the card looks again when the decoder has not kept up.
const RETRY: Duration = Duration::from_millis(5);

/// A frame as the decoder hands it: RGBA rows, its size, and how long it shows.
type Decoded = (Vec<u8>, (u32, u32), Duration);

struct Playing {
    topic: String,
    frames: Receiver<Decoded>,
    timer: Timer,
}

thread_local! {
    static PLAYING: RefCell<Option<Playing>> = const { RefCell::new(None) };
}

/// The card answers `Tip.play`: a topic with a demo plays it, any other, or none, stops it.
pub(super) fn wire(window: &MainWindow) {
    let weak = window.as_weak();
    window.global::<Tip>().on_play(move |topic| {
        if let Some(w) = weak.upgrade() {
            play(&w, &topic);
        }
    });
}

fn play(w: &MainWindow, topic: &str) {
    if PLAYING.with(|p| p.borrow().as_ref().is_some_and(|p| p.topic == topic)) {
        return;
    }
    // Dropping the receiver ends the decoder at its next frame.
    PLAYING.with(|p| p.borrow_mut().take());
    let tip = w.global::<Tip>();
    let clip = demos::clip(topic);
    tip.set_has_clip(clip.is_some());
    tip.set_clip(Image::default());
    let Some(clip) = clip else { return };
    let (sender, frames) = sync_channel::<Decoded>(1);
    let spawned = std::thread::Builder::new().name("help demo".into()).spawn(move || {
        let Some(mut decoder) = Frames::new(clip) else { return };
        let size = decoder.size();
        while let Some((picture, delay)) = decoder.next() {
            if sender.send((picture.to_vec(), size, delay)).is_err() {
                return;
            }
        }
    });
    if spawned.is_err() {
        return;
    }
    PLAYING.with(|p| *p.borrow_mut() = Some(Playing { topic: topic.to_string(), frames, timer: Timer::default() }));
    next_frame(w.as_weak());
}

/// The decoder's next frame on the card, the one after asked for once this one has shown its time.
fn next_frame(weak: slint::Weak<MainWindow>) {
    let Some(w) = weak.upgrade() else { return };
    PLAYING.with(|p| {
        let guard = p.borrow();
        let Some(playing) = guard.as_ref() else { return };
        let wait = match playing.frames.try_recv() {
            Ok((pixels, (width, height), delay)) => {
                let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&pixels, width, height);
                w.global::<Tip>().set_clip(Image::from_rgba8(buffer));
                delay
            }
            Err(TryRecvError::Empty) => RETRY,
            Err(TryRecvError::Disconnected) => return,
        };
        let weak = weak.clone();
        playing.timer.start(TimerMode::SingleShot, wait, move || next_frame(weak.clone()));
    });
}
