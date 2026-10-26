//! One writer at a time per sidecar across the process, from a helper's read to its rename.

use super::url_for;
use std::path::{Path, PathBuf};

/// The sidecars a write holds, one writer at a time each across the process: a field set on one
/// thread is never lost under a whole rewrite another read before it.
static HELD: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());
static FREED: std::sync::Condvar = std::sync::Condvar::new();

/// A sidecar held, let go when dropped.
pub(super) struct Held(PathBuf);

pub(super) fn hold(original: &Path) -> Held {
    let path = url_for(original);
    let mut held = HELD.lock().unwrap_or_else(|e| e.into_inner());
    while held.contains(&path) {
        held = FREED.wait(held).unwrap_or_else(|e| e.into_inner());
    }
    held.push(path.clone());
    Held(path)
}

impl Drop for Held {
    fn drop(&mut self) {
        HELD.lock().unwrap_or_else(|e| e.into_inner()).retain(|p| *p != self.0);
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
        assert_eq!(read(&photo, false, true).unwrap().label, Medal::Gold);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
