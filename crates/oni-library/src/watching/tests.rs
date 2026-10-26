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

/// The descriptors this process holds, as /dev/fd lists them.
#[cfg(target_os = "macos")]
fn descriptors() -> usize {
    std::fs::read_dir("/dev/fd").map_or(0, |d| d.count())
}

#[cfg(target_os = "macos")]
#[test]
fn a_deep_watch_holds_no_descriptor_per_file() {
    let (_scratch, computer, root) = rig("watching-descriptors");
    let deep = paths::join(&root, "Home/Scans");
    for i in 0..400 {
        std::fs::write(Path::new(&deep).join(format!("Roll 8/f{i}.NEF")), [0u8]).unwrap();
    }
    let before = descriptors();
    let (watch, _calls) = live(&computer, &deep, &paths::join(&root, "Home/Scans/Other"));
    assert!(descriptors() < before + 50, "FSEvents: {} descriptors more", descriptors() - before);
    drop(watch);
    // Twin: kqueue, which Slint's live preview adds to notify, holds one per file of the same tree.
    let mut kqueue = notify::KqueueWatcher::new(|_| {}, notify::Config::default()).unwrap();
    notify::Watcher::watch(&mut kqueue, Path::new(&deep), notify::RecursiveMode::Recursive).unwrap();
    assert!(descriptors() >= before + 400, "kqueue: {} descriptors more", descriptors().saturating_sub(before));
}

#[test]
fn a_release_lets_every_watch_go_and_its_end_watches_what_was_asked_meanwhile() {
    let (a, b) = (vec!["/A".to_owned()], vec!["/B".to_owned()]);
    let mut asked = Asked::default();
    assert_eq!(asked.set(a.clone(), Vec::new()), Some((a.clone(), Vec::new())));
    assert_eq!(asked.release(), (Vec::new(), Vec::new()));
    assert_eq!(asked.set(b.clone(), Vec::new()), None, "nothing is watched while a rename runs");
    assert_eq!(asked.release(), (Vec::new(), Vec::new()));
    assert_eq!(asked.resume(), None, "an inner release's end leaves the outer one standing");
    assert_eq!(asked.resume(), Some((b.clone(), Vec::new())));
    // Twin: a resume with no release standing changes nothing.
    assert_eq!(asked.resume(), None);
    assert_eq!(asked.set(a.clone(), Vec::new()), Some((a, Vec::new())));
}

#[test]
fn a_photo_dropped_while_the_watches_are_released_is_not_told() {
    let (_scratch, computer, root) = rig("watch-released");
    let (roll, scans) = (paths::join(&root, "Home/Scans/Roll 8"), paths::join(&root, "Home/Scans"));
    let (watch, calls) = live(&computer, &roll, &scans);
    let released = watch.released();
    std::fs::write(Path::new(&roll).join("C.nef"), b"x").unwrap();
    std::thread::sleep(Duration::from_millis(1500));
    assert!(calls.lock().unwrap().is_empty(), "{:?}", calls.lock().unwrap());
    // Twin: once the release ends, a photo dropped is told again.
    drop(released);
    let mut n = 0;
    until("the watch back", Duration::from_secs(30), || {
        n += 1;
        std::fs::write(Path::new(&roll).join(format!("D-{n}.nef")), b"x").unwrap();
        std::thread::sleep(Duration::from_millis(100));
        !calls.lock().unwrap().is_empty()
    });
}

#[test]
fn an_overflow_on_windows_is_a_rescan_of_the_folder_and_the_read_goes_on() {
    use notify::rdcw_completion::{completion, rescan, Completion, ERROR_NOTIFY_ENUM_DIR};
    let exists = || true;
    assert_eq!(completion(ERROR_NOTIFY_ENUM_DIR, 0, exists), Completion::Rescan);
    assert_eq!(completion(0, 0, exists), Completion::Rescan, "a buffer discarded whole reads nothing");
    assert_eq!(completion(5, 0, || false), Completion::Gone);
    // Twin: a read that wrote its changes is parsed, and an unknown error still ends the watch.
    assert_eq!(completion(0, 120, exists), Completion::Changes);
    assert_eq!(completion(5, 120, exists), Completion::Changes);
    assert_eq!(completion(87, 120, exists), Completion::Failed);
    assert_eq!(completion(995, 0, exists), Completion::Stopped);

    // The rescan touches every watched folder under the one whose read overflowed.
    let held = Held { deep: vec![("D:/Scans/Roll 8".into(), "D:/Scans/Roll 8".into())], flat: vec![("D:/Scans".into(), "D:/Scans".into())] };
    let touched = held.touched(&rescan(std::path::PathBuf::from("D:/Scans")));
    assert_eq!(touched, Touched { deep: vec!["D:/Scans/Roll 8".into()], flat: vec!["D:/Scans".into()] });
    // Twin: the same path told as a plain change touches nothing, it being a folder that stays.
    let plain = notify::Event::new(EventKind::Other).add_path(std::path::PathBuf::from("D:/Scans"));
    assert_eq!(held.touched(&plain), Touched::default());
}
