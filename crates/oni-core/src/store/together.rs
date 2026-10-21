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
    /// Settings just written, kept for the frames among them.
    pub(super) fn landed(&mut self, written: &Settings) {
        for (path, held) in self.paths.iter().zip(&mut self.held) {
            if let Some(s) = written.get(path) {
                *held = s.clone();
            }
        }
    }
}

/// The fields moved from `before` to `after` that are no frame's own.
fn shared(before: &PipelineSettings, after: &PipelineSettings) -> Vec<Field> {
    FieldSet::between(before, after).iter().filter(|f| !OWN.contains(f)).collect()
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
                fields.iter().for_each(|f| f.carry(&self.settings, &mut live));
                (path.clone(), live)
            })
            .collect()
    }

    /// The fields this step moved, but each frame's own, laid on the frames edited together.
    pub(super) fn propagate_together(&mut self, before: &PipelineSettings, after: &PipelineSettings, serial: u64) {
        let fields = shared(before, after);
        if self.together.paths.is_empty() || fields.is_empty() {
            return;
        }
        let others = self.together.paths.clone();
        let (was, now) = self.modify(&others, |_, loaded| {
            let mut out = loaded.clone();
            fields.iter().for_each(|f| f.carry(after, &mut out));
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
        for p in [&b, &c] {
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
        assert_eq!(read(&c).vignette.stops, -0.7, "twin: a frame left out keeps its value");
        store.undo(Surface::Editor);
        assert_eq!(read(&b).vignette.stops, -0.7, "one undo takes the others back");
        assert_eq!(store.together_live()[0].1.vignette.stops, -0.7, "and shows them back");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
