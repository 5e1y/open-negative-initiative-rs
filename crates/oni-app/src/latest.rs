//! The latest value asked of a thread: one waits at most, a newer one replacing it unseen, so a
//! thread that renders never runs behind what is asked of it.

use std::sync::{Arc, Condvar, Mutex};
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

/// A thread of its own running `work` on the latest value asked, one at a time: the values asked
/// while it works replace one another, so a burst of asks runs it twice at most.
pub struct Worker<T> {
    slot: Arc<Latest<T>>,
}

impl<T: Send + 'static> Worker<T> {
    pub fn spawn(name: &str, mut work: impl FnMut(T) + Send + 'static) -> Worker<T> {
        let slot = Arc::new(Latest::new());
        let taken = slot.clone();
        let _ = std::thread::Builder::new().name(name.into()).spawn(move || {
            while let Taken::Value(value) = taken.take(None) {
                work(value);
            }
        });
        Worker { slot }
    }

    pub fn ask(&self, value: T) {
        self.slot.put(value);
    }
}

impl<T> Drop for Worker<T> {
    fn drop(&mut self) {
        self.slot.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Twenty asks while a slow piece of work runs; `coalesced` asks the worker, else a thread each.
    fn runs(coalesced: bool) -> (usize, usize) {
        let (count, last) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
        let (c, l) = (count.clone(), last.clone());
        let work = move |v: usize| {
            std::thread::sleep(Duration::from_millis(20));
            c.fetch_add(1, Ordering::SeqCst);
            l.store(v, Ordering::SeqCst);
        };
        if coalesced {
            let worker = Worker::spawn("check", work);
            (1..=20).for_each(|v| worker.ask(v));
            while last.load(Ordering::SeqCst) != 20 {
                std::thread::sleep(Duration::from_millis(5));
            }
        } else {
            let work = Arc::new(Mutex::new(work));
            let threads: Vec<_> = (1..=20).map(|v| { let w = work.clone(); std::thread::spawn(move || (w.lock().unwrap())(v)) }).collect();
            threads.into_iter().for_each(|t| t.join().unwrap());
        }
        std::thread::sleep(Duration::from_millis(50));
        (count.load(Ordering::SeqCst), last.load(Ordering::SeqCst))
    }

    #[test]
    fn a_burst_of_asks_runs_the_work_twice_at_most_and_last_on_the_latest() {
        let (count, last) = runs(true);
        assert!(count <= 2, "the work ran {count} times");
        assert_eq!(last, 20, "the latest value asked is the one worked on last");
        // Twin: a thread for each ask runs it every time.
        assert_eq!(runs(false).0, 20);
    }

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
