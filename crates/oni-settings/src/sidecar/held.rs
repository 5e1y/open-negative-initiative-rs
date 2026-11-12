//! One writer at a time per sidecar across the process, from a helper's read to its rename; the
//! thread holding one may hold it again, as a write migrates the file it reads first.

use super::url_for;
use std::path::{Path, PathBuf};
use std::thread::ThreadId;

/// The sidecars a write holds, each by one thread, as many times as it took it: a field set on one
/// thread is never lost under a whole rewrite another read before it.
static HELD: std::sync::Mutex<Vec<(PathBuf, ThreadId, usize)>> = std::sync::Mutex::new(Vec::new());
static FREED: std::sync::Condvar = std::sync::Condvar::new();

/// A sidecar held, let go when dropped.
pub(crate) struct Held(PathBuf);

/// The sidecar by its folder's resolved path: two spellings of one folder, `/tmp` and `/private/tmp`,
/// are one file, so one writer.
fn key(original: &Path) -> PathBuf {
    let sidecar = url_for(original);
    match (sidecar.parent().and_then(|p| std::fs::canonicalize(p).ok()), sidecar.file_name()) {
        (Some(folder), Some(name)) => folder.join(name),
        _ => sidecar,
    }
}

pub(crate) fn hold(original: &Path) -> Held {
    let path = key(original);
    let me = std::thread::current().id();
    let mut held = HELD.lock().unwrap_or_else(|e| e.into_inner());
    loop {
        match held.iter_mut().find(|(p, _, _)| *p == path) {
            None => break,
            Some((_, owner, count)) if *owner == me => {
                *count += 1;
                return Held(path);
            }
            Some(_) => held = FREED.wait(held).unwrap_or_else(|e| e.into_inner()),
        }
    }
    held.push((path.clone(), me, 1));
    Held(path)
}

impl Drop for Held {
    fn drop(&mut self) {
        let mut held = HELD.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(at) = held.iter().position(|(p, _, _)| *p == self.0) {
            held[at].2 -= 1;
            if held[at].2 == 0 {
                held.remove(at);
            }
        }
        drop(held);
        FREED.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sidecar::{read, set_label};
    use crate::modes::Medal;
    use std::time::Duration;
    use std::sync::mpsc;

    #[test]
    fn a_field_set_waits_for_the_write_holding_its_sidecar() {
        let dir = std::env::temp_dir().join(format!("oni-sidecar-held-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (photo, other) = (dir.join("A.tif"), dir.join("B.tif"));
        std::fs::write(&photo, b"a").unwrap();
        std::fs::write(&other, b"b").unwrap();
        let held = hold(&photo);
        let (tx, rx) = mpsc::channel();
        let path = photo.clone();
        std::thread::spawn(move || tx.send(set_label(Medal::Gold, &path, false).is_ok()).unwrap());
        assert!(rx.recv_timeout(Duration::from_millis(300)).is_err(), "no read of the sidecar while a write holds it");
        // Twin: another photo's sidecar is free.
        set_label(Medal::Silver, &other, false).unwrap();
        drop(held);
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)), Ok(true), "once let go, the label is set");
        assert_eq!(read(&photo, false).unwrap().label, Medal::Gold);
        // The thread holding a sidecar takes it again without waiting on itself.
        let again = (hold(&other), hold(&other));
        drop(again);
        set_label(Medal::Gold, &other, false).unwrap();
        // Another spelling of the same folder holds the same sidecar.
        let long = Path::new("/private").join(photo.strip_prefix("/").unwrap());
        if long.exists() {
            let held = hold(&long);
            let (tx, rx) = mpsc::channel();
            let path = photo.clone();
            std::thread::spawn(move || tx.send(set_label(Medal::Bronze, &path, false).is_ok()).unwrap());
            assert!(rx.recv_timeout(Duration::from_millis(300)).is_err(), "the other spelling waits");
            drop(held);
            assert_eq!(rx.recv_timeout(Duration::from_secs(5)), Ok(true));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
