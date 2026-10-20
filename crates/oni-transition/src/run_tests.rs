use super::*;
use crate::test_support::Scratch;
use oni_settings::sidecar;
use std::time::{Duration, SystemTime};

#[test]
fn the_frames_tried_spread_over_the_roll() {
    assert_eq!(spread(38), vec![9, 22, 34, 3, 15, 28]);
    assert_eq!(spread(2), vec![0, 1]);
    assert!(spread(0).is_empty());
    // Twin: the roll's first frames are not the first tried.
    assert_ne!(spread(38)[..3], [0, 1, 2]);
}

/// Sets a file's modification date `days` before now.
fn aged(path: &Path, days: u64) {
    let file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(days * 86_400)).unwrap();
}

fn frame(folder: &Path, name: &str) -> PathBuf {
    let raw = folder.join(name);
    std::fs::write(&raw, b"raw").unwrap();
    let mut s = oni_settings::PipelineSettings::rest(oni_settings::ConversionMode::Negative, true);
    s.gains.stops = [-1.0, 0.0, 1.0];
    sidecar::write(&s, &raw, sidecar::WriteOptions::default()).unwrap();
    raw
}

#[test]
fn only_raw_sidecars_written_before_the_install_are_found() {
    let roll = Scratch::new("survey");
    let old = frame(roll.path(), "old.NEF");
    aged(&sidecar::url_for(&old), 30);
    let recent = frame(roll.path(), "recent.NEF");
    let scan = frame(roll.path(), "scan.tif");
    aged(&sidecar::url_for(&scan), 30);
    let roots = vec![roll.path().to_string_lossy().into_owned(), "/Volumes/oni-unplugged-disk".into()];
    let found = survey::survey(&roots, records::now() - 86_400.0, &Ledger::default());
    let names: Vec<_> = found.raws.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["old.NEF"]);
    assert_eq!(found.unreachable, ["/Volumes/oni-unplugged-disk"]);
    assert!(survey::written_at(&sidecar::url_for(&recent)).is_some());
    // Twin: once the ledger settles it, the same sidecar is not found again.
    let mut ledger = Ledger::default();
    ledger.entries.insert(
        sidecar::url_for(&old).to_string_lossy().into_owned(),
        Entry { sidecar: String::new(), outcome: Outcome::Corrected, reason: None, before: String::new(), after: None, stops: None, backup: None, at: 0.0 },
    );
    assert!(survey::survey(&roots, records::now() - 86_400.0, &ledger).raws.is_empty());
}

#[test]
fn a_run_measures_corrects_and_never_twice_on_real_frames() {
    let Some(dir) = oni_fixtures::raw_dir("a_run_measures_corrects_and_never_twice_on_real_frames") else { return };
    let mut nefs: Vec<PathBuf> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|e| e == "NEF")).collect();
    nefs.sort();
    let (support, roll) = (Scratch::new("support"), Scratch::new("roll"));
    for nef in nefs.iter().take(2) {
        let copy = roll.path().join(nef.file_name().unwrap());
        std::fs::copy(nef, &copy).unwrap();
        let mut s = oni_settings::PipelineSettings::rest(oni_settings::ConversionMode::Negative, true);
        s.gains.stops = [-1.0, 0.0, 1.0];
        sidecar::write(&s, &copy, sidecar::WriteOptions::default()).unwrap();
        aged(&sidecar::url_for(&copy), 30);
    }
    let roots = vec![roll.path().to_string_lossy().into_owned()];
    assert_eq!(pending(support.path(), &roots), 2);
    let apple = crate::Simulated { stops: [0.063, -0.026, -0.126] };
    let mut events = Vec::new();
    let summary = run(support.path(), &roots, &apple, &mut |e| events.push(e)).unwrap();
    assert_eq!(summary.corrected.len(), 2, "{summary:?}");
    let stops = summary.cameras[0].stops.clone().unwrap();
    for c in 0..3 {
        assert!((stops[c] - apple.stops[c]).abs() < 0.002, "{stops:?}");
    }
    assert!(events.contains(&Event::Corrected { done: 2, of: 2 }));
    let read = sidecar::read(&summary.corrected[0], true, true).unwrap().settings;
    assert!((read.gains.stops[2] - (1.0 + stops[2] as f32)).abs() < 1e-5);
    // Twin: the run again finds nothing, and the gains stay where one run put them.
    assert_eq!(pending(support.path(), &roots), 0);
    let second = run(support.path(), &roots, &apple, &mut |_| {}).unwrap();
    assert!(second.corrected.is_empty() && second.cameras.is_empty());
    assert_eq!(sidecar::read(&summary.corrected[0], true, true).unwrap().settings.gains, read.gains);
}
