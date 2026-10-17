//! What the shell asks of the open photo's settings. The shell reads and writes them only through
//! `Store`, which `oni_core::EditorStore` answers; a notch is one `apply`, a drag one undo step.

use crate::fields::{Block, Field};
use oni_core::{Command, EditorStore, Surface};
use oni_settings::{ConversionMode, PipelineSettings};

pub trait Store {
    /// Opens a frame on its sidecar, writing the previous one first.
    fn open(&mut self, path: &str) -> Result<(), String>;
    /// Every frame of the library, which a Roll Settings gesture reaches.
    fn set_archive(&mut self, paths: Vec<String>);
    /// Writes the open frame's sidecar when the disk differs; the failure's message otherwise.
    fn flush(&mut self) -> Option<String>;
    fn settings(&self) -> &PipelineSettings;
    /// One command, one write; the moved fields are reported by `changed`.
    fn apply(&mut self, command: Command);
    /// A hand on a control: its notches land at once, and the step is recorded on `release`.
    fn hold(&mut self);
    fn release(&mut self);
    /// The sliders whose store field moved since the last call, drained; a field holds several
    /// sliders, so the shell compares each value with the one it shows.
    fn changed(&mut self) -> Vec<Field>;
    fn can_undo(&self) -> bool;
    fn can_redo(&self) -> bool;
    /// Bumped by every change, so a render job can say which state it shows.
    fn revision(&self) -> u64;

    fn get(&self, field: Field) -> f32 {
        field.knob().get(self.settings())
    }
}

impl Store for EditorStore {
    fn open(&mut self, path: &str) -> Result<(), String> {
        EditorStore::open(self, path).map_err(|e| e.to_string())
    }

    fn set_archive(&mut self, paths: Vec<String>) {
        self.set_archive_paths(paths);
    }

    fn flush(&mut self) -> Option<String> {
        EditorStore::flush(self)
    }

    fn settings(&self) -> &PipelineSettings {
        EditorStore::settings(self)
    }

    fn apply(&mut self, command: Command) {
        EditorStore::apply(self, command);
    }

    fn hold(&mut self) {
        EditorStore::hold(self);
    }

    fn release(&mut self) {
        EditorStore::release(self);
    }

    fn changed(&mut self) -> Vec<Field> {
        let moved = self.take_changes();
        Field::ALL.into_iter().filter(|f| moved.contains(f.knob().field())).collect()
    }

    fn can_undo(&self) -> bool {
        EditorStore::can_undo(self, Surface::Editor)
    }

    fn can_redo(&self) -> bool {
        EditorStore::can_redo(self, Surface::Editor)
    }

    fn revision(&self) -> u64 {
        EditorStore::revision(self)
    }
}

/// What a block's reset key lands on: `Parameters.rest(for:)`, the frame's own decode kept.
pub fn rest(settings: &PipelineSettings) -> PipelineSettings {
    PipelineSettings::rest(ConversionMode::Negative, settings.decoded_from_mosaic)
}

/// Whether the block's reset key shows: one of its sliders is off its rest.
pub fn is_modified(settings: &PipelineSettings, block: Block) -> bool {
    let rest = rest(settings);
    block.fields().iter().any(|f| (f.knob().get(settings) - f.knob().get(&rest)).abs() > 1e-6)
}

/// The block's sliders at their rest, as one write so the reset is one undo step.
pub fn reset(settings: &PipelineSettings, block: Block) -> Command {
    let rest = rest(settings);
    let mut next = settings.clone();
    for f in block.fields() {
        next = f.knob().set(&next, f.knob().get(&rest));
    }
    Command::Write(Box::new(next))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use oni_core::Archive;
    use std::path::{Path, PathBuf};

    /// A store opened on a frame with no sidecar in a fresh folder, read as a RAW mosaic.
    pub(crate) fn fresh(label: &str) -> (EditorStore, PathBuf) {
        let dir = std::env::temp_dir().join(format!("oni-app-store-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let frame = dir.join("frame.NEF").to_string_lossy().into_owned();
        let mut store = EditorStore::new(Archive::new(vec![frame.clone()], |_: &Path| true));
        store.open(&frame).unwrap();
        (store, dir)
    }

    #[test]
    fn a_drag_is_one_undo_step_and_reports_only_its_slider() {
        let (mut s, dir) = fresh("drag");
        Store::changed(&mut s);
        let rest = s.get(Field::Exposure);
        s.hold();
        for k in 1..=10 {
            s.apply(Command::Set { knob: Field::Exposure.knob(), value: rest + k as f32 * 0.1 });
        }
        let moved = Store::changed(&mut s);
        assert!(moved.contains(&Field::Exposure) && !moved.contains(&Field::Vignette));
        assert!(Store::changed(&mut s).is_empty(), "drained once published");
        s.release();
        assert!((s.get(Field::Exposure) - rest - 1.0).abs() < 1e-5);
        s.apply(Command::Undo);
        assert!((s.get(Field::Exposure) - rest).abs() < 1e-6, "one step undoes the whole drag");
        // Twin: a second undo finds nothing, the ten notches having been one step.
        assert!(!Store::can_undo(&s));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_block_reset_lands_on_the_rest_in_one_write() {
        let (mut s, dir) = fresh("reset");
        s.apply(Command::Set { knob: Field::Contrast.knob(), value: 0.5 });
        s.apply(Command::Set { knob: Field::Vignette.knob(), value: -1.0 });
        assert!(is_modified(s.settings(), Block::Effects) && !is_modified(s.settings(), Block::Lighting));
        let revision = s.revision();
        let command = reset(s.settings(), Block::Effects);
        s.apply(command);
        assert_eq!(s.revision(), revision + 1);
        assert!(!is_modified(s.settings(), Block::Effects));
        s.apply(Command::Undo);
        assert_eq!(s.get(Field::Contrast), 0.5, "the reset is one step, undone whole");
        assert_eq!(s.get(Field::Vignette), -1.0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
