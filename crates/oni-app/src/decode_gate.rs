//! One full decode at a time, the latest asked winning: walking a roll never piles decodes up, each
//! 0.4 to 0.7 GB of pixels at 24 MP. Across every kind, at most `DECODES` decode at once, one of them
//! always left to the hand, so an 8 GB Mac never holds a thumbnail pass and an open's decodes together.

use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};

/// Decodes running at once across the app, background work taking at most one less.
pub const DECODES: usize = 2;

/// Whose decode a thread asks: the hand's goes first, then the roll being prepared, then the rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Hand,
    /// Prepare the Roll's previews and scans: never held, and decoded beside the hand's.
    Roll,
    Background,
}

thread_local! {
    static KIND: Cell<Kind> = const { Cell::new(Kind::Hand) };
    static WANTED: std::cell::RefCell<Option<Box<dyn Fn() -> bool>>> = const { std::cell::RefCell::new(None) };
}

/// Marks the calling thread's decodes as background work: thumbnails, measuring copies, neighbours,
/// dust patches, which yield to any decode the hand waits on and stand still while a roll is prepared.
pub fn in_background() {
    KIND.with(|k| k.set(Kind::Background));
}

/// Marks the calling thread's decodes as the roll being prepared's: its tiles, card and scans, which
/// yield to the hand's alone.
pub fn for_the_roll() {
    KIND.with(|k| k.set(Kind::Roll));
}

/// The calling thread's decodes are wanted while `wanted` holds: one superseded while it waited for
/// its place decodes nothing, and leaves the place to the work that superseded it.
pub fn only_while(wanted: impl Fn() -> bool + 'static) {
    WANTED.with(|w| *w.borrow_mut() = Some(Box::new(wanted)));
}

fn wanted() -> bool {
    WANTED.with(|w| w.borrow().as_ref().is_none_or(|f| f()))
}

struct Places {
    running: usize,
    /// The hand's decodes waiting for a place.
    hands: usize,
    /// The roll's decodes running, at most one less than `DECODES` beside the hand's.
    roll: usize,
    /// The rolls being prepared, each holding every background decode back.
    held: usize,
    /// Background decodes that took a place, which a check counts.
    background: u64,
}

pub struct Slots {
    state: Mutex<Places>,
    turn: Condvar,
}

static SLOTS: Slots = Slots::new();

/// A place among the decodes running, given back when dropped.
pub struct Slot<'a>(&'a Slots, Kind);

impl Drop for Slot<'_> {
    fn drop(&mut self) {
        let mut p = self.0.places();
        p.running -= 1;
        if self.1 == Kind::Roll {
            p.roll -= 1;
        }
        drop(p);
        self.0.turn.notify_all();
    }
}

/// A roll being prepared: no background decode takes a place until it is dropped.
pub struct Held<'a>(&'a Slots);

impl Drop for Held<'_> {
    fn drop(&mut self) {
        self.0.places().held -= 1;
        self.0.turn.notify_all();
    }
}

/// The app's decodes held for the roll being prepared, until the guard is dropped.
pub fn hold_for_roll() -> Held<'static> {
    SLOTS.hold()
}

/// How many background decodes took a place so far, which a check counts.
pub fn background_taken() -> u64 {
    SLOTS.places().background
}

/// A place among the app's decodes, for the calling thread's kind of work; none when the thread's
/// work was superseded while it waited (`only_while`).
pub fn slot() -> Option<Slot<'static>> {
    let (kind, asked) = (KIND.with(Cell::get), std::time::Instant::now());
    let slot = SLOTS.take_wanted(kind);
    let waited = asked.elapsed();
    if waited > std::time::Duration::from_millis(5) {
        let kind = match kind {
            Kind::Hand => "the hand's",
            Kind::Roll => "the roll's",
            Kind::Background => "a background",
        };
        crate::timing::line(&format!("{kind} decode waited {:.0} ms for its place", waited.as_secs_f64() * 1000.0));
    }
    slot
}

impl Default for Slots {
    fn default() -> Self {
        Self::new()
    }
}

impl Slots {
    pub const fn new() -> Slots {
        Slots { state: Mutex::new(Places { running: 0, hands: 0, roll: 0, held: 0, background: 0 }), turn: Condvar::new() }
    }

    fn places(&self) -> std::sync::MutexGuard<'_, Places> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Waits for a place: the hand's while one is free; the roll's while one is, no hand waits and one more
    /// stays beside its own; background work while one more stays beside every decode, no hand waits, no roll held.
    pub fn take(&self, kind: Kind) -> Slot<'_> {
        let mut p = self.places();
        match kind {
            Kind::Hand => {
                p.hands += 1;
                while p.running >= DECODES {
                    p = self.turn.wait(p).unwrap_or_else(|e| e.into_inner());
                }
                p.hands -= 1;
            }
            Kind::Roll => {
                while p.running >= DECODES || p.hands > 0 || p.roll + 1 >= DECODES {
                    p = self.turn.wait(p).unwrap_or_else(|e| e.into_inner());
                }
                p.roll += 1;
            }
            Kind::Background => {
                while p.running + 1 >= DECODES || p.hands > 0 || p.held > 0 {
                    p = self.turn.wait(p).unwrap_or_else(|e| e.into_inner());
                }
                p.background += 1;
            }
        }
        p.running += 1;
        Slot(self, kind)
    }

    /// `take`, then the place given back at once when the calling thread's work is no longer wanted.
    pub fn take_wanted(&self, kind: Kind) -> Option<Slot<'_>> {
        let slot = self.take(kind);
        wanted().then_some(slot)
    }

    /// Every background decode held back until the guard is dropped; one already running ends.
    pub fn hold(&self) -> Held<'_> {
        self.places().held += 1;
        Held(self)
    }
}

pub struct Gate {
    latest: AtomicU64,
    running: Mutex<()>,
}

/// The open frame's preview, its full resolution, and Prepare the Roll's full scan.
pub static PREVIEW: Gate = Gate::new();
pub static FULL: Gate = Gate::new();
pub static ROLL_FULL: Gate = Gate::new();

impl Default for Gate {
    fn default() -> Self {
        Self::new()
    }
}

impl Gate {
    pub const fn new() -> Gate {
        Gate { latest: AtomicU64::new(0), running: Mutex::new(()) }
    }

    /// A ticket for the decode just asked, which supersedes every one asked before.
    pub fn ask(&self) -> u64 {
        self.latest.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// How many decodes were asked so far, which a check counts.
    pub fn asked(&self) -> u64 {
        self.latest.load(Ordering::SeqCst)
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

    /// A decode of `kind` asked on a thread of its own: told when it takes its place, which it gives back when released.
    fn took(slots: &'static Slots, kind: Kind) -> (mpsc::Receiver<()>, mpsc::Sender<()>) {
        let (tx, rx) = mpsc::channel();
        let (release, hold) = mpsc::channel::<()>();
        std::thread::spawn(move || {
            let slot = slots.take(kind);
            tx.send(()).unwrap();
            let _ = hold.recv();
            drop(slot);
        });
        (rx, release)
    }

    #[test]
    fn background_decodes_leave_a_place_to_the_hand_and_yield_to_it() {
        static OWN: Slots = Slots::new();
        let took = |kind: Kind| took(&OWN, kind);
        let short = Duration::from_millis(200);
        let (first, free_first) = took(Kind::Background);
        assert!(first.recv_timeout(Duration::from_secs(5)).is_ok(), "one background decode runs");
        let (second, free_second) = took(Kind::Background);
        assert!(second.recv_timeout(short).is_err(), "a second background decode waits");
        let (hand, free_hand) = took(Kind::Hand);
        assert!(hand.recv_timeout(Duration::from_secs(5)).is_ok(), "the hand finds its place beside it");
        // Twin: a second hand waits, the places all taken.
        let (hand2, free_hand2) = took(Kind::Hand);
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
    fn a_roll_held_stills_every_background_decode_and_never_its_own() {
        static OWN: Slots = Slots::new();
        let (short, long) = (Duration::from_millis(200), Duration::from_secs(5));
        let held = OWN.hold();
        // Twin: the roll's work asked as background work waits, as the gallery's does, every place free.
        let (gallery, free_gallery) = took(&OWN, Kind::Background);
        assert!(gallery.recv_timeout(short).is_err(), "the gallery's decode stands still while the roll is held");
        let (roll, free_roll) = took(&OWN, Kind::Roll);
        assert!(roll.recv_timeout(long).is_ok(), "the roll's decode takes its place");
        free_roll.send(()).unwrap();
        let (roll, free_roll) = took(&OWN, Kind::Roll);
        assert!(roll.recv_timeout(long).is_ok(), "and the next, ahead of the gallery's waiting");
        free_roll.send(()).unwrap();
        assert!(gallery.recv_timeout(short).is_err(), "the places free, the gallery still waits");
        assert_eq!(OWN.places().background, 0, "no background decode took a place");
        // The roll's decode beside the hand's: its scan never waits for the loupe's full resolution.
        let (hand, free_hand) = took(&OWN, Kind::Hand);
        assert!(hand.recv_timeout(long).is_ok());
        let (roll, free_roll) = took(&OWN, Kind::Roll);
        assert!(roll.recv_timeout(long).is_ok(), "the roll's decode runs beside the hand's");
        free_hand.send(()).unwrap();
        // Twin: a second of the roll's waits, the place beside it left to the hand.
        let (second, free_second) = took(&OWN, Kind::Roll);
        assert!(second.recv_timeout(short).is_err(), "the roll never takes the hand's last place");
        let (hand, free_hand) = took(&OWN, Kind::Hand);
        assert!(hand.recv_timeout(long).is_ok(), "the hand finds it");
        free_hand.send(()).unwrap();
        free_roll.send(()).unwrap();
        assert!(second.recv_timeout(long).is_ok());
        free_second.send(()).unwrap();
        drop(held);
        assert!(gallery.recv_timeout(long).is_ok(), "the roll left, the gallery resumes");
        assert_eq!(OWN.places().background, 1);
        free_gallery.send(()).unwrap();
    }

    #[test]
    fn a_decode_superseded_while_it_waits_takes_no_place() {
        static OWN: Slots = Slots::new();
        let held = [OWN.take(Kind::Hand), OWN.take(Kind::Hand)];
        let waiting = |still: bool| {
            let (tx, rx) = mpsc::channel();
            let wanted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
            let flag = wanted.clone();
            std::thread::spawn(move || {
                only_while(move || flag.load(Ordering::SeqCst));
                let slot = OWN.take_wanted(Kind::Hand);
                tx.send(slot.is_some()).unwrap();
            });
            std::thread::sleep(Duration::from_millis(50));
            wanted.store(still, Ordering::SeqCst);
            rx
        };
        let superseded = waiting(false);
        let [first, second] = held;
        drop(first);
        assert_eq!(superseded.recv_timeout(Duration::from_secs(5)), Ok(false), "superseded while it waited, it takes nothing");
        // Twin: one still wanted when its place frees takes it.
        let again = OWN.take(Kind::Hand);
        let wanted = waiting(true);
        drop(again);
        assert_eq!(wanted.recv_timeout(Duration::from_secs(5)), Ok(true));
        drop(second);
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
