use super::*;
use crate::scratch::Scratch;
use std::sync::{Arc, Mutex};

fn rig(label: &str) -> (Scratch, Computer, String) {
    let scratch = Scratch::new(label);
    let root = paths::normal(&scratch.path().to_string_lossy()).into_owned();
    for folder in ["Home/Scans/Roll 8", "Home/Scans/Other", "Volumes/T7"] {
        std::fs::create_dir_all(Path::new(&root).join(folder)).unwrap();
    }
    let computer = Computer { home: paths::join(&root, "Home"), volumes: paths::join(&root, "Volumes") };
    (scratch, computer, root)
}

fn until(what: &str, limit: Duration, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < limit, "{what} within {limit:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The calls told so far, and a watch that is live: a probe written until one lands, then quiet.
fn live(computer: &Computer, deep: &str, flat: &str) -> (Watch, Arc<Mutex<Vec<Touched>>>) {
    let calls: Arc<Mutex<Vec<Touched>>> = Arc::default();
    let into = calls.clone();
    let pace = Pace { quiet: Duration::from_millis(300), longest: Duration::from_secs(3) };
    let watch = Watch::start(computer.clone(), pace, move |t| into.lock().unwrap().push(t));
    watch.set(vec![deep.to_owned()], vec![flat.to_owned()]);
    let mut n = 0;
    until("the watch live", Duration::from_secs(30), || {
        n += 1;
        std::fs::write(Path::new(deep).join(format!("probe-{n}.tif")), b"x").unwrap();
        std::thread::sleep(Duration::from_millis(100));
        !calls.lock().unwrap().is_empty()
    });
    quiet(&calls);
    calls.lock().unwrap().clear();
    (watch, calls)
}

/// Waits until no call has come for a second, the probes' last bursts told.
fn quiet(calls: &Arc<Mutex<Vec<Touched>>>) {
    let (mut seen, mut since) = (usize::MAX, Instant::now());
    until("the disk quiet", Duration::from_secs(30), || {
        let now = calls.lock().unwrap().len();
        if now != seen {
            (seen, since) = (now, Instant::now());
        }
        since.elapsed() > Duration::from_secs(1)
    });
}

#[test]
fn a_photo_dropped_in_a_watched_folder_is_told_and_one_deeper_than_a_flat_watch_is_not() {
    let (_scratch, computer, root) = rig("watch-drop");
    let (roll, scans) = (paths::join(&root, "Home/Scans/Roll 8"), paths::join(&root, "Home/Scans"));
    let (_watch, calls) = live(&computer, &roll, &scans);

    // A sidecar and a hidden file count for nothing; a photo in the deep watch does.
    std::fs::write(Path::new(&roll).join("probe-1.tif.json"), b"{}").unwrap();
    std::fs::write(Path::new(&roll).join(".DS_Store"), b"x").unwrap();
    std::fs::write(Path::new(&root).join("Home/Scans/Other/A.nef"), b"x").unwrap();
    std::fs::write(Path::new(&roll).join("B.nef"), b"x").unwrap();
    until("B told", Duration::from_secs(20), || !calls.lock().unwrap().is_empty());
    quiet(&calls);
    let told: Vec<Touched> = std::mem::take(&mut *calls.lock().unwrap());
    assert!(told.iter().all(|t| t.deep == vec![roll.clone()]), "{told:?}");
    // Twin: the flat watch on Scans sees nothing of a photo two levels down.
    assert!(told.iter().all(|t| t.flat.is_empty()), "a flat watch saw a photo below its level: {told:?}");

    // A folder made right inside the flat watch is told to it.
    std::fs::create_dir(Path::new(&scans).join("Roll 9")).unwrap();
    until("Roll 9 told", Duration::from_secs(20), || calls.lock().unwrap().iter().any(|t| t.flat == vec![scans.clone()]));
}

#[test]
fn three_hundred_photos_copied_are_told_in_one_or_two_calls() {
    let (_scratch, computer, root) = rig("watch-burst");
    let roll = paths::join(&root, "Home/Scans/Roll 8");
    let (_watch, calls) = live(&computer, &roll, &paths::join(&root, "Home/Scans/Other"));
    for i in 0..300 {
        std::fs::write(Path::new(&roll).join(format!("scan_{i:04}.tif")), b"x").unwrap();
    }
    until("the copy told", Duration::from_secs(20), || !calls.lock().unwrap().is_empty());
    quiet(&calls);
    let n = calls.lock().unwrap().len();
    assert!(n <= 2, "{n} calls for one copy");
}

#[test]
fn a_burst_waits_for_the_disk_to_quiet() {
    let pace = Pace { quiet: Duration::from_millis(500), longest: Duration::from_secs(3) };
    let touched = || Touched { deep: vec!["/Roll".into()], flat: Vec::new() };
    let tell = |pace: Pace| {
        let (mut burst, start, mut told) = (Burst::new(pace), Instant::now(), 0);
        for i in 0..300 {
            let now = start + Duration::from_millis(i);
            burst.note(now, touched());
            told += usize::from(burst.take(now).is_some());
        }
        told + usize::from(burst.take(start + Duration::from_secs(1)).is_some())
    };
    assert_eq!(tell(pace), 1);
    // Twin: with no quiet asked, every event is a call of its own.
    assert_eq!(tell(Pace { quiet: Duration::ZERO, longest: Duration::ZERO }), 300);

    // A burst that never quiets is told at its longest wait, then gathers again.
    let (mut burst, start) = (Burst::new(pace), Instant::now());
    let mut at = Vec::new();
    for i in 0..80 {
        let now = start + Duration::from_millis(100 * i);
        burst.note(now, touched());
        if burst.take(now).is_some() {
            at.push(i);
        }
    }
    assert_eq!(at, vec![30, 61]);
}

#[test]
fn neither_a_disk_nor_the_home_is_ever_watched() {
    let (_scratch, computer, root) = rig("watch-wide");
    for wide in [computer.home.as_str(), &paths::join(&root, "Volumes/T7"), &root, "/", ""] {
        assert!(!may_watch(&computer, wide), "{wide} watched");
    }
    // Twin: a roll folder under either is watched.
    assert!(may_watch(&computer, &paths::join(&root, "Home/Scans/Roll 8")));
    assert!(may_watch(&computer, &paths::join(&root, "Volumes/T7/Film")));
}
