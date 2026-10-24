//! Drives and mounts looked at with a deadline, so one that sleeps never holds Computer's rows.

use crate::paths;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// The drive letters that answer, `C:/` and on.
pub(super) fn drives() -> Vec<String> {
    let letters = (b'A'..=b'Z').map(|l| format!("{}:/", l as char)).collect();
    answered(letters, |d| Path::new(&d).is_dir().then_some(d))
}

/// The disks mounted in `volumes`, each as its realpath names it, the boot one left out.
pub(super) fn mounted(volumes: &str) -> Vec<String> {
    let entries: Vec<String> = std::fs::read_dir(volumes)
        .map(|listing| {
            listing
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
                .map(|p| p.to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    let folder = volumes.to_owned();
    let mut mounted = answered(entries, move |entry| {
        let real = paths::canonical(Path::new(&entry));
        (real != "/" && paths::parent(&real) == folder).then_some(real)
    });
    mounted.sort_by(|a, b| paths::standard_order(a, b));
    mounted.dedup();
    mounted
}

/// How long a drive or a mount is waited for: a network drive asleep stalls its look for seconds.
const ANSWER: Duration = Duration::from_millis(400);

/// Each key's look in flight, and its last answer.
#[derive(Default)]
struct Probe {
    flight: Option<Arc<OnceLock<Option<String>>>>,
    last: Option<String>,
}

static PROBES: Mutex<BTreeMap<String, Probe>> = Mutex::new(BTreeMap::new());

/// `look` on each key, on threads of their own, waited `ANSWER` at most: one still looking gives its
/// last answer and is not asked again until it lands. The answers in the keys' order.
pub(super) fn answered(keys: Vec<String>, look: impl Fn(String) -> Option<String> + Clone + Send + 'static) -> Vec<String> {
    let flights: Vec<Arc<OnceLock<Option<String>>>> = {
        let mut probes = PROBES.lock().unwrap_or_else(|e| e.into_inner());
        keys.iter()
            .map(|key| {
                let probe = probes.entry(key.clone()).or_default();
                probe
                    .flight
                    .get_or_insert_with(|| {
                        let slot = Arc::new(OnceLock::new());
                        let (into, key, look) = (slot.clone(), key.clone(), look.clone());
                        let started = std::thread::Builder::new().name("volume".into()).spawn(move || {
                            let _ = into.set(look(key));
                        });
                        if started.is_err() {
                            let _ = slot.set(None);
                        }
                        slot
                    })
                    .clone()
            })
            .collect()
    };
    let deadline = Instant::now() + ANSWER;
    while Instant::now() < deadline && flights.iter().any(|f| f.get().is_none()) {
        std::thread::sleep(Duration::from_millis(2));
    }
    let mut probes = PROBES.lock().unwrap_or_else(|e| e.into_inner());
    keys.iter()
        .filter_map(|key| {
            let probe = probes.get_mut(key)?;
            if let Some(answer) = probe.flight.as_ref().and_then(|f| f.get().cloned()) {
                probe.last = answer;
                probe.flight = None;
            }
            probe.last.clone()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    #[test]
    fn a_mount_that_hangs_is_left_out_and_asked_once_until_it_answers() {
        static HELD: AtomicBool = AtomicBool::new(true);
        static LOOKS: AtomicUsize = AtomicUsize::new(0);
        let keys: Vec<String> = ["hang-test/A", "hang-test/NAS", "hang-test/B"].map(String::from).to_vec();
        let look = |key: String| {
            if key.ends_with("NAS") {
                LOOKS.fetch_add(1, Ordering::Relaxed);
                while HELD.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
            Some(key)
        };
        let start = Instant::now();
        assert_eq!(answered(keys.clone(), look), vec!["hang-test/A", "hang-test/B"]);
        assert_eq!(answered(keys.clone(), look), vec!["hang-test/A", "hang-test/B"]);
        assert!(start.elapsed() < Duration::from_secs(3), "the hung look held the probe {:?}", start.elapsed());
        assert_eq!(LOOKS.load(Ordering::Relaxed), 1, "a look still in flight is not asked again");

        // Twin: the same mount answering is listed, so it was left out for its silence alone.
        HELD.store(false, Ordering::Relaxed);
        let deadline = Instant::now() + Duration::from_secs(20);
        while answered(keys.clone(), look).len() < 3 {
            assert!(Instant::now() < deadline, "the mount listed once it answers");
        }
        assert_eq!(answered(keys, look), vec!["hang-test/A", "hang-test/NAS", "hang-test/B"]);
    }
}
