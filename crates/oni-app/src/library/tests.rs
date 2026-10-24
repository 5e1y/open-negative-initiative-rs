use super::*;
use oni_settings::Date;

/// Roll #8's folder, when this machine has it.
pub(crate) fn roll8() -> Option<std::path::PathBuf> {
    let dir = oni_fixtures::folder(oni_fixtures::RAW_DIR);
    dir.join("DSC_6440.NEF").exists().then_some(dir)
}

fn write(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn a_folder_read_twice_reads_its_files_once_and_drops_a_photo_gone() {
    let root = std::env::temp_dir().join(format!("oni-app-library-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let folder = root.join("Scans");
    for (name, bytes) in [("A.NEF", &b"first"[..]), ("B.NEF", b"second"), ("Sub/C.NEF", b"third")] {
        write(&folder.join(name), bytes);
    }
    let at = root.join("Caches").join("index.sqlite");
    let index = LibraryIndex::open(&at).unwrap();
    let mut lands: Vec<(Vec<String>, bool)> = Vec::new();
    read(&folder, &index, None, &|| true, &mut |a, done, _| lands.push((a.entries().iter().map(|e| e.fingerprint.clone()).collect(), done)));
    let (first, _) = &lands[0];
    assert_eq!(first.len(), 3);
    assert!(first.iter().all(|f| f.starts_with(library::WAITING_MARK)), "unread photos land as waiting rows: {first:?}");
    let (last, done) = lands.last().unwrap();
    assert!(*done && last.iter().all(|f| !f.starts_with(library::WAITING_MARK)), "{last:?}");
    // A second session finds every row in the index: one landing, whole, no file read again.
    let index = LibraryIndex::open(&at).unwrap();
    let mut again = Vec::new();
    read(&folder, &index, None, &|| true, &mut |a, done, _| again.push((a.entries().len(), done)));
    assert_eq!(again, vec![(3, true)]);
    // Twin: a photo gone from the disk leaves the index on the next read covering its folder.
    std::fs::remove_file(folder.join("B.NEF")).unwrap();
    read(&folder, &index, None, &|| true, &mut |_, _, _| {});
    let held: Vec<String> = index.all().into_iter().map(|e| paths::name(&e.path)).collect();
    assert_eq!(held.len(), 2, "{held:?}");
    assert!(!held.contains(&"B.NEF".to_string()));
    // A photo marked in the ONI Trash leaves every listing once its row is read again.
    let a = paths::canonical(&folder.join("A.NEF"));
    oni_library::trash::mark(std::slice::from_ref(&a), Some(oni_settings::Date::now()), &|_: &Path| true);
    refresh(&index, std::slice::from_ref(&a));
    let shown: Vec<String> = shown_under(&index, &paths::canonical(&folder)).into_iter().map(|e| e.path).collect();
    assert!(!shown.contains(&a) && shown.len() == 1, "{shown:?}");
    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn a_folder_reached_through_a_link_lists_the_photos_on_its_real_path() {
    let root = std::env::temp_dir().join(format!("oni-app-linked-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let real = root.join("Real").join("Scans");
    for name in ["A.NEF", "B.NEF"] {
        write(&real.join(name), name.as_bytes());
    }
    // `/tmp` is a link to `/private/tmp` on macOS: the folder opened by its link must still group.
    let link = root.join("Link");
    std::os::unix::fs::symlink(root.join("Real"), &link).unwrap();
    let through = link.join("Scans");
    let archive = Archive::load(&through);
    assert_eq!(archive.entries().len(), 2);
    assert!(archive.entries().iter().all(|e| e.path.starts_with(&paths::canonical(&real))));
    // Twin: the same rows laid under the link as written fall outside every group.
    let survey = roots::survey(&[paths::canonical(&real)], &|| true);
    let rows: Vec<Entry> = archive.entries().into_iter().cloned().collect();
    let raw = Archive::from_rows(&through.to_string_lossy(), &rows, &survey, None);
    assert!(raw.entries().is_empty(), "the twin must lose the photos the link hides");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn roll_8_opens_as_one_named_roll_of_its_photos() {
    let Some(dir) = roll8() else {
        eprintln!("roll #8 is absent: the archive is not exercised");
        return;
    };
    // Ten frames in the shared fixture, the whole roll's 38 and its contact sheet where a tester holds it.
    let n = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).filter(|e| { let n = e.file_name().to_string_lossy().into_owned(); !n.starts_with('.') && !n.ends_with(".json") }).count();
    let a = Archive::load(&dir);
    assert_eq!(a.groups.len(), 1);
    assert_eq!((a.groups[0].title.as_str(), a.groups[0].entries.len()), ("Roll #8", n));
    assert_eq!(a.groups[0].stock, "Fujifilm Fujicolor");
    let rows = a.rows();
    assert_eq!(rows[0].title, "Roll #8");
    assert_eq!((rows[0].kind, rows.len()), (1, n + 1), "the roll, then its photos");
    // Twin: folding the roll leaves its row alone.
    let mut folded = a;
    let id = folded.rows()[0].id.clone();
    folded.toggle(&id);
    assert_eq!(folded.rows().len(), 1);
}

#[test]
fn a_rolls_row_shows_the_stock_picked_else_its_modes_generic_one() {
    use oni_library::film_group::FilmKey;
    use oni_settings::ConversionMode;
    let mut entry = Entry::new("/R/A.NEF", oni_library::distant_past(), "A");
    entry.mode = Some(ConversionMode::Positive);
    let mut film = oni_settings::film::Film::new("Roll 1");
    let mut group = FilmGroup { id: FilmKey::Roll(film.id.clone()), film: Some(film.clone()), start: oni_library::distant_past(), entries: vec![entry], heading: None };
    assert_eq!(default_stock(&group), "generic-color-positive");
    film.stock = Some("kodak-portra".into());
    group.film = Some(film);
    assert_eq!(default_stock(&group), "kodak-portra");
    assert!(crate::stocks::icon(default_stock(&group)).is_some(), "the picked stock has artwork");
    // Twin: an unknown key falls back on the mode's generic stock, never on a name nobody can draw.
    group.film.as_mut().unwrap().stock = Some("nobody-film".into());
    group.entries[0].mode = Some(ConversionMode::Monochrome);
    assert_eq!(default_stock(&group), "generic-black-and-white");
}

#[test]
fn a_row_whose_sidecar_cannot_be_seen_is_kept_as_it_was() {
    let root = std::env::temp_dir().join(format!("oni-app-refreshed-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let mut row = Entry::new(&root.join("T7").join("P1.RW2").to_string_lossy(), Date::since_reference_date(8e8), "f");
    row.label = Medal::Gold;
    row.trashed_at = Some(Date::since_reference_date(9e8));
    assert_eq!(refreshed(&row), row, "a disk away clears no mark, medal or roll");
    let side = oni_settings::sidecar::url_for(Path::new(&row.path));
    write(Path::new(&row.path), b"raw");
    write(&side, b"{ trunc");
    assert_eq!(refreshed(&row), row, "an unreadable sidecar leaves the cached row");
    // Twin: beside its present original, an absent sidecar is a photo back at rest.
    std::fs::remove_file(&side).unwrap();
    let rest = refreshed(&row);
    assert_eq!((rest.label, rest.trashed_at), (Medal::None, None));
    let _ = std::fs::remove_dir_all(&root);
}

/// Reads `folder` whole through `place`: the survey it landed with and the rows of its last landing.
fn read_whole(folder: &str, index: &LibraryIndex, place: &Place) -> (Arc<Survey>, Vec<String>) {
    let mut last = None;
    read_rows(folder, index, place, &|| true, &mut |rows, _, survey| last = Some((survey.clone(), rows.into_iter().map(|e| e.path).collect())));
    last.expect("a whole read lands")
}

#[cfg(unix)]
#[test]
fn read_rows_tells_gone_away_and_denied_apart() {
    use oni_library::presence::{Away, Presence};
    use std::os::unix::fs::PermissionsExt;
    let root = std::env::temp_dir().join(format!("oni-app-presence-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for (name, bytes) in [("Volumes/T7/Scans/A.NEF", &b"a"[..]), ("Volumes/T7/Scans/Locked/B.NEF", b"b"), ("Home/Roll/C.NEF", b"c")] {
        write(&root.join(name), bytes);
    }
    let p = |rel: &str| paths::canonical(&root.join(rel));
    let (scans, a, b, roll) = (p("Volumes/T7/Scans"), p("Volumes/T7/Scans/A.NEF"), p("Volumes/T7/Scans/Locked/B.NEF"), p("Home/Roll"));
    let computer = Computer { home: p("Home"), volumes: p("Volumes") };
    let place = Place { computer: &computer, kept: None };
    let index = LibraryIndex::open(Path::new(":memory:")).unwrap();
    assert_eq!(read_whole(&scans, &index, &place).1.len(), 2);

    // A folder the disk refuses keeps its rows, told denied.
    let locked = root.join("Volumes/T7/Scans/Locked");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    let (survey, rows) = read_whole(&scans, &index, &place);
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(rows.len(), 2, "a folder denied loses no row: {rows:?}");
    assert_eq!((survey.presence(&b), survey.presence(&a)), (Presence::Offline(Away::Denied), Presence::Present));

    // Its volume away: every row kept, told offline, never missing.
    std::fs::rename(root.join("Volumes/T7"), root.join("T7 away")).unwrap();
    let (survey, rows) = read_whole(&scans, &index, &place);
    std::fs::rename(root.join("T7 away"), root.join("Volumes/T7")).unwrap();
    assert_eq!(rows.len(), 2, "a disk away loses no row");
    assert_eq!((survey.presence(&scans), survey.presence(&a)), (Presence::Offline(Away::Disconnected), Presence::Offline(Away::Disconnected)));

    // Moved off a disk that answers: kept, told missing, for the hand to locate or forget.
    assert_eq!(read_whole(&roll, &index, &place).1.len(), 1);
    std::fs::rename(root.join("Home/Roll"), root.join("Home/Roll moved")).unwrap();
    let (survey, rows) = read_whole(&roll, &index, &place);
    assert_eq!((rows.len(), survey.presence(&roll)), (1, Presence::Missing));

    // Twin: in a folder that reads, a file removed loses its row.
    std::fs::remove_file(root.join("Volumes/T7/Scans/A.NEF")).unwrap();
    let (_, rows) = read_whole(&scans, &index, &place);
    assert_eq!(rows, [b.clone()]);
    assert!(index.all().iter().all(|e| e.path != a), "the removed file's row leaves the index");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_read_stopped_part_way_drops_no_row() {
    let root = std::env::temp_dir().join(format!("oni-app-stopped-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for name in ["Scans/A.NEF", "Scans/Sub/B.NEF"] {
        write(&root.join(name), name.as_bytes());
    }
    let scans = paths::canonical(&root.join("Scans"));
    let computer = Computer::none();
    let place = Place { computer: &computer, kept: None };
    let index = LibraryIndex::open(Path::new(":memory:")).unwrap();
    read_whole(&scans, &index, &place);
    std::fs::remove_file(root.join("Scans/Sub/B.NEF")).unwrap();
    read_rows(&scans, &index, &place, &|| false, &mut |_, _, _| {});
    assert_eq!(index.all().len(), 2, "a read stopped knows nothing of what it did not reach");
    // Twin: let run, the same read drops the row of the file removed.
    read_whole(&scans, &index, &place);
    assert_eq!(index.all().len(), 1);
    let _ = std::fs::remove_dir_all(&root);
}
