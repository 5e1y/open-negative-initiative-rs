//! The help card's demo: the topic's GIF decoded frame by frame on a thread of its own, each frame
//! laid on the card as the card asks, at its own delay, the clip again from its start while it stands.

use crate::demos;
use crate::tutorial::Frames;
use crate::{MainWindow, Tip};
use slint::{ComponentHandle, Image, Rgba8Pixel, SharedPixelBuffer};
use std::cell::RefCell;
use std::sync::mpsc::{Receiver, TryRecvError, sync_channel};
use std::time::Duration;

/// How soon the card looks again when the decoder has not kept up.
const RETRY: Duration = Duration::from_millis(5);

/// A frame as the decoder hands it: RGBA rows, its size, and how long it shows.
type Decoded = (Vec<u8>, (u32, u32), Duration);

struct Playing {
    clip: &'static [u8],
    frames: Receiver<Decoded>,
}

thread_local! {
    static PLAYING: RefCell<Option<Playing>> = const { RefCell::new(None) };
}

/// The card answers `Tip.play`: a topic with a demo plays it, any other, or none, stops it; and
/// `Tip.advance`, which only a card standing asks, so a card gone leaves the decoder waiting.
pub(super) fn wire(window: &MainWindow) {
    let weak = window.as_weak();
    window.global::<Tip>().on_play(move |topic| {
        if let Some(w) = weak.upgrade() {
            play(&w, &topic);
        }
    });
    let weak = window.as_weak();
    window.global::<Tip>().on_advance(move || {
        if let Some(w) = weak.upgrade() {
            advance(&w);
        }
    });
}

fn play(w: &MainWindow, topic: &str) {
    // A card standing again on the clip playing, the same topic in the same language, takes it up where it was.
    let clip = demos::clip(topic);
    if PLAYING.with(|p| p.borrow().as_ref().is_some_and(|p| clip.is_some_and(|c| std::ptr::eq(p.clip, c)))) {
        return;
    }
    // Dropping the receiver ends the decoder at its next frame.
    PLAYING.with(|p| p.borrow_mut().take());
    let tip = w.global::<Tip>();
    // Set before `has-clip` starts the card's timer, which a zero interval would spin.
    tip.set_delay(RETRY.as_millis() as i64);
    tip.set_has_clip(clip.is_some());
    tip.set_clip(Image::default());
    let Some(clip) = clip else { return };
    let (sender, frames) = sync_channel::<Decoded>(1);
    let spawned = std::thread::Builder::new().name("help demo".into()).spawn(move || {
        while let Some(mut decoder) = Frames::new(clip) {
            let size = decoder.size();
            while let Some((picture, delay)) = decoder.next() {
                if sender.send((picture.to_vec(), size, delay)).is_err() {
                    return;
                }
            }
        }
    });
    if spawned.is_err() {
        return;
    }
    PLAYING.with(|p| *p.borrow_mut() = Some(Playing { clip, frames }));
}

/// The decoder's next frame on the card, and how long it shows; not yet decoded, a short wait.
fn advance(w: &MainWindow) {
    PLAYING.with(|p| {
        let guard = p.borrow();
        let Some(playing) = guard.as_ref() else { return };
        let tip = w.global::<Tip>();
        let wait = match playing.frames.try_recv() {
            Ok((pixels, (width, height), delay)) => {
                let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&pixels, width, height);
                tip.set_clip(Image::from_rgba8(buffer));
                delay
            }
            Err(TryRecvError::Empty) => RETRY,
            Err(TryRecvError::Disconnected) => return,
        };
        tip.set_delay(wait.as_millis() as i64);
    });
}
