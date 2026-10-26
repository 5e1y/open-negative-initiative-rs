//! Opening a frame on its sidecar and writing it back.

use super::*;

/// What a photo whose original cannot be found says as it opens read-only.
pub const UNREACHABLE: &str = oni_i18n::marked("The original cannot be found; its disk may be disconnected. The photo opens read-only, and nothing is saved.");

/// What a retry says while the original of edits kept in memory is still out of sight.
pub const UNSEEN: &str = oni_i18n::marked("The original cannot be found; its disk may be disconnected.");

/// What a retry says when another file stands where the original was, as a disk of the same name.
pub const ANOTHER: &str = oni_i18n::marked("Another file now stands where the original was; the edits are kept for it.");

/// An original's size and modification date.
pub(super) type Stamp = (u64, Option<std::time::SystemTime>);

fn stamp(path: &str) -> Option<Stamp> {
    std::fs::metadata(path).ok().map(|m| (m.len(), m.modified().ok()))
}

/// A frame left while its sidecar refused the write: the edits it held, kept in memory.
#[derive(Debug, Clone)]
pub struct Unsaved {
    pub path: String,
    pub settings: PipelineSettings,
    /// What the disk held when the frame opened, so a retry of the same settings writes nothing.
    persisted: Option<PipelineSettings>,
    stamp: Option<Stamp>,
    /// The refusal, in the disk's words.
    pub message: String,
}

impl EditorStore {
    /// Opens a frame on its sidecar, writing the previous one first; its history starts here, so
    /// an undo never climbs into another photo's.
    pub fn open(&mut self, path: &str) -> Result<(), sidecar::Error> {
        self.open_adjusted(path, |_| {})
    }

    /// `open`, the settings read handed to `adjust` before the history starts on them, as Swift
    /// lays the remembered flat-field sliders (`EditorScene+Actions.swift:71-72`).
    pub fn open_adjusted(&mut self, path: &str, adjust: impl FnOnce(&mut PipelineSettings)) -> Result<(), sidecar::Error> {
        self.leave();
        // A sidecar is absent for a new photo, or when its disk is gone: the original's absence says
        // which, so an unplugged roll never opens as a new one that a reconnection would overwrite.
        let original = stamp(path);
        let absent = original.is_none();
        let read = if absent {
            Err(sidecar::Error::Io(std::io::Error::new(std::io::ErrorKind::NotFound, tr(UNREACHABLE))))
        } else {
            sidecar::read(Path::new(path), self.mosaic(path), true)
        };
        let (mut settings, writable, persisted, result) = match read {
            Ok(loaded) => {
                let persisted = (!loaded.is_new).then(|| loaded.settings.clone());
                (loaded.settings, loaded.writable, persisted, Ok(()))
            }
            // A corrupt sidecar is reported and never overwritten, so it stays repairable by hand.
            Err(e) => (PipelineSettings::rest(ConversionMode::Negative, self.mosaic(path)), false, None, Err(e)),
        };
        // Edits a refused write kept come back with their frame; a writable one takes them again
        // as its own, to write at the next rest, while one still unreachable shows them and keeps them.
        // Another file at the same path (a disk of the same name) never takes them.
        match self.unsaved.iter().position(|u| u.path == path && (absent || u.stamp == original)) {
            Some(i) if writable => settings = self.unsaved.remove(i).settings,
            Some(i) => settings = self.unsaved[i].settings.clone(),
            None => adjust(&mut settings),
        }
        self.open = Some(Open { path: path.to_string(), writable, persisted, absent, stamp: original });
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
        self.leave();
        self.open = None;
        self.history = History::default();
        self.replace(PipelineSettings::default());
        self.pending = FieldSet::all();
    }

    /// The open frame's last step recorded and its sidecar written; a write refused keeps its edits
    /// in `unsaved`, so leaving a frame on a disk gone never drops them.
    fn leave(&mut self) {
        self.release();
        let Some(message) = self.flush() else { return };
        let Some(open) = self.open.as_ref() else { return };
        let kept = Unsaved { path: open.path.clone(), settings: self.settings.clone(), persisted: open.persisted.clone(), stamp: open.stamp, message };
        self.unsaved.retain(|u| u.path != kept.path);
        self.unsaved.push(kept);
    }

    /// Every path the store holds goes where the disk moved it: the open frame, the edits kept for
    /// a refused write, the frames together and the editor's steps; the files are the same ones.
    pub fn follow(&mut self, moves: &[gallery_history::Move]) {
        let moved = |path: &str| {
            moves.iter().find(|m| oni_library::paths::holds(&m.from, path)).map(|m| oni_library::paths::moved(path, &m.from, &m.to))
        };
        if let Some(open) = self.open.as_mut()
            && let Some(to) = moved(&open.path)
        {
            open.stamp = stamp(&to).or(open.stamp);
            open.path = to;
        }
        for kept in &mut self.unsaved {
            if let Some(to) = moved(&kept.path) {
                kept.stamp = stamp(&to).or(kept.stamp);
                kept.path = to;
            }
        }
        self.together.follow(&moved);
        self.history.follow(&moved);
        self.measured.clear();
    }

    /// The gallery's steps follow a move made outside them, a folder renamed or found elsewhere; a
    /// step's own moves never do, or its undo would walk from where it already stands.
    pub fn follow_gallery(&mut self, moves: &[gallery_history::Move]) {
        self.gallery.follow(moves);
    }

    /// The frames left with edits no write took yet, oldest first.
    pub fn unsaved(&self) -> &[Unsaved] {
        &self.unsaved
    }

    /// Writes each frame left unsaved again, never beside an original out of sight nor on another
    /// file at its path; the first refusal's message, `None` once every one is written.
    pub fn retry_unsaved(&mut self) -> Option<String> {
        let mut refused = None;
        self.unsaved.retain(|u| {
            let failure = match stamp(&u.path) {
                None => Some(tr(UNSEEN).to_string()),
                Some(now) if Some(now) != u.stamp => Some(tr(ANOTHER).to_string()),
                Some(_) => sidecar::flush(&u.settings, Some(Path::new(&u.path)), u.persisted.as_ref(), true),
            };
            if let Some(m) = &failure {
                refused.get_or_insert_with(|| m.clone());
            }
            failure.is_some()
        });
        refused
    }

    /// The edits kept for frames whose write was refused, dropped at the hand's word.
    pub fn discard_unsaved(&mut self) {
        self.unsaved.clear();
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
