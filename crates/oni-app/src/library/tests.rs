use super::*;

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
