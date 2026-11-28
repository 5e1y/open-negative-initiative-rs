//! The interface thread's answers: a thread of its own posts to it every 10 ms and keeps each post's wait, so a gap
//! between frames reads as rest, every post answered at once, or as a block, the posts waiting.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

type Waits = Arc<Mutex<Vec<(Instant, Duration)>>>;

pub(crate) struct Answers {
    waits: Waits,
    stop: Arc<AtomicBool>,
}

/// Posts to the interface thread until dropped.
pub(crate) fn answers() -> Answers {
    let (waits, stop): (Waits, Arc<AtomicBool>) = (Arc::default(), Arc::default());
    let (kept, stopped) = (waits.clone(), stop.clone());
    std::thread::spawn(move || {
        while !stopped.load(Ordering::Relaxed) {
            let (sent, kept) = (Instant::now(), kept.clone());
            let _ = slint::invoke_from_event_loop(move || kept.lock().expect("waits").push((sent, sent.elapsed())));
            std::thread::sleep(Duration::from_millis(10));
        }
    });
    Answers { waits, stop }
}

impl Answers {
    /// The longest any post sent since `since` waited for the interface thread.
    pub(crate) fn worst_since(&self, since: Instant) -> Duration {
        let waits = self.waits.lock().expect("waits");
        waits.iter().filter(|(sent, _)| *sent >= since).map(|(_, wait)| *wait).max().unwrap_or_default()
    }
}

impl Drop for Answers {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
