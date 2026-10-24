//! A mark that does not land names its cause: a newer sidecar, a missing original, a disk that
//! refuses the write, each counted, never one hidden behind another.

use oni_library::trash;
use oni_settings::Date;
use oni_settings::sidecar::url_for;
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("oni-trash-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[cfg(unix)]
fn mode(path: &Path, bits: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(bits)).unwrap();
}

#[cfg(unix)]
#[test]
fn a_mark_that_fails_on_one_sidecar_names_it_beside_a_refused_one() {
    let dir = scratch("refused");
    let newer = dir.join("A.NEF");
    std::fs::write(&newer, b"").unwrap();
    std::fs::write(url_for(&newer), br#"{"version": 99, "settings": {}}"#).unwrap();
    let locked = dir.join("Card");
    std::fs::create_dir(&locked).unwrap();
    let blocked = locked.join("B.NEF");
    std::fs::write(&blocked, b"").unwrap();
    let gone = dir.join("C.NEF");
    mode(&locked, 0o555);
    // Root writes through any mode: the check has nothing to see there.
    if std::fs::write(locked.join("probe"), b"").is_ok() {
        mode(&locked, 0o755);
        return;
    }
    let files: Vec<String> = [&newer, &blocked, &gone].iter().map(|p| p.to_string_lossy().into_owned()).collect();
    let outcome = trash::mark(&files, Some(Date::from_unix_seconds(1.0)), &|_| true);
    assert_eq!((outcome.refused.len(), outcome.unreachable.len(), outcome.failed.len()), (1, 1, 1));
    let said = trash::report(&outcome, false).unwrap();
    assert!(said.contains("a newer version of ONI"), "{said}");
    assert!(said.contains("cannot be found"), "{said}");
    assert!(said.contains("B.NEF.json") && said.contains("ermission denied"), "names the file and its cause: {said}");
    assert!(!url_for(&gone).exists(), "no sidecar beside a missing original");
    // Twin: the card writable again, the same mark lands and says nothing.
    mode(&locked, 0o755);
    let again = trash::mark(&files[1..2], Some(Date::from_unix_seconds(1.0)), &|_| true);
    assert_eq!((again.done.len(), trash::report(&again, false)), (1, None));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn many_failures_name_a_few_and_count_the_rest() {
    let failed: Vec<(String, String)> = (0..5).map(|i| (format!("/x/P{i}.NEF"), "Read-only file system".to_owned())).collect();
    let outcome = trash::Outcome { failed, ..trash::Outcome::default() };
    let said = trash::report(&outcome, true).unwrap();
    assert!(said.starts_with("5 photos were not restored") && said.contains("P2.NEF.json"), "{said}");
    assert!(!said.contains("P3.NEF.json") && said.contains("and 2 others"), "{said}");
}
