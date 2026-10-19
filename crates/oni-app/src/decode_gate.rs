//! One full decode at a time, the latest asked winning: walking a roll never piles decodes up, each
//! 0.4 to 0.7 GB of pixels at 24 MP.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct Gate {
    latest: AtomicU64,
    running: Mutex<()>,
}

/// The open frame's preview, its full resolution, and Prepare the Roll's full scan.
pub static PREVIEW: Gate = Gate::new();
pub static FULL: Gate = Gate::new();
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
                s.spawn(move || {
                    gate.run(ticket, || assert_eq!(busy.fetch_add(1, Ordering::SeqCst), 0, "two decodes at once"));
                    busy.store(0, Ordering::SeqCst);
                });
            }
        });
    }
}
