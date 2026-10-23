//! The latest value asked of a thread: one waits at most, a newer one replacing it unseen, so a
//! thread that renders never runs behind what is asked of it.

use std::sync::{Condvar, Mutex};
use std::time::Duration;

pub struct Latest<T> {
    slot: Mutex<(Option<T>, bool)>,
    wake: Condvar,
}

/// What a wait on the slot found.
pub enum Taken<T> {
    Value(T),
    TimedOut,
    Closed,
}

impl<T> Latest<T> {
    pub const fn new() -> Latest<T> {
        Latest { slot: Mutex::new((None, false)), wake: Condvar::new() }
    }

    /// Lays `value` in the slot, replacing one still waiting there.
    pub fn put(&self, value: T) {
        self.slot.lock().expect("latest slot").0 = Some(value);
        self.wake.notify_one();
    }

    /// Ends the waits: every one from now on finds the slot closed.
    pub fn close(&self) {
        self.slot.lock().expect("latest slot").1 = true;
        self.wake.notify_one();
    }

    /// A value is waiting.
    pub fn waiting(&self) -> bool {
        self.slot.lock().map(|s| s.0.is_some()).unwrap_or(false)
    }

    /// The value waiting, after at most `patience`, or forever without one.
    pub fn take(&self, patience: Option<Duration>) -> Taken<T> {
        let mut s = self.slot.lock().expect("latest slot");
        loop {
            if s.1 {
                return Taken::Closed;
            }
            if let Some(value) = s.0.take() {
                return Taken::Value(value);
            }
            match patience {
                None => s = self.wake.wait(s).expect("latest slot"),
                Some(p) => {
                    let (next, waited) = self.wake.wait_timeout(s, p).expect("latest slot");
                    s = next;
                    if waited.timed_out() && s.0.is_none() && !s.1 {
                        return Taken::TimedOut;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_newer_value_replaces_the_one_waiting_and_a_closed_slot_ends_the_wait() {
        let slot = Latest::new();
        slot.put(1);
        slot.put(2);
        assert!(matches!(slot.take(None), Taken::Value(2)));
        // Twin: a queue would still hold the first value, which the thread has no use for.
        assert!(!slot.waiting());
        assert!(matches!(slot.take(Some(Duration::from_millis(5))), Taken::TimedOut));
        slot.put(3);
        slot.close();
        assert!(matches!(slot.take(None), Taken::Closed));
    }

}
