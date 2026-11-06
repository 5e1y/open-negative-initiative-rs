//! What the shell asks of the open photo's settings. The shell reads and writes them only through
//! `Store`, which `oni_core::EditorStore` answers; a notch is one `apply`, a drag one undo step.

use crate::fields::{Block, Field};
use oni_core::{Command, Each, EditorStore};
use oni_settings::blocks::SeparationFamily;
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
        EditorStore::can_undo(self)
    }

    fn can_redo(&self) -> bool {
        EditorStore::can_redo(self)
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

/// The hand's value for `field` on the open frame, as a move from where it stands: the frames edited
/// with it move by the same distance, each keeping its own offset.
pub fn moved(store: &dyn Store, field: Field, to: f32) -> Command {
    let knob = field.knob();
    Command::Step { knob, by: to - knob.get(&store.face_view()) }
}

/// What a block's reset key lands on: `Parameters.rest(for:)` in the frame's own mode and decode.
pub fn rest(settings: &PipelineSettings) -> PipelineSettings {
    PipelineSettings::rest(settings.mode, settings.decoded_from_mosaic)
}

/// Whether the block's reset key shows: its value differs from what its reset writes. `dyes` is
/// the family the roll's stock names, which the dyes rest on; none rests on the mode's own.
pub fn is_modified(settings: &PipelineSettings, block: Block, dyes: Option<SeparationFamily>) -> bool {
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
    off || (block == Block::DyeSeparation && (settings.separation_family != dyes || settings.separation_by_hand))
}

/// The block's sliders at their rest, one step: each frame shown with the open one at its own, and the
/// open frame's roll with it where the block is the roll's.
pub fn reset(block: Block, dyes: Option<SeparationFamily>) -> Command {
    Command::Each(rest_of(block, dyes))
}

/// The block's rest, read off each frame it lands on.
pub fn rest_of(block: Block, dyes: Option<SeparationFamily>) -> Each {
    Each::new(move |s| rested(s, block, dyes))
}

/// The settings with the block's sliders at the rest of the frame's own mode, decode and measure; the
/// dyes land on the family the roll's stock names.
pub fn rested(settings: &PipelineSettings, block: Block, dyes: Option<SeparationFamily>) -> PipelineSettings {
    let rest = rest(settings);
    let mut next = settings.clone();
    match block {
        Block::FlatField => {
            next.flat_field = Default::default();
            return next;
        }
        Block::SourceBalance => {
            for f in block.fields() {
                next = f.knob().set(&next, f.base(settings));
            }
            return next;
        }
        _ => {}
    }
    for f in block.fields() {
        next = f.knob().set(&next, f.knob().get(&rest));
    }
    if block == Block::DyeSeparation {
        // The dose at its base and the stock's dyes, which the dose's knob alone would mark by hand.
        next.separation = rest.separation;
        next.separation_family = dyes;
        next.separation_by_hand = false;
    }
    next
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use oni_core::Archive;
    use oni_settings::ConversionMode;
    use oni_settings::blocks::separation;
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
        s.apply(Command::Set { knob: Field::Chrome.knob(), value: 0.5 });
        s.apply(Command::Set { knob: Field::Vignette.knob(), value: -1.0 });
        assert!(is_modified(s.settings(), Block::Effects, None) && !is_modified(s.settings(), Block::Lighting, None));
        let revision = s.revision();
        let command = reset(Block::Effects, None);
        s.apply(command);
        assert_eq!(s.revision(), revision + 1);
        assert!(!is_modified(s.settings(), Block::Effects, None));
        s.apply(Command::Undo);
        assert_eq!(s.get(Field::Chrome), 0.5, "the reset is one step, undone whole");
        assert_eq!(s.get(Field::Vignette), -1.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_block_reset_lands_on_the_rest_of_the_frames_own_mode() {
        let (mut s, dir) = fresh("mode-rest");
        s.apply(Command::Mode(ConversionMode::Positive));
        s.apply(Command::Set { knob: Field::Exposure.knob(), value: 0.7 });
        s.apply(Command::Set { knob: Field::DensityCeiling.knob(), value: 2.9 });
        let command = reset(Block::Lighting, None);
        s.apply(command);
        let command = reset(Block::DensityCeiling, None);
        s.apply(command);
        let slide = PipelineSettings::rest(ConversionMode::Positive, true);
        assert_eq!(s.get(Field::Exposure), Field::Exposure.knob().get(&slide));
        assert_eq!(s.get(Field::DensityCeiling), Field::DensityCeiling.knob().get(&slide));
        assert!(!is_modified(s.settings(), Block::Lighting, None) && !is_modified(s.settings(), Block::DensityCeiling, None));
        // Twin: a negative's rest sits elsewhere on both, so a reset to it would fail above.
        let negative = PipelineSettings::rest(ConversionMode::Negative, true);
        assert_ne!(Field::Exposure.knob().get(&negative), Field::Exposure.knob().get(&slide));
        assert_ne!(Field::DensityCeiling.knob().get(&negative), Field::DensityCeiling.knob().get(&slide));
        assert!(is_modified(&negative_on(s.settings()), Block::Lighting, None), "a negative's rest reads as moved on a slide");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_block_reset_together_reaches_the_open_roll_whole_and_shows_while_a_frame_beside_is_off_rest() {
        let dir = std::env::temp_dir().join(format!("oni-app-store-together-reset-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let reference = |id| Some(oni_settings::frame::RollReference { id, stops: [0.0; 3], black: [0.0; 3], white: [1.0; 3], base: None, median: None });
        let (portra, fuji) = (oni_settings::Id::new(), oni_settings::Id::new());
        let dyed = |id, family| PipelineSettings { separation: 1.1, separation_family: Some(family), separation_by_hand: true, roll_reference: reference(id), ..PipelineSettings::rest(ConversionMode::Negative, false) };
        let kodak = SeparationFamily::KodakC41;
        let mut beside = dyed(fuji, SeparationFamily::FujiC41);
        beside.print.exposure = 0.4;
        let laid = [("a.tif", dyed(portra, kodak)), ("b.tif", beside), ("c.tif", dyed(portra, kodak))];
        let paths: Vec<String> = laid.iter().map(|(n, _)| dir.join(n).to_string_lossy().into_owned()).collect();
        for (p, (_, s)) in paths.iter().zip(&laid) {
            std::fs::write(p, b"").unwrap();
            oni_settings::sidecar::write(s, Path::new(p), Default::default()).unwrap();
        }
        let read = |p: &str| oni_settings::sidecar::read(Path::new(p), false, true).unwrap().settings;
        let mut s = EditorStore::new(Archive::new(paths.clone(), |_: &Path| false));
        s.open(&paths[0]).unwrap();
        s.set_together(vec![paths[1].clone()]);
        let beside = |s: &EditorStore| s.together_where(oni_core::Field::Print, |b| is_modified(b, Block::Lighting, Some(kodak)));
        assert!(!is_modified(s.settings(), Block::Lighting, Some(kodak)) && beside(&s), "the key shows for the frame beside");
        s.apply(reset(Block::DyeSeparation, Some(kodak)));
        s.apply(reset(Block::Lighting, Some(kodak)));
        let dyes = |s: PipelineSettings| (s.separation, s.separation_family, s.separation_by_hand);
        assert_eq!(dyes(read(&paths[2])), (separation::BASE, Some(kodak), false), "the roll's frame left out takes its stock's dyes");
        assert_eq!(dyes(read(&paths[1])), (1.1, Some(SeparationFamily::FujiC41), true), "the Fuji frame keeps its own");
        assert_eq!(read(&paths[1]).print.exposure, Field::Exposure.knob().get(&rest(&read(&paths[1]))), "and its Lighting at its own rest");
        // Twin: once every frame stands at rest, the key no longer shows.
        assert!(!beside(&s));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The settings with a negative's Lighting and ceiling, everything else kept.
    fn negative_on(settings: &PipelineSettings) -> PipelineSettings {
        let negative = PipelineSettings::rest(ConversionMode::Negative, true);
        PipelineSettings { print: negative.print, density_ceiling: negative.density_ceiling, ..settings.clone() }
    }

    #[test]
    fn the_dye_separation_reset_lands_on_the_stocks_dyes_and_clears_the_dose_and_the_hand() {
        let (mut s, dir) = fresh("dyes");
        let fuji = Some(SeparationFamily::FujiC41);
        let mut picked = s.settings().clone();
        picked.separation_family = Some(SeparationFamily::KodakC41);
        s.apply(Command::Write(Box::new(picked)));
        // The family alone, the dose at its base, still shows the block's reset key.
        assert_eq!(s.get(Field::DyeSeparation), separation::BASE);
        assert!(is_modified(s.settings(), Block::DyeSeparation, None) && is_modified(s.settings(), Block::DyeSeparation, fuji));
        s.apply(Command::Set { knob: Field::DyeSeparation.knob(), value: 1.1 });
        assert!(s.settings().separation_by_hand);
        let command = reset(Block::DyeSeparation, fuji);
        s.apply(command);
        let after = s.settings().clone();
        assert_eq!((after.separation, after.separation_family, after.separation_by_hand), (separation::BASE, fuji, false));
        assert!(!is_modified(&after, Block::DyeSeparation, fuji), "the stock's dyes are the block's rest");
        assert!(is_modified(&after, Block::DyeSeparation, None), "twin: with no stock they are a pick");
        // With no stock naming dyes, the reset clears the family to the mode's own.
        let command = reset(Block::DyeSeparation, None);
        s.apply(command);
        assert_eq!(s.settings().separation_family, None);
        assert!(!is_modified(s.settings(), Block::DyeSeparation, None));
        assert!(is_modified(s.settings(), Block::DyeSeparation, fuji), "twin: off the stock's dyes is a pick");
        // Twin: the dose's knob alone lands on the base yet marks it by hand and keeps the family.
        let by_knob = Field::DyeSeparation.knob().set(&PipelineSettings { separation_family: fuji, ..after.clone() }, separation::BASE);
        assert!(by_knob.separation_by_hand && by_knob.separation_family.is_some());
        assert!(is_modified(&by_knob, Block::DyeSeparation, fuji));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
