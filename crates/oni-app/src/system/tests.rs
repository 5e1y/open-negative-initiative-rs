//! The system calls a check can make: the double click, the Trash, Explorer's spelling, helpers reaped.

use super::*;

#[test]
fn the_double_click_is_the_systems_and_never_zero() {
    let d = double_click();
    assert!(d > Duration::ZERO && d <= Duration::from_secs(5), "{d:?}");
    // Twin: a system answering zero would make every second press a double click; it falls back.
    assert_eq!(Some(Duration::ZERO).filter(|d| !d.is_zero()).unwrap_or(DEFAULT_DOUBLE_CLICK), DEFAULT_DOUBLE_CLICK);
}

#[test]
fn a_missing_file_is_never_reported_trashed() {
    let asked = std::cell::Cell::new(0);
    let platform = |_: &Path| io::Result::Ok(asked.set(asked.get() + 1));
    let [gone, here] = ["absent", "here"].map(|n| std::env::temp_dir().join(format!("oni-trash-{}-{n}", std::process::id())));
    assert_eq!(guarded(&gone, platform).unwrap_err().kind(), io::ErrorKind::NotFound);
    assert_eq!(asked.get(), 0, "the platform is never asked about a missing file");
    // Twin: an existing file passes the guard, reaches the platform once, and its answer comes back.
    std::fs::write(&here, b"x").unwrap();
    assert!(guarded(&here, platform).is_ok() && asked.get() == 1);
    assert_eq!(guarded(&here, |_| Err(io::Error::other("refused"))).unwrap_err().to_string(), "refused");
    let _ = std::fs::remove_file(&here);
}

#[test]
fn reveal_hands_explorer_backslashes() {
    let photo = Path::new("D:/Scans/Roll 8/DSC_0001.NEF");
    assert_eq!(explorer_select(photo), r#"/select,"D:\Scans\Roll 8\DSC_0001.NEF""#);
    assert_eq!(explorer_path(Path::new("//nas/film/Roll 8")), r"\\nas\film\Roll 8");
    assert_eq!(explorer_path(Path::new(r"\\?\C:\Scans")), r"C:\Scans");
    assert_eq!(explorer_path(Path::new(r"\\?\UNC\nas\film")), r"\\nas\film");
    // Twin: the path as the archive spells it, which Explorer answered with Documents.
    assert!(format!("/select,\"{}\"", photo.display()).contains('/'));
}

#[test]
fn windows_keeps_rebuildable_files_off_the_roaming_profile() {
    let (roaming, local) = (Some("C:/Users/a/AppData/Roaming".into()), Some("C:/Users/a/AppData/Local".into()));
    let support = Path::new("C:/Users/a/AppData/Roaming").join(crate::timing::FOLDER);
    let chosen = local_of(&support, true, roaming.clone(), local.clone());
    assert_eq!(chosen, Path::new("C:/Users/a/AppData/Local").join(crate::timing::FOLDER));
    // Twin: a check's own support folder, and every other system, keep the support folder.
    let own = Path::new("C:/Temp/oni-check/Support");
    assert_eq!(local_of(own, true, roaming.clone(), local.clone()), own);
    assert_eq!(local_of(&support, false, roaming, local), support);
}

/// `AppFolders.selfCheck`: a check's folders are stand-ins named after its run, and the
/// platform's own folders are named in the two routes alone.
#[test]
fn a_checks_folders_are_its_own_and_the_platforms_have_one_route() {
    let options = crate::app::Options::default();
    let run = std::process::id().to_string();
    let temp = std::env::temp_dir();
    for folder in [&options.support, &options.caches] {
        assert!(folder.starts_with(&temp) && folder.to_string_lossy().contains(&run), "{}", folder.display());
    }
    // Twin: the folder the app itself hands is not a stand-in.
    if std::env::var_os("ONI_CACHES").is_none() {
        assert!(!caches().starts_with(&temp), "{}", caches().display());
    }
    let platform = ["Library/Caches", "Library/Application Support", "LOCALAPPDATA", "\"APPDATA", "XDG_CACHE_HOME", "XDG_DATA_HOME"];
    let names = |text: &str| platform.iter().any(|p| text.contains(p));
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() { walk(&path, out) } else if path.extension().is_some_and(|e| e == "rs") { out.push(path) }
        }
    }
    let mut files = Vec::new();
    walk(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    let offenders: Vec<String> = files
        .iter()
        .filter(|f| !["system.rs", "system/tests.rs", "timing.rs"].iter().any(|r| f.ends_with(r)))
        .filter(|f| names(&std::fs::read_to_string(f).unwrap()))
        .map(|f| f.display().to_string())
        .collect();
    assert!(files.len() > 50 && offenders.is_empty(), "{offenders:?}");
    // Twin: a hand-built caches path is refused.
    assert!(names("home().join(\"Library/Caches\")"));
}

/// Whether `pid` still has an entry in the process table, a zombie's included.
#[cfg(target_os = "macos")]
fn listed(pid: u32) -> bool {
    // SAFETY: signal 0 only asks whether the process exists.
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[cfg(target_os = "macos")]
#[test]
fn a_helper_program_leaves_no_zombie_once_it_ends() {
    let gone = |pid: u32| {
        let start = std::time::Instant::now();
        while listed(pid) && start.elapsed() < Duration::from_secs(3) {
            std::thread::sleep(Duration::from_millis(10));
        }
        !listed(pid)
    };
    let pid = launched(std::process::Command::new("/usr/bin/true")).unwrap();
    assert!(gone(pid), "the helper's entry is reaped");
    // Twin: the same program spawned and its handle dropped stays listed after its end.
    let pid = std::process::Command::new("/usr/bin/true").spawn().unwrap().id();
    std::thread::sleep(Duration::from_millis(300));
    assert!(listed(pid), "a dropped child stays a zombie");
    // SAFETY: reaps the twin's own child, which nothing else waits for.
    unsafe { libc::waitpid(pid as i32, std::ptr::null_mut(), 0) };
    assert!(gone(pid));
}
