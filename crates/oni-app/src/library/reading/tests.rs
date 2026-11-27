use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

/// A photo whose sidecar turns into a pipe once surveyed, so its read holds until the check writes to
/// it: the check acts while the read is inside its file read, never before nor after it.
#[cfg(unix)]
struct Held {
    root: PathBuf,
    folder: String,
    photo: String,
    index_at: PathBuf,
}

#[cfg(unix)]
fn held(label: &str) -> Held {
    let root = std::env::temp_dir().join(format!("oni-app-held-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("Scans/Roll")).unwrap();
    let scan = root.join("Scans/scan_01.tif");
    std::fs::write(&scan, b"pixels").unwrap();
    let (folder, photo) = (paths::canonical(&root.join("Scans")), paths::canonical(&scan));
    let index_at = root.join("Caches/index.sqlite");
    let index = LibraryIndex::open(&index_at).unwrap();
    let place = Place { computer: &Computer::none(), kept: None };
    read_rows(&folder, &index, &place, &|| true, &mut |_, _, _| {});
    assert!(index.row(&photo).is_some());
    // A sidecar now beside it: the next read looks at the row again.
    std::fs::write(oni_settings::sidecar::url_for(&scan), b"{}").unwrap();
    Held { root, folder, photo, index_at }
}

#[cfg(unix)]
impl Held {
    /// A read on its own thread, `alive` its token, blocked on the pipe until `meanwhile` has run.
    fn read(&self, alive: Arc<AtomicBool>, meanwhile: impl FnOnce()) {
        let (folder, at) = (self.folder.clone(), self.index_at.clone());
        let side = oni_settings::sidecar::url_for(Path::new(&self.photo));
        let pipe_at = side.clone();
        let reading = std::thread::spawn(move || {
            let index = LibraryIndex::open(&at).unwrap();
            let place = Place { computer: &Computer::none(), kept: None };
            let mut first = true;
            // Once surveyed, the sidecar becomes a pipe, which the read's look at it then waits on.
            let mut piped = |_: Vec<Entry>, _: bool, _: &Arc<Survey>| {
                if std::mem::take(&mut first) {
                    std::fs::remove_file(&pipe_at).unwrap();
                    oni_fixtures::fifo::make(&pipe_at);
                }
            };
            read_rows(&folder, &index, &place, &|| alive.load(Ordering::SeqCst), &mut piped);
        });
        use std::os::unix::fs::FileTypeExt;
        while !std::fs::symlink_metadata(&side).is_ok_and(|m| m.file_type().is_fifo()) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        oni_fixtures::fifo::give(&side, b"{}", meanwhile, || reading.is_finished());
        reading.join().unwrap();
    }
}

/// A move landing while a read waits on the photo's file, the read superseded first when `supersede`:
/// whether a row stays under the old path, and the copies the index then counts.
#[cfg(unix)]
fn moved_meanwhile(label: &str, supersede: bool) -> (bool, usize) {
    let h = held(label);
    let index = LibraryIndex::open(&h.index_at).unwrap();
    let moved = paths::join(&h.folder, "Roll/scan_01.tif");
    let alive = Arc::new(AtomicBool::new(true));
    let token = alive.clone();
    h.read(alive, || {
        // As `on_disk` then `followed` do: the token goes first, then the file and its row move.
        token.store(!supersede, Ordering::SeqCst);
        std::fs::rename(&h.photo, &moved).unwrap();
        let row = index.row(&h.photo).unwrap();
        index.remove(&h.photo).unwrap();
        index.upsert(&Entry { path: moved.clone(), ..row }).unwrap();
    });
    assert!(index.row(&moved).is_some());
    let left = (index.row(&h.photo).is_some(), index.copies().len());
    let _ = std::fs::remove_dir_all(&h.root);
    left
}

#[cfg(unix)]
#[test]
fn a_read_superseded_during_a_file_read_writes_no_row_under_a_moved_path() {
    assert_eq!(moved_meanwhile("superseded", true), (false, 0), "a superseded read writes back no row the move took away");
    // Twin: a read whose token holds writes its row back under the old path, a false copy.
    assert_eq!(moved_meanwhile("alive", false), (true, 1));
}

#[test]
fn a_reread_cut_where_the_writes_ran_lays_the_marked_photo_out() {
    let root = std::env::temp_dir().join(format!("oni-app-reread-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("Scans")).unwrap();
    for name in ["A.NEF", "B.NEF"] {
        std::fs::write(root.join("Scans").join(name), name).unwrap();
    }
    let folder = paths::canonical(&root.join("Scans"));
    let index = LibraryIndex::open(&root.join("Caches/index.sqlite")).unwrap();
    read_rows(&folder, &index, &Place { computer: &Computer::none(), kept: None }, &|| true, &mut |_, _, _| {});
    let a = paths::join(&folder, "A.NEF");
    oni_library::trash::mark(std::slice::from_ref(&a), Some(Date::now()), &|_: &Path| true);
    // Twin: the index not read again still shows the photo the gesture marked.
    assert!(shown_under(&index, &folder).iter().any(|e| e.path == a));
    // Cut on a connection of its own, as a gesture's thread does.
    let apart = LibraryIndex::open(&root.join("Caches/index.sqlite")).unwrap();
    let cut = reread(&apart, std::slice::from_ref(&a), Some(&folder));
    let shown: Vec<String> = cut.rows.iter().map(|e| paths::name(&e.path)).collect();
    assert_eq!(shown, ["B.NEF"]);
    assert_eq!(cut.folder.as_deref(), Some(folder.as_str()));
    assert_eq!(cut.copies, index.copies());
    assert!(index.row(&a).unwrap().trashed_at.is_some(), "the row written apart is the index's");
    // No folder open: the rows are read again all the same, and none is cut.
    let none = reread(&apart, std::slice::from_ref(&a), None);
    assert!(none.folder.is_none() && none.rows.is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

/// A folder made a roll outside the app, or its body changed there, reaches its rows at the next read of the
/// folder, so the Kit's counts follow the marker without reading it.
#[test]
fn a_marker_written_outside_the_app_reaches_its_rows_at_the_next_read() {
    let root = std::env::temp_dir().join(format!("oni-app-marker-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("Scans/Roll")).unwrap();
    std::fs::write(root.join("Scans/Roll/a.tif"), b"pixels").unwrap();
    let (folder, roll) = (paths::canonical(&root.join("Scans")), paths::canonical(&root.join("Scans/Roll")));
    let photo = format!("{roll}/a.tif");
    let index = LibraryIndex::open(&root.join("Caches/index.sqlite")).unwrap();
    let place = Place { computer: &Computer::none(), kept: None };
    let read = || read_rows(&folder, &index, &place, &|| true, &mut |_, _, _| {});
    read();
    assert_eq!(index.row(&photo).unwrap().marker_body, None);
    let mut film = oni_settings::Film::new("Roll");
    film.camera_body = Some("Nikon FM2".into());
    oni_settings::roll_folder::set_roll(Some(&film), Path::new(&roll)).unwrap();
    // Twin: until the folder is read again, the row holds what it held.
    assert_eq!(index.gear_counts().unwrap().body("Nikon FM2"), 0);
    read();
    assert_eq!(index.row(&photo).unwrap().marker_body.as_deref(), Some("Nikon FM2"));
    assert_eq!(index.gear_counts().unwrap().body("Nikon FM2"), 1);
    let _ = std::fs::remove_dir_all(&root);
}

/// A row read again off a sidecar the gate brings takes what the brought file says, its format included,
/// so it settles at once rather than at the next folder read.
#[test]
fn a_row_read_again_takes_the_line_its_sidecar_was_brought_to() {
    use oni_settings::migrate::{self, Facts};
    let root = std::env::temp_dir().join(format!("oni-app-gated-line-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let scan = root.join("DSC_6439.NEF");
    std::fs::write(&scan, b"pixels").unwrap();
    let camera = "NIKON CORPORATION NIKON Z f";
    let mut stored = oni_settings::Document::default();
    (stored.version, stored.exif_line.camera) = (migrate::chain::NAMED_VERSION, Some(camera.into()));
    let old = oni_settings::document::encode(&stored).unwrap();
    let side = oni_settings::sidecar::url_for(&scan);
    std::fs::write(&side, &old).unwrap();
    let photo = paths::canonical(&scan);
    let row = Entry { kept_camera: Some(camera.into()), ..Entry::new(&photo, Date::now(), "x") };
    // Twin: the row read off the sidecar as it stands, never brought, says its earlier format.
    let unbrought = refreshed_with(&row, false);
    assert_eq!((unbrought.kept_camera.as_deref(), unbrought.sidecar_format), (Some(camera), Some(migrate::chain::NAMED_VERSION)));
    let installed = crate::migration::facts::INSTALLED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let ours = PathBuf::from(&photo);
    migrate::install(Some(Arc::new(move |p: &Path, _: Option<bool>, then: &mut dyn FnMut(&Facts)| {
        if p == ours {
            let said = oni_settings::ExifMetadata { camera: "NIKON CORPORATION NIKON Z f".into(), ..Default::default() };
            then(&Facts { exif: &|| Some(said.clone()), ..Facts::bare(true) });
        }
    })));
    let read = refreshed_with(&row, true);
    migrate::install(None);
    drop(installed);
    assert_eq!((read.kept_camera, read.sidecar_format), (None, Some(oni_settings::document::VERSION)), "settled at once");
    assert_ne!(std::fs::read(&side).unwrap(), old, "the gate brought the file");
    let _ = std::fs::remove_dir_all(&root);
}

/// A photo whose row was read off an earlier format on a disk that refused the new file, its line read
/// with no facts to release it; its index and a twin index that records the row as of this format.
#[cfg(unix)]
fn stale_row(label: &str) -> (PathBuf, String, LibraryIndex, LibraryIndex) {
    use std::os::unix::fs::PermissionsExt;
    let root = std::env::temp_dir().join(format!("oni-app-stale-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let folder = root.join("Roll");
    std::fs::create_dir_all(&folder).unwrap();
    let scan = folder.join("DSC_6439.NEF");
    std::fs::write(&scan, b"pixels").unwrap();
    let mut stored = oni_settings::Document::default();
    (stored.version, stored.exif_line.camera) = (oni_settings::migrate::chain::NAMED_VERSION, Some(STALE_CAMERA.into()));
    std::fs::write(oni_settings::sidecar::url_for(&scan), oni_settings::document::encode(&stored).unwrap()).unwrap();
    let (index, twin) = (LibraryIndex::open(&root.join("index.sqlite")).unwrap(), LibraryIndex::open(&root.join("twin.sqlite")).unwrap());
    let place = Place { computer: &Computer::none(), kept: None };
    let at = paths::canonical(&folder);
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o555)).unwrap();
    for i in [&index, &twin] {
        read_rows(&at, i, &place, &|| true, &mut |_, _, _| {});
    }
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).unwrap();
    let photo = paths::canonical(&scan);
    let row = index.row(&photo).unwrap();
    assert_eq!((row.kept_camera.as_deref(), row.sidecar_format), (Some(STALE_CAMERA), Some(oni_settings::migrate::chain::NAMED_VERSION)));
    twin.upsert(&Entry { sidecar_format: Some(oni_settings::document::VERSION), ..row }).unwrap();
    (root, photo, index, twin)
}

const STALE_CAMERA: &str = "NIKON CORPORATION NIKON Z f";

/// `bring` run with the scan answering its own camera, as the app's facts do once the original reads.
#[cfg(unix)]
fn with_the_scan_s_facts(photo: &str, bring: impl FnOnce()) {
    use oni_settings::migrate::{self, Facts};
    let installed = crate::migration::facts::INSTALLED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let ours = PathBuf::from(photo);
    migrate::install(Some(Arc::new(move |p: &Path, _: Option<bool>, then: &mut dyn FnMut(&Facts)| {
        if p == ours {
            let said = oni_settings::ExifMetadata { camera: STALE_CAMERA.into(), ..Default::default() };
            then(&Facts { exif: &|| Some(said.clone()), ..Facts::bare(true) });
        }
    })));
    bring();
    migrate::install(None);
    drop(installed);
}

/// The camera each index's row names after a folder read.
#[cfg(unix)]
fn read_again(root: &Path, photo: &str, index: &LibraryIndex, twin: &LibraryIndex) -> (Option<String>, Option<String>) {
    let place = Place { computer: &Computer::none(), kept: None };
    let at = paths::canonical(&root.join("Roll"));
    for i in [index, twin] {
        read_rows(&at, i, &place, &|| true, &mut |_, _, _| {});
    }
    (index.row(photo).unwrap().kept_camera, twin.row(photo).unwrap().kept_camera)
}

/// A balance measured brings the sidecar keeping its date: the folder's next read takes the brought line
/// because the row says it was read off an earlier format.
#[cfg(unix)]
#[test]
fn a_row_read_off_an_earlier_format_is_read_again_once_a_measure_brings_it() {
    let (root, photo, index, twin) = stale_row("measure");
    let balance = oni_settings::document::AutoBalance { fingerprint: "f".into(), filters: [0.1, 0.0, -0.1], reference: None };
    with_the_scan_s_facts(&photo, || assert!(oni_settings::sidecar::set_auto_balance(Path::new(&photo), true, &balance).unwrap()));
    // Twin: a row recorded as of this format keeps the line it was read with, the sidecar's date unmoved.
    assert_eq!(read_again(&root, &photo, &index, &twin), (None, Some(STALE_CAMERA.into())));
    assert_eq!(index.row(&photo).unwrap().sidecar_format, Some(oni_settings::document::VERSION), "settled");
    let _ = std::fs::remove_dir_all(&root);
}

/// A read that took the earlier bytes while another path brought the file records the earlier format,
/// so the next folder read takes the brought line.
#[cfg(unix)]
#[test]
fn a_row_raced_by_another_bring_is_read_again() {
    let (root, photo, index, twin) = stale_row("race");
    with_the_scan_s_facts(&photo, || assert!(crate::migration::bring(&photo)));
    // Twin: the row recorded as of this format is never looked at again.
    assert_eq!(read_again(&root, &photo, &index, &twin), (None, Some(STALE_CAMERA.into())));
    let _ = std::fs::remove_dir_all(&root);
}
