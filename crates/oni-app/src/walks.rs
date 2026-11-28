//! Each walk beside a photo in the log, one line: none and why, else the frames drawn while it ran, how far it
//! had gone at the first of them, the renders landed meanwhile, and what ended it, the pace run or the next walk.

use crate::{MainWindow, Walk};
use slint::ComponentHandle;
use std::cell::RefCell;
use std::time::{Duration, Instant};

struct Watch {
    serial: i32,
    photo: String,
    way: i32,
    began: Instant,
    frames: u32,
    // The renders that reached the canvas meanwhile, the photo walked to sharpening under the walk.
    landed: u32,
    // When the first frame was drawn, and how far the walk had gone in it, 0 to 1.
    first: Option<(Duration, f32)>,
}

thread_local! {
    static WATCH: RefCell<Option<Watch>> = const { RefCell::new(None) };
}

/// No walk to `to`, and why.
pub fn refused(to: &str, why: &str) {
    crate::timing::line(&format!("walk to {}: none, {why}", crate::anonymity::token(to)));
}

/// A walk to `to` starting, the one still running cut by it.
pub fn started(serial: i32, to: &str, way: i32) {
    finish("the next walk");
    let watch = Watch { serial, photo: crate::anonymity::token(to), way, began: Instant::now(), frames: 0, landed: 0, first: None };
    WATCH.with(|w| *w.borrow_mut() = Some(watch));
}

/// A render reached the canvas.
pub fn landed() {
    WATCH.with(|w| {
        if let Some(watch) = w.borrow_mut().as_mut() {
            watch.landed += 1;
        }
    });
}

/// A frame drawn: counted while the walk runs, its line written once it has ended.
pub fn drawn(window: &MainWindow) {
    let walk = window.global::<Walk>();
    let ended = WATCH.with(|w| {
        let mut w = w.borrow_mut();
        let Some(watch) = w.as_mut().filter(|watch| watch.serial == walk.get_serial()) else { return false };
        if !walk.get_walking() {
            return true;
        }
        watch.first.get_or_insert_with(|| (watch.began.elapsed(), walk.get_gone()));
        watch.frames += 1;
        false
    });
    if ended {
        finish("its pace");
    }
}

fn finish(how: &str) {
    let Some(w) = WATCH.with(|w| w.borrow_mut().take()) else { return };
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    let first = w.first.map_or("none drawn".to_string(), |(at, gone)| format!("the first {:.0} ms after the key, {:.0} % of its way", ms(at), gone * 100.0));
    crate::timing::line(&format!(
        "walk {} to {}, {}: {} frames drawn in {:.0} ms, {first}, {} renders landed; ended by {how}",
        w.serial,
        w.photo,
        if w.way < 0 { "back" } else { "forward" },
        w.frames,
        ms(w.began.elapsed()),
        w.landed,
    ));
}
