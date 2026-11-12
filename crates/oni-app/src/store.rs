//! What the shell asks of the open photo's settings, which `oni_core::EditorStore` answers; a notch
//! is one `apply`, a drag one undo step.

use crate::fields::{Block, Field};
use oni_core::{Command, Each, EditorStore};
use oni_settings::blocks::SeparationFamily;
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
    Command::Step { knob, by: to - knob.get(&store.face_view()) }
}

/// What a block's reset key lands on: `Parameters.rest(for:)` in the frame's own mode and decode.
pub fn rest(settings: &PipelineSettings) -> PipelineSettings {
    PipelineSettings::rest(settings.mode, settings.decoded_from_mosaic)
}

/// Whether the block's reset key shows: its value differs from what its reset writes. `dyes` is
/// the family the roll's stock names, which the dyes rest on; none rests on the mode's own.
pub fn is_modified(settings: &PipelineSettings, block: Block, dyes: Option<SeparationFamily>) -> bool {
    // `FlatField()`: the block's value is the whole flat-field, its mask included.
    if block == Block::FlatField {
        return settings.flat_field != Default::default();
    }
    let off = block.fields().iter().any(|f| (f.knob().get(settings) - f.base(settings)).abs() > 1e-6);
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
    let mut next = settings.clone();
    if block == Block::FlatField {
        next.flat_field = Default::default();
        return next;
    }
    for f in block.fields() {
        next = f.knob().set(&next, f.base(settings));
    }
    if block == Block::DyeSeparation {
        // The dose at its base and the stock's dyes, which the dose's knob alone would mark by hand.
        next.separation = rest(settings).separation;
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
        let command = reset(Block::Lighting, None);
        s.apply(command);
        let command = reset(Block::DensityCeiling, None);
        s.apply(command);
        let slide = PipelineSettings::rest(ConversionMode::Positive, true);
        assert_eq!(get(&s, Field::Exposure), Field::Exposure.knob().get(&slide));
        assert_eq!(get(&s, Field::DensityCeiling), Field::DensityCeiling.knob().get(&slide));
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
        let read = |p: &str| oni_settings::sidecar::read(Path::new(p), false).unwrap().settings;
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
        assert_eq!(get(&s, Field::DyeSeparation), separation::BASE);
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
