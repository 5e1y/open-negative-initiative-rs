//! What the shell asks of the open photo's settings. The shell reads and writes them only through
//! `Store`, which `oni_core::EditorStore` answers; a notch is one `apply`, a drag one undo step.

use crate::fields::{Block, Field};
use oni_core::{Command, EditorStore, Surface};
use oni_settings::{Face, PipelineSettings};

pub trait Store {
    /// Opens a frame on its sidecar, writing the previous one first.
    fn open(&mut self, path: &str) -> Result<(), String>;
    /// `open`, the settings read adjusted before the history starts on them.
    fn open_adjusted(&mut self, path: &str, _adjust: &dyn Fn(&mut PipelineSettings)) -> Result<(), String> {
        self.open(path)
    }
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
    /// The hand lifts and the step closes once the edit has rested, a press landing sooner
    /// joining it; a stand-in closes it at once.
    fn release_deferred(&mut self) {
        self.release();
    }
    /// The next change outside a press waits like a lifted hand's; nothing while a hand holds.
    fn defer(&mut self) {}
    /// Records a waiting step once due; how long it still waits, `None` when nothing does.
    fn settle_if_due(&mut self) -> Option<std::time::Duration> {
        None
    }
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

    /// The editing model itself, which the phone link drives; `None` for a stand-in.
    fn editor(&mut self) -> Option<&mut EditorStore> {
        None
    }

    fn editor_ref(&self) -> Option<&EditorStore> {
        None
    }

    /// The face the open frame is edited on, which `Command::Face` switches.
    fn face(&self) -> Face {
        Face::Pro
    }

    /// The settings as the open face's Color tab reads them.
    fn face_view(&self) -> PipelineSettings {
        self.settings().clone()
    }
}

impl Store for EditorStore {
    fn open(&mut self, path: &str) -> Result<(), String> {
        EditorStore::open(self, path).map_err(|e| e.to_string())
    }

    fn open_adjusted(&mut self, path: &str, adjust: &dyn Fn(&mut PipelineSettings)) -> Result<(), String> {
        EditorStore::open_adjusted(self, path, adjust).map_err(|e| e.to_string())
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

    fn release_deferred(&mut self) {
        EditorStore::release_deferred(self);
    }

    fn settle_if_due(&mut self) -> Option<std::time::Duration> {
        EditorStore::settle_if_due(self)
    }

    fn defer(&mut self) {
        EditorStore::defer(self);
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

    fn editor(&mut self) -> Option<&mut EditorStore> {
        Some(self)
    }

    fn editor_ref(&self) -> Option<&EditorStore> {
        Some(self)
    }

    fn face(&self) -> Face {
        EditorStore::face(self)
    }

    fn face_view(&self) -> PipelineSettings {
        EditorStore::face_view(self)
    }
}

/// What a block's reset key lands on: `Parameters.rest(for:)` in the frame's own mode and decode.
pub fn rest(settings: &PipelineSettings) -> PipelineSettings {
    PipelineSettings::rest(settings.mode, settings.decoded_from_mosaic)
}

/// Whether the block's reset key shows: its value differs from what its reset writes.
pub fn is_modified(settings: &PipelineSettings, block: Block) -> bool {
    match block {
        // `FlatField()`: the block's value is the whole flat-field, its mask included.
        Block::FlatField => return settings.flat_field != Default::default(),
        // `restingGains`: the measured roll's own stops.
        Block::SourceBalance => return block.fields().iter().any(|f| (f.knob().get(settings) - f.base(settings)).abs() > 1e-6),
        _ => {}
    }
    let rest = rest(settings);
    let off = block.fields().iter().any(|f| (f.knob().get(settings) - f.knob().get(&rest)).abs() > 1e-6);
    // `SettingsPane.dyes`: the block's value is the dose, the family and the hand's mark together.
    off || (block == Block::DyeSeparation && (settings.separation_family.is_some() || settings.separation_by_hand))
}

/// The block's sliders at their rest, as one write so the reset is one undo step.
pub fn reset(settings: &PipelineSettings, block: Block) -> Command {
    let rest = rest(settings);
    let mut next = settings.clone();
    match block {
        Block::FlatField => {
            next.flat_field = Default::default();
            return Command::Write(Box::new(next));
        }
        Block::SourceBalance => {
            for f in block.fields() {
                next = f.knob().set(&next, f.base(settings));
            }
            return Command::Write(Box::new(next));
        }
        _ => {}
    }
    for f in block.fields() {
        next = f.knob().set(&next, f.knob().get(&rest));
    }
    if block == Block::DyeSeparation {
        // `Dyes(dose: Separation.base, family: nil, byHand: false)`, which the dose's knob alone would mark by hand.
        next.separation = rest.separation;
        next.separation_family = None;
        next.separation_by_hand = false;
    }
    Command::Write(Box::new(next))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use oni_core::Archive;
    use oni_settings::ConversionMode;
    use oni_settings::blocks::{SeparationFamily, separation};
    use std::path::{Path, PathBuf};

    /// A store opened on a frame with no sidecar in a fresh folder, read as a RAW mosaic.
    pub(crate) fn fresh(label: &str) -> (EditorStore, PathBuf) {
        let dir = std::env::temp_dir().join(format!("oni-app-store-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let frame = dir.join("frame.NEF").to_string_lossy().into_owned();
        std::fs::write(&frame, b"").unwrap();
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

    #[test]
    fn a_block_reset_lands_on_the_rest_of_the_frames_own_mode() {
        let (mut s, dir) = fresh("mode-rest");
        s.apply(Command::Mode(ConversionMode::Positive));
        s.apply(Command::Set { knob: Field::Exposure.knob(), value: 0.7 });
        s.apply(Command::Set { knob: Field::DensityCeiling.knob(), value: 2.9 });
        let command = reset(s.settings(), Block::Lighting);
        s.apply(command);
        let command = reset(s.settings(), Block::DensityCeiling);
        s.apply(command);
        let slide = PipelineSettings::rest(ConversionMode::Positive, true);
        assert_eq!(s.get(Field::Exposure), Field::Exposure.knob().get(&slide));
        assert_eq!(s.get(Field::DensityCeiling), Field::DensityCeiling.knob().get(&slide));
        assert!(!is_modified(s.settings(), Block::Lighting) && !is_modified(s.settings(), Block::DensityCeiling));
        // Twin: a negative's rest sits elsewhere on both, so a reset to it would fail above.
        let negative = PipelineSettings::rest(ConversionMode::Negative, true);
        assert_ne!(Field::Exposure.knob().get(&negative), Field::Exposure.knob().get(&slide));
        assert_ne!(Field::DensityCeiling.knob().get(&negative), Field::DensityCeiling.knob().get(&slide));
        assert!(is_modified(&negative_on(s.settings()), Block::Lighting), "a negative's rest reads as moved on a slide");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The settings with a negative's Lighting and ceiling, everything else kept.
    fn negative_on(settings: &PipelineSettings) -> PipelineSettings {
        let negative = PipelineSettings::rest(ConversionMode::Negative, true);
        PipelineSettings { print: negative.print, density_ceiling: negative.density_ceiling, ..settings.clone() }
    }

    #[test]
    fn the_dye_separation_reset_clears_the_dose_the_family_and_the_hand() {
        let (mut s, dir) = fresh("dyes");
        let mut picked = s.settings().clone();
        picked.separation_family = Some(SeparationFamily::KodakC41);
        s.apply(Command::Write(Box::new(picked)));
        // The family alone, the dose at its base, still shows the block's reset key.
        assert_eq!(s.get(Field::DyeSeparation), separation::BASE);
        assert!(is_modified(s.settings(), Block::DyeSeparation));
        s.apply(Command::Set { knob: Field::DyeSeparation.knob(), value: 1.1 });
        assert!(s.settings().separation_by_hand);
        let command = reset(s.settings(), Block::DyeSeparation);
        s.apply(command);
        let after = s.settings();
        assert_eq!((after.separation, after.separation_family, after.separation_by_hand), (separation::BASE, None, false));
        assert!(!is_modified(after, Block::DyeSeparation));
        // Twin: the dose's knob alone lands on the base yet marks it by hand and keeps the family.
        let by_knob = Field::DyeSeparation.knob().set(&PipelineSettings { separation_family: Some(SeparationFamily::KodakC41), ..after.clone() }, separation::BASE);
        assert!(by_knob.separation_by_hand && by_knob.separation_family.is_some());
        assert!(is_modified(&by_knob, Block::DyeSeparation));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
