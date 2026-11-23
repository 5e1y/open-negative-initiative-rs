//! What the shell asks of the open photo's settings, which `oni_core::EditorStore` answers; a notch
//! is one `apply`, a drag one undo step.

use crate::fields::{Block, Field};
use oni_core::{Command, Each, EditorStore};
use oni_settings::PipelineSettings;

/// The sliders whose store field moved since the last call, drained; a field holds several
/// sliders, so the shell compares each value with the one it shows.
pub fn changed(store: &mut EditorStore) -> Vec<Field> {
    let moved = store.take_changes();
    Field::ALL.into_iter().filter(|f| moved.contains(f.knob().field())).collect()
}

pub fn get(store: &EditorStore, field: Field) -> f32 {
    field.knob().get(store.settings())
}

/// The hand's value for `field` on the open frame, as a move from where it stands: the frames edited
/// with it move by the same distance, each keeping its own offset.
pub fn moved(store: &EditorStore, field: Field, to: f32) -> Command {
    let knob = field.knob();
    Command::Step { knob, by: to - knob.get(store.settings()) }
}

/// What a block's reset key lands on: `Parameters.rest(for:)` in the frame's own mode and decode.
pub fn rest(settings: &PipelineSettings) -> PipelineSettings {
    PipelineSettings::rest(settings.mode, settings.decoded_from_mosaic)
}

/// Whether the block's reset key shows: its value differs from what its reset writes.
pub fn is_modified(settings: &PipelineSettings, block: Block) -> bool {
    // `FlatField()`: the block's value is the whole flat-field, its mask included.
    if block == Block::FlatField {
        return settings.flat_field != Default::default();
    }
    let shown = block.fields().iter().filter(|f| f.shown_in(settings));
    shown.into_iter().any(|f| (f.knob().get(settings) - f.base(settings)).abs() > 1e-6)
}

/// Which of red, green and blue More Color Settings shows moved: any of its tab's values off its rest.
pub fn channel_marked(settings: &PipelineSettings) -> [bool; 3] {
    let tabs = [
        [Field::RedWhitePoint, Field::RedBlackPoint, Field::RedMid, Field::RedContrast],
        [Field::GreenWhitePoint, Field::GreenBlackPoint, Field::GreenMid, Field::GreenContrast],
        [Field::BlueWhitePoint, Field::BlueBlackPoint, Field::BlueMid, Field::BlueContrast],
    ];
    tabs.map(|tab| tab.iter().any(|f| (f.knob().get(settings) - f.base(settings)).abs() > 1e-6))
}

/// The block's sliders at their rest, one step: each frame shown with the open one at its own, and the
/// open frame's roll with it where the block is the roll's.
pub fn reset(block: Block) -> Command {
    Command::Each(rest_of(block))
}

/// The block's rest, read off each frame it lands on.
pub fn rest_of(block: Block) -> Each {
    Each::new(move |s| rested(s, block))
}

/// The settings with the block's sliders at the rest of the frame's own mode, decode and measure.
pub fn rested(settings: &PipelineSettings, block: Block) -> PipelineSettings {
    let mut next = settings.clone();
    if block == Block::FlatField {
        next.flat_field = Default::default();
        return next;
    }
    for f in block.fields().iter().filter(|f| f.shown_in(settings)) {
        next = f.knob().set(&next, f.base(settings));
    }
    next
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use oni_core::Archive;
    use oni_settings::{ConversionMode, DyeSet};
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
        changed(&mut s);
        let rest = get(&s, Field::Exposure);
        s.hold();
        for k in 1..=10 {
            s.apply(Command::Set { knob: Field::Exposure.knob(), value: rest + k as f32 * 0.1 });
        }
        let moved = changed(&mut s);
        assert!(moved.contains(&Field::Exposure) && !moved.contains(&Field::Vignette));
        assert!(changed(&mut s).is_empty(), "drained once published");
        s.release();
        assert!((get(&s, Field::Exposure) - rest - 1.0).abs() < 1e-5);
        s.apply(Command::Undo);
        assert!((get(&s, Field::Exposure) - rest).abs() < 1e-6, "one step undoes the whole drag");
        // Twin: a second undo finds nothing, the ten notches having been one step.
        assert!(!s.can_undo());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_channel_tab_is_marked_while_one_of_its_values_moved() {
        let rest = PipelineSettings::rest(oni_settings::ConversionMode::Digital, true);
        assert_eq!(channel_marked(&rest), [false; 3]);
        let mut s = Field::GreenContrast.knob().set(&rest, 0.2);
        assert_eq!(channel_marked(&s), [false, true, false]);
        s = Field::BlueMid.knob().set(&s, -0.1);
        assert_eq!(channel_marked(&s), [false, true, true]);
        // Twin: a white point moved on red marks red alone, never the tab beside it.
        let red = Field::RedWhitePoint.knob().set(&rest, 0.1);
        assert_eq!(channel_marked(&red), [true, false, false]);
        // The block's reset clears every tab's mark at once.
        assert_eq!(channel_marked(&rested(&s, Block::ChannelPoints)), [false; 3]);
    }

    #[test]
    fn a_block_reset_lands_on_the_rest_in_one_write() {
        let (mut s, dir) = fresh("reset");
        s.apply(Command::Set { knob: Field::Chrome.knob(), value: 0.5 });
        s.apply(Command::Set { knob: Field::Vignette.knob(), value: -1.0 });
        assert!(is_modified(s.settings(), Block::Effects) && !is_modified(s.settings(), Block::Lighting));
        let revision = s.revision();
        let command = reset(Block::Effects);
        s.apply(command);
        assert_eq!(s.revision(), revision + 1);
        assert!(!is_modified(s.settings(), Block::Effects));
        s.apply(Command::Undo);
        assert_eq!(get(&s, Field::Chrome), 0.5, "the reset is one step, undone whole");
        assert_eq!(get(&s, Field::Vignette), -1.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_block_reset_lands_on_the_rest_of_the_frames_own_mode() {
        let (mut s, dir) = fresh("mode-rest");
        s.apply(Command::Mode(ConversionMode::Positive));
        s.apply(Command::Set { knob: Field::Exposure.knob(), value: 0.7 });
        s.apply(Command::Set { knob: Field::DensityCeiling.knob(), value: 2.9 });
        let command = reset(Block::Lighting);
        s.apply(command);
        let command = reset(Block::DensityCeiling);
        s.apply(command);
        let slide = PipelineSettings::rest(ConversionMode::Positive, true);
        assert_eq!(get(&s, Field::Exposure), Field::Exposure.knob().get(&slide));
        assert_eq!(get(&s, Field::DensityCeiling), Field::DensityCeiling.knob().get(&slide));
        assert!(!is_modified(s.settings(), Block::Lighting) && !is_modified(s.settings(), Block::DensityCeiling));
        // Twin: a negative's ceiling rests elsewhere, so a reset to it would fail above; Exposure rests at 0 on both.
        let negative = PipelineSettings::rest(ConversionMode::Negative, true);
        assert_eq!(Field::Exposure.knob().get(&negative), Field::Exposure.knob().get(&slide));
        assert_ne!(Field::DensityCeiling.knob().get(&negative), Field::DensityCeiling.knob().get(&slide));
        assert!(is_modified(&negative_on(s.settings()), Block::DensityCeiling), "a negative's rest reads as moved on a slide");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_block_reset_together_reaches_the_open_roll_whole_and_shows_while_a_frame_beside_is_off_rest() {
        let dir = std::env::temp_dir().join(format!("oni-app-store-together-reset-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let reference = |id| Some(oni_settings::frame::RollReference { id, stops: [0.0; 3], black: [0.0; 3], white: [1.0; 3], base: None, median: None });
        let (portra, fuji) = (oni_settings::Id::new(), oni_settings::Id::new());
        let dyed = |id, set| PipelineSettings { dye_set: set, roll_reference: reference(id), ..PipelineSettings::rest(ConversionMode::Negative, false) };
        let kodak = DyeSet::of("kodak-portra-400");
        let mut beside = dyed(fuji, DyeSet::of("fujifilm-xtra-400"));
        beside.print.exposure = 0.4;
        let laid = [("a.tif", dyed(portra, kodak)), ("b.tif", beside), ("c.tif", dyed(portra, kodak))];
        let paths: Vec<String> = laid.iter().map(|(n, _)| dir.join(n).to_string_lossy().into_owned()).collect();
        for (p, (_, s)) in paths.iter().zip(&laid) {
            std::fs::write(p, b"").unwrap();
            oni_settings::sidecar::write(s, Path::new(p), Default::default()).unwrap();
        }
        let read = |p: &str| oni_settings::sidecar::read(Path::new(p), false).unwrap().settings;
        let mut s = EditorStore::new(Archive::new(paths.clone(), |_: &Path| false));
        s.open(&paths[0]).unwrap();
        s.set_together(vec![paths[1].clone()]);
        let beside = |s: &EditorStore| s.together_where(oni_core::Field::Print, |b| is_modified(b, Block::Lighting));
        assert!(!is_modified(s.settings(), Block::Lighting) && beside(&s), "the key shows for the frame beside");
        s.apply(reset(Block::Lighting));
        assert_eq!(read(&paths[1]).dye_set, DyeSet::of("fujifilm-xtra-400"), "the Fuji frame keeps its own stock");
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
}
