//! The archive on roll #8, the reference roll: its sidecars and roll marker copied into a folder of
//! the test's own, an empty placeholder standing for each NEF. The staged files are only ever read.

use oni_library::{
    Id, LibraryIndex, film_group, folders, index, library, project, relink, roll_membership, roots, trash,
};
use oni_settings::{Date, sidecar};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

const ROLL_ID: &str = "BC402E4A-035E-4A3A-B279-FB7E35D1DDEA";
const REFERENCE_ID: &str = "16399A5F-7AED-48E4-A01C-F5F0E7266154";
const FRAMES: usize = 38;

fn staged() -> Option<PathBuf> {
    let folder = oni_fixtures::raw_dir("roll #8 archive check")?;
    assert!(folder.join(".oni-roll.json").exists(), "{} holds no roll marker", folder.display());
    Some(folder)
}

/// Every file of a folder with its size and modification time, so a test can prove it wrote nothing there.
fn snapshot(folder: &Path) -> BTreeMap<String, (u64, std::time::SystemTime)> {
    std::fs::read_dir(folder)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| {
            let m = e.metadata().unwrap();
            (e.file_name().to_string_lossy().into_owned(), (m.len(), m.modified().unwrap()))
        })
        .collect()
}

/// Every file under `folder`, hidden ones included, by its path from it and its size.
fn listing(folder: &Path) -> BTreeMap<String, u64> {
    fn walk(base: &Path, folder: &Path, out: &mut BTreeMap<String, u64>) {
        for entry in std::fs::read_dir(folder).unwrap().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                walk(base, &path, out);
            } else {
                let relative = path.strip_prefix(base).unwrap().to_string_lossy().into_owned();
                out.insert(relative, entry.metadata().unwrap().len());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(folder, folder, &mut out);
    out
}

struct Copy {
    root: PathBuf,
    roll: PathBuf,
    staged: PathBuf,
    before: BTreeMap<String, (u64, std::time::SystemTime)>,
}

impl Copy {
    fn new(label: &str) -> Option<Copy> {
        Copy::made(label, false)
    }

    /// With `real`, each NEF the staged roll holds is copied whole instead of left empty.
    fn made(label: &str, real: bool) -> Option<Copy> {
        let staged = staged()?;
        let before = snapshot(&staged);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("oni-library-roll8-{label}-{}-{stamp}", std::process::id()));
        let root = PathBuf::from(library::canonical(&{
            std::fs::create_dir_all(&root).unwrap();
            root
        }));
        // Named apart from the roll, so a heading read off the folder cannot pass for the roll's own.
        let roll = root.join("roll8-copy");
        std::fs::create_dir(&roll).unwrap();
        std::fs::write(roll.join(".oni-roll.json"), std::fs::read(staged.join(".oni-roll.json")).unwrap()).unwrap();
        for name in before.keys().filter(|n| n.ends_with(".NEF.json")) {
            std::fs::write(roll.join(name), std::fs::read(staged.join(name)).unwrap()).unwrap();
            let original = name.trim_end_matches(".json");
            if real && before.contains_key(original) && ORACLE.iter().any(|(f, _)| *f == original) {
                std::fs::copy(staged.join(original), roll.join(original)).unwrap();
            } else {
                std::fs::write(roll.join(original), b"").unwrap();
            }
        }
        Some(Copy { root, roll, staged, before })
    }

    fn root_text(&self) -> String {
        self.root.to_string_lossy().into_owned()
    }

    fn members(&self) -> Vec<String> {
        roots::scan(&[self.root_text()])
    }
}

impl Drop for Copy {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
        if !std::thread::panicking() {
            assert_eq!(snapshot(&self.staged), self.before, "the staged roll was written to");
        }
    }
}

fn indexed_all(members: &[String]) -> Vec<index::Entry> {
    members.iter().filter_map(|m| library::indexed(m)).collect()
}

#[test]
fn an_index_rebuilt_from_the_sidecars_finds_every_frame() {
    let Some(copy) = Copy::new("rebuild") else { return };
    let members = copy.members();
    assert_eq!(members.len(), FRAMES);
    assert!(members.iter().all(|m| m.ends_with(".NEF")));
    // Twin: every file of the folder, sidecars and marker included, is not the membership.
    assert_eq!(listing(&copy.root).len(), 2 * FRAMES + 1);

    let index = LibraryIndex::open(&copy.root.join("caches").join("index.sqlite")).unwrap();
    index.rebuild(&indexed_all(&members)).unwrap();
    assert_eq!(index.count(), FRAMES);
    assert!(library::unresolved(&members, &index).is_empty());
    let rows = index.all();
    let paths: HashSet<&str> = rows.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(paths, members.iter().map(String::as_str).collect());
    assert!(rows.iter().all(|e| e.mode == Some(oni_settings::ConversionMode::Negative)
        && e.film.is_none()
        && e.trashed_at.is_none()
        && e.fingerprint == "0-no-date-0"));

    // The roll is the folder's marker, read off the disk, never the index.
    let rolls = folders::rolls(&members);
    let roll_folder = copy.roll.to_string_lossy().into_owned();
    let film = rolls.get(&roll_folder).expect("the folder carries its roll");
    assert_eq!(
        (film.id, film.name.as_str(), film.stock.as_deref()),
        (Id::parse(ROLL_ID).unwrap(), "Roll #8", Some("fujifilm-fujicolor"))
    );
    let groups = folders::groups(&copy.root_text(), &index.shown(), &rolls, &Default::default());
    assert_eq!(groups.len(), 1);
    assert_eq!((groups[0].display_title(), groups[0].entries.len()), ("Roll #8".to_owned(), FRAMES));
    // Twin: without the marker the folder heads with its own name.
    let bare = folders::groups(&copy.root_text(), &index.shown(), &Default::default(), &Default::default());
    assert_eq!(bare[0].display_title(), "roll8-copy");

    // A disk gone with one frame: no row is invented, and putting it back is the repair.
    let away = &members[7];
    let hidden = copy.root.join("away.bin");
    std::fs::rename(away, &hidden).unwrap();
    assert!(library::indexed(away).is_none());
    index.rebuild(&indexed_all(&members)).unwrap();
    assert_eq!(index.count(), FRAMES - 1);
    assert_eq!(library::unresolved(&members, &index), std::slice::from_ref(away));
    std::fs::rename(&hidden, away).unwrap();
    for path in library::unresolved(&members, &index) {
        index.upsert(&library::indexed(&path).unwrap()).unwrap();
    }
    assert_eq!(index.count(), FRAMES);
    assert!(library::unresolved(&members, &index).is_empty());
}

#[test]
fn roll_membership_reads_the_reference_id_each_sidecar_names() {
    let Some(copy) = Copy::new("membership") else { return };
    let members = copy.members();
    let reference = Id::parse(REFERENCE_ID).unwrap();
    assert_eq!(roll_membership::measured(reference, &members).len(), FRAMES);
    // Twin: another id names no frame of the roll.
    assert!(roll_membership::measured(Id::new(), &members).is_empty());

    // Twin: one sidecar moved onto another reference leaves the roll, and only it.
    let strayed = &members[3];
    let side = sidecar::url_for(Path::new(strayed));
    let mut document: serde_json::Value = serde_json::from_slice(&std::fs::read(&side).unwrap()).unwrap();
    let other = Id::new();
    document["settings"]["rollReference"]["id"] = serde_json::Value::String(other.to_string());
    std::fs::write(&side, serde_json::to_vec(&document).unwrap()).unwrap();
    let measured = roll_membership::measured(reference, &members);
    assert_eq!(measured.len(), FRAMES - 1);
    assert!(!measured.contains(strayed));
    assert_eq!(roll_membership::reference(strayed), Some(other));
    // A frame with no sidecar stands on no reference.
    let lone = copy.roll.join("lone.NEF");
    std::fs::write(&lone, b"").unwrap();
    assert_eq!(roll_membership::reference(&lone.to_string_lossy()), None);
}

#[test]
fn the_trash_marks_sidecars_and_never_moves_a_file() {
    let Some(copy) = Copy::new("trash") else { return };
    let members = copy.members();
    let before = listing(&copy.root);
    let bytes = |p: &str| std::fs::read(sidecar::url_for(Path::new(p))).unwrap();
    let picked: Vec<String> = [2, 11, 30].iter().map(|&i| members[i].clone()).collect();
    let untouched: Vec<(String, Vec<u8>)> =
        members.iter().filter(|m| !picked.contains(m)).map(|m| (m.clone(), bytes(m))).collect();
    let settings_before: Vec<_> =
        picked.iter().map(|p| sidecar::read(Path::new(p), true, true).unwrap().settings).collect();

    let when = Date::since_reference_date(812_345_678.0);
    let outcome = trash::mark(&picked, Some(when), &|_| true);
    assert_eq!((outcome.done.clone(), outcome.refused.len(), outcome.failure.clone()), (picked.clone(), 0, None));
    assert_eq!(trash::report(&outcome, false), None);
    let after = listing(&copy.root);
    assert_eq!(
        after.keys().collect::<Vec<_>>(),
        before.keys().collect::<Vec<_>>(),
        "no file moved, appeared or vanished"
    );
    assert!(
        members.iter().all(|m| after[&m[copy.root.to_string_lossy().len() + 1..]] == 0),
        "every original kept its bytes"
    );
    for (path, settings) in picked.iter().zip(&settings_before) {
        let read = sidecar::read(Path::new(path), true, true).unwrap();
        assert_eq!(read.trashed_at, Some(when));
        assert_eq!(&read.settings, settings, "the settings ride through the mark unchanged");
    }
    assert!(untouched.iter().all(|(m, b)| bytes(m) == *b), "an unpicked sidecar is not rewritten");

    let index = LibraryIndex::open(Path::new(":memory:")).unwrap();
    index.rebuild(&indexed_all(&members)).unwrap();
    assert_eq!(index.shown().len(), FRAMES - 3);
    let binned: BTreeSet<String> = index::trashed(&index.all()).into_iter().map(|e| e.path).collect();
    assert_eq!(binned, picked.iter().cloned().collect());
    let listed = trash::folders(&index::trashed(&index.all()));
    assert_eq!((listed.len(), listed[0].count, listed[0].latest), (1, 3, when));

    // Twin: restored, the frames show again and the sidecars no longer carry the key.
    let restored = trash::mark(&picked, None, &|_| true);
    assert_eq!(restored.done, picked);
    index.rebuild(&indexed_all(&members)).unwrap();
    assert_eq!(index.shown().len(), FRAMES);
    assert!(picked.iter().all(|p| !String::from_utf8(bytes(p)).unwrap().contains("trashedAt")));

    // A sidecar from a newer version is refused, its bytes left as they were, and the gesture says so.
    let future = &members[5];
    let side = sidecar::url_for(Path::new(future));
    std::fs::write(&side, br#"{"version": 99, "settings": {}}"#).unwrap();
    let refused = trash::mark(std::slice::from_ref(future), Some(when), &|_| true);
    assert_eq!((refused.refused.clone(), refused.done.len()), (vec![future.clone()], 0));
    assert_eq!(std::fs::read(&side).unwrap(), br#"{"version": 99, "settings": {}}"#);
    assert!(trash::report(&refused, false).is_some());

    // Twin: the listing sees a move, so a trash that moved its file would fail above.
    let moved_to = copy.root.join("Trash");
    std::fs::create_dir(&moved_to).unwrap();
    std::fs::rename(&picked[0], moved_to.join("frame.NEF")).unwrap();
    assert_ne!(listing(&copy.root).keys().collect::<Vec<_>>(), before.keys().collect::<Vec<_>>());
}

/// `DateTimeOriginal` of each staged file, read by Pillow's TIFF directory parser.
const ORACLE: [(&str, &str); 10] = [
    ("DSC_6432.NEF", "2026:09:14 16:46:19"),
    ("DSC_6436.NEF", "2026:09:14 16:47:18"),
    ("DSC_6440.NEF", "2026:09:14 16:48:17"),
    ("DSC_6444.NEF", "2026:09:14 16:48:42"),
    ("DSC_6448.NEF", "2026:09:14 16:49:36"),
    ("DSC_6452.NEF", "2026:09:14 16:50:33"),
    ("DSC_6456.NEF", "2026:09:14 16:50:58"),
    ("DSC_6460.NEF", "2026:09:14 16:51:53"),
    ("DSC_6464.NEF", "2026:09:14 16:52:46"),
    ("DSC_6468.NEF", "2026:09:14 16:53:10"),
];

#[test]
fn the_real_nefs_and_rw2s_stamp_their_capture_and_fingerprint() {
    let Some(folder) = staged() else { return };
    let before = snapshot(&folder);
    let mut prints = HashSet::new();
    for (name, date) in ORACLE {
        let path = folder.join(name);
        assert_eq!(oni_library::exif::capture_date(&path).as_deref(), Some(date), "{name}");
        let stamp = relink::stamp(&path).unwrap();
        assert!(stamp.fingerprint.contains(date));
        prints.insert(stamp.fingerprint);
        let row = library::indexed(&path.to_string_lossy()).unwrap();
        assert_eq!(row.captured_at, relink::date(date));
    }
    assert_eq!(prints.len(), ORACLE.len(), "ten frames, ten fingerprints");
    // The digest computed apart over the same three blocks.
    assert_eq!(
        relink::fingerprint(&folder.join("DSC_6432.NEF")).as_deref(),
        Some("30005388-2026:09:14 16:46:19-58c5f1206c22f7cb")
    );
    // Twin: the empty placeholder standing for the same frame is another file.
    let Some(copy) = Copy::new("prints") else { return };
    assert_eq!(relink::fingerprint(&copy.roll.join("DSC_6432.NEF")).as_deref(), Some("0-no-date-0"));
    assert_eq!(snapshot(&folder), before);

    let Some(calibration) = oni_fixtures::lumix_dir("the Lumix capture dates") else { return };
    for (name, date, digest) in [
        ("P1072392.RW2", "2026:06:18 19:13:10", Some("ac7effbd917cad1b")),
        ("P1072404.RW2", "2026:06:18 19:15:20", None),
    ] {
        let path = calibration.join(name);
        if !path.exists() {
            eprintln!("{name} not found: not exercised");
            continue;
        }
        assert_eq!(oni_library::exif::capture_date(&path).as_deref(), Some(date), "{name}");
        if let Some(digest) = digest {
            let size = std::fs::metadata(&path).unwrap().len();
            assert_eq!(relink::fingerprint(&path), Some(format!("{size}-{date}-{digest}")));
        }
    }
}

#[test]
fn the_real_frames_index_by_their_own_capture_and_keep_their_print_once_moved() {
    let Some(copy) = Copy::made("real", true) else { return };
    let members = copy.members();
    assert_eq!(members.len(), FRAMES);
    let name = |p: &str| Path::new(p).file_name().unwrap().to_string_lossy().into_owned();
    let real: Vec<&String> = members.iter().filter(|m| ORACLE.iter().any(|(f, _)| *f == name(m))).collect();
    assert_eq!(real.len(), ORACLE.len());

    let rows = indexed_all(&members);
    let index = LibraryIndex::open(&copy.root.join("index.sqlite")).unwrap();
    index.rebuild(&rows).unwrap();
    assert_eq!(index.count(), FRAMES);
    assert!(library::unresolved(&members, &index).is_empty());

    // A copied file keeps the print its original carries: the identity a project and a relink read.
    let all = index.all();
    for (file, date) in ORACLE {
        let row = all.iter().find(|e| name(&e.path) == file).unwrap();
        assert_eq!(Some(row.fingerprint.clone()), relink::fingerprint(&copy.staged.join(file)), "{file}");
        assert_eq!(row.captured_at, relink::date(date), "{file}");
    }
    let prints: HashSet<&str> =
        all.iter().filter(|e| e.captured_at.is_some()).map(|e| e.fingerprint.as_str()).collect();
    assert_eq!(prints.len(), ORACLE.len(), "ten frames, ten prints");
    // Twin: the empty placeholders all collide on one print, which is what a sample digest prevents.
    let empty: HashSet<&str> = all.iter().filter(|e| e.captured_at.is_none()).map(|e| e.fingerprint.as_str()).collect();
    assert_eq!(empty, HashSet::from(["0-no-date-0"]));

    // The index orders on capture: the ten shot frames first, in their EXIF order.
    let shot: Vec<String> = all.iter().take(ORACLE.len()).map(|e| name(&e.path)).collect();
    assert_eq!(shot, ORACLE.map(|(file, _)| file.to_owned()));
    let groups = folders::groups(&copy.root_text(), &index.shown(), &folders::rolls(&members), &Default::default());
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].start, relink::date(ORACLE[0].1).unwrap());
    let laid: Vec<String> = groups[0].entries.iter().take(ORACLE.len()).map(|e| name(&e.path)).collect();
    assert_eq!(laid, shot);
    // Without the folder, the ten shot frames fall on their capture day and the rest on today's.
    let days = film_group::grouped(&all);
    assert_eq!(days.iter().map(|g| g.entries.len()).collect::<Vec<_>>(), [ORACLE.len(), FRAMES - ORACLE.len()]);
    assert_eq!(days[0].display_title(), "Monday, September 14, 2026");

    // A project from the other app names its photos by print: they resolve on the copies.
    let named: Vec<String> =
        ORACLE.iter().rev().map(|(f, _)| relink::fingerprint(&copy.staged.join(f)).unwrap()).collect();
    let trip = project::Project::new("Roll #8 picks", Date::since_reference_date(8e8), named);
    let resolved = project::grouped(std::slice::from_ref(&trip), &all);
    let picked: Vec<String> = resolved[0].entries.iter().map(|e| name(&e.path)).collect();
    assert_eq!(picked, ORACLE.iter().rev().map(|(f, _)| f.to_string()).collect::<Vec<_>>());
    // Twin: a print stamped off another size of the same frame resolves nothing.
    let stale = relink::fingerprint(&copy.staged.join(ORACLE[0].0)).unwrap().replacen("30005388", "30005389", 1);
    let lost = project::Project::new("Stale", Date::since_reference_date(8e8), vec![stale]);
    assert!(project::grouped(std::slice::from_ref(&lost), &all)[0].entries.is_empty());
}

/// `CaptureExposure.offsets` on the NEFs present: each frame's stops past the roll's common settings
/// are those Prepare the Roll wrote in its sidecar, the camera having been left on an automatic mode.
#[test]
fn roll8_capture_stops_read_off_the_exif_are_those_swift_wrote() {
    use oni_library::capture::CaptureExposure;
    let Some(folder) = staged() else { return };
    let mut names: Vec<String> = std::fs::read_dir(&folder)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".NEF"))
        .collect();
    names.sort();
    assert!(names.len() >= 2, "too few NEFs to read a roll's common settings");
    let captures: Vec<Option<CaptureExposure>> = names.iter().map(|n| CaptureExposure::read(&folder.join(n))).collect();
    assert!(captures.iter().all(Option::is_some), "every NEF carries its exposure");
    let wrote: Vec<f32> = names
        .iter()
        .map(|n| sidecar::read(&folder.join(n), true, false).unwrap().settings.capture_stops)
        .collect();
    let ours = CaptureExposure::offsets(&captures);
    for (((name, o), w), c) in names.iter().zip(&ours).zip(&wrote).zip(&captures) {
        println!("{name}: {} reads {o:+.6} for Swift's {w:+.6}", c.map(|c| c.label()).unwrap_or_default());
    }
    assert!(ours.iter().zip(&wrote).all(|(o, w)| (o - w).abs() < 1e-5), "{ours:?} for {wrote:?}");
    // Twin: read against the first frame instead of the roll's common settings, the stops part.
    let first = captures[0].unwrap().stops();
    let against_first: Vec<f32> = captures.iter().map(|c| c.unwrap().stops() - first).collect();
    assert!(against_first.iter().zip(&wrote).any(|(o, w)| (o - w).abs() > 0.1), "the check cannot tell the common from the first frame");
}
