//! Edit Together: the frames beside the open one take each step it takes, in that one step: a knob the
//! hand moves by its distance from each one's own value, a value set as it is, and nothing else.

use super::*;
use crate::fields::{Field, FieldSet};
use crate::knob::Knob;
use super::commands::scanned;

/// A frame's own fields, which a step on the open frame never lays on the others: its file, its
/// framing, its dust, its roll.
const OWN: [Field; 16] = [
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
    Field::MeasuredGains,
    Field::BalanceMethod,
];

/// What the open frame took during a step, as each frame together takes it.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Took {
    /// A knob the hand moved, by the open frame's distance.
    Moved(Knob, f32),
    /// A knob set at a value, a double click's rest.
    Set(Knob, f32),
    /// A scanner key, pressed on each frame's own counts.
    Scanned(crate::command::Scan),
    /// A structured edit, from the first settings to the last: the values it moved, leaf by leaf.
    Wrote(Box<PipelineSettings>, Box<PipelineSettings>),
}

/// The frames edited with the open one, in their order, each one's settings as last written, and
/// the step under way, laid on them until it is recorded.
#[derive(Default)]
pub(super) struct Together {
    paths: Vec<String>,
    held: Vec<PipelineSettings>,
    took: Vec<Took>,
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

    /// A command the open frame took, joined to the last of its kind on the same knob, so a drag is
    /// one move however many notches it lands.
    fn took(&mut self, took: Took) {
        match (self.took.last_mut(), took) {
            (Some(Took::Moved(k, by)), Took::Moved(knob, more)) if *k == knob => *by += more,
            (Some(Took::Set(k, at)), Took::Set(knob, value)) if *k == knob => *at = value,
            (Some(Took::Wrote(_, last)), Took::Wrote(_, now)) => *last = now,
            (_, took) => self.took.push(took),
        }
    }
}

/// `frame` with the step `took` laid on it, the open frame gone from `before` to `after`; a frame of
/// another roll or decode keeps the roll's own values, as their windows were measured on it.
fn laid(took: &[Took], before: &PipelineSettings, after: &PipelineSettings, frame: &PipelineSettings, scanner: bool) -> PipelineSettings {
    let same_roll = after.roll_reference.as_ref().map(|r| r.id) == frame.roll_reference.as_ref().map(|r| r.id)
        && after.decoded_from_mosaic == frame.decoded_from_mosaic;
    let reaches = |f: Field| !OWN.contains(&f) && f != Field::Face && (same_roll || f.family() != Some(Family::Roll));
    // A knob reads and writes the face edited, as the open frame's panel does.
    let knobbed = |s: &PipelineSettings, edit: &dyn Fn(&PipelineSettings) -> PipelineSettings| {
        if scanner { s.storing_scanner_edit(&edit(&s.as_scanner())) } else { edit(s) }
    };
    let mut out = frame.clone();
    for t in took {
        match t {
            Took::Moved(knob, by) if reaches(knob.field()) => out = knobbed(&out, &|v| knob.set(v, knob.get(v) + by)),
            Took::Set(knob, value) if reaches(knob.field()) => out = knobbed(&out, &|v| knob.set(v, *value)),
            Took::Scanned(scan) => out = scanned(&out, *scan),
            Took::Wrote(was, now) => FieldSet::between(was, now).iter().filter(|f| reaches(*f)).for_each(|f| f.patch(was, now, &mut out)),
            Took::Moved(..) | Took::Set(..) => {}
        }
    }
    // A frame left on the other face would hold the values and show none of them.
    let face_alone = FieldSet::between(before, after).iter().collect::<Vec<_>>() == [Field::Face];
    if out != *frame || face_alone {
        out.face = after.face;
    }
    out
}

impl EditorStore {
    /// The frames edited with the open one, in their order; none for the open frame alone. A step
    /// still waiting is recorded first, on the frames it was taken with.
    pub fn set_together(&mut self, paths: Vec<String>) {
        self.release();
        let held = paths
            .iter()
            .map(|p| sidecar::read(Path::new(p), self.mosaic(p), true).map(|l| l.settings).unwrap_or_default())
            .collect();
        self.together = Together { paths, held, took: Vec::new() };
    }

    pub fn together(&self) -> &[String] {
        &self.together.paths
    }

    /// Each frame together as it shows now: as written, with the step still under the hand laid on it.
    pub fn together_live(&self) -> Vec<(String, PipelineSettings)> {
        let t = &self.together;
        let live = |held| laid(&t.took, &self.recorded, &self.settings, held, self.scanner_face);
        t.paths.iter().zip(&t.held).map(|(path, held)| (path.clone(), live(held))).collect()
    }

    /// What `command` did to the open frame, which stood at `was`, kept for the frames together.
    pub(super) fn take_together(&mut self, command: &Command, was: &PipelineSettings) {
        let view = |s: &PipelineSettings| if self.scanner_face { s.as_scanner() } else { s.clone() };
        let took = match *command {
            Command::Step { knob, .. } => Took::Moved(knob, knob.get(&view(&self.settings)) - knob.get(&view(was))),
            Command::Set { knob, value } => Took::Set(knob, value),
            Command::Scan(scan) => Took::Scanned(scan),
            _ => Took::Wrote(Box::new(was.clone()), Box::new(self.settings.clone())),
        };
        self.together.took(took);
    }

    /// The step under way, handed over once it is recorded.
    pub(super) fn step_taken(&mut self) -> Vec<Took> {
        std::mem::take(&mut self.together.took)
    }

    /// The step `took`, the open frame gone from `before` to `after`, laid on the frames together but
    /// those of `handled`, which the gesture writes itself; what it wrote.
    pub(super) fn propagate_together(&mut self, before: &PipelineSettings, after: &PipelineSettings, took: &[Took], handled: &[String]) -> Written {
        let others: Vec<String> = self.together.paths.iter().filter(|p| !handled.contains(p)).cloned().collect();
        if others.is_empty() || took.is_empty() {
            return Written::default();
        }
        let scanner = self.scanner_face;
        self.modify(&others, |_, loaded| {
            let out = laid(took, before, after, loaded, scanner);
            (out != *loaded).then_some(out)
        })
    }
}
