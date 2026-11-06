//! Edit Together: the frames beside the open one take each step it takes, a move by its distance from
//! each one's own value, a rest at each one's own; what a roll shares goes by the roll's rule alone.

use super::*;
use crate::command::Each;
use crate::fields::{Field, FieldSet};
use crate::knob::Knob;
use super::commands::scanned;
use oni_settings::settings::roll_settings::Field as Shared;

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

/// What the open frame took during a step, as each frame together takes it; a knob is read and
/// written on the face it was moved on, the Scanner's when set.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Took {
    /// A knob the hand moved, by the open frame's distance.
    Moved(Knob, f32, bool),
    /// A knob set at a value, a double click's rest.
    Set(Knob, f32, bool),
    /// A scanner key, pressed on each frame's own counts.
    Scanned(crate::command::Scan),
    /// A structured edit, from the first settings to the last: the values it moved, leaf by leaf.
    Wrote(Box<PipelineSettings>, Box<PipelineSettings>),
    /// An edit read off each frame's own settings.
    Each(Each),
}

/// The frames edited with the open one, in their order, each one's settings as last written, and
/// the step under way, laid on them until it is recorded; `generation` counts what they held.
#[derive(Default)]
pub(super) struct Together {
    paths: Vec<String>,
    held: Vec<PipelineSettings>,
    took: Vec<Took>,
    generation: u64,
}

impl Together {
    pub(super) fn follow(&mut self, moved: &dyn Fn(&str) -> Option<String>) {
        for path in &mut self.paths {
            if let Some(to) = moved(path) {
                *path = to;
            }
        }
    }

    /// Settings just written, kept for the frames among them; whether one of them moved.
    pub(super) fn landed(&mut self, written: &Settings) -> bool {
        let mut moved = false;
        for (path, held) in self.paths.iter().zip(&mut self.held) {
            if let Some(s) = written.get(path).filter(|s| *s != held) {
                *held = s.clone();
                moved = true;
            }
        }
        self.generation += u64::from(moved);
        moved
    }

    /// A command the open frame took, joined to the last of its kind on the same knob and face, so a
    /// drag is one move however many notches it lands.
    fn took(&mut self, took: Took) {
        match (self.took.last_mut(), took) {
            (Some(Took::Moved(k, by, f)), Took::Moved(knob, more, face)) if (*k, *f) == (knob, face) => *by += more,
            (Some(Took::Set(k, at, f)), Took::Set(knob, value, face)) if (*k, *f) == (knob, face) => *at = value,
            (Some(Took::Wrote(_, last)), Took::Wrote(_, now)) => *last = now,
            (_, took) => self.took.push(took),
        }
    }
}

/// What the frames of roll `key` share of what moved from `before` to `after`: a slide keeps its own
/// levels and film type, those its Auto levels placed and that make it a slide.
fn shared(key: &RollKey, before: &PipelineSettings, after: &PipelineSettings) -> BTreeSet<Shared> {
    let mut fields = roll_settings::changed(before, after);
    if matches!(key, RollKey::Slides(_)) {
        fields.remove(&Shared::Levels);
        fields.remove(&Shared::Mode);
    }
    fields
}

/// The fields a slide keeps of its own beside its roll, those `shared` leaves out.
fn slide_owns(f: Field) -> bool {
    matches!(f, Field::Levels | Field::Mode | Field::InvertedSource)
}

/// `frame` taking roll `key`'s `fields` from `after`: each slide keeps its own measured balance, the
/// hand's move landing on it as an offset; a contact sheet takes no second flat-field.
fn rolled(fields: &BTreeSet<Shared>, key: &RollKey, after: &PipelineSettings, frame: &PipelineSettings) -> PipelineSettings {
    let mut out = roll_settings::copied(fields, after, frame);
    if matches!(key, RollKey::Slides(_)) && fields.contains(&Shared::Gains) {
        let base = |s: &PipelineSettings| s.measured_gains.unwrap_or_default();
        out.gains.stops = std::array::from_fn(|c| base(frame)[c] + after.gains.stops[c] - base(after)[c]);
    }
    out
}

/// `stored` read and written on the Scanner face's view when `scanner`, as the open frame's panel did.
fn on_face(stored: &PipelineSettings, scanner: bool, edit: &dyn Fn(&PipelineSettings) -> PipelineSettings) -> PipelineSettings {
    if scanner { stored.storing_scanner_edit(&edit(&stored.as_scanner())) } else { edit(stored) }
}

/// The step, the open frame gone from `before` to `after` on roll `key`, as the frames together take
/// it: `took` on what no roll shares, and the roll's `fields` by its rule on a frame of that roll.
struct Step<'a> {
    took: &'a [Took],
    before: &'a PipelineSettings,
    after: &'a PipelineSettings,
    key: Option<&'a RollKey>,
    fields: &'a BTreeSet<Shared>,
}

impl Step<'_> {
    fn on_roll(&self, path: &str, frame: &PipelineSettings) -> bool {
        self.key.is_some() && RollKey::of(path, frame).as_ref() == self.key
    }

    /// Whether `field` reaches the frame together at `path` here: never its own nor what a roll
    /// shares, so a frame on no roll takes none of its roll's fields.
    fn reaches(&self, path: &str, frame: &PipelineSettings) -> impl Fn(Field) -> bool {
        let mine = self.on_roll(path, frame);
        let slide = matches!(self.key, Some(RollKey::Slides(_)));
        move |f: Field| !OWN.contains(&f) && f != Field::Face && (f.family() != Some(Family::Roll) || (mine && slide && slide_owns(f)))
    }

    /// The frame together with the step laid.
    fn laid(&self, path: &str, frame: &PipelineSettings) -> PipelineSettings {
        let mine = self.on_roll(path, frame);
        let reaches = self.reaches(path, frame);
        let mut out = frame.clone();
        for t in self.took {
            match t {
                Took::Moved(knob, by, face) if reaches(knob.field()) => out = on_face(&out, *face, &|v| knob.set(v, knob.get(v) + by)),
                Took::Set(knob, value, face) if reaches(knob.field()) => out = on_face(&out, *face, &|v| knob.set(v, *value)),
                Took::Scanned(scan) => out = scanned(&out, *scan),
                Took::Wrote(was, now) => FieldSet::between(was, now).iter().filter(|f| reaches(*f)).for_each(|f| f.patch(was, now, &mut out)),
                Took::Each(each) => {
                    let next = each.on(&out);
                    FieldSet::between(&out, &next).iter().filter(|f| reaches(*f)).for_each(|f| f.carry(&next, &mut out));
                }
                Took::Moved(..) | Took::Set(..) => {}
            }
        }
        if mine && !self.fields.is_empty() {
            out = rolled(self.fields, self.key.expect("on the roll"), self.after, &out);
        }
        // A frame left on the other face would hold the values and show none of them.
        let face_alone = FieldSet::between(self.before, self.after).iter().collect::<Vec<_>>() == [Field::Face];
        if out != *frame || face_alone {
            out.face = self.after.face;
        }
        out
    }
}

impl EditorStore {
    /// The frames edited with the open one, in their order; none for the open frame alone. A step
    /// still waiting is recorded first, on the frames it was taken with; a hand still down holds on.
    pub fn set_together(&mut self, paths: Vec<String>) {
        if self.held || self.deferred.is_some() {
            let now = self.now();
            self.settle(now);
        }
        let held = paths
            .iter()
            .map(|p| sidecar::read(Path::new(p), self.mosaic(p), true).map(|l| l.settings).unwrap_or_default())
            .collect();
        self.together = Together { paths, held, took: Vec::new(), generation: self.together.generation + 1 };
    }

    pub fn together(&self) -> &[String] {
        &self.together.paths
    }

    /// Moves on whenever the frames together or what they hold change.
    pub fn together_generation(&self) -> u64 {
        self.together.generation
    }

    /// Each frame together as it shows now: as written, with the step still under the hand laid on it.
    pub fn together_live(&self) -> Vec<(String, PipelineSettings)> {
        let (before, after) = (&self.recorded, &self.settings);
        let key = self.path().and_then(|p| RollKey::of(p, after).or_else(|| RollKey::of(p, before)));
        let fields = key.as_ref().filter(|_| self.roll_touched).map(|k| shared(k, before, after)).unwrap_or_default();
        let step = Step { took: &self.together.took, before, after, key: key.as_ref(), fields: &fields };
        self.together.paths.iter().zip(&self.together.held).map(|(path, held)| (path.clone(), step.laid(path, held))).collect()
    }

    /// Whether a frame together that a step on `field` reaches stands where `off` says: a reset key
    /// reads the frames beside the open one by the open frame's own rule.
    pub fn together_where(&self, field: Field, off: impl Fn(&PipelineSettings) -> bool) -> bool {
        let none = BTreeSet::new();
        let key = self.roll_key();
        let step = Step { took: &[], before: &self.settings, after: &self.settings, key: key.as_ref(), fields: &none };
        self.together.paths.iter().zip(&self.together.held).any(|(path, held)| step.reaches(path, held)(field) && off(held))
    }

    /// What `command` did to the open frame, which stood at `was`, kept for the frames together.
    pub(super) fn take_together(&mut self, command: &Command, was: &PipelineSettings) {
        let scanner = self.scanner_face;
        let view = |s: &PipelineSettings| if scanner { s.as_scanner() } else { s.clone() };
        let took = match command {
            Command::Step { knob, .. } => Took::Moved(*knob, knob.get(&view(&self.settings)) - knob.get(&view(was)), scanner),
            Command::Set { knob, value } => Took::Set(*knob, *value, scanner),
            Command::Scan(scan) => Took::Scanned(*scan),
            Command::Each(each) => Took::Each(each.clone()),
            _ => Took::Wrote(Box::new(was.clone()), Box::new(self.settings.clone())),
        };
        self.together.took(took);
    }

    /// The step under way, handed over once it is recorded.
    pub(super) fn step_taken(&mut self) -> Vec<Took> {
        std::mem::take(&mut self.together.took)
    }

    /// The open frame's step from `before` to `after`, `took` on the frames together but `handled`, with
    /// `roll` what the roll shares on every other frame of it, each sidecar written once; what it wrote.
    pub(super) fn lay_step(&mut self, before: &PipelineSettings, after: &PipelineSettings, took: &[Took], roll: bool, handled: &[String]) -> Written {
        let Some(path) = self.path().map(str::to_string) else { return Written::default() };
        let key = RollKey::of(&path, after).or_else(|| RollKey::of(&path, before));
        let fields = key.as_ref().filter(|_| roll).map(|k| shared(k, before, after)).unwrap_or_default();
        let together: Vec<String> = self.together.paths.iter().filter(|p| !handled.contains(p)).cloned().collect();
        let mut reached = if took.is_empty() && fields.is_empty() { Vec::new() } else { together.clone() };
        if let Some(key) = key.as_ref().filter(|_| !fields.is_empty()) {
            reached.extend(self.others_on_roll(key).into_iter().filter(|p| !handled.contains(p)));
        }
        if reached.is_empty() {
            return Written::default();
        }
        let step = Step { took, before, after, key: key.as_ref(), fields: &fields };
        self.modify(&reached, |p, loaded| {
            let out = if together.iter().any(|t| t == p) {
                step.laid(p, loaded)
            } else if step.on_roll(p, loaded) {
                rolled(&fields, key.as_ref().expect("on the roll"), after, loaded)
            } else {
                return None;
            };
            (out != *loaded).then_some(out)
        })
    }
}
