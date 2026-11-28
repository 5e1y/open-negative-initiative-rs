//! Each photo travelling in the log, one line a walk beside a photo or a flight between the screens: none and why,
//! else the frames drawn while it moved, how far it had gone at the first, the renders landed meanwhile, what ended it.

use crate::{Flight, MainWindow, Walk};
use slint::ComponentHandle;
use std::cell::RefCell;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Travel {
    Walk,
    Flight,
}

struct Watch {
    serial: i32,
    // What travels, the photo a token: "walk 3 to #a1b2c3, forward".
    what: String,
    began: Instant,
    frames: u32,
    // The renders that reached the canvas meanwhile, the photo sharpening under it.
    landed: u32,
    // When the first frame was drawn, and how far it had gone in it, 0 to 1.
    first: Option<(Duration, f32)>,
}

thread_local! {
    static WATCHES: RefCell<[Option<Watch>; 2]> = const { RefCell::new([None, None]) };
}

fn at(travel: Travel) -> usize {
    match travel {
        Travel::Walk => 0,
        Travel::Flight => 1,
    }
}

/// None of `what`, and why.
pub fn refused(what: &str, why: &str) {
    crate::timing::line(&format!("{what}: none, {why}"));
}

/// `what` starting as travel `serial`, the one of its kind still running cut by it.
pub fn started(travel: Travel, serial: i32, what: String) {
    finish(travel, "the next one");
    let watch = Watch { serial, what, began: Instant::now(), frames: 0, landed: 0, first: None };
    WATCHES.with(|w| w.borrow_mut()[at(travel)] = Some(watch));
}

/// A render reached the canvas.
pub fn landed() {
    WATCHES.with(|w| w.borrow_mut().iter_mut().flatten().for_each(|watch| watch.landed += 1));
}

/// A frame drawn: counted while each travel moves, its line written once it has ended.
pub fn drawn(window: &MainWindow) {
    let (walk, flight) = (window.global::<Walk>(), window.global::<Flight>());
    let walking = (walk.get_serial(), walk.get_walking(), walk.get_walking(), walk.get_gone());
    let flying = (flight.get_serial(), flight.get_flying(), flight.get_flying() && !flight.get_arrived(), flight.get_gone());
    for (travel, (serial, running, moving, gone)) in [(Travel::Walk, walking), (Travel::Flight, flying)] {
        let ended = WATCHES.with(|w| {
            let mut w = w.borrow_mut();
            let Some(watch) = w[at(travel)].as_mut().filter(|watch| watch.serial == serial) else { return false };
            if moving {
                watch.first.get_or_insert_with(|| (watch.began.elapsed(), gone));
                watch.frames += 1;
            }
            !running
        });
        if ended {
            finish(travel, if travel == Travel::Walk { "its pace" } else { "its landing" });
        }
    }
}

fn finish(travel: Travel, how: &str) {
    let Some(w) = WATCHES.with(|w| w.borrow_mut()[at(travel)].take()) else { return };
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    let first = w.first.map_or("none drawn".to_string(), |(at, gone)| format!("the first {:.0} ms after it began, {:.0} % of its way", ms(at), gone * 100.0));
    crate::timing::line(&format!("{}: {} frames drawn in {:.0} ms, {first}, {} renders landed; ended by {how}", w.what, w.frames, ms(w.began.elapsed()), w.landed));
}
