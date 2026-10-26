//! Edit Together: frames opened beside the open one, each step taken on the open frame laid on them at
//! the same values, as part of that one step; what belongs to each file stays its own.

use super::*;
use crate::fields::{Field, FieldSet};

/// A frame's own fields, which a step on the open frame never lays on the others: its file, its
/// framing, its dust, its roll.
const OWN: [Field; 15] = [
    Field::Film,
    Field::ManualOrder,
    Field::Sheet,
    Field::DecodedFromMosaic,
    Field::CaptureStops,
    Field::LensCorrected,
    Field::PrimariesConverted,
    Field::SourceEncoding,
    Field::RgbPasses,
    Field::PassOf,
    Field::Geometry,
    Field::Corrections,
    Field::RollReference,
    Field::RollReferenceDeclined,
    Field::BalanceMethod,
];

/// The frames edited with the open one, in their order, and each one's settings as last written.
#[derive(Default)]
pub(super) struct Together {
    paths: Vec<String>,
    held: Vec<PipelineSettings>,
}

impl Together {
    pub(super) fn follow(&mut self, moved: &dyn Fn(&str) -> Option<String>) {
        for path in &mut self.paths {
            if let Some(to) = moved(path) {
                *path = to;
            }
        }
    }

    /// Settings just written, kept for the frames among them.
    pub(super) fn landed(&mut self, written: &Settings) {
        for (path, held) in self.paths.iter().zip(&mut self.held) {
            if let Some(s) = written.get(path) {
                *held = s.clone();
            }
        }
    }
}

/// The fields moved from `before` to `after` that are no frame's own, and the face they were moved on:
/// a frame left on the other face would hold the value and show none of it.
fn shared(before: &PipelineSettings, after: &PipelineSettings) -> Vec<Field> {
    let mut fields: Vec<Field> = FieldSet::between(before, after).iter().filter(|f| !OWN.contains(f)).collect();
    if !fields.is_empty() && !fields.contains(&Field::Face) {
        fields.push(Field::Face);
    }
    fields
}

/// The `fields` moved on `open` that are laid on `other`: a roll's measured windows, gains and mode
/// belong to its roll and its decode, so a frame of another keeps its own, and its face with them.
fn laid(fields: &[Field], open: &PipelineSettings, other: &PipelineSettings) -> Vec<Field> {
    let same_roll = open.roll_reference.as_ref().map(|r| r.id) == other.roll_reference.as_ref().map(|r| r.id)
        && open.decoded_from_mosaic == other.decoded_from_mosaic;
    let reached: Vec<Field> = fields.iter().copied().filter(|f| same_roll || f.family() != Some(Family::Roll)).collect();
    if reached.iter().all(|f| *f == Field::Face) { Vec::new() } else { reached }
}

impl EditorStore {
    /// The frames edited with the open one, in their order; none for the open frame alone.
    pub fn set_together(&mut self, paths: Vec<String>) {
        let held = paths
            .iter()
            .map(|p| sidecar::read(Path::new(p), self.mosaic(p), true).map(|l| l.settings).unwrap_or_default())
            .collect();
        self.together = Together { paths, held };
    }

    pub fn together(&self) -> &[String] {
        &self.together.paths
    }

    /// Each frame together as it shows now: as written, with the step still under the hand laid on it.
    pub fn together_live(&self) -> Vec<(String, PipelineSettings)> {
        let fields = shared(self.history.current(), &self.settings);
        let paths = self.together.paths.iter().zip(&self.together.held);
        paths
            .map(|(path, held)| {
                let mut live = held.clone();
                laid(&fields, &self.settings, held).iter().for_each(|f| f.carry(&self.settings, &mut live));
                (path.clone(), live)
            })
            .collect()
    }

    /// The fields this step moved, but each frame's own, laid on the frames edited together.
    pub(super) fn propagate_together(&mut self, before: &PipelineSettings, after: &PipelineSettings, serial: u64, handled: &[String]) {
        let fields = shared(before, after);
        let others: Vec<String> = self.together.paths.iter().filter(|p| !handled.contains(p)).cloned().collect();
        if others.is_empty() || fields.is_empty() {
            return;
        }
        let (was, now) = self.modify(&others, |_, loaded| {
            let mut out = loaded.clone();
            laid(&fields, after, loaded).iter().for_each(|f| f.carry(after, &mut out));
            (out != *loaded).then_some(out)
        });
        if !was.is_empty() {
            self.history.attach(Companion { before: was, after: now }, serial);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Command;

    #[test]
    fn a_step_sets_the_same_value_on_the_frames_together_keeps_their_own_and_undoes_as_one() {
        let dir = std::env::temp_dir().join(format!("oni-core-store-together-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let [a, b, c] = ["a.tif", "b.tif", "c.tif"].map(|n| dir.join(n).to_string_lossy().into_owned());
        std::fs::write(&a, b"").unwrap();
        let mut theirs = PipelineSettings::default();
        theirs.vignette.stops = -0.7;
        theirs.manual_order = Some(4.0);
        theirs.face = oni_settings::Face::Scanner;
        for p in [&b, &c] {
            std::fs::write(p, b"").unwrap();
            sidecar::write(&theirs, Path::new(p), WriteOptions::default()).unwrap();
        }
        let mut store = EditorStore::new(Archive::new(vec![a.clone(), b.clone(), c.clone()], |_: &Path| false));
        store.open(&a).unwrap();
        store.set_together(vec![b.clone()]);
        let mut next = store.settings().clone();
        next.vignette.stops = 0.5;
        next.manual_order = Some(9.0);
        store.apply_at(Command::Write(Box::new(next)), 0.0);
        let read = |p: &str| sidecar::read(Path::new(p), false, true).unwrap().settings;
        assert_eq!(store.together_live()[0].1.vignette.stops, 0.5, "shown as written");
        assert_eq!(read(&b).vignette.stops, 0.5, "the value set, not the difference");
        assert_eq!(read(&b).manual_order, Some(4.0), "a frame's own field stays its own");
        assert_eq!(read(&b).face, store.settings().face, "on the face the step was taken on, or it would show none of it");
        assert_eq!(read(&c).vignette.stops, -0.7, "twin: a frame left out keeps its value");
        assert_eq!(read(&c).face, oni_settings::Face::Scanner, "and its face");
        store.undo(Surface::Editor);
        assert_eq!(read(&b).vignette.stops, -0.7, "one undo takes the others back");
        assert_eq!(store.together_live()[0].1.vignette.stops, -0.7, "and shows them back");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Frames a, b and c, b and c with a vignette of their own, and the store open on a.
    fn three(label: &str, b_settings: PipelineSettings, c_settings: PipelineSettings) -> (EditorStore, std::path::PathBuf, [String; 3]) {
        let dir = std::env::temp_dir().join(format!("oni-core-store-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let [a, b, c] = ["a.tif", "b.tif", "c.tif"].map(|n| dir.join(n).to_string_lossy().into_owned());
        std::fs::write(&a, b"").unwrap();
        for (p, s) in [(&b, b_settings), (&c, c_settings)] {
            std::fs::write(p, b"").unwrap();
            sidecar::write(&s, Path::new(p), WriteOptions::default()).unwrap();
        }
        // c stays out of the archive, so no roll gesture reaches it but the step laid together.
        let mut store = EditorStore::new(Archive::new(vec![a.clone(), b.clone()], |_: &Path| false));
        store.open(&a).unwrap();
        (store, dir, [a, b, c])
    }

    fn read(p: &str) -> PipelineSettings {
        sidecar::read(Path::new(p), false, true).unwrap().settings
    }

    #[test]
    fn a_paste_on_frames_together_is_undone_from_the_gallery_to_their_own_settings() {
        let own = |stops| PipelineSettings { vignette: oni_settings::blocks::Vignette { stops, ..Default::default() }, ..PipelineSettings::default() };
        let (mut store, dir, [a, b, c]) = three("together-paste", own(-0.7), own(-0.4));
        store.set_together(vec![b.clone(), c.clone()]);
        store.board.settings = Some(codec::encode_settings(&own(0.2)).unwrap());
        store.paste_settings(&[a.clone(), b.clone()]);
        assert_eq!((read(&b).vignette.stops, read(&c).vignette.stops), (0.2, 0.2), "a frame together left out of the targets takes the step");
        store.undo(Surface::Gallery);
        assert_eq!(read(&b).vignette.stops, -0.7, "the gallery undo lays its own settings back, not a mix");
        // Twin: the frame no target named is the editor's step, which takes it back.
        assert_eq!(read(&c).vignette.stops, 0.2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_roll_field_moved_together_stays_on_the_frames_of_that_roll() {
        let roll = |id| oni_settings::frame::RollReference { id, stops: [0.0; 3], black: [0.0; 3], white: [1.0; 3], base: None, median: None };
        let (r1, r2) = (Id::new(), Id::new());
        let on = |id| PipelineSettings { roll_reference: Some(roll(id)), ..PipelineSettings::default() };
        let (mut store, dir, [_, b, c]) = three("together-rolls", on(r2), on(r1));
        let mut next = PipelineSettings { roll_reference: Some(roll(r1)), ..store.settings().clone() };
        store.apply_at(Command::Write(Box::new(next.clone())), 0.0);
        store.set_together(vec![b.clone(), c.clone()]);
        next.levels.red.black = 0.2;
        next.vignette.stops = 0.5;
        store.apply_at(Command::Write(Box::new(next)), 5.0);
        let live: Vec<f32> = store.together_live().iter().map(|(_, s)| s.levels.red.black).collect();
        assert_eq!(read(&b).levels.red.black, 0.0, "another roll keeps its own windows");
        assert_eq!(read(&b).vignette.stops, 0.5, "and takes the look");
        assert_eq!(live[0], 0.0, "shown as written");
        // Twin: a frame of the same roll takes the window moved.
        assert_eq!(read(&c).levels.red.black, 0.2);
        assert_eq!(live[1], 0.2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
