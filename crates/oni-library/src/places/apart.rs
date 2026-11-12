//! The pool the navigator reads its rows on: each folder on a thread of its own, so one stuck in its
//! disk, a NAS asleep, holds no other.

use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

/// How long one read may hold a thread of `read_apart`'s before the next folder goes without it.
pub const STUCK: Duration = Duration::from_millis(250);

/// Each folder read on a thread of its own, `threads` counted at once, answers landing together as
/// they come; a read held past `stuck` stops counting, so one stuck in its disk holds no other.
pub fn read_apart<T: Send + 'static>(
    folders: Vec<String>,
    threads: usize,
    stuck: Duration,
    read: impl Fn(&str) -> T + Send + Sync + 'static,
    land: impl Fn(Vec<(String, T)>) + Send + 'static,
) {
    if folders.is_empty() {
        return;
    }
    let (send, receive) = mpsc::channel::<(String, T)>();
    let _ = std::thread::Builder::new().name("landing".into()).spawn(move || {
        while let Ok(first) = receive.recv() {
            let mut landed = vec![first];
            landed.extend(receive.try_iter());
            land(landed);
        }
    });
    let read = Arc::new(read);
    let _ = std::thread::Builder::new().name("reading".into()).spawn(move || {
        let held: Arc<(Mutex<Vec<(usize, Instant)>>, Condvar)> = Arc::default();
        for (n, folder) in folders.into_iter().enumerate() {
            let (lock, freed) = &*held;
            let Ok(mut running) = lock.lock() else { return };
            loop {
                running.retain(|(_, at)| at.elapsed() < stuck);
                if running.len() < threads.max(1) {
                    break;
                }
                let wait = running.iter().map(|(_, at)| stuck.saturating_sub(at.elapsed())).min().unwrap_or(stuck);
                let Ok((again, _)) = freed.wait_timeout(running, wait) else { return };
                running = again;
            }
            running.push((n, Instant::now()));
            drop(running);
            let (read, send, held) = (read.clone(), send.clone(), held.clone());
            let _ = std::thread::Builder::new().name("folder".into()).spawn(move || {
                let answer = read(&folder);
                let _ = send.send((folder, answer));
                let (lock, freed) = &*held;
                if let Ok(mut running) = lock.lock() {
                    running.retain(|(i, _)| *i != n);
                }
                freed.notify_one();
            });
        }
    });
}

