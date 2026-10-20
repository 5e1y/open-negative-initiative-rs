//! Opening a frame on its sidecar and writing it back.

use super::*;

/// What a photo whose original cannot be found says as it opens read-only.
pub const UNREACHABLE: &str = "The original cannot be found; its disk may be disconnected. The photo opens read-only, and nothing is saved.";

impl EditorStore {
    /// Opens a frame on its sidecar, writing the previous one first; its history starts here, so
    /// an undo never climbs into another photo's.
    pub fn open(&mut self, path: &str) -> Result<(), sidecar::Error> {
        self.open_adjusted(path, |_| {})
    }

    /// `open`, the settings read handed to `adjust` before the history starts on them, as Swift
    /// lays the remembered flat-field sliders (`EditorScene+Actions.swift:71-72`).
    pub fn open_adjusted(&mut self, path: &str, adjust: impl FnOnce(&mut PipelineSettings)) -> Result<(), sidecar::Error> {
        self.release();
        self.flush();
        // A sidecar is absent for a new photo, or when its disk is gone: the original's absence says
        // which, so an unplugged roll never opens as a new one that a reconnection would overwrite.
        let read = if Path::new(path).exists() {
            sidecar::read(Path::new(path), self.mosaic(path), true)
        } else {
            Err(sidecar::Error::Io(std::io::Error::new(std::io::ErrorKind::NotFound, UNREACHABLE)))
        };
        let (mut settings, writable, persisted, result) = match read {
            Ok(loaded) => {
                let persisted = (!loaded.is_new).then(|| loaded.settings.clone());
                (loaded.settings, loaded.writable, persisted, Ok(()))
            }
            // A corrupt sidecar is reported and never overwritten, so it stays repairable by hand.
            Err(e) => (PipelineSettings::rest(ConversionMode::Negative, self.mosaic(path)), false, None, Err(e)),
        };
        adjust(&mut settings);
        self.open = Some(Open { path: path.to_string(), writable, persisted });
        // `adopt` starts the history on the stored settings, then `adoptFace` changes the face, which
        // the deferred `record` keeps as the first undo step (`EditorScene+Actions.swift:91-95`).
        self.history = History::new(settings.clone());
        let adopted = PipelineSettings { face: self.face(), ..settings };
        self.history.record(adopted.clone(), f64::NEG_INFINITY);
        self.replace(adopted);
        self.pending = FieldSet::all();
        self.roll_touched = false;
        self.roll_look = None;
        result
    }

    pub fn close(&mut self) {
        self.release();
        self.flush();
        self.open = None;
        self.history = History::default();
        self.replace(PipelineSettings::default());
        self.pending = FieldSet::all();
    }

    /// Writes the open frame's sidecar when the disk differs; the failure's message otherwise.
    pub fn flush(&mut self) -> Option<String> {
        let open = self.open.as_ref()?;
        let failure =
            sidecar::flush(&self.settings, Some(Path::new(&open.path)), open.persisted.as_ref(), open.writable);
        if failure.is_none() && open.writable && sidecar::should_write(&self.settings, Path::new(&open.path)) {
            self.open.as_mut().expect("open frame").persisted = Some(self.settings.clone());
        }
        if let Some(message) = &failure {
            self.failure = Some(message.clone());
        }
        failure
    }

    /// Flushes once the hand has rested `WRITE_DELAY`, so a drag writes the sidecar once.
    pub fn flush_if_due(&mut self, now: f64) -> Option<String> {
        let due = !self.held && self.last_change.is_some_and(|t| now - t >= sidecar::WRITE_DELAY.as_secs_f64());
        if !due {
            return None;
        }
        self.last_change = None;
        self.flush()
    }

    pub(super) fn replace(&mut self, next: PipelineSettings) -> FieldSet {
        let changed = FieldSet::between(&self.settings, &next);
        if !changed.is_empty() {
            self.settings = next;
            self.pending = self.pending.union(changed);
            self.revision += 1;
        }
        changed
    }
}
