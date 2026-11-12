//! The thread an export writes on and the stop it honours between frames, which a quit waits for.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

#[derive(Default)]
pub struct Writer {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Writer {
    /// A new export: the stop is lowered.
    pub fn begin(&self) {
        self.stop.store(false, Ordering::Relaxed);
    }

    /// The flag the batch reads between frames.
    pub fn flag(&self) -> Arc<AtomicBool> {
        self.stop.clone()
    }

    /// No frame after the one under way begins.
    pub fn halt(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    /// Runs `write` on its own thread, which `ended_whole` joins.
    pub fn spawn(&mut self, name: &str, write: impl FnOnce() + Send + 'static) {
        self.handle = Some(std::thread::Builder::new().name(name.into()).spawn(write).expect("export thread"));
    }

    /// The frame under way lands whole, none after it begins: a process ending cannot cut a file.
    pub fn ended_whole(&mut self) {
        self.halt();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::time::Duration;

    const FRAMES: usize = 10;

    /// Frames written through a hidden temporary and a rename, then vouched for by the ledger.
    fn frame(folder: &Path, ledger: &mut oni_export::file::Ledger, i: usize) {
        let tmp = folder.join(format!(".oni-{i}.tmp"));
        std::fs::write(&tmp, b"frame").unwrap();
        std::thread::sleep(Duration::from_millis(150));
        let done = folder.join(format!("P{i}.tif"));
        std::fs::rename(&tmp, &done).unwrap();
        ledger.note(&done).unwrap();
    }

    /// Whether a quit mid-frame leaves the folder whole, ending with `ending`.
    fn whole_after(ending: impl FnOnce(&mut Writer)) -> bool {
        let root = std::env::temp_dir().join(format!("oni-writer-{}-{:?}", std::process::id(), std::thread::current().id()));
        let (folder, support) = (root.join("out"), root.join("support"));
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::create_dir_all(&support).unwrap();
        let mut writer = Writer::default();
        let (stop, work, kept) = (writer.flag(), folder.clone(), support.clone());
        writer.spawn("test export", move || {
            let mut ledger = oni_export::file::Ledger::in_support(&kept);
            for i in 0..FRAMES {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                frame(&work, &mut ledger, i);
            }
        });
        std::thread::sleep(Duration::from_millis(50));
        ending(&mut writer);
        let ledger = oni_export::file::Ledger::in_support(&support);
        let names: Vec<_> = std::fs::read_dir(&folder).unwrap().map(|e| e.unwrap().path()).collect();
        let whole = names.len() < FRAMES
            && names.iter().all(|p| !p.file_name().unwrap().to_string_lossy().ends_with(".tmp") && ledger.vouches(p));
        std::thread::sleep(Duration::from_millis(400));
        let _ = std::fs::remove_dir_all(&root);
        whole
    }

    #[test]
    fn a_quit_waits_for_the_frame_under_way() {
        assert!(whole_after(Writer::ended_whole));
    }

    #[test]
    fn a_stop_without_the_wait_cuts_the_frame() {
        assert!(!whole_after(|w| w.halt()));
    }
}
