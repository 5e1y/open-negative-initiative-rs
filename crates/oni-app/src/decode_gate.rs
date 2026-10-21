//! One full decode at a time, the latest asked winning: walking a roll never piles decodes up, each
//! 0.4 to 0.7 GB of pixels at 24 MP. Across every kind, at most `DECODES` decode at once, one of them
//! always left to the hand, so an 8 GB Mac never holds a thumbnail pass and an open's decodes together.

use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};

/// Decodes running at once across the app, background work taking at most one less.
pub const DECODES: usize = 2;

thread_local! {
    static BACKGROUND: Cell<bool> = const { Cell::new(false) };
}

/// Marks the calling thread's decodes as background work: thumbnails, roll tiles, measuring copies,
/// dust patches, which yield to any decode the hand waits on.
pub fn in_background() {
    BACKGROUND.with(|b| b.set(true));
}

pub struct Slots {
    /// Decodes running, and the hand's decodes waiting for a place.
    state: Mutex<(usize, usize)>,
    turn: Condvar,
}

static SLOTS: Slots = Slots::new();

/// A place among the decodes running, given back when dropped.
pub struct Slot<'a>(&'a Slots);

impl Drop for Slot<'_> {
    fn drop(&mut self) {
        let mut s = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        s.0 -= 1;
        self.0.turn.notify_all();
    }
}

/// A place among the app's decodes, for the calling thread's kind of work.
pub fn slot() -> Slot<'static> {
    let (background, asked) = (BACKGROUND.with(Cell::get), std::time::Instant::now());
    let slot = SLOTS.take(background);
    let waited = asked.elapsed();
    if waited > std::time::Duration::from_millis(5) {
        let kind = if background { "a background" } else { "the hand's" };
        crate::timing::line(&format!("{kind} decode waited {:.0} ms for its place", waited.as_secs_f64() * 1000.0));
    }
    slot
}

impl Slots {
    pub const fn new() -> Slots {
        Slots { state: Mutex::new((0, 0)), turn: Condvar::new() }
    }

    /// Waits for a place: the hand's decode while fewer than `DECODES` run, background work while
    /// fewer than `DECODES - 1` run and no hand waits.
    pub fn take(&self, background: bool) -> Slot<'_> {
        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if background {
            while s.0 + 1 >= DECODES || s.1 > 0 {
                s = self.turn.wait(s).unwrap_or_else(|e| e.into_inner());
            }
        } else {
            s.1 += 1;
            while s.0 >= DECODES {
                s = self.turn.wait(s).unwrap_or_else(|e| e.into_inner());
            }
            s.1 -= 1;
        }
        s.0 += 1;
        Slot(self)
    }
}

pub struct Gate {
    latest: AtomicU64,
    running: Mutex<()>,
}

/// The open frame's preview, its full resolution, its measuring copy, and Prepare the Roll's full scan.
pub static PREVIEW: Gate = Gate::new();
pub static FULL: Gate = Gate::new();
pub static MEASURE: Gate = Gate::new();
pub static ROLL_FULL: Gate = Gate::new();

impl Gate {
    pub const fn new() -> Gate {
        Gate { latest: AtomicU64::new(0), running: Mutex::new(()) }
    }

    /// A ticket for the decode just asked, which supersedes every one asked before.
    pub fn ask(&self) -> u64 {
        self.latest.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Runs `decode` once the one before it is done, unless a later ask has come meanwhile.
    pub fn run<T>(&self, ticket: u64, decode: impl FnOnce() -> T) -> Option<T> {
        let _turn = self.running.lock().unwrap_or_else(|e| e.into_inner());
        (self.latest.load(Ordering::SeqCst) == ticket).then(decode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn background_decodes_leave_a_place_to_the_hand_and_yield_to_it() {
        static OWN: Slots = Slots::new();
        let took = |background: bool| {
            let (tx, rx) = mpsc::channel();
            let (release, hold) = mpsc::channel::<()>();
            std::thread::spawn(move || {
                let slot = OWN.take(background);
                tx.send(()).unwrap();
                let _ = hold.recv();
                drop(slot);
            });
            (rx, release)
        };
        let short = Duration::from_millis(200);
        let (first, free_first) = took(true);
        assert!(first.recv_timeout(Duration::from_secs(5)).is_ok(), "one background decode runs");
        let (second, free_second) = took(true);
        assert!(second.recv_timeout(short).is_err(), "a second background decode waits");
        let (hand, free_hand) = took(false);
        assert!(hand.recv_timeout(Duration::from_secs(5)).is_ok(), "the hand finds its place beside it");
        // Twin: a second hand waits, the places all taken.
        let (hand2, free_hand2) = took(false);
        assert!(hand2.recv_timeout(short).is_err(), "a third decode waits");
        free_first.send(()).unwrap();
        assert!(hand2.recv_timeout(Duration::from_secs(5)).is_ok(), "the hand waiting goes first");
        assert!(second.recv_timeout(short).is_err(), "the background still waits behind the hand");
        free_hand.send(()).unwrap();
        free_hand2.send(()).unwrap();
        assert!(second.recv_timeout(Duration::from_secs(5)).is_ok(), "then the background decode runs");
        free_second.send(()).unwrap();
    }

    #[test]
    fn only_the_latest_ask_decodes_and_one_runs_at_a_time() {
        let gate = Gate::new();
        let (first, second) = (gate.ask(), gate.ask());
        // Twin: the earlier ticket, superseded before its turn, decodes nothing.
        assert_eq!(gate.run(first, || 1), None);
        assert_eq!(gate.run(second, || 2), Some(2));
        let busy = std::sync::Arc::new(AtomicU64::new(0));
        let ticket = gate.ask();
        let gate = &gate;
        std::thread::scope(|s| {
            for _ in 0..4 {
                let busy = busy.clone();
                // The count is given back inside the turn, so no other turn can find it raised.
                s.spawn(move || {
                    gate.run(ticket, || {
                        assert_eq!(busy.fetch_add(1, Ordering::SeqCst), 0, "two decodes at once");
                        busy.fetch_sub(1, Ordering::SeqCst);
                    });
                });
            }
        });
    }
}
