//! A roll folder on a disk's own terms: exFAT's AppleDouble twins and a file another program holds.

use super::*;

/// A disk image formatted exFAT like the T7, attached in the scratch folder, detached on drop.
#[cfg(target_os = "macos")]
struct Exfat(std::path::PathBuf);

#[cfg(target_os = "macos")]
impl Exfat {
    fn attach(scratch: &Scratch) -> Option<Exfat> {
        use std::process::Command;
        let (image, mount) = (scratch.path().join("t7.dmg"), scratch.path().join("T7"));
        let run = |c: &mut Command| c.status().is_ok_and(|s| s.success());
        let made = run(Command::new("hdiutil").args(["create", "-quiet", "-size", "8m", "-fs", "ExFAT", "-volname", "T7", "-layout", "NONE"]).arg(&image));
        std::fs::create_dir_all(&mount).ok()?;
        let attached = made && run(Command::new("hdiutil").args(["attach", "-quiet", "-nobrowse", "-noverify", "-mountpoint"]).arg(&mount).arg(&image));
        attached.then_some(Exfat(mount))
    }
}

#[cfg(target_os = "macos")]
impl Drop for Exfat {
    fn drop(&mut self) {
        let _ = std::process::Command::new("hdiutil").args(["detach", "-quiet", "-force"]).arg(&self.0).status();
    }
}

#[cfg(target_os = "macos")]
fn tag(path: &str) {
    let (name, value) = (c"com.oni.check", b"oni");
    let path = std::ffi::CString::new(path).unwrap();
    // SAFETY: NUL-terminated strings and a buffer of the length given, all outliving the call.
    unsafe { libc::setxattr(path.as_ptr(), name.as_ptr(), value.as_ptr().cast(), value.len(), 0, 0) };
}

/// On exFAT macOS lays a `._` twin beside each file it tags, the roll's marker included: the undo
/// still finds the folder empty of anything of the user's and removes it with its twins.
#[cfg(target_os = "macos")]
#[test]
fn undo_on_exfat_removes_the_folder_and_its_appledouble() {
    let scratch = Scratch::new("roll-folder-exfat");
    let Some(disk) = Exfat::attach(&scratch) else {
        eprintln!("hdiutil refused an exFAT image: the undo on exFAT is not exercised");
        return;
    };
    let parent = crate::paths::canonical(&disk.0);
    let photo = paths::join(&parent, "a.tif");
    touch(&photo, b"a");
    touch(&side(&photo), b"{}");
    let planned = plan(std::slice::from_ref(&photo), crate::relink::date("2026:01:14 12:00:00").unwrap(), &parent);
    let made = carry_out(&planned, &rename_new).unwrap();
    let folder = made.disk.folder.clone().unwrap();
    // Each file tagged as macOS tags what an app writes, which lays the `._` twins seen on the T7.
    for f in [folders::marker(&folder).to_string_lossy().into_owned(), made.disk.moves[0].to.clone()] {
        tag(&f);
    }
    let back = take_back(&made.disk, &rename_new);
    assert_eq!(back.made.len(), 1);
    assert!(exists(&photo) && exists(&side(&photo)), "the photo and its sidecar are home");
    assert!(!exists(&folder), "the folder went with its marker and their twins");
    // Twin: the same listing read literally, the marker alone, keeps the folder.
    let laid = lay(&made.disk, &rename_new);
    assert_eq!(laid.made.len(), 1);
    let left = carry(&[Unit { moves: made.disk.moves.iter().map(|m| Move { from: m.to.clone(), to: m.from.clone() }).collect() }], &rename_new);
    assert_eq!(left.moves().len(), 1);
    let names = listed(&folder);
    assert!(names.iter().any(|n| n.starts_with("._")), "exFAT lays a `._` twin: {names:?}");
    assert!(!names.iter().all(|n| n == folders::ROLL_MARKER), "the literal check finds the folder not empty");
    assert!(names.iter().all(|n| n == folders::ROLL_MARKER || leftover(n)));
    assert!(remove_if_left(&folder, made.disk.film.as_ref()) && !exists(&folder));
}

#[test]
fn a_locked_file_is_retried_then_named() {
    use crate::names::Platform::{Mac, Windows};
    use std::sync::Mutex;
    let sharing = || io::Error::from_raw_os_error(32);
    // A hold lifted on the third try: two pauses, then the move.
    let tries = Mutex::new(0);
    let waited = Mutex::new(Vec::new());
    let lifts = |_: &Path, _: &Path| {
        let mut n = tries.lock().unwrap();
        *n += 1;
        if *n < 3 { Err(sharing()) } else { Ok(()) }
    };
    let pause = |d: std::time::Duration| waited.lock().unwrap().push(d);
    assert!(retried(Path::new("/a"), Path::new("/b"), &lifts, &pause, Windows).is_ok());
    assert_eq!((*tries.lock().unwrap(), waited.lock().unwrap().clone()), (3, PAUSES[..2].to_vec()));

    // A hold never lifted: every pause spent, then the frame stays and its file is named.
    let scratch = Scratch::new("roll-held");
    let parent = root(&scratch);
    let photo = paths::join(&parent, "DSC_0001.NEF");
    touch(&photo, b"raw");
    let units = plan_units(std::slice::from_ref(&photo), &paths::join(&parent, "Roll"));
    std::fs::create_dir(paths::join(&parent, "Roll")).unwrap();
    let held = |f: &Path, t: &Path| retried(f, t, &|_: &Path, _: &Path| Err(io::Error::from(io::ErrorKind::ResourceBusy)), &|_| {}, Windows);
    let carried = carry(&units, &held);
    let said = oni_i18n::spoken(&said(&carried).unwrap_or_default());
    assert!(carried.made.is_empty() && exists(&photo), "nothing moved");
    assert_eq!(said, "“DSC_0001.NEF” stayed where it was: DSC_0001.NEF: another app is using it.");

    // Twin: a refusal that is no hold is said at once, and a Mac reads no Windows code as one.
    for (error, platform) in [(io::Error::from(io::ErrorKind::NotFound), Windows), (sharing(), Mac), (io::Error::from_raw_os_error(5), Mac)] {
        let calls = Mutex::new(0);
        let refuses = |_: &Path, _: &Path| {
            *calls.lock().unwrap() += 1;
            Err(io::Error::from_raw_os_error(error.raw_os_error().unwrap_or(2)))
        };
        let _ = retried(Path::new("/a"), Path::new("/b"), &refuses, &|_| panic!("no pause"), platform);
        assert_eq!(*calls.lock().unwrap(), 1, "{error:?} on {platform:?}");
    }
    // Twin: Windows' denial on a folder with a file open inside is tried again.
    let denials = Mutex::new(0);
    let denies = |_: &Path, _: &Path| {
        *denials.lock().unwrap() += 1;
        Err(io::Error::from_raw_os_error(5))
    };
    assert!(retried(Path::new("/a"), Path::new("/b"), &denies, &|_| {}, Windows).is_err());
    assert_eq!(*denials.lock().unwrap(), PAUSES.len() + 1);
}
