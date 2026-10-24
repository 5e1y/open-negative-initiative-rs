//! `OniTrash.selfCheck` on files of the test's own: the mark rides in the sidecar alone, a triplet
//! goes and comes back whole, and every listing the library cuts leaves it out.

use super::*;
use crate::index::{self, LibraryIndex};
use crate::project::{self, Project};
use crate::scratch::Scratch;
use oni_settings::frame::RgbPasses;
use oni_settings::sidecar::{ExifMetadata, WriteOptions};
use oni_settings::PipelineSettings;
use std::collections::BTreeMap;

/// Every file under `folder` by its name and its bytes, so a check proves the mark moved nothing.
fn listing(folder: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(folder)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file())
        .map(|e| (e.file_name().to_string_lossy().into_owned(), std::fs::read(e.path()).unwrap()))
        .collect()
}

fn photo(folder: &Path, name: &str, settings: Option<&PipelineSettings>) -> String {
    let path = folder.join(name);
    std::fs::write(&path, b"raw").unwrap();
    if let Some(settings) = settings {
        sidecar::write(settings, &path, WriteOptions::default()).unwrap();
    }
    path.to_string_lossy().into_owned()
}

fn read(path: &str) -> sidecar::Loaded {
    sidecar::read(Path::new(path), true, true).unwrap()
}

fn when() -> Date {
    Date::since_reference_date(812_345_678.0)
}

#[test]
fn a_mark_rides_in_the_sidecar_alone_and_a_restore_clears_its_key() {
    let scratch = Scratch::new("trash-round-trip");
    let mut edited = PipelineSettings::default();
    edited.gains.stops = [0.5, 0.0, -0.25];
    let frame = photo(scratch.path(), "frame.RW2", Some(&edited));
    assert_eq!(read(&frame).trashed_at, None, "a sidecar without the key reads not trashed");
    let exif = ExifMetadata { camera: "Nikon FM2".into(), ..ExifMetadata::default() };
    sidecar::set_exif(exif, Path::new(&frame), true).unwrap();
    let before = listing(scratch.path());

    let marked = mark(std::slice::from_ref(&frame), Some(when()), &|_| true);
    assert_eq!((marked.done.clone(), report(&marked, false)), (vec![frame.clone()], None));
    let loaded = read(&frame);
    assert_eq!((loaded.trashed_at, &loaded.settings), (Some(when()), &edited), "the date round-trips, the settings ride through");
    assert_eq!(loaded.exif.map(|e| e.camera).as_deref(), Some("Nikon FM2"), "the EXIF left as it was");
    let after = listing(scratch.path());
    assert_eq!(after.keys().collect::<Vec<_>>(), before.keys().collect::<Vec<_>>(), "no file moved, appeared or vanished");
    assert_eq!(after["frame.RW2"], b"raw", "the photograph's own file is not touched by a byte");

    let mut later = edited.clone();
    later.gains.stops[0] = 1.0;
    sidecar::write(&later, Path::new(&frame), WriteOptions::default()).unwrap();
    assert_eq!(read(&frame).trashed_at, Some(when()), "a settings write after it keeps the mark");
    // Twin: a write that does not read the mark back loses it, which the line above would see.
    let bare = WriteOptions { trashed_at: Some(None), ..WriteOptions::default() };
    sidecar::write(&later, Path::new(&frame), bare).unwrap();
    assert_eq!(read(&frame).trashed_at, None);
    assert_eq!(mark(std::slice::from_ref(&frame), Some(when()), &|_| true).done, [frame.clone()]);

    let restored = mark(std::slice::from_ref(&frame), None, &|_| true);
    let text = std::fs::read_to_string(sidecar::url_for(Path::new(&frame))).unwrap();
    assert_eq!(restored.done, [frame.clone()]);
    assert!(read(&frame).trashed_at.is_none() && !text.contains("trashedAt"), "the key itself goes: {text}");

    // Twin: the listing sees a file moved away, so a trash that moved its photo would fail above.
    std::fs::create_dir(scratch.path().join("Trash")).unwrap();
    std::fs::rename(&frame, scratch.path().join("Trash/frame.RW2")).unwrap();
    assert_ne!(listing(scratch.path()).keys().collect::<Vec<_>>(), before.keys().collect::<Vec<_>>());
}

#[test]
fn a_newer_sidecar_is_left_byte_for_byte_and_the_gesture_says_so() {
    let scratch = Scratch::new("trash-newer");
    let future = photo(scratch.path(), "future.RW2", None);
    let side = sidecar::url_for(Path::new(&future));
    let bytes = br#"{"version": 99, "settings": {}}"#;
    std::fs::write(&side, bytes).unwrap();
    let refused = mark(std::slice::from_ref(&future), Some(when()), &|_| true);
    assert_eq!((refused.refused.clone(), refused.done.len()), (vec![future.clone()], 0));
    assert_eq!(std::fs::read(&side).unwrap(), bytes);
    assert!(report(&refused, false).is_some());
    // Twin: the same photo under a sidecar of this version is marked.
    std::fs::remove_file(&side).unwrap();
    let marked = mark(std::slice::from_ref(&future), Some(when()), &|_| true);
    assert_eq!((report(&marked, false), marked.done), (None, vec![future]));
}

#[test]
fn a_triplet_goes_on_its_three_files_and_comes_back_from_any_of_them() {
    let scratch = Scratch::new("trash-triplet");
    let passes = RgbPasses { red: "t-r.ARW".into(), green: "t-g.ARW".into(), blue: "t-b.ARW".into(), registration: None };
    let lead = PipelineSettings { rgb_passes: Some(passes), ..PipelineSettings::default() };
    let pass = PipelineSettings { pass_of: Some("t-r.ARW".into()), ..PipelineSettings::default() };
    let triplet =
        [photo(scratch.path(), "t-r.ARW", Some(&lead)), photo(scratch.path(), "t-g.ARW", Some(&pass)), photo(scratch.path(), "t-b.ARW", Some(&pass))];
    let neighbour = photo(scratch.path(), "n.ARW", Some(&PipelineSettings::default()));
    let trashed = || -> HashSet<String> {
        triplet.iter().chain([&neighbour]).filter(|p| read(p).trashed_at.is_some()).cloned().collect()
    };
    // Twin: the lead alone, unexpanded, marks one file, so the expansion is what reaches the passes.
    mark(&triplet[..1], Some(when()), &|_| true);
    assert_eq!(trashed(), HashSet::from([triplet[0].clone()]));
    mark(&crate::rgb_passes::expanded(&triplet[..1]), Some(when()), &|_| true);
    assert_eq!(trashed(), triplet.iter().cloned().collect(), "the triplet on its three files, the neighbour left alone");
    mark(&crate::rgb_passes::expanded(&triplet[1..2]), None, &|_| true);
    assert!(trashed().is_empty(), "restored on its three files from a pass");
}

#[test]
fn every_listing_leaves_a_trashed_frame_out_and_a_project_keeps_it_a_member() {
    let day = Date::since_reference_date(800_000_000.0);
    let entry = |path: &str, pass_of: Option<&str>, leads: bool| Entry {
        captured_at: Some(day),
        pass_of: pass_of.map(str::to_owned),
        leads_passes: leads,
        ..Entry::new(path, day, path)
    };
    let (kept, gone) = ("/roll/kept.RW2", "/roll/gone.RW2");
    let index = LibraryIndex::open(Path::new(":memory:")).unwrap();
    for row in [
        entry(kept, None, false),
        entry(gone, None, false),
        entry("/roll/p-r.ARW", None, true),
        entry("/roll/p-g.ARW", Some("p-r.ARW"), false),
        entry("/roll/p-b.ARW", Some("p-r.ARW"), false),
    ] {
        index.upsert(&row).unwrap();
    }
    for path in [gone, "/roll/p-r.ARW", "/roll/p-g.ARW", "/roll/p-b.ARW"] {
        index.set_trashed(Some(when()), path).unwrap();
    }
    let paths = |rows: &[Entry]| rows.iter().map(|e| e.path.clone()).collect::<Vec<_>>();
    assert_eq!(paths(&index.shown()), [kept], "the gallery's rows leave out a trashed frame and a trashed triplet");
    let groups = crate::folders::groups("/roll", &index.shown(), &HashMap::new(), &HashMap::new());
    assert_eq!(groups.iter().flat_map(|g| paths(&g.entries)).collect::<Vec<_>>(), [kept], "and so do the folder's groups");
    let trip = Project::new("Trip", day, vec![gone.into(), kept.into()]);
    let resolved = project::grouped(std::slice::from_ref(&trip), &index.shown());
    assert_eq!(paths(&resolved[0].entries), [kept], "a project hides it");
    assert!(trip.members.iter().any(|m| m == gone), "and keeps it a member");
    let binned: HashSet<String> = index::trashed(&index.all()).into_iter().map(|e| e.path).collect();
    assert_eq!(binned, HashSet::from([gone.to_owned(), "/roll/p-r.ARW".to_owned()]), "a triplet listed once, as its lead");

    // Twin: restored, the frame shows again beside the one never marked, and in the project.
    index.set_trashed(None, gone).unwrap();
    let mut shown = paths(&index.shown());
    shown.sort();
    assert_eq!(shown, [gone, kept]);
    assert_eq!(project::grouped(std::slice::from_ref(&trip), &index.shown())[0].entries.len(), 2);

    // A rebuild from the sidecars carries the mark back in, as it travels with the entry.
    let marked = Entry { trashed_at: Some(when()), ..Entry::new(gone, day, gone) };
    index.rebuild(&[entry(kept, None, false), marked]).unwrap();
    assert_eq!(paths(&index.shown()), [kept]);
    assert_eq!(index.row(gone).and_then(|e| e.trashed_at), Some(when()));
}
