use super::*;
use oni_library::navigator::Computer;
use oni_library::{library, roots};
use oni_settings::Date;
use std::sync::Arc;

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
    let raw = Archive::from_rows(&through.to_string_lossy(), &rows, &Arc::new(survey), None);
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

/// Sets a file's modification date, as a scanner rewriting a frame or a copy keeping its date does.
fn dated(path: &Path, at: std::time::SystemTime) {
    std::fs::File::options().write(true).open(path).unwrap().set_modified(at).unwrap();
}

#[test]
fn a_photo_replaced_by_another_size_or_mtime_is_read_again() {
    let root = std::env::temp_dir().join(format!("oni-app-replaced-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let scan = root.join("Scans/scan_01.tif");
    let (day, later) = (std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000), std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_600));
    write(&scan, &[1u8; 5]);
    dated(&scan, day);
    let folder = paths::canonical(&root.join("Scans"));
    let path = paths::canonical(&scan);
    let (computer, index) = (Computer::none(), LibraryIndex::open(Path::new(":memory:")).unwrap());
    let place = Place { computer: &computer, kept: None };
    let print = |index: &LibraryIndex| index.row(&path).unwrap().fingerprint;
    read_whole(&folder, &index, &place);
    let first = print(&index);
    // Rescanned at another size: the row is read again.
    write(&scan, &[2u8; 9]);
    dated(&scan, day);
    read_whole(&folder, &index, &place);
    let second = print(&index);
    assert!(second != first && second.starts_with("9-"), "{second}");
    // Rewritten at the same size, its date moved: stamped again, a new print.
    write(&scan, &[3u8; 9]);
    dated(&scan, later);
    read_whole(&folder, &index, &place);
    let third = print(&index);
    assert_ne!(third, second, "a file of the same size with a new date is read again");
    // Touched with the same bytes: the cheap look again keeps the print.
    dated(&scan, day);
    read_whole(&folder, &index, &place);
    assert_eq!(print(&index), third);
    // Twin: rewritten with its date put back as it was, nothing tells the read, and the print stays stale.
    write(&scan, &[4u8; 9]);
    dated(&scan, day);
    read_whole(&folder, &index, &place);
    assert_eq!(print(&index), third, "the date and the size are all a read looks at");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_sidecar_edited_by_another_app_is_read_again() {
    let root = std::env::temp_dir().join(format!("oni-app-sidecar-moved-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let scan = root.join("Scans/scan_01.tif");
    write(&scan, b"pixels");
    let (folder, path) = (paths::canonical(&root.join("Scans")), paths::canonical(&scan));
    let (computer, index) = (Computer::none(), LibraryIndex::open(Path::new(":memory:")).unwrap());
    let place = Place { computer: &computer, kept: None };
    read_whole(&folder, &index, &place);
    let side = oni_settings::sidecar::url_for(&scan);
    oni_settings::sidecar::set_label(Medal::Gold, &scan, false).unwrap();
    read_whole(&folder, &index, &place);
    assert_eq!(index.row(&path).unwrap().label, Medal::Gold, "the medal the other app gave shows");
    std::fs::remove_file(&side).unwrap();
    read_whole(&folder, &index, &place);
    assert_eq!(index.row(&path).unwrap().label, Medal::None, "a sidecar deleted by hand puts the photo back at rest");
    // Twin: a sidecar rewritten with its date put back is not read again.
    oni_settings::sidecar::set_label(Medal::Silver, &scan, false).unwrap();
    read_whole(&folder, &index, &place);
    let silver = std::fs::metadata(&side).unwrap().modified().unwrap();
    oni_settings::sidecar::set_label(Medal::Bronze, &scan, false).unwrap();
    dated(&side, silver);
    read_whole(&folder, &index, &place);
    assert_eq!(index.row(&path).unwrap().label, Medal::Silver);
    let _ = std::fs::remove_dir_all(&root);
}

/// A RAW+JPEG pair read off the disk lays one tile, the RAW's, its JPEG named behind it.
#[test]
fn a_raw_jpeg_pair_is_one_tile_in_the_archive() {
    let root = std::env::temp_dir().join(format!("oni-app-pairs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let folder = root.join("Roll");
    for (name, bytes) in [("DSC_1.NEF", &b"raw one"[..]), ("DSC_1.JPG", b"jpeg one"), ("DSC_2.JPG", b"jpeg two")] {
        write(&folder.join(name), bytes);
    }
    let names = |a: &Archive| a.entries().iter().map(|e| paths::name(&e.path)).collect::<Vec<_>>();
    let archive = Archive::load(&folder);
    let mut shown = names(&archive);
    shown.sort();
    assert_eq!(shown, ["DSC_1.NEF", "DSC_2.JPG"]);
    let raw = paths::join(&paths::canonical(&folder), "DSC_1.NEF");
    assert_eq!(archive.companions.get(&raw).map(|j| paths::name(j)), Some("DSC_1.JPG".to_owned()));
    // Twin: the JPEG edited before, its own sidecar beside it, stays a tile of its own.
    write(&folder.join("DSC_1.JPG.json"), b"{}");
    assert_eq!(names(&Archive::load(&folder)).len(), 3);
    let _ = std::fs::remove_dir_all(&root);
}

/// Two rolls of three photos and an empty folder under `/R/Scans`, as a survey would list them.
fn synthetic() -> (Vec<Entry>, Arc<Survey>) {
    let rows: Vec<Entry> = ["Roll A/a1.NEF", "Roll A/a2.NEF", "Roll A/a3.NEF", "Roll B/b1.NEF", "Roll B/b2.NEF", "Roll B/b3.NEF"]
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut e = Entry::new(&format!("/R/Scans/{p}"), Date::since_reference_date(8e8), &format!("print-{i}"));
            e.captured_at = Some(Date::since_reference_date(8e8 + i as f64 * 60.0));
            e
        })
        .collect();
    let survey = Survey {
        images: rows.iter().map(|e| e.path.clone()).collect(),
        folders: ["Roll A", "Roll B", "Empty"].iter().map(|f| format!("/R/Scans/{f}")).collect(),
        ..Survey::default()
    };
    (rows, Arc::new(survey))
}

#[test]
fn a_lay_on_the_same_rows_keeps_the_tree_and_a_medal_moves_its_row_alone() {
    let (rows, survey) = synthetic();
    let folder = "/R/Scans";
    let cut = Archive::from_rows(folder, &rows, &survey, None);
    let laid = cut.rows();
    // The same rows under a survey read again, its sidecars' dates moved: the tree held stands.
    let mut again = (*survey).clone();
    again.sidecars.insert(rows[0].path.clone(), Date::since_reference_date(9e8));
    let (kept, how) = Archive::relaid(Some(cut), None, folder, &rows, &Arc::new(again), None);
    assert_eq!((how, kept.rows()), (Relaid::Kept, laid.clone()));
    // A medal moves its photo's caption and its gallery entry, nothing else.
    let mut medalled = rows.clone();
    medalled[4].label = Medal::Gold;
    let (archive, how) = Archive::relaid(Some(kept), None, folder, &medalled, &survey, None);
    assert_eq!(how, Relaid::Medals);
    let changed: Vec<usize> = archive.rows().iter().zip(&laid).enumerate().filter(|(_, (a, b))| a != b).map(|(i, _)| i).collect();
    let b2 = archive.rows().iter().position(|r| r.id == NodeId::Photo(rows[4].path.clone())).unwrap();
    assert_eq!((changed, archive.rows()[b2].caption.clone()), (vec![b2], medal(Medal::Gold)));
    assert_eq!(archive.entries().iter().find(|e| e.path == rows[4].path).map(|e| e.label), Some(Medal::Gold));
    // Under a filter the medal cuts the gallery again, the tree kept.
    let (filtered, how) = Archive::relaid(Some(archive), None, folder, &medalled, &survey, Some(Medal::Gold));
    assert_eq!((how, filtered.entries().len(), filtered.rows().len()), (Relaid::Regrouped, 1, laid.len()));
    // Twin: a row that moved otherwise, its print here, cuts the tree whole.
    let mut moved = medalled.clone();
    moved[1].fingerprint = "print-new".into();
    assert_eq!(Archive::relaid(Some(filtered), None, folder, &moved, &survey, None).1, Relaid::Cut);
    // Twin: a folder new on the disk cuts it too.
    let mut grown = (*survey).clone();
    grown.folders.push("/R/Scans/Roll C".into());
    let held = Archive::from_rows(folder, &rows, &survey, None);
    assert_eq!(Archive::relaid(Some(held), None, folder, &rows, &Arc::new(grown), None).1, Relaid::Cut);
}

#[test]
fn a_tree_cut_off_the_thread_is_taken_only_for_the_rows_it_was_cut_from() {
    let (rows, survey) = synthetic();
    let folder = "/R/Scans";
    // A staged archive marked by its folds, so taking it shows in its rows.
    let staged = |rows: &[Entry]| {
        let mut a = Archive::from_rows(folder, rows, &survey, None);
        a.set_expanded(HashSet::new());
        a
    };
    let (taken, how) = Archive::relaid(None, Some(staged(&rows)), folder, &rows, &survey, None);
    assert_eq!((how, taken.rows().len()), (Relaid::Cut, 1), "the staged archive is taken as cut");
    // Twin: one cut from other rows is refused, the lay cutting its own from the rows held.
    let fewer = &rows[..5];
    let (cut, _) = Archive::relaid(None, Some(staged(fewer)), folder, &rows, &survey, None);
    assert_eq!((cut.rows().len(), cut.entries().len()), (10, 6), "every folder unfolded over the six photos held");
}
