//! `OniTrash.selfCheck` through the store, on files of the test's own: a triplet goes to the ONI
//! Trash on its three sidecars and comes back from any of its files; one step, which ⌘Z hands back.

use oni_core::{Archive, EditorStore, Surface};
use oni_settings::frame::RgbPasses;
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::PipelineSettings;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

struct Folder(PathBuf);

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn folder(label: &str) -> Folder {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("oni-core-trash-{label}-{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    Folder(dir)
}

/// Every file of the folder by name and bytes, sidecars apart from originals.
fn files(dir: &Path, sidecars: bool) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".json") == sidecars)
        .map(|n| (n.clone(), std::fs::read(dir.join(&n)).unwrap()))
        .collect()
}

fn trashed(paths: &[String]) -> Vec<bool> {
    paths.iter().map(|p| sidecar::read(Path::new(p), false, true).unwrap().trashed_at.is_some()).collect()
}

#[test]
fn a_triplet_goes_whole_in_one_step_and_undo_and_redo_hand_it_back() {
    let dir = folder("triplet");
    let at = |n: &str| dir.0.join(n).to_string_lossy().into_owned();
    let (lead, green, blue, neighbour) = (at("t-r.ARW"), at("t-g.ARW"), at("t-b.ARW"), at("n.ARW"));
    for (i, p) in [&lead, &green, &blue, &neighbour].iter().enumerate() {
        std::fs::write(p, format!("original {i}")).unwrap();
    }
    let passes = RgbPasses { red: "t-r.ARW".into(), green: "t-g.ARW".into(), blue: "t-b.ARW".into(), registration: None };
    let mut led = PipelineSettings { rgb_passes: Some(passes), ..PipelineSettings::default() };
    led.print.exposure = 0.4;
    sidecar::write(&led, Path::new(&lead), WriteOptions::default()).unwrap();
    let pass = PipelineSettings { pass_of: Some("t-r.ARW".into()), ..PipelineSettings::default() };
    for p in [&green, &blue] {
        sidecar::write(&pass, Path::new(p), WriteOptions::default()).unwrap();
    }
    sidecar::write(&PipelineSettings::default(), Path::new(&neighbour), WriteOptions::default()).unwrap();
    let all = [lead.clone(), green.clone(), blue.clone(), neighbour.clone()];
    let originals = files(&dir.0, false);
    let mut store = EditorStore::new(Archive::new(all.to_vec(), |_: &Path| false));
    store.open(&lead).unwrap();

    assert_eq!(store.move_to_trash(std::slice::from_ref(&lead)), 1);
    assert!(store.path().is_none(), "the open frame closes");
    assert_eq!(trashed(&all), [true, true, true, false], "the triplet on its three files, the neighbour left alone");
    assert_eq!(files(&dir.0, false), originals, "no original moves, vanishes or changes");
    assert_eq!(store.gallery_history().undo_label(), Some("Move to ONI Trash"));

    store.undo(Surface::Gallery);
    assert_eq!(trashed(&all), [false; 4], "one undo restores the three files");
    assert_eq!(sidecar::read(Path::new(&lead), false, true).unwrap().settings, led, "the settings untouched");
    store.redo(Surface::Gallery);
    assert_eq!(trashed(&all), [true, true, true, false], "redo marks them again");

    // Restored from a pass, the three come back; twin: a restore that moved nothing opens no step.
    assert_eq!(store.restore_from_trash(std::slice::from_ref(&blue)), 1);
    assert_eq!(trashed(&all), [false; 4]);
    let label = store.gallery_history().undo_label().map(str::to_owned);
    assert_eq!(store.restore_from_trash(&[]), 0);
    assert_eq!(store.gallery_history().undo_label().map(str::to_owned), label);
    assert_eq!(files(&dir.0, false), originals);
}
