use super::*;

/// A child holds a copy of every descriptor open at its spawn until its exec, a lock's included, so
/// a check judging a lock let go never runs while a sibling spawns.
static SPAWNS: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn no_spawn() -> std::sync::MutexGuard<'static, ()> {
    SPAWNS.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[test]
fn a_long_name_writes_through_a_short_temporary() {
    let dir = std::env::temp_dir().join(format!("oni-atomic-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // 240 characters with `.json`: within the limit, though its name and a temporary's suffix are not.
    let long = dir.join(format!("{}.NEF.json", "a".repeat(231)));
    assert_eq!(long.file_name().unwrap().len(), 240);
    write_atomically(&long, b"{}").unwrap();
    assert_eq!(std::fs::read(&long).unwrap(), b"{}");
    assert!(temporary(&long, 0).file_name().unwrap().len() < 60);
    // Twin: the former temporary's name, the target's own plus 37 bytes, is refused by the disk.
    let former = dir.join(format!(".{}.{}.1761400000000000000.tmp", long.file_name().unwrap().to_string_lossy(), std::process::id()));
    assert!(std::fs::File::create(&former).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A rename refused `refusals` times with `code`, as a file another program holds is on
/// Windows, then done; the pauses it waited counted in `paused`.
fn held_for(refusals: usize, code: i32) -> (impl Fn(&Path, &Path) -> io::Result<()>, std::rc::Rc<std::cell::Cell<usize>>) {
    let tries = std::rc::Rc::new(std::cell::Cell::new(0));
    let counted = tries.clone();
    let rename = move |from: &Path, to: &Path| {
        counted.set(counted.get() + 1);
        if counted.get() <= refusals { Err(io::Error::from_raw_os_error(code)) } else { std::fs::rename(from, to) }
    };
    (rename, tries)
}

#[test]
fn a_file_held_for_a_moment_on_windows_is_written_once_it_is_let_go() {
    let dir = std::env::temp_dir().join(format!("oni-atomic-held-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("A.NEF.json");
    std::fs::write(&target, b"old").unwrap();
    let paused = std::cell::Cell::new(Duration::ZERO);
    let pause = |d: Duration| paused.set(paused.get() + d);
    for code in [5, 32, 33] {
        let (rename, tries) = held_for(3, code);
        written(&target, b"new", &|f, t| retried(f, t, &rename, &pause, true)).unwrap();
        assert_eq!((std::fs::read(&target).unwrap(), tries.get()), (b"new".to_vec(), 4), "error {code}");
        std::fs::write(&target, b"old").unwrap();
    }
    assert_eq!(paused.get(), 3 * (PAUSES[0] + PAUSES[1] + PAUSES[2]));
    // Twin: held past every pause, or a refusal of another kind, fails and leaves no temporary.
    let (rename, tries) = held_for(PAUSES.len() + 1, 32);
    assert!(written(&target, b"new", &|f, t| retried(f, t, &rename, &pause, true)).is_err());
    assert_eq!(tries.get(), PAUSES.len() + 1);
    let (rename, tries) = held_for(1, 2);
    assert!(written(&target, b"new", &|f, t| retried(f, t, &rename, &pause, true)).is_err());
    assert_eq!(tries.get(), 1, "a missing folder is no hold");
    // Twin: the same violation off Windows is no hold either.
    let (rename, tries) = held_for(1, 32);
    assert!(written(&target, b"new", &|f, t| retried(f, t, &rename, &pause, false)).is_err());
    assert_eq!(tries.get(), 1);
    assert_eq!(std::fs::read(&target).unwrap(), b"old");
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "every temporary removed");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_writer_that_fails_leaves_the_old_file_and_no_temporary() {
    let dir = std::env::temp_dir().join(format!("oni-atomic-writer-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("patch.tiff");
    std::fs::write(&target, b"old").unwrap();
    for land in [write_with as fn(&Path, fn(&mut File) -> io::Result<()>) -> io::Result<()>, cached_with] {
        assert!(land(&target, |f| f.write_all(b"half").and(Err(io::Error::other("stopped")))).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"old");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "the temporary removed");
        // Twin: the same writer finishing lands its bytes whole.
        land(&target, |f| f.write_all(b"new")).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        std::fs::write(&target, b"old").unwrap();
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The names in `dir`, sorted.
fn listing(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

/// Set in a child of the check below: the target of a write it quits midway.
const QUITTING: &str = "ONI_ATOMIC_QUITTING";

#[test]
fn a_write_cut_short_by_a_quit_leaves_nothing_once_its_file_is_written_again() {
    if let Ok(target) = std::env::var(QUITTING) {
        // Half the bytes on the drive, then gone without unwinding, as a crash or a force-quit.
        let _ = write_with(Path::new(&target), |file| {
            file.write_all(b"half")?;
            std::process::exit(0)
        });
        return;
    }
    let dir = std::env::temp_dir().join(format!("oni-atomic-quit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (key, list) = (dir.join("host.key"), dir.join("trusted.txt"));
    let quit = |target: &Path| {
        let test = "atomic::tests::a_write_cut_short_by_a_quit_leaves_nothing_once_its_file_is_written_again";
        let spawning = no_spawn();
        let child = std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact", test]).env(QUITTING, target).output().unwrap();
        drop(spawning);
        assert!(child.status.success(), "{child:?}");
    };
    quit(&key);
    quit(&key);
    quit(&list);
    assert_eq!(listing(&dir).len(), 2, "one leftover a file, however many quits: {:?}", listing(&dir));
    write_private(&key, b"secret").unwrap();
    let left = listing(&dir);
    assert_eq!(left.len(), 2, "the key's leftover gone: {left:?}");
    // Twin: another file's leftover is not the key's, and stays until that file is written.
    assert!(left.contains(&"host.key".to_string()) && left.iter().any(|n| n.ends_with(".tmp")), "{left:?}");
    write_atomically(&list, b"").unwrap();
    assert_eq!(listing(&dir), ["host.key", "trusted.txt"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_write_in_flight_keeps_its_temporary_while_its_file_is_written_again() {
    let dir = std::env::temp_dir().join(format!("oni-atomic-flight-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("Portra.json");
    let first = staged(&target, b"first").unwrap();
    write_atomically(&target, b"second").unwrap();
    renamed(&first, &target).unwrap();
    drop(first);
    assert_eq!((std::fs::read(&target).unwrap(), listing(&dir)), (b"first".to_vec(), vec!["Portra.json".to_string()]));
    // Twin: a write let go before its rename, its rank free again, loses its temporary to the next write.
    let third = staged(&target, b"third").unwrap().to_path_buf();
    write_atomically(&target, b"first").unwrap();
    assert!(renamed(&third, &target).is_err());
    assert_eq!((std::fs::read(&target).unwrap(), listing(&dir)), (b"first".to_vec(), vec!["Portra.json".to_string()]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_folder_reclaimed_loses_every_leftover_and_keeps_the_writes_in_flight() {
    let _alone = no_spawn();
    let dir = std::env::temp_dir().join(format!("oni-atomic-reclaimed-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let flying = staged(&dir.join("P1.tif"), b"in flight").unwrap();
    let left = staged(&dir.join("P2.tif"), b"cut").unwrap().to_path_buf();
    for kept in ["P3.tif", ".oni-roll.json", ".oni-0123-0.tmp", ".oni-0123456789abcdef-.tmp"] {
        std::fs::write(dir.join(kept), b"").unwrap();
    }
    assert!(left.exists());
    reclaimed(&dir);
    let names = listing(&dir);
    assert!(!left.exists() && flying.exists(), "{names:?}");
    // Twin: nothing but a temporary's name goes, whatever it opens on.
    assert_eq!(names.len(), 5, "{names:?}");
    drop(flying);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_temporary_another_process_holds_is_never_reclaimed() {
    let _alone = no_spawn();
    let dir = std::env::temp_dir().join(format!("oni-atomic-other-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // Another ONI's sidecar save in flight: a temporary this process never staged, its lock held.
    let theirs = temporary(&dir.join("P1.NEF.json"), 0);
    let held = OpenOptions::new().write(true).create_new(true).open(&theirs).unwrap();
    held.try_lock().unwrap();
    reclaimed(&dir);
    assert!(theirs.exists(), "a write in flight elsewhere keeps its temporary");
    // A temporary staged here is locked as well, whatever this process knows of it.
    let ours = staged(&dir.join("P2.tif"), b"in flight").unwrap();
    assert!(File::open(&*ours).unwrap().try_lock().is_err());
    // Twin: let go, as by a writer that died, the same temporary is a leftover and goes.
    drop(held);
    reclaimed(&dir);
    assert!(!theirs.exists() && ours.exists(), "{:?}", listing(&dir));
    drop(ours);
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn a_private_file_is_readable_by_its_owner_alone() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("oni-atomic-private-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (key, plain) = (dir.join("host.key"), dir.join("plain.json"));
    write_private(&key, b"secret").unwrap();
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!((std::fs::read(&key).unwrap(), mode(&key)), (b"secret".to_vec(), 0o600));
    // Twin: a plain write takes the mask's mode as any new file does, open to others under the usual one.
    write_atomically(&plain, b"{}").unwrap();
    let reference = dir.join("reference");
    File::create(&reference).unwrap();
    assert_eq!(mode(&plain), mode(&reference));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A folder holding `target` at `old`, its date a day back, so a kept date is told from a new one.
fn dated(name: &str, old: &[u8]) -> (PathBuf, PathBuf, std::time::SystemTime) {
    let dir = std::env::temp_dir().join(format!("oni-atomic-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("A.NEF.json");
    std::fs::write(&target, old).unwrap();
    let day = std::time::SystemTime::now() - Duration::from_secs(86_400);
    File::options().write(true).open(&target).unwrap().set_modified(day).unwrap();
    let day = std::fs::metadata(&target).unwrap().modified().unwrap();
    (dir, target, day)
}

fn leftovers(dir: &Path) -> usize {
    std::fs::read_dir(dir).unwrap().flatten().filter(|e| temporary_named(&e.file_name().to_string_lossy())).count()
}

#[test]
fn a_verified_write_lands_once_read_back_and_a_refused_one_leaves_the_target_as_it_was() {
    let (dir, target, day) = dated("verified", b"old");
    let refused = write_verified(&target, b"new", &|_| false, &|| true, true);
    assert!(matches!(refused, Err(Unwritten::Unverified)), "{refused:?}");
    assert_eq!((std::fs::read(&target).unwrap(), leftovers(&dir)), (b"old".to_vec(), 0));
    assert_eq!(std::fs::metadata(&target).unwrap().modified().unwrap(), day);
    // A rename refused leaves it too.
    let failing = |_: &Path, _: &Path| Err(io::Error::other("refused"));
    assert!(matches!(verified_by(&target, b"new", &|_| true, &|| true, true, &failing), Err(Unwritten::Io(_))));
    assert_eq!((std::fs::read(&target).unwrap(), leftovers(&dir)), (b"old".to_vec(), 0));
    // Accepted, it lands with the date the target had.
    write_verified(&target, b"new", &|back| back == b"new", &|| true, true).unwrap();
    assert_eq!((std::fs::read(&target).unwrap(), leftovers(&dir)), (b"new".to_vec(), 0));
    assert_eq!(std::fs::metadata(&target).unwrap().modified().unwrap(), day);
    // Twin: a verify judging the bytes read back, wrongly expecting others, is heard and lands nothing.
    std::fs::write(&target, b"old").unwrap();
    let seen = std::cell::RefCell::new(Vec::new());
    let wrong = |back: &[u8]| {
        seen.replace(back.to_vec());
        back == b"other"
    };
    assert!(matches!(write_verified(&target, b"new", &wrong, &|| true, true), Err(Unwritten::Unverified)));
    assert_eq!((seen.take(), std::fs::read(&target).unwrap(), leftovers(&dir)), (b"new".to_vec(), b"old".to_vec(), 0));
    let _ = std::fs::remove_dir_all(&dir);
}

/// As Windows' lock does to every other handle, the temporary sealed against a new one by its mode.
#[cfg(unix)]
#[test]
fn a_verified_write_reads_back_through_the_handle_it_holds() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, target, _) = dated("sealed", b"old");
    let opened = std::cell::Cell::new(None);
    let sealed = |file: &mut File| {
        file.write_all(b"new")?;
        file.set_permissions(std::fs::Permissions::from_mode(0o200))?;
        opened.set(Some(std::fs::read(temporary(&target, 0)).is_ok()));
        Ok(())
    };
    staged_and_verified(&target, b"new", &|back| back == b"new", &|| true, false, &renamed, sealed).unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!((std::fs::read(&target).unwrap(), leftovers(&dir)), (b"new".to_vec(), 0));
    // Twin: the seal holds, so a read through a new handle, as the path gives, is refused.
    assert_eq!(opened.get(), Some(false));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_target_another_writer_moves_meanwhile_is_kept_whole() {
    let (dir, target, _) = dated("moved", b"old");
    // The other writer lands while the bytes are staged, between the read and the rename.
    let other = |_: &[u8]| std::fs::write(&target, b"theirs").is_ok();
    let unmoved = || std::fs::read(&target).is_ok_and(|now| now == b"old");
    let refused = write_verified(&target, b"mine", &other, &unmoved, true);
    assert!(matches!(refused, Err(Unwritten::Moved)), "{refused:?}");
    assert_eq!((std::fs::read(&target).unwrap(), leftovers(&dir)), (b"theirs".to_vec(), 0));
    // Twin: with `unmoved` always true, the other writer's bytes are lost.
    std::fs::write(&target, b"old").unwrap();
    write_verified(&target, b"mine", &other, &|| true, true).unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"mine");
    let _ = std::fs::remove_dir_all(&dir);
}
