use super::*;
use std::io::Write;
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
                    assert!(std::process::Command::new("mkfifo").arg(&pipe_at).status().unwrap().success());
                }
            };
            read_rows(&folder, &index, &place, &|| alive.load(Ordering::SeqCst), &mut piped);
        });
        use std::os::unix::fs::FileTypeExt;
        while !std::fs::symlink_metadata(&side).is_ok_and(|m| m.file_type().is_fifo()) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        // Opening the pipe to write waits for the read to open it: the read is past any check before it.
        let mut pipe = std::fs::File::options().write(true).open(side).unwrap();
        meanwhile();
        pipe.write_all(b"{}").unwrap();
        drop(pipe);
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
